//! Read-only planning context and untrusted, budget-free advice ingestion.
//! Advice never supplies authority, resource limits, or execution approval.
use super::*;
const MAX_ADVICE_BYTES: usize = 65_536;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanAdviceQuery {
    pub scope_id: String,
    pub task_id: String,
    pub expected_task_revision: u64,
    pub expected_spec_revision: u64,
    pub expected_scope_revision: u64,
    pub expected_scope_digest: String,
    pub expected_authority_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanAdviceOutput {
    pub artifact_id: String,
    pub contract_id: String,
    pub relative_path: String,
    pub max_bytes: u64,
    pub expected_sha256: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanAdviceContext {
    pub source: GoalPlanDraftSource,
    pub approved_acceptance_criteria: String,
    pub inputs: BTreeMap<String, GoalInputView>,
    pub outputs: Vec<GoalPlanAdviceOutput>,
    pub writable_prefixes: Vec<String>,
    pub root_budget: BudgetLimits,
    pub project_budget: BudgetLimits,
    pub max_nodes: u32,
    pub planning_only: bool,
    pub dispatched: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanAdviceStep {
    pub id: String,
    pub goal: String,
    pub dependencies: Vec<String>,
    pub input_artifact_ids: Vec<String>,
    pub output_artifact_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanAdviceImportRequest {
    pub query: GoalPlanAdviceQuery,
    pub candidate_json: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdviceEnvelope {
    schema_version: u32,
    source: GoalPlanDraftSource,
    steps: Vec<GoalPlanAdviceStep>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanAdviceImport {
    pub source: GoalPlanDraftSource,
    pub steps: Vec<GoalPlanAdviceStep>,
    pub planning_only: bool,
    pub dispatched: bool,
}
pub(super) fn current_scope(
    db: &Connection,
    q: &GoalPlanAdviceQuery,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<(ScopeRecord, GoalScopeView, GoalPlanDraftSource)> {
    key(&q.scope_id)?;
    key(&q.task_id)?;
    if q.expected_scope_digest.len() != 64
        || !q
            .expected_scope_digest
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(invalid());
    }
    let (record, scope) = read_scope(db, &q.scope_id, data)?;
    let view = scope_view(db, &record, &scope)?;
    let task: Task = raw(db, "tasks", &record.refs.task_id)?;
    if task.id != q.task_id
        || task.revision != q.expected_task_revision
        || task.spec_revision != q.expected_spec_revision
        || view.revision != q.expected_scope_revision
        || view.scope_digest != q.expected_scope_digest
        || view.authority_revision != q.expected_authority_revision
    {
        return Err(changed());
    }
    if db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM goal_plan_heads WHERE scope_id=?1)",
            [&q.scope_id],
            |r| r.get::<_, bool>(0),
        )
        .map_err(db_error)?
    {
        return Err(err(
            "STALE_REVISION",
            "Planning advice cannot replace an existing Goal plan",
        ));
    }
    let source = GoalPlanDraftSource {
        task_id: task.id,
        task_revision: task.revision,
        spec_revision: task.spec_revision,
        task_goal: task.goal,
        scope_id: view.id.clone(),
        scope_revision: view.revision,
        scope_digest: view.scope_digest.clone(),
        authority_revision: view.authority_revision,
    };
    Ok((record, view, source))
}
fn references(items: &[String]) -> Result<()> {
    if items.len() > 256
        || items.iter().any(|v| !id(v))
        || items.iter().collect::<BTreeSet<_>>().len() != items.len()
    {
        return Err(invalid());
    }
    Ok(())
}
fn validate_steps(steps: &mut [GoalPlanAdviceStep], view: &GoalScopeView) -> Result<()> {
    if !(1..=8).contains(&steps.len()) {
        return Err(invalid());
    }
    let ids: BTreeSet<String> = steps.iter().map(|s| s.id.clone()).collect();
    if ids.len() != steps.len() {
        return Err(invalid());
    }
    let mut producers = BTreeMap::new();
    let mut count = steps.len();
    for step in steps.iter() {
        if !id(&step.id)
            || step.id.starts_with("baton-review-")
            || step.goal.trim().is_empty()
            || step.goal.len() > 4096
            || step.goal.contains('\0')
            || step.output_artifact_ids.is_empty()
        {
            return Err(invalid());
        }
        references(&step.dependencies)?;
        references(&step.input_artifact_ids)?;
        references(&step.output_artifact_ids)?;
        if step
            .dependencies
            .iter()
            .any(|d| d == &step.id || !ids.contains(d))
        {
            return Err(invalid());
        }
        let mut rules = BTreeSet::new();
        for output in &step.output_artifact_ids {
            let contract = view.outputs.get(output).ok_or_else(invalid)?;
            if producers.insert(output.clone(), step.id.clone()).is_some() {
                return Err(invalid());
            }
            rules.extend(view.contracts.get(contract).ok_or_else(changed)?.iter());
        }
        count = count.checked_add(rules.len()).ok_or_else(limited)?;
    }
    if producers.len() != view.outputs.len() {
        return Err(invalid());
    }
    let context = view.edit_context.as_ref().ok_or_else(changed)?;
    if count > context.max_nodes as usize
        || count > view.root_budget.attempts as usize
        || count > view.project_budget.attempts as usize
    {
        return Err(err(
            "RESOURCE_LIMIT",
            "Suggested work and required review gates exceed approved node or attempt capacity",
        ));
    }
    let mut edges = BTreeMap::<String, BTreeSet<String>>::new();
    for step in steps.iter_mut() {
        let mut deps: BTreeSet<String> = step.dependencies.iter().cloned().collect();
        for artifact in &step.input_artifact_ids {
            if view.inputs.contains_key(artifact) {
                continue;
            }
            let producer = producers.get(artifact).ok_or_else(invalid)?;
            if producer == &step.id {
                return Err(invalid());
            }
            deps.insert(producer.clone());
        }
        step.dependencies = deps.iter().cloned().collect();
        edges.insert(step.id.clone(), deps);
    }
    while !edges.is_empty() {
        let ready: Vec<String> = edges
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(id, _)| id.clone())
            .collect();
        if ready.is_empty() {
            return Err(invalid());
        }
        for id in ready {
            edges.remove(&id);
            for deps in edges.values_mut() {
                deps.remove(&id);
            }
        }
    }
    Ok(())
}
impl Store {
    pub fn goal_plan_advice_context(
        &mut self,
        query: &GoalPlanAdviceQuery,
    ) -> Result<GoalPlanAdviceContext> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let (record, view, source) = current_scope(&tx, query, self.data_input.as_ref())?;
        let outputs = record
            .selection
            .outputs
            .iter()
            .map(|o| {
                Ok(GoalPlanAdviceOutput {
                    artifact_id: o.artifact.artifact_id.clone(),
                    contract_id: view
                        .outputs
                        .get(&o.artifact.artifact_id)
                        .ok_or_else(changed)?
                        .clone(),
                    relative_path: o
                        .artifact
                        .relative_path
                        .to_str()
                        .ok_or_else(changed)?
                        .into(),
                    max_bytes: o.artifact.max_bytes,
                    expected_sha256: o.artifact.expected_sha256.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let result = GoalPlanAdviceContext {
            source,
            approved_acceptance_criteria: record.selection.owner_acceptance_criteria,
            inputs: view.inputs,
            outputs,
            writable_prefixes: record.selection.writable_prefixes,
            root_budget: view.root_budget,
            project_budget: view.project_budget,
            max_nodes: record.selection.max_nodes,
            planning_only: true,
            dispatched: false,
        };
        if encode(&result)?.len() > MAX_ADVICE_BYTES {
            return Err(limited());
        }
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
    pub fn import_goal_plan_advice(
        &mut self,
        request: &GoalPlanAdviceImportRequest,
    ) -> Result<GoalPlanAdviceImport> {
        if request.candidate_json.len() > MAX_ADVICE_BYTES {
            return Err(limited());
        }
        // Decode the original text directly. Parsing through Value first would
        // discard duplicate object keys before this closed schema could reject.
        let mut advice: AdviceEnvelope =
            serde_json::from_str(&request.candidate_json).map_err(|_| invalid())?;
        if advice.schema_version != 1 {
            return Err(invalid());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let (_, view, source) = current_scope(&tx, &request.query, self.data_input.as_ref())?;
        if encode(&advice.source)? != encode(&source)? {
            return Err(changed());
        }
        validate_steps(&mut advice.steps, &view)?;
        let result = GoalPlanAdviceImport {
            source,
            steps: advice.steps,
            planning_only: true,
            dispatched: false,
        };
        if encode(&result)?.len() > MAX_ADVICE_BYTES {
            return Err(limited());
        }
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
}
