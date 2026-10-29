//! Factory-derived immutable GoalPlan metadata. No Goal approval, Grant ceiling,
//! worker, workspace, reservation, Task acceptance or second plan DSL is created.
use super::*;
use crate::goal_plan::{self, *};
use std::collections::{BTreeMap, BTreeSet};

const AUTHORITY: &str = "factory_child_planning_only";
const OWNER_RULE: &str = "factory-child-owner-review";
const INPUT_RULE: &str = "factory-child-input-sha256";
const MAX_RAW: usize = 262_144;
const MAX_SCOPE_HISTORY: i64 = 4_194_304;

pub(crate) fn migrate(db: &mut Connection, version: u32) -> Result<()> {
    if version < 14 {
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute_batch("CREATE TABLE factory_child_plan_revisions(id TEXT PRIMARY KEY,scope_id TEXT NOT NULL REFERENCES factory_scopes(id),run_id TEXT NOT NULL REFERENCES factory_runs(id),task_id TEXT NOT NULL REFERENCES tasks(id),revision INTEGER NOT NULL CHECK(revision>0 AND revision<=256),plan_json TEXT NOT NULL CHECK(octet_length(plan_json)<=262144),raw_sha256 TEXT NOT NULL,plan_digest TEXT NOT NULL,receipt_sequence INTEGER NOT NULL UNIQUE REFERENCES events(sequence),record_digest TEXT NOT NULL,document TEXT NOT NULL,UNIQUE(scope_id,task_id,revision));
CREATE TABLE factory_child_plan_heads(scope_id TEXT NOT NULL REFERENCES factory_scopes(id),run_id TEXT NOT NULL REFERENCES factory_runs(id),task_id TEXT NOT NULL REFERENCES tasks(id),revision INTEGER NOT NULL CHECK(revision>0 AND revision<=256),revision_id TEXT NOT NULL UNIQUE REFERENCES factory_child_plan_revisions(id),PRIMARY KEY(scope_id,task_id));
PRAGMA user_version=14;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}
fn planning_error(e: PlanError) -> CoreError {
    match e {
        PlanError::StaleRevision => CoreError::new(
            "STALE_REVISION",
            "Compare the persisted child plan head before revising",
        ),
        PlanError::InputChanged | PlanError::StaleEvidence => changed(),
        PlanError::Limit => limited(),
        PlanError::AuthorityExpansion => denied(),
        _ => invalid(),
    }
}
pub(super) fn policy_limits(
    s: &FactoryScopeSelection,
    p: &FactoryChildPlanningPolicy,
) -> Result<()> {
    if p.max_nodes == 0
        || p.max_nodes > 256
        || p.max_revisions == 0
        || p.max_revisions > 256
        || p.max_plan_bytes == 0
        || p.max_plan_bytes as usize > MAX_RAW
        || p.outputs.len() != s.outputs.len()
        || p.writable_prefixes.len() > 16
        || p.owner_acceptance_criteria.trim().is_empty()
        || p.owner_acceptance_criteria.len() > 16384
    {
        return Err(invalid());
    }
    let mut artifacts = BTreeSet::new();
    for output in &p.outputs {
        if !s.outputs.contains(&output.artifact)
            || !artifacts.insert(&output.artifact.artifact_id)
            || output.validators.len() > 16
        {
            return Err(denied());
        }
        let mut refs = BTreeSet::new();
        for r in &output.validators {
            key(&r.policy_id)?;
            if r.revision == 0 || !refs.insert(encode(r)?) {
                return Err(invalid());
            }
        }
    }
    let mut writes = BTreeSet::new();
    for path in &p.writable_prefixes {
        if !writes.insert(path)
            || !p
                .outputs
                .iter()
                .any(|o| o.artifact.relative_path.to_str() == Some(path))
        {
            return Err(denied());
        }
    }
    Ok(())
}
pub(crate) fn resolve_binding(
    db: &Connection,
    s: &FactoryScopeSelection,
    workspace: &crate::registry::ManagedWorkspace,
) -> Result<Option<FactoryChildPlanningBinding>> {
    let Some(p) = &s.child_planning_policy else {
        return Ok(None);
    };
    if !s.capabilities.child_planning
        || s.capabilities.child_goal_execution
        || s.capabilities.create_workspace
    {
        return Err(denied());
    }
    policy_limits(s, p)?;
    let mut definitions = BTreeMap::new();
    for output in &p.outputs {
        for reference in &output.validators {
            let value = crate::goal_store::resolve_factory_child_validator(
                db,
                reference,
                &workspace.project_id,
                workspace,
                &output.artifact,
            )?;
            let id = value["rule_id"].as_str().ok_or_else(changed)?.to_owned();
            if let Some(previous) = definitions.insert(id, value.clone()) {
                if previous != value {
                    return Err(changed());
                }
            }
        }
    }
    if definitions.len() > 254 {
        return Err(limited());
    }
    let base = json!({"schema_version":1,"authority_kind":AUTHORITY,"policy":p,"owner_rule_id":OWNER_RULE,"input_rule_id":INPUT_RULE,"validator_definitions":definitions,"goal_execution_authorized":false,"new_workspace_authorized":false});
    Ok(Some(FactoryChildPlanningBinding {
        schema_version: 1,
        authority_kind: AUTHORITY.into(),
        policy: p.clone(),
        owner_rule_id: OWNER_RULE.into(),
        input_rule_id: INPUT_RULE.into(),
        validator_definitions: definitions,
        definition_digest: sha(&base)?,
        goal_execution_authorized: false,
        new_workspace_authorized: false,
    }))
}
fn approved_binding(s: &FactoryScope) -> Result<&FactoryChildPlanningBinding> {
    let p = s
        .selection
        .child_planning_policy
        .as_ref()
        .ok_or_else(denied)?;
    let b = s.bindings.child_planning.as_ref().ok_or_else(denied)?;
    if !s.selection.capabilities.child_planning
        || s.selection.capabilities.child_goal_execution
        || s.selection.capabilities.create_workspace
        || b.schema_version != 1
        || b.authority_kind != AUTHORITY
        || b.policy != *p
        || b.owner_rule_id != OWNER_RULE
        || b.input_rule_id != INPUT_RULE
        || b.goal_execution_authorized
        || b.new_workspace_authorized
    {
        return Err(denied());
    }
    policy_limits(&s.selection, p)?;
    let mut value = serde_json::to_value(b).map_err(|_| changed())?;
    value
        .as_object_mut()
        .ok_or_else(changed)?
        .remove("definition_digest");
    if sha(&value)? != b.definition_digest {
        return Err(changed());
    }
    Ok(b)
}
fn host_policy(s: &FactoryScope, b: &TaskBinding) -> Result<HostGoalPolicy> {
    let binding = approved_binding(s)?;
    let p = &binding.policy;
    let w = s
        .bindings
        .workspaces
        .iter()
        .find(|w| w.id == b.request.workspace_id)
        .ok_or_else(changed)?;
    let pin = w.identity.as_ref().ok_or_else(changed)?;
    let c = s
        .bindings
        .approved_grant
        .connection_bindings
        .get(&b.request.connection_id)
        .ok_or_else(changed)?;
    let provider = "local-unverified-native-account".to_string();
    let mut rules = BTreeMap::from([
        (
            OWNER_RULE.into(),
            ApprovedRuleRef {
                kind: RuleKind::Owner,
                policy_digest: sha(&p.owner_acceptance_criteria)?,
                goal_acceptance: true,
            },
        ),
        (
            INPUT_RULE.into(),
            ApprovedRuleRef {
                kind: RuleKind::Deterministic,
                policy_digest: sha(&s.bindings.inputs)?,
                goal_acceptance: false,
            },
        ),
    ]);
    let mut contracts = BTreeMap::new();
    let mut inputs = BTreeMap::new();
    for input in &s.bindings.inputs {
        let id = &input.contract.requirement.artifact_id;
        if !b.request.input_artifact_ids.contains(id) {
            continue;
        }
        let contract = format!("input:{id}");
        contracts.insert(
            contract.clone(),
            ArtifactContract {
                required_validators: BTreeSet::from([INPUT_RULE.into()]),
            },
        );
        inputs.insert(
            id.clone(),
            ExternalInput {
                contract_id: contract,
                sha256: input.sha256.clone(),
                verified: true,
            },
        );
    }
    let mut outputs = BTreeMap::new();
    for id in &b.request.output_artifact_ids {
        let output = p
            .outputs
            .iter()
            .find(|o| &o.artifact.artifact_id == id)
            .ok_or_else(changed)?;
        let mut required = BTreeSet::from([OWNER_RULE.into()]);
        for r in &output.validators {
            let value = binding
                .validator_definitions
                .values()
                .find(|v| v["reference"] == json!(r))
                .ok_or_else(changed)?;
            let rule = value["rule_id"].as_str().ok_or_else(changed)?;
            required.insert(rule.into());
            rules.insert(
                rule.into(),
                ApprovedRuleRef {
                    kind: RuleKind::Deterministic,
                    policy_digest: value["digest"].as_str().ok_or_else(changed)?.into(),
                    goal_acceptance: false,
                },
            );
        }
        let contract = format!("output:{id}");
        contracts.insert(
            contract.clone(),
            ArtifactContract {
                required_validators: required,
            },
        );
        outputs.insert(id.clone(), contract);
    }
    if inputs.len() != b.request.input_artifact_ids.len()
        || outputs.len() != b.request.output_artifact_ids.len()
    {
        return Err(changed());
    }
    let writable = p
        .writable_prefixes
        .iter()
        .filter(|path| {
            p.outputs.iter().any(|o| {
                b.request
                    .output_artifact_ids
                    .contains(&o.artifact.artifact_id)
                    && o.artifact.relative_path.to_str() == Some(path)
            })
        })
        .cloned()
        .collect();
    Ok(HostGoalPolicy {
        root_task_id: s.root_task_id.clone(),
        input_revision: w.base_revision.clone(),
        max_nodes: p.max_nodes,
        max_depth: s.selection.max_depth,
        root_budget: b.request.budget.clone(),
        project_budget: s.selection.project_budget.clone(),
        resources: ResourceLimits {
            global: 1,
            hosts: BTreeMap::from([(c.host_id.clone(), 1)]),
            providers: BTreeMap::from([(provider.clone(), 1)]),
            workspaces: BTreeMap::from([(w.id.clone(), 1)]),
        },
        connections: BTreeMap::from([(
            b.request.connection_id.clone(),
            ConnectionPermit {
                provider_quota_key: provider,
                binding: c.clone(),
            },
        )]),
        workspaces: BTreeMap::from([(
            w.id.clone(),
            WorkspacePermit {
                host_id: c.host_id.clone(),
                canonical_root: pin.canonical_root.to_str().ok_or_else(changed)?.into(),
                device: pin.device,
                inode: pin.inode,
                writable_prefixes: writable,
            },
        )]),
        rules,
        contracts,
        external_inputs: inputs,
        external_writes: BTreeMap::new(),
        external_actions: BTreeMap::new(),
        required_goal_outputs: outputs,
        acceptance_rules: BTreeSet::from([OWNER_RULE.into()]),
    })
}
fn parse_plan(raw: &str, limit: usize) -> Result<GoalPlanProposal> {
    if raw.is_empty() || raw.len() > limit || raw.len() > MAX_RAW {
        return Err(limited());
    }
    // Typed decoding rejects duplicate fields/unknown keys at every plan object.
    let p: GoalPlanProposal = serde_json::from_str(raw).map_err(|_| invalid())?;
    fn safe(value: &Value) -> bool {
        match value {
            Value::Number(n) => n.as_u64().is_some_and(|v| v <= 9_007_199_254_740_991),
            Value::Array(a) => a.iter().all(safe),
            Value::Object(o) => o.values().all(safe),
            _ => true,
        }
    }
    let value: Value = serde_json::from_str(raw).map_err(|_| invalid())?;
    if !safe(&value) {
        return Err(invalid());
    }
    Ok(p)
}
fn check_view_wire(view: &FactoryChildPlanView) -> Result<()> {
    let frame =
        json!({"request_id":"\0".repeat(128),"result":view,"revision":u64::MAX,"warnings":[]});
    check_wire(&frame)?;
    check_wire(
        &json!({"jsonrpc":"2.0","id":"\0".repeat(128),"result":{"content":[{"type":"text","text":encode(&frame)?}],"isError":false}}),
    )
}
fn commitment(v: &FactoryChildPlanView) -> Result<String> {
    let mut canonical = v.clone();
    canonical.reference.receipt_sequence = 0;
    sha(&canonical)
}
fn stored_budget(db: &Connection, scope_id: &str, task_id: &str) -> Result<()> {
    let(n,bytes):(i64,i64)=db.query_row("SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(plan_json)+octet_length(id)+octet_length(scope_id)+octet_length(run_id)+octet_length(task_id)+octet_length(raw_sha256)+octet_length(plan_digest)+octet_length(record_digest)),0) FROM factory_child_plan_revisions WHERE scope_id=?1 AND task_id=?2",params![scope_id,task_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    if !(0..=256).contains(&n) || !(0..=MAX_SCOPE_HISTORY).contains(&bytes) {
        return Err(limited());
    }
    Ok(())
}
fn row(
    db: &Connection,
    scope_id: &str,
    task_id: &str,
    revision: u64,
) -> Result<FactoryChildPlanView> {
    key(scope_id)?;
    key(task_id)?;
    stored_budget(db, scope_id, task_id)?;
    let stored:Option<(Option<String>,Option<String>,String,String,String,i64,String,String)>=db.query_row(
        "SELECT CASE WHEN octet_length(document)<=1048576 THEN document END,CASE WHEN octet_length(plan_json)<=262144 THEN plan_json END,id,raw_sha256,plan_digest,receipt_sequence,record_digest,run_id FROM factory_child_plan_revisions WHERE scope_id=?1 AND task_id=?2 AND revision=?3",
        params![scope_id,task_id,i64::try_from(revision).map_err(|_| invalid())?],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).optional().map_err(db_error)?;
    let (document, raw, id, raw_sha, plan_digest, seq, record_digest, run_id) = stored
        .ok_or_else(|| CoreError::new("NOT_FOUND", "Factory child plan revision not found"))?;
    let view: FactoryChildPlanView = decode(&document.ok_or_else(limited)?)?;
    let raw = raw.ok_or_else(limited)?;
    if view.reference.scope_id != scope_id
        || view.reference.task_id != task_id
        || view.reference.revision != revision
        || view.reference.id != id
        || view.reference.run_id != run_id
        || view.reference.raw_sha256 != raw_sha
        || view.reference.plan_digest != plan_digest
        || view.reference.receipt_sequence != u64::try_from(seq).map_err(|_| changed())?
        || raw != view.plan_json
        || agentbaton_workspaces::validation::sha256(raw.as_bytes()) != raw_sha
        || parse_plan(&raw, MAX_RAW)? != view.proposal
        || commitment(&view)? != record_digest
        || !view.planning_only
        || view.dispatched
        || view.task_accepted
        || view.authority_kind != AUTHORITY
    {
        return Err(changed());
    }
    let raw:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE sequence=?1",[seq],|r|r.get(0)).optional().map_err(db_error)?;
    let receipt: crate::storage::Event = decode(&raw.flatten().ok_or_else(changed)?)?;
    if receipt.source != "factory_child_planning"
        || receipt.kind != "factory.child_plan_stored"
        || receipt.aggregate_id != id
        || receipt.revision != revision
        || receipt.payload["record_digest"] != record_digest
        || receipt.payload["scope_id"] != scope_id
        || receipt.payload["task_id"] != task_id
        || receipt.payload["plan_digest"] != plan_digest
        || receipt.payload["raw_sha256"] != raw_sha
        || receipt.payload["dispatched"] != false
        || receipt.payload["task_accepted"] != false
    {
        return Err(changed());
    }
    let s = scope(db, scope_id)?;
    let b = binding(db, &run_id, task_id)?;
    let policy = approved_binding(&s)?;
    if b.scope_id != scope_id
        || view.scope_digest != s.contract_digest
        || view.planning_policy_digest != policy.definition_digest
        || view.root_task_id != s.root_task_id
        || view.root_grant_id != s.root_grant_id
        || view.root_grant_revision != s.root_grant_revision
        || view.proposal.goal.task_id != task_id
        || view.proposal.goal.root_task_id != s.root_task_id
        || view.proposal.goal.project_id != s.project_id
        || view.target_spec_revision != view.proposal.goal.spec_revision
        || view.input_revision != view.proposal.goal.input_revision
    {
        return Err(changed());
    }
    match (
        &view.caller_session_id,
        &view.caller_attempt_id,
        view.caller_fencing_token,
    ) {
        (None, None, None) if view.caller_task_id == task_id => {}
        (Some(session), Some(attempt), Some(fence)) => {
            if !ancestry(db, task_id)?.contains(&view.caller_task_id) {
                return Err(changed());
            }
            let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM factory_attempts a JOIN sessions s ON s.id=a.session_id WHERE a.session_id=?1 AND a.attempt_id=?2 AND a.scope_id=?3 AND a.run_id=?4 AND a.task_id=?5 AND a.root_task_id=?6 AND json_extract(s.document,'$.fencing_token')=?7)",params![session,attempt,scope_id,run_id,view.caller_task_id,view.root_task_id,i64::try_from(fence).map_err(|_|changed())?],|r|r.get(0)).map_err(db_error)?;
            if !valid {
                return Err(changed());
            }
        }
        _ => return Err(changed()),
    }
    check_view_wire(&view)?;
    Ok(view)
}
fn head(db: &Connection, scope_id: &str, task_id: &str) -> Result<Option<FactoryChildPlanView>> {
    let h:Option<(String,String,i64)>=db.query_row("SELECT revision_id,run_id,revision FROM factory_child_plan_heads WHERE scope_id=?1 AND task_id=?2",params![scope_id,task_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?;
    match h {
        None => Ok(None),
        Some((id, run_id, rev)) => {
            let v = row(
                db,
                scope_id,
                task_id,
                u64::try_from(rev).map_err(|_| changed())?,
            )?;
            if v.reference.id != id || v.reference.run_id != run_id {
                return Err(changed());
            }
            Ok(Some(v))
        }
    }
}
pub(super) fn latest_reference(
    db: &Connection,
    scope_id: &str,
    task_id: &str,
) -> Result<Option<FactoryChildPlanRef>> {
    Ok(head(db, scope_id, task_id)?.map(|v| v.reference))
}
fn mutable_target(db: &Connection, s: &FactoryScope, b: &TaskBinding) -> Result<()> {
    base_live(db, s)?;
    let current = binding(db, &b.run_id, &b.handle.task_id)?;
    if current.handle.revision != b.handle.revision
        || encode(&current.request)? != encode(&b.request)?
    {
        return Err(changed());
    }

    let r = run(db, &s.id)?;
    let task: crate::storage::Task = Store::factory_child_read_task(db, &b.handle.task_id)?;
    if r.cancel_requested
        || matches!(
            r.state.as_str(),
            "outcome_unknown" | "cancelled" | "blocked" | "completed"
        )
        || b.handle.cancel_requested
        || !matches!(
            b.handle.state.as_str(),
            "queued"
                | "blocked_on_live_parent"
                | "starting"
                | "running"
                | "waiting_child"
                | "resuming"
        )
        || matches!(
            task.state,
            TaskState::Accepted | TaskState::CancelRequested | TaskState::Cancelled
        )
        || task.project_id != s.project_id
        || b.planning_spec_revision != Some(task.spec_revision)
        || task.goal != b.request.goal
    {
        return Err(denied());
    }
    require_planning_task_state(db, s, b, &task)?;
    Ok(())
}
fn require_planning_task_state(
    db: &Connection,
    s: &FactoryScope,
    b: &TaskBinding,
    task: &crate::storage::Task,
) -> Result<()> {
    if task.state != TaskState::OutcomeUnknown {
        return Ok(());
    }
    let current_run = run(db, &s.id)?;
    if current_run.cancel_requested
        || !matches!(
            current_run.state.as_str(),
            "running" | "waiting_child" | "starting" | "resuming"
        )
    {
        return Err(denied());
    }
    // The generic dispatch ledger records a stopped Factory parent as unknown.
    // Only the exact, durably joined normal handoff may continue planning;
    // neither an unknown Factory outcome nor a state label supplies this proof.
    if b.scope_id != s.id
        || b.handle.state != "waiting_child"
        || !b.handle.confirmed_stop
        || b.handle.cancel_requested
        || b.stop_reason.as_deref() != Some("completed_turn_handoff")
    {
        return Err(denied());
    }
    let session = b.session_id.as_deref().ok_or_else(denied)?;
    let attempt = b.handle.attempt_id.as_deref().ok_or_else(denied)?;
    let proven: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM factory_attempts a JOIN sessions se ON se.id=a.session_id JOIN events p JOIN events j JOIN events f WHERE a.session_id=?1 AND a.attempt_id=?2 AND a.scope_id=?3 AND a.run_id=?4 AND a.task_id=?5 AND a.root_task_id=?6 AND a.state='waiting_child' AND json_extract(a.document,'$.stop_confirmed')=1 AND json_extract(a.document,'$.never_started')=0 AND json_extract(se.document,'$.state')='stopped' AND json_extract(se.document,'$.attempt_id')=?2 AND json_extract(se.document,'$.turn_state')='completed' AND p.sequence<j.sequence AND j.sequence<f.sequence AND json_extract(p.document,'$.source')='factory_core' AND json_extract(p.document,'$.kind')='factory.internal_stop_prepared' AND json_extract(p.document,'$.aggregate_id')=?1 AND json_extract(p.document,'$.payload.attempt_id')=?2 AND json_extract(p.document,'$.payload.turn_state')='completed' AND json_extract(j.document,'$.source')='baton_supervisor' AND json_extract(j.document,'$.kind')='session.stopped' AND json_extract(j.document,'$.aggregate_id')=?1 AND json_extract(j.document,'$.payload.process_reaped')=1 AND json_extract(j.document,'$.payload.no_process_started')=0 AND json_extract(f.document,'$.source')='factory_core' AND json_extract(f.document,'$.kind')='factory.attempt_stopped' AND json_extract(f.document,'$.aggregate_id')=?4 AND json_extract(f.document,'$.payload.session_id')=?1 AND json_extract(f.document,'$.payload.attempt_id')=?2 AND json_extract(f.document,'$.payload.task_id')=?5 AND json_extract(f.document,'$.payload.owned_reaped')=1 AND json_extract(f.document,'$.payload.normal_completed_turn')=1)",
        params![session, attempt, s.id, b.run_id, task.id, s.root_task_id], |r| r.get(0),
    ).map_err(db_error)?;
    if !proven {
        return Err(denied());
    }
    Ok(())
}
pub(super) fn recorded_context(
    db: &Connection,
    s: &FactoryScope,
    b: &TaskBinding,
) -> Result<FactoryChildPlanningContext> {
    let task = Store::factory_child_read_task(db, &b.handle.task_id)?;
    let a = approved_binding(s)?;
    let p = host_policy(s, b)?;
    let bindings = p
        .connections
        .iter()
        .map(|(id, c)| crate::execution::ExecutionBinding {
            connection_id: id.clone(),
            environment_id: c.binding.environment_id.clone(),
            identity_ref: c.binding.identity_ref.clone(),
            host_id: c.binding.host_id.clone(),
            workspace_id: b.request.workspace_id.clone(),
            config_revision: c.binding.config_revision.clone(),
            input_revision: p.input_revision.clone(),
            capability_snapshot_id: c.binding.capability_snapshot_id.clone(),
            policy_revision: format!("{}:{}", s.root_grant_id, s.root_grant_revision),
        })
        .collect();
    let output_artifacts = a
        .policy
        .outputs
        .iter()
        .filter(|o| {
            b.request
                .output_artifact_ids
                .contains(&o.artifact.artifact_id)
        })
        .map(|o| (o.artifact.artifact_id.clone(), o.artifact.clone()))
        .collect();
    let view = FactoryChildPlanningContext {
        scope_id: s.id.clone(),
        run_id: b.run_id.clone(),
        scope_revision: 1,
        scope_digest: s.contract_digest.clone(),
        planning_policy_digest: a.definition_digest.clone(),
        goal: GoalBinding {
            project_id: s.project_id.clone(),
            root_task_id: s.root_task_id.clone(),
            task_id: task.id,
            spec_revision: b.planning_spec_revision.ok_or_else(changed)?,
            input_revision: p.input_revision.clone(),
        },
        policy: a.policy.clone(),
        execution_bindings: bindings,
        edit_context: crate::goal_store::GoalPlanEditingContext {
            max_nodes: p.max_nodes,
            writable_prefixes: p.workspaces[&b.request.workspace_id]
                .writable_prefixes
                .clone(),
            output_artifacts,
            rule_executors: p
                .rules
                .iter()
                .map(|(id, r)| {
                    (
                        id.clone(),
                        if r.kind == RuleKind::Owner {
                            crate::goal_store::GoalRuleExecutorKind::Owner
                        } else {
                            crate::goal_store::GoalRuleExecutorKind::Validator
                        },
                    )
                })
                .collect(),
        },
        program_sha256: p
            .connections
            .iter()
            .map(|(id, c)| (id.clone(), c.binding.executable_sha256.clone()))
            .collect(),
        inputs: p
            .external_inputs
            .iter()
            .map(|(id, i)| {
                (
                    id.clone(),
                    crate::goal_store::GoalInputView {
                        contract_id: i.contract_id.clone(),
                        sha256: i.sha256.clone(),
                    },
                )
            })
            .collect(),
        outputs: p.required_goal_outputs.clone(),
        contracts: p
            .contracts
            .iter()
            .map(|(id, c)| (id.clone(), c.required_validators.iter().cloned().collect()))
            .collect(),
        rules: p
            .rules
            .iter()
            .map(|(id, r)| (id.clone(), r.policy_digest.clone()))
            .collect(),
        required_acceptance_rules: p.acceptance_rules.into_iter().collect(),
        budget: b.request.budget.clone(),
        project_budget: s.selection.project_budget.clone(),
        current_plan_revision: head(db, &s.id, &b.handle.task_id)?
            .map_or(0, |h| h.reference.revision),
        planning_only: true,
        goal_execution_authorized: false,
        new_workspace_authorized: false,
    };
    check_wire(
        &json!({"request_id":"\0".repeat(128),"result":view,"revision":u64::MAX,"warnings":[]}),
    )?;
    Ok(view)
}
impl Store {
    fn factory_child_read_task(db: &Connection, id: &str) -> Result<crate::storage::Task> {
        let raw:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM tasks WHERE id=?1",[id],|r|r.get(0)).optional().map_err(db_error)?;
        decode(&raw.flatten().ok_or_else(changed)?)
    }
    pub(crate) fn check_factory_child_plan_scope(&self, s: &FactoryScope) -> Result<()> {
        approved_binding(s)?;
        base_live(&self.conn, s)?;
        let r = run(&self.conn, &s.id)?;
        if r.cancel_requested
            || matches!(
                r.state.as_str(),
                "cancelled" | "blocked" | "completed" | "outcome_unknown"
            )
        {
            return Err(denied());
        }
        let root = binding(&self.conn, &r.id, &s.root_task_id)?;
        let task = Self::factory_child_read_task(&self.conn, &s.root_task_id)?;
        require_planning_task_state(&self.conn, s, &root, &task)?;
        Ok(())
    }
    pub(crate) fn check_factory_child_plan_bindings(
        &self,
        selection: &FactoryScopeSelection,
        bindings: &FactoryScopeBindings,
    ) -> Result<()> {
        let resolved = resolve_binding(&self.conn, selection, &bindings.workspaces[0])?;
        if json!(resolved) != json!(bindings.child_planning) {
            return Err(changed());
        }
        Ok(())
    }
    pub(crate) fn prepare_factory_scoped_child_plan_read(
        &self,
        handle: &str,
        token: &str,
        request: &FactoryChildPlanSubmission,
        raw: &str,
    ) -> Result<crate::factory::FactoryReadPlan> {
        let caller = self.authenticate_factory_base(handle, token)?.0;
        let target = authorized_target(&self.conn, &caller, &request.task_id)?;
        mutable_target(&self.conn, &caller.scope, &target)?;
        self.prepare_factory_child_plan_read(&caller.scope, raw)
    }
    fn require_factory_child_plan_observation(&self, scope: &FactoryScope) -> Result<()> {
        if !self
            .data_input
            .as_ref()
            .and_then(|w| w.factory.as_ref())
            .is_some_and(|w| w.is_child_planning())
        {
            return Err(CoreError::new("WORKSPACE_BUSY","A new child plan requires the exact request's joined bounded Factory input observation"));
        }
        let current =
            self.resolve_factory_bindings(&scope.selection, &scope.bindings.companion.executable)?;
        if json!(current) != json!(scope.bindings) {
            return Err(changed());
        }
        Ok(())
    }
    pub(crate) fn factory_child_plan_owner_replay(
        &self,
        p: &FactoryChildPlanUpdate,
        revision: u64,
        key_id: &str,
    ) -> Result<bool> {
        let s = scope(&self.conn, &p.scope_id)?;
        approved_binding(&s)?;
        let payload = encode(&(
            revision,
            FactoryChildPlanSubmission {
                task_id: p.task_id.clone(),
                plan_json: p.plan_json.clone(),
                expected_plan_revision: p.expected_plan_revision,
            },
        ))?;
        let ns = format!("factory:{}:owner:{}:child-plan", s.id, p.task_id);
        match replay::<FactoryChildPlanView>(&self.conn, &ns, key_id, &payload)? {
            None => Ok(false),
            Some(old) => {
                let stored = row(&self.conn, &s.id, &p.task_id, old.reference.revision)?;
                if encode(&old)? != encode(&stored)? {
                    return Err(changed());
                }
                Ok(true)
            }
        }
    }
    pub fn get_factory_child_plan(
        &self,
        q: &FactoryChildPlanQuery,
    ) -> Result<FactoryChildPlanView> {
        let s = scope(&self.conn, &q.scope_id)?;
        let r = run(&self.conn, &s.id)?;
        let b = binding(&self.conn, &r.id, &q.task_id)?;
        if b.scope_id != s.id {
            return Err(denied());
        }
        approved_binding(&s)?;
        match q.plan_revision {
            Some(revision) => row(&self.conn, &s.id, &q.task_id, revision),
            None => head(&self.conn, &s.id, &q.task_id)?
                .ok_or_else(|| CoreError::new("NOT_FOUND", "Factory child has no stored plan")),
        }
    }
    pub fn list_factory_child_plans(
        &self,
        q: &FactoryChildPlanQuery,
    ) -> Result<Vec<FactoryChildPlanRef>> {
        if q.plan_revision.is_some() {
            return Err(invalid());
        }
        let s = scope(&self.conn, &q.scope_id)?;
        let r = run(&self.conn, &s.id)?;
        let b = binding(&self.conn, &r.id, &q.task_id)?;
        if b.scope_id != s.id {
            return Err(denied());
        }
        approved_binding(&s)?;
        stored_budget(&self.conn, &s.id, &q.task_id)?;
        // Project references inside SQLite; never collect historical raw plans
        // or complete View documents into the Actor merely to list revisions.
        let mut stmt=self.conn.prepare("SELECT json_extract(document,'$.reference'),id,run_id,revision,raw_sha256,plan_digest,receipt_sequence,record_digest FROM factory_child_plan_revisions WHERE scope_id=?1 AND task_id=?2 ORDER BY revision").map_err(db_error)?;
        let rows = stmt
            .query_map(params![s.id, q.task_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, String>(7)?,
                ))
            })
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        let mut refs = Vec::new();
        for (raw, id, run_id, revision, raw_sha, plan_digest, seq, record_digest) in rows {
            let value: FactoryChildPlanRef = decode(&raw)?;
            if value.id != id
                || value.scope_id != s.id
                || value.task_id != q.task_id
                || value.run_id != run_id
                || run_id != r.id
                || value.revision != u64::try_from(revision).map_err(|_| changed())?
                || value.raw_sha256 != raw_sha
                || value.plan_digest != plan_digest
                || value.receipt_sequence != u64::try_from(seq).map_err(|_| changed())?
            {
                return Err(changed());
            }
            let valid:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE sequence=?1 AND octet_length(document)<=1048576 AND json_extract(document,'$.source')='factory_child_planning' AND json_extract(document,'$.kind')='factory.child_plan_stored' AND json_extract(document,'$.aggregate_id')=?2 AND json_extract(document,'$.payload.record_digest')=?3 AND json_extract(document,'$.payload.scope_id')=?4 AND json_extract(document,'$.payload.task_id')=?5 AND json_extract(document,'$.payload.raw_sha256')=?6 AND json_extract(document,'$.payload.plan_digest')=?7)",params![seq,id,record_digest,s.id,q.task_id,raw_sha,plan_digest],|row|row.get(0)).map_err(db_error)?;
            if !valid || revision != refs.len() as i64 + 1 || seq == 0 {
                return Err(changed());
            }
            refs.push(value);
        }
        check_wire(
            &json!({"request_id":"\0".repeat(128),"result":refs,"revision":u64::MAX,"warnings":[]}),
        )?;
        Ok(refs)
    }
    pub fn factory_child_planning_context(
        &self,
        q: &FactoryChildPlanQuery,
    ) -> Result<FactoryChildPlanningContext> {
        if q.plan_revision.is_some() {
            return Err(invalid());
        }
        let s = scope(&self.conn, &q.scope_id)?;
        let r = run(&self.conn, &s.id)?;
        let b = binding(&self.conn, &r.id, &q.task_id)?;
        let task = Self::factory_child_read_task(&self.conn, &q.task_id)?;
        if b.scope_id != s.id || task.project_id != s.project_id {
            return Err(denied());
        }
        if !s.revoked
            && s.expires_at_ms > ms()?
            && !matches!(
                r.state.as_str(),
                "cancelled" | "blocked" | "completed" | "outcome_unknown"
            )
        {
            self.require_factory_child_plan_observation(&s)?;
        }
        recorded_context(&self.conn, &s, &b)
    }
    pub fn submit_factory_child_plan(
        &mut self,
        p: &FactoryChildPlanUpdate,
        scope_revision: u64,
        key_id: &str,
    ) -> Result<FactoryChildPlanView> {
        let s = scope(&self.conn, &p.scope_id)?;
        let r = run(&self.conn, &s.id)?;
        let b = binding(&self.conn, &r.id, &p.task_id)?;
        if b.scope_id != s.id {
            return Err(denied());
        }
        self.save_factory_child_plan(
            s,
            b,
            &FactoryChildPlanSubmission {
                task_id: p.task_id.clone(),
                plan_json: p.plan_json.clone(),
                expected_plan_revision: p.expected_plan_revision,
            },
            scope_revision,
            key_id,
            None,
        )
    }
    pub(crate) fn factory_submit_child_plan(
        &mut self,
        caller: FactoryCaller,
        p: &FactoryChildPlanSubmission,
        key_id: &str,
    ) -> Result<FactoryToolOutcome> {
        let target = authorized_target(&self.conn, &caller, &p.task_id)?;
        let identity = Some((
            caller.task.handle.task_id.clone(),
            caller.session.id.clone(),
            caller.session.attempt_id.clone(),
            caller.session.fencing_token,
        ));
        let revision = caller.scope.revision;
        let view =
            self.save_factory_child_plan(caller.scope, target, p, revision, key_id, identity)?;
        Ok(FactoryToolOutcome {
            result: json!(view),
            stops: vec![],
        })
    }
    fn save_factory_child_plan(
        &mut self,
        s: FactoryScope,
        b: TaskBinding,
        p: &FactoryChildPlanSubmission,
        scope_revision: u64,
        key_id: &str,
        caller: Option<(String, String, String, u64)>,
    ) -> Result<FactoryChildPlanView> {
        key(key_id)?;
        key(&p.task_id)?;
        let a = approved_binding(&s)?;
        let raw = parse_plan(&p.plan_json, a.policy.max_plan_bytes as usize)?;
        let caller_task = caller.as_ref().map_or(p.task_id.as_str(), |v| v.0.as_str());
        let ns = match &caller {
            None => format!("factory:{}:owner:{}:child-plan", s.id, p.task_id),
            Some(_) => format!(
                "factory:{}:scoped:{}:{}:child-plan",
                s.id, caller_task, p.task_id
            ),
        };
        let payload = encode(&(scope_revision, p))?;
        if let Some(old) = replay::<FactoryChildPlanView>(&self.conn, &ns, key_id, &payload)? {
            let stored = row(&self.conn, &s.id, &p.task_id, old.reference.revision)?;
            if encode(&old)? != encode(&stored)? {
                return Err(changed());
            }
            return Ok(stored);
        }
        if s.revision != scope_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Factory scope changed before child planning",
            ));
        }
        mutable_target(&self.conn, &s, &b)?;
        self.require_factory_child_plan_observation(&s)?;
        let context = self.factory_child_planning_context(&FactoryChildPlanQuery {
            scope_id: s.id.clone(),
            task_id: p.task_id.clone(),
            plan_revision: None,
        })?;
        let previous = head(&self.conn, &s.id, &p.task_id)?;
        if previous.as_ref().map_or(0, |v| v.reference.revision) != p.expected_plan_revision
            || raw.revision > u64::from(a.policy.max_revisions)
        {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Child plan head/CAS or revision bound changed",
            ));
        }
        let expected_inputs: BTreeSet<_> = b.request.input_artifact_ids.iter().cloned().collect();
        let actual_inputs: BTreeSet<_> = raw
            .nodes
            .iter()
            .flat_map(|n| n.required_inputs.iter())
            .filter(|i| matches!(i.source, InputSource::ApprovedExternal { .. }))
            .map(|i| i.artifact_id.clone())
            .collect();
        if actual_inputs != expected_inputs {
            return Err(changed());
        }
        if raw.nodes.iter().any(|n| {
            n.budget.tokens.is_some()
                || n.budget.cost_microunits.is_some()
                || n.reservation.tokens.is_some()
                || n.reservation.cost_microunits.is_some()
        }) {
            return Err(CoreError::new(
                "ENFORCEMENT_UNAVAILABLE",
                "Factory child plans cannot invent reliable token/cost limits",
            ));
        }
        let policy = host_policy(&s, &b)?;
        // Factory output approvals name exact regular-file destinations. A
        // file spelling must not become an authority prefix for descendants.
        let allowed_writes = &policy.workspaces[&b.request.workspace_id].writable_prefixes;
        if raw
            .nodes
            .iter()
            .flat_map(|n| &n.write_set)
            .any(|w| match w {
                WriteTarget::Workspace { path } => !allowed_writes.contains(path),
                WriteTarget::External { .. } => true,
            })
        {
            return Err(denied());
        }
        let (digest, _) = goal_plan::validate_factory_child_plan(
            &context.goal,
            s.revision,
            &s.bindings.approved_grant,
            &policy,
            &a.definition_digest,
            &raw,
            previous.as_ref().map(|v| &v.proposal),
        )
        .map_err(planning_error)?;
        let mut view = FactoryChildPlanView {
            reference: FactoryChildPlanRef {
                id: Uuid::new_v4().to_string(),
                scope_id: s.id.clone(),
                run_id: b.run_id.clone(),
                task_id: p.task_id.clone(),
                revision: raw.revision,
                plan_digest: digest,
                raw_sha256: agentbaton_workspaces::validation::sha256(p.plan_json.as_bytes()),
                receipt_sequence: 0,
            },
            proposal: raw,
            plan_json: p.plan_json.clone(),
            authority_kind: AUTHORITY.into(),
            scope_digest: s.contract_digest.clone(),
            planning_policy_digest: a.definition_digest.clone(),
            root_task_id: s.root_task_id.clone(),
            root_grant_id: s.root_grant_id.clone(),
            root_grant_revision: s.root_grant_revision,
            caller_task_id: caller_task.into(),
            caller_session_id: caller.as_ref().map(|v| v.1.clone()),
            caller_attempt_id: caller.as_ref().map(|v| v.2.clone()),
            caller_fencing_token: caller.as_ref().map(|v| v.3),
            target_spec_revision: context.goal.spec_revision,
            input_revision: context.goal.input_revision,
            created_at: now()?,
            planning_only: true,
            dispatched: false,
            task_accepted: false,
        };
        check_view_wire(&view)?;
        let record_digest = commitment(&view)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        mutable_target(&tx, &s, &b)?;
        if head(&tx, &s.id, &p.task_id)?.map_or(0, |v| v.reference.revision)
            != p.expected_plan_revision
        {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Child plan changed before atomic commit",
            ));
        }
        let(count,bytes):(i64,i64)=tx.query_row("SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(plan_json)),0) FROM factory_child_plan_revisions WHERE scope_id=?1",[&s.id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        if count >= i64::from(s.selection.max_total_nodes) * i64::from(a.policy.max_revisions)
            || bytes + encode(&view)?.len() as i64 + p.plan_json.len() as i64 > MAX_SCOPE_HISTORY
        {
            return Err(limited());
        }
        let seq = event(
            &tx,
            &view.reference.id,
            view.reference.revision,
            "factory.child_plan_stored",
            json!({"scope_id":s.id,"run_id":b.run_id,"task_id":p.task_id,"plan_digest":view.reference.plan_digest,"raw_sha256":view.reference.raw_sha256,"record_digest":record_digest,"planning_only":true,"dispatched":false,"task_accepted":false,"new_grant":false}),
            "factory_child_planning",
        )?;
        view.reference.receipt_sequence = u64::try_from(seq).map_err(|_| limited())?;
        tx.execute("INSERT INTO factory_child_plan_revisions(id,scope_id,run_id,task_id,revision,plan_json,raw_sha256,plan_digest,receipt_sequence,record_digest,document) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![view.reference.id,s.id,b.run_id,p.task_id,i64::try_from(view.reference.revision).map_err(|_|limited())?,p.plan_json,view.reference.raw_sha256,view.reference.plan_digest,seq,record_digest,encode(&view)?]).map_err(db_error)?;
        tx.execute("INSERT INTO factory_child_plan_heads(scope_id,run_id,task_id,revision,revision_id) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(scope_id,task_id) DO UPDATE SET revision=excluded.revision,revision_id=excluded.revision_id",params![s.id,b.run_id,p.task_id,i64::try_from(view.reference.revision).map_err(|_|limited())?,view.reference.id]).map_err(db_error)?;
        // Pre-decode aggregate caps and the full history response must fit; a
        // failed check rolls back head, revision, event and receipt together.
        let(c,n):(i64,i64)=tx.query_row("SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(plan_json)),0) FROM factory_child_plan_revisions WHERE scope_id=?1 AND task_id=?2",params![s.id,p.task_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        if c > 256 || n > MAX_SCOPE_HISTORY {
            return Err(limited());
        }
        check_view_wire(&view)?;
        remember(&tx, &ns, key_id, &payload, &view)?;
        tx.commit().map_err(db_error)?;
        Ok(view)
    }
}

#[cfg(test)]
mod recorded_child_planning_tests {
    use super::*;
    #[test]
    fn actual_recorded_parent_yield_requires_normal_ordered_joined_same_attempt_not_unknown_run() {
        let v: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/factory_actual_parent_yield.json"
        ))
        .unwrap();
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE factory_runs(id TEXT,scope_id TEXT,root_task_id TEXT,project_id TEXT,revision INTEGER,state TEXT,document TEXT); CREATE TABLE factory_attempts(session_id TEXT,attempt_id TEXT,scope_id TEXT,run_id TEXT,task_id TEXT,root_task_id TEXT,state TEXT,document TEXT); CREATE TABLE sessions(id TEXT,document TEXT); CREATE TABLE events(sequence INTEGER PRIMARY KEY,document TEXT);").unwrap();
        let s: FactoryScope = serde_json::from_value(v["scope"].clone()).unwrap();
        let b: TaskBinding = serde_json::from_value(v["binding"].clone()).unwrap();
        let t: crate::storage::Task = serde_json::from_value(v["task"].clone()).unwrap();
        let r = &v["run"];
        let a = &v["attempt"];
        db.execute(
            "INSERT INTO factory_runs VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                r["id"].as_str(),
                r["scope_id"].as_str(),
                r["root_task_id"].as_str(),
                r["project_id"].as_str(),
                r["revision"].as_i64(),
                r["state"].as_str(),
                r["document"].as_str()
            ],
        )
        .unwrap();
        db.execute(
            "INSERT INTO factory_attempts VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                a["session_id"].as_str(),
                a["attempt_id"].as_str(),
                a["scope_id"].as_str(),
                a["run_id"].as_str(),
                a["task_id"].as_str(),
                a["root_task_id"].as_str(),
                a["state"].as_str(),
                a["document"].as_str()
            ],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sessions VALUES(?1,?2)",
            params![v["session"]["id"].as_str(), v["session"].to_string()],
        )
        .unwrap();
        for e in v["events"].as_array().unwrap() {
            db.execute(
                "INSERT INTO events VALUES(?1,?2)",
                params![e["sequence"].as_i64(), e["document"].as_str()],
            )
            .unwrap();
        }
        assert_eq!(t.state, TaskState::OutcomeUnknown);
        require_planning_task_state(&db, &s, &b, &t).unwrap();
        // These are controls on recorded real rows, not new execution/join proof.
        for (name, sql) in [
            ("actual_factory_unknown", "UPDATE factory_runs SET state='outcome_unknown',document=json_set(document,'$.state','outcome_unknown')"),
            ("cancelled_run", "UPDATE factory_runs SET document=json_set(document,'$.cancel_requested',json('true'))"),
            ("normal_flag_false", "UPDATE events SET document=json_set(document,'$.payload.normal_completed_turn',json('false')) WHERE json_extract(document,'$.kind')='factory.attempt_stopped'"),
            ("foreign_source", "UPDATE events SET document=json_set(document,'$.source','untrusted_agent') WHERE json_extract(document,'$.kind')='factory.attempt_stopped'"),
            ("join_false", "UPDATE events SET document=json_set(document,'$.payload.process_reaped',json('false')) WHERE json_extract(document,'$.kind')='session.stopped'"),
            ("wrong_attempt", "UPDATE factory_attempts SET attempt_id='foreign-attempt'"),
            ("missing_prepared", "DELETE FROM events WHERE json_extract(document,'$.kind')='factory.internal_stop_prepared'"),
            ("reversed_chain", "UPDATE events SET sequence=sequence+1000 WHERE json_extract(document,'$.kind')='factory.internal_stop_prepared'"),
            ("unconfirmed_attempt", "UPDATE factory_attempts SET document=json_set(document,'$.stop_confirmed',json('false'))"),
        ] {
            db.execute_batch("SAVEPOINT negative").unwrap();
            db.execute(sql, []).unwrap();
            let before: i64 = db.query_row("SELECT count(*) FROM events", [], |r|r.get(0)).unwrap();
            assert_eq!(require_planning_task_state(&db, &s, &b, &t).unwrap_err().code, "AUTH_REQUIRED", "{name}");
            assert_eq!(db.query_row("SELECT count(*) FROM events", [], |r|r.get::<_,i64>(0)).unwrap(), before);
            db.execute_batch("ROLLBACK TO negative;RELEASE negative").unwrap();
            require_planning_task_state(&db, &s, &b, &t).unwrap();
        }
        for state in ["outcome_unknown", "running", "queued"] {
            let mut forged = b.clone();
            forged.handle.state = state.into();
            assert_eq!(
                require_planning_task_state(&db, &s, &forged, &t)
                    .unwrap_err()
                    .code,
                "AUTH_REQUIRED"
            );
        }
    }
}
