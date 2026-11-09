//! Read-only, deterministic initial plan templates. No Provider or dispatch.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanDraftRequest {
    pub scope_id: String,
    pub task_id: String,
    pub expected_task_revision: u64,
    pub expected_spec_revision: u64,
    pub expected_scope_revision: u64,
    pub expected_scope_digest: String,
    pub expected_authority_revision: u64,
    pub risk_notes: String,
    pub prohibited_actions: String,
    pub runtime_ms: u64,
    pub output_bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanDraftSource {
    pub task_id: String,
    pub task_revision: u64,
    pub spec_revision: u64,
    pub task_goal: String,
    pub scope_id: String,
    pub scope_revision: u64,
    pub scope_digest: String,
    pub authority_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanDraft {
    pub source: GoalPlanDraftSource,
    pub approved_acceptance_criteria: String,
    pub approved_outputs: Vec<GoalOutputSelection>,
    pub writable_prefixes: Vec<String>,
    pub root_budget: BudgetLimits,
    pub project_budget: BudgetLimits,
    pub risk_facts: Vec<String>,
    pub plan: GoalPlanProposal,
    pub plan_digest: String,
    pub planning_only: bool,
    pub dispatched: bool,
}
impl Store {
    /// Generate a candidate from an authenticated native scope, without storing
    /// a proposal, reserving capacity, or minting execution authority. A later
    /// normal submit and separate execution approval revalidate current sources.
    pub fn goal_plan_draft(&mut self, request: &GoalPlanDraftRequest) -> Result<GoalPlanDraft> {
        key(&request.scope_id)?;
        key(&request.task_id)?;
        if request.expected_scope_digest.len() != 64
            || !request
                .expected_scope_digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || [&request.risk_notes, &request.prohibited_actions]
                .iter()
                .any(|s| s.trim().is_empty() || s.len() > 4096 || s.contains('\0'))
        {
            return Err(invalid());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        // read_scope verifies original intent/confirmation, scope digest,
        // current Task goal/spec, grant, connection, workspace and input hashes.
        // In particular its selection text is not an editable renderer form.
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
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM goal_plan_heads WHERE scope_id=?1)",
                [&request.scope_id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if exists {
            return Err(err(
                "STALE_REVISION",
                "An initial draft cannot replace an existing Goal plan",
            ));
        }
        let budget = BudgetLimits {
            attempts: 1,
            parallel: 1,
            runtime_ms: request.runtime_ms,
            output_bytes: request.output_bytes,
            children: 0,
            reworks: 0,
            tokens: None,
            cost_microunits: None,
        };
        if !budget.narrower(&view.root_budget) || !budget.narrower(&view.project_budget) {
            return Err(denied());
        }
        let goal = draft_goal(&task.goal, &request.risk_notes, &request.prohibited_actions)?;
        let outputs: Vec<PlannedOutput> = view
            .outputs
            .iter()
            .map(|(artifact, contract)| PlannedOutput {
                artifact_id: artifact.clone(),
                contract_id: contract.clone(),
            })
            .collect();
        let required_validators: BTreeSet<String> = outputs
            .iter()
            .flat_map(|output| view.contracts[&output.contract_id].iter().cloned())
            .collect();
        let plan = GoalPlanProposal {
            schema_version: 1,
            plan_id: Uuid::new_v4().to_string(),
            goal: view.goal.clone(),
            scope_revision: view.authority_revision,
            revision: 1,
            expected_revision: 0,
            acceptance_rules: view.required_acceptance_rules.clone(),
            nodes: vec![PlanNode {
                id: "step-1".into(),
                parent_id: None,
                kind: NodeKind::Work,
                goal,
                dependencies: vec![],
                workspace_id: view.workspace_id.clone(),
                executor: Executor::Connection {
                    binding: Box::new(view.binding.clone()),
                    program_sha256: view.program_sha256.clone(),
                },
                recipients: vec![],
                required_inputs: view
                    .inputs
                    .iter()
                    .map(|(id, input)| RequiredInput {
                        goal_task_id: task.id.clone(),
                        artifact_id: id.clone(),
                        contract_id: input.contract_id.clone(),
                        source: InputSource::ApprovedExternal {
                            sha256: input.sha256.clone(),
                        },
                    })
                    .collect(),
                outputs,
                required_validators: required_validators.into_iter().collect(),
                side_effect: SideEffect::WorkspaceWrite,
                write_set: record
                    .selection
                    .outputs
                    .iter()
                    .map(|output| {
                        Ok(WriteTarget::Workspace {
                            path: output
                                .artifact
                                .relative_path
                                .to_str()
                                .ok_or_else(changed)?
                                .into(),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
                budget,
                reservation: AttemptReservation {
                    runtime_ms: request.runtime_ms,
                    output_bytes: request.output_bytes,
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
            }],
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
                "single_step_template",
                "workspace_write",
                "owner_acceptance_required",
                "budget_ceiling_not_remaining",
                "no_os_sandbox_claim",
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

fn draft_goal(goal: &str, risks: &str, prohibitions: &str) -> Result<String> {
    let text = format!("Task goal:\n{goal}\n\nUser-reviewed risks:\n{risks}\n\nProhibited actions:\n{prohibitions}");
    if text.len() > 16384 {
        return Err(limited());
    }
    Ok(text)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn goal_draft_combined_text_boundary_rejects_without_truncating() {
        // Individual fields can be bounded yet overflow the composed node.
        assert_eq!(
            draft_goal(&"g".repeat(12000), &"r".repeat(4096), &"p".repeat(4096))
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        let overhead = draft_goal("", "", "").unwrap().len();
        let exact = draft_goal(&"g".repeat(16384 - overhead), "", "").unwrap();
        assert_eq!(exact.len(), 16384);
        assert!(draft_goal(&"g".repeat(16385 - overhead), "", "").is_err());
    }
}
