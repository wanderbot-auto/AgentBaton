//! One to eight user-defined initial work steps, with required review gates.
//! Authority and artifact contracts come from Core; the legacy work-only draft
//! endpoint remains unchanged.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalDraftStep {
    pub id: String,
    pub goal: String,
    pub dependencies: Vec<String>,
    pub input_artifact_ids: Vec<String>,
    pub output_artifact_ids: Vec<String>,
    pub runtime_ms: u64,
    pub output_bytes: u64,
    pub review_runtime_ms: u64,
    pub review_output_bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalMultiPlanDraftRequest {
    pub scope_id: String,
    pub task_id: String,
    pub expected_task_revision: u64,
    pub expected_spec_revision: u64,
    pub expected_scope_revision: u64,
    pub expected_scope_digest: String,
    pub expected_authority_revision: u64,
    pub risk_notes: String,
    pub prohibited_actions: String,
    pub steps: Vec<GoalDraftStep>,
}
fn references(ids: &[String]) -> Result<()> {
    if ids.len() > 256
        || ids.iter().any(|s| !id(s))
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(invalid());
    }
    Ok(())
}
fn text_valid(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 4096 && !s.contains('\0')
}
impl Store {
    pub fn goal_multi_plan_draft(
        &mut self,
        request: &GoalMultiPlanDraftRequest,
    ) -> Result<GoalPlanDraft> {
        key(&request.scope_id)?;
        key(&request.task_id)?;
        if !(1..=8).contains(&request.steps.len())
            || !text_valid(&request.risk_notes)
            || !text_valid(&request.prohibited_actions)
            || request.expected_scope_digest.len() != 64
            || !request
                .expected_scope_digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(invalid());
        }
        for step in &request.steps {
            key(&step.id)?;
            if !text_valid(&step.goal)
                || step.output_artifact_ids.is_empty()
                || step.id.starts_with("baton-review-")
            {
                return Err(invalid());
            }
            references(&step.dependencies)?;
            references(&step.input_artifact_ids)?;
            references(&step.output_artifact_ids)?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let (record, scope) = read_scope(&tx, &request.scope_id, self.data_input.as_ref())?;
        let view = scope_view(&tx, &record, &scope)?;
        let task: Task = raw(&tx, "tasks", &record.refs.task_id)?;
        if task.id != request.task_id
            || task.revision != request.expected_task_revision
            || task.spec_revision != request.expected_spec_revision
            || view.revision != request.expected_scope_revision
            || view.scope_digest != request.expected_scope_digest
            || view.authority_revision != request.expected_authority_revision
        {
            return Err(changed());
        }
        if tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM goal_plan_heads WHERE scope_id=?1)",
                [&request.scope_id],
                |r| r.get::<_, bool>(0),
            )
            .map_err(db_error)?
        {
            return Err(err(
                "STALE_REVISION",
                "An initial draft cannot replace an existing Goal plan",
            ));
        }
        if request.steps.len() > record.selection.max_nodes as usize
            || request.steps.len() > view.outputs.len()
        {
            return Err(invalid());
        }
        let step_ids: BTreeSet<&String> = request.steps.iter().map(|s| &s.id).collect();
        if step_ids.len() != request.steps.len() {
            return Err(invalid());
        }
        let mut producers = BTreeMap::<String, String>::new();
        let mut total = BudgetLimits {
            attempts: 0,
            parallel: 1,
            runtime_ms: 0,
            output_bytes: 0,
            children: 0,
            reworks: 0,
            tokens: None,
            cost_microunits: None,
        };
        for step in &request.steps {
            if step
                .dependencies
                .iter()
                .any(|d| d == &step.id || !step_ids.contains(d))
            {
                return Err(invalid());
            }
            total.attempts = total.attempts.checked_add(1).ok_or_else(limited)?;
            total.runtime_ms = total
                .runtime_ms
                .checked_add(step.runtime_ms)
                .ok_or_else(limited)?;
            total.output_bytes = total
                .output_bytes
                .checked_add(step.output_bytes)
                .ok_or_else(limited)?;
            for output in &step.output_artifact_ids {
                if !view.outputs.contains_key(output)
                    || producers.insert(output.clone(), step.id.clone()).is_some()
                {
                    return Err(invalid());
                }
            }
        }
        if producers.len() != view.outputs.len() {
            return Err(invalid());
        }
        let mut nodes = Vec::with_capacity(request.steps.len());
        for (step_index, step) in request.steps.iter().enumerate() {
            let mut dependencies: BTreeSet<String> = step.dependencies.iter().cloned().collect();
            let mut inputs = Vec::new();
            for artifact in &step.input_artifact_ids {
                let (contract_id, source) = if let Some(input) = view.inputs.get(artifact) {
                    (
                        input.contract_id.clone(),
                        InputSource::ApprovedExternal {
                            sha256: input.sha256.clone(),
                        },
                    )
                } else if let Some(producer) = producers.get(artifact) {
                    if producer == &step.id {
                        return Err(invalid());
                    }
                    dependencies.insert(producer.clone());
                    (
                        view.outputs[artifact].clone(),
                        InputSource::Node {
                            node_id: producer.clone(),
                            candidate_for_validation: false,
                        },
                    )
                } else {
                    return Err(invalid());
                };
                inputs.push(RequiredInput {
                    goal_task_id: task.id.clone(),
                    artifact_id: artifact.clone(),
                    contract_id,
                    source,
                });
            }
            let outputs: Vec<PlannedOutput> = step
                .output_artifact_ids
                .iter()
                .map(|id| PlannedOutput {
                    artifact_id: id.clone(),
                    contract_id: view.outputs[id].clone(),
                })
                .collect();
            let validators: BTreeSet<String> = outputs
                .iter()
                .flat_map(|o| view.contracts[&o.contract_id].iter().cloned())
                .collect();
            let goal=format!("Task goal:\n{}\n\nStep goal:\n{}\n\nUser-reviewed risks:\n{}\n\nProhibited actions:\n{}",task.goal,step.goal,request.risk_notes,request.prohibited_actions);
            if goal.len() > 16384 {
                return Err(limited());
            }
            let budget = BudgetLimits {
                attempts: 1,
                parallel: 1,
                runtime_ms: step.runtime_ms,
                output_bytes: step.output_bytes,
                children: 0,
                reworks: 0,
                tokens: None,
                cost_microunits: None,
            };
            if !budget.narrower(&view.root_budget) || !budget.narrower(&view.project_budget) {
                return Err(denied());
            }
            let writes = step
                .output_artifact_ids
                .iter()
                .map(|id| {
                    let output = record
                        .selection
                        .outputs
                        .iter()
                        .find(|o| &o.artifact.artifact_id == id)
                        .ok_or_else(changed)?;
                    Ok(WriteTarget::Workspace {
                        path: output
                            .artifact
                            .relative_path
                            .to_str()
                            .ok_or_else(changed)?
                            .into(),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            nodes.push(PlanNode {
                id: step.id.clone(),
                parent_id: None,
                kind: NodeKind::Work,
                goal,
                dependencies: dependencies.into_iter().collect(),
                workspace_id: view.workspace_id.clone(),
                executor: Executor::Connection {
                    binding: Box::new(view.binding.clone()),
                    program_sha256: view.program_sha256.clone(),
                },
                recipients: vec![],
                required_inputs: inputs,
                outputs,
                required_validators: validators.iter().cloned().collect(),
                side_effect: SideEffect::WorkspaceWrite,
                write_set: writes,
                budget,
                reservation: AttemptReservation {
                    runtime_ms: step.runtime_ms,
                    output_bytes: step.output_bytes,
                    tokens: None,
                    cost_microunits: None,
                },
                retry: RetryPolicy {
                    max_attempts: 1,
                    max_reworks: 0,
                    retry_classes: vec![],
                },
                idempotency_key: Uuid::new_v4().to_string(),
                external_action_id: None,
            });
            let gate_budget = BudgetLimits {
                attempts: 1,
                parallel: 1,
                runtime_ms: step.review_runtime_ms,
                output_bytes: step.review_output_bytes,
                children: 0,
                reworks: 0,
                tokens: None,
                cost_microunits: None,
            };
            if !gate_budget.narrower(&view.root_budget)
                || !gate_budget.narrower(&view.project_budget)
            {
                return Err(denied());
            }
            let context = view.edit_context.as_ref().ok_or_else(changed)?;
            let gates: Vec<(String, String, GoalRuleExecutorKind)> = validators
                .iter()
                .enumerate()
                .map(|(index, rule)| {
                    Ok((
                        format!("baton-review-{}-{}", step_index + 1, index + 1),
                        rule.clone(),
                        context
                            .rule_executors
                            .get(rule)
                            .ok_or_else(changed)?
                            .clone(),
                    ))
                })
                .collect::<Result<_>>()?;
            for (gate_id, rule, kind) in &gates {
                total.attempts = total.attempts.checked_add(1).ok_or_else(limited)?;
                total.runtime_ms = total
                    .runtime_ms
                    .checked_add(step.review_runtime_ms)
                    .ok_or_else(limited)?;
                total.output_bytes = total
                    .output_bytes
                    .checked_add(step.review_output_bytes)
                    .ok_or_else(limited)?;
                let mut dependencies = BTreeSet::from([step.id.clone()]);
                if *kind == GoalRuleExecutorKind::Owner {
                    dependencies.extend(
                        gates
                            .iter()
                            .filter(|(_, _, kind)| *kind == GoalRuleExecutorKind::Validator)
                            .map(|(id, _, _)| id.clone()),
                    );
                }
                let required_inputs = step
                    .output_artifact_ids
                    .iter()
                    .filter(|id| view.contracts[&view.outputs[*id]].contains(rule))
                    .map(|id| RequiredInput {
                        goal_task_id: task.id.clone(),
                        artifact_id: id.clone(),
                        contract_id: view.outputs[id].clone(),
                        source: InputSource::Node {
                            node_id: step.id.clone(),
                            candidate_for_validation: true,
                        },
                    })
                    .collect();
                let (kind, executor) = match kind {
                    GoalRuleExecutorKind::Owner => (
                        NodeKind::OwnerApproval,
                        Executor::Owner {
                            rule_id: rule.clone(),
                        },
                    ),
                    GoalRuleExecutorKind::Validator => (
                        NodeKind::Validate,
                        Executor::Validator {
                            rule_id: rule.clone(),
                        },
                    ),
                };
                nodes.push(PlanNode{id:gate_id.clone(),parent_id:None,kind,goal:format!("Review approved rule {rule} for user step {}. Follow the original approved acceptance criteria; this gate grants no new authority.",step.id),dependencies:dependencies.into_iter().collect(),workspace_id:view.workspace_id.clone(),executor,recipients:vec![],required_inputs,outputs:vec![],required_validators:vec![rule.clone()],side_effect:SideEffect::AdvisoryReadOnly,write_set:vec![],budget:gate_budget.clone(),reservation:AttemptReservation{runtime_ms:step.review_runtime_ms,output_bytes:step.review_output_bytes,tokens:None,cost_microunits:None},retry:RetryPolicy{max_attempts:1,max_reworks:0,retry_classes:vec![]},idempotency_key:Uuid::new_v4().to_string(),external_action_id:None});
            }
        }
        if nodes.len() > record.selection.max_nodes as usize || nodes.len() > 256 {
            return Err(err(
                "RESOURCE_LIMIT",
                "Work and required review nodes exceed the approved node limit; review a new scope",
            ));
        }
        if !total.narrower(&view.root_budget) || !total.narrower(&view.project_budget) {
            return Err(err("PERMISSION_DENIED","Work and required review budgets exceed approved ceilings; no authority was expanded"));
        }
        let plan = GoalPlanProposal {
            schema_version: 1,
            plan_id: Uuid::new_v4().to_string(),
            goal: view.goal.clone(),
            scope_revision: view.authority_revision,
            revision: 1,
            expected_revision: 0,
            acceptance_rules: view.required_acceptance_rules.clone(),
            nodes,
        };
        let validated = ValidatedPlan::initial(&scope, plan.clone()).map_err(plan_error)?;
        let result = GoalPlanDraft {
            source: GoalPlanDraftSource {
                task_id: task.id,
                task_revision: task.revision,
                spec_revision: task.spec_revision,
                task_goal: task.goal,
                scope_id: view.id,
                scope_revision: view.revision,
                scope_digest: view.scope_digest,
                authority_revision: view.authority_revision,
            },
            approved_acceptance_criteria: record.selection.owner_acceptance_criteria,
            approved_outputs: record.selection.outputs,
            writable_prefixes: record.selection.writable_prefixes,
            root_budget: view.root_budget,
            project_budget: view.project_budget,
            risk_facts: [
                "multi_step_user_defined",
                "workspace_write",
                "owner_acceptance_required",
                "upstream_acceptance_required",
                "budget_ceiling_not_remaining",
                "no_os_sandbox_claim",
                "generated_review_gates",
            ]
            .map(String::from)
            .to_vec(),
            plan,
            plan_digest: validated.digest().into(),
            planning_only: true,
            dispatched: false,
        };
        if encode(&result)?.len() > MAX_GOAL_PLAN_BYTES {
            return Err(limited());
        }
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
}
