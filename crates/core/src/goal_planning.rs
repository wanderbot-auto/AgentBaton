//! Explicit, single-turn planning using the approved connection. This is a real
//! writer with shared admission, never a sandbox or execution approval.
use super::*;
use crate::conversations::Conversation;
const PURPOSE: &str = "goal-planning-purpose-v1";
const COMPLETION: &str = "goal-planning-completion-v1";
const MAX_RECORD: usize = 524_288;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanningPrepareRequest {
    pub query: GoalPlanAdviceQuery,
    pub runtime_ms: u64,
    pub response_bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanningPrepared {
    pub source: GoalPlanDraftSource,
    pub context: GoalPlanAdviceContext,
    pub binding: ExecutionBinding,
    pub program_sha256: String,
    pub grant_id: String,
    pub grant_revision: u64,
    pub requested_scope: crate::policy::GrantScope,
    pub enforcement: crate::policy::GrantEnforcement,
    pub runtime_ms: u64,
    pub response_bytes: u64,
    pub prompt: String,
    pub prompt_sha256: String,
    pub digest: String,
    pub planning_only: bool,
    pub dispatched: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanningStartRequest {
    pub preparation: GoalPlanningPrepareRequest,
    pub expected_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanningId {
    pub conversation_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanningListRequest {
    pub task_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanningRun {
    pub conversation_id: String,
    pub conversation: Conversation,
    pub source: GoalPlanDraftSource,
    pub preparation_digest: String,
    pub prompt_sha256: String,
    pub runtime_ms: u64,
    pub response_bytes: u64,
    pub request_key: String,
    pub reply: Option<String>,
    pub reply_complete: bool,
    pub process_reaped: bool,
    pub worker_joined: bool,
    pub observed_output_bytes: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanningPurpose {
    pub conversation_id: String,
    pub attempt_id: String,
    pub turn_id: String,
    pub request_key: String,
    pub request: GoalPlanningStartRequest,
    pub prepared: GoalPlanningPrepared,
    pub project_id: String,
    pub grant_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanningCompletion {
    pub purpose_digest: String,
    pub accounted_output_bytes: Option<u64>,
    pub elapsed_ms: u64,
    pub reply: Option<String>,
    pub reply_complete: bool,
    pub process_reaped: bool,
    pub worker_joined: bool,
    pub observed_output_bytes: Option<u64>,
}
fn digest_prepared(p: &GoalPlanningPrepared) -> Result<String> {
    let mut p = p.clone();
    p.digest.clear();
    sha(&p)
}
pub(crate) fn prepare_from(
    db: &Connection,
    r: &GoalPlanningPrepareRequest,
) -> Result<(GoalPlanningPrepared, String, String)> {
    if r.runtime_ms == 0 || !(1..=65_536).contains(&r.response_bytes) {
        return Err(invalid());
    }
    let (record, view, source) = advice::current_scope(db, &r.query, None)?;
    if !matches!(
        record.refs.workspace.identity.as_ref().map(|i| &i.kind),
        Some(agentbaton_workspaces::WorkspaceKind::Git { .. })
    ) {
        return Err(err(
            "CAPABILITY_UNSUPPORTED",
            "Planning requires a managed Git workspace",
        ));
    }
    let root = &view.root_budget;
    let project = &view.project_budget;
    if root.tokens.is_some()
        || root.cost_microunits.is_some()
        || project.tokens.is_some()
        || project.cost_microunits.is_some()
    {
        return Err(err(
            "BUDGET_UNKNOWN",
            "Planning has no reliable token or cost meter",
        ));
    }
    if r.runtime_ms
        > root
            .runtime_ms
            .min(project.runtime_ms)
            .min(record.refs.grant_after.scope.max_duration_ms)
        || r.response_bytes > root.output_bytes.min(project.output_bytes)
    {
        return Err(err(
            "RESOURCE_LIMIT",
            "Planning exceeds approved finite limits",
        ));
    }
    let connection: crate::sessions::ProviderConnection =
        serde_json::from_value(record.refs.connection.clone()).map_err(|_| changed())?;
    crate::runtime_config::approved_plan(&connection)?;
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
    let context = GoalPlanAdviceContext {
        source: source.clone(),
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
    let prompt=format!("Provide planning advice only. Do not execute the goal or change files. Return exactly one JSON object, without markdown: {{\"schema_version\":1,\"source\":{},\"steps\":[{{\"id\":\"step-1\",\"goal\":\"describe work\",\"dependencies\":[],\"input_artifact_ids\":[],\"output_artifact_ids\":[]}}]}}. Preserve source exactly. Use 1 to 8 work steps, assign every approved output exactly once, only reference approved inputs or another step's outputs, and keep dependencies acyclic. Do not include budgets, authority, execution approval, or extra fields. Required validation and owner review gates will be added by the application. Approved context follows:\n{}",encode(&source)?,encode(&context)?);
    if prompt.len() > 65_536 {
        return Err(limited());
    }
    let prompt_sha256 = format!("{:x}", Sha256::digest(prompt.as_bytes()));
    let mut prepared = GoalPlanningPrepared {
        source,
        context,
        binding: view.binding,
        program_sha256: view.program_sha256,
        grant_id: record.refs.grant_after.id.clone(),
        grant_revision: record.refs.grant_after.revision,
        requested_scope: record.refs.grant_after.scope.clone(),
        enforcement: record.refs.grant_after.enforcement.clone(),
        runtime_ms: r.runtime_ms,
        response_bytes: r.response_bytes,
        prompt,
        prompt_sha256,
        digest: String::new(),
        planning_only: true,
        dispatched: false,
    };
    prepared.digest = digest_prepared(&prepared)?;
    Ok((prepared, record.refs.project_id, record.refs.grant_after.id))
}
fn bounded_row(db: &Connection, scope: &str, id: &str) -> Result<Option<(String, String)>> {
    let sizes:Option<(i64,i64)>=db.query_row("SELECT octet_length(payload),octet_length(result) FROM idempotency WHERE scope=?1 AND key=?2",params![scope,id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
    let Some((a, b)) = sizes else { return Ok(None) };
    if a < 0 || b < 0 || a + b > MAX_RECORD as i64 {
        return Err(limited());
    }
    db.query_row(
        "SELECT payload,result FROM idempotency WHERE scope=?1 AND key=?2",
        params![scope, id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(db_error)
}
pub(crate) fn purpose(db: &Connection, id: &str) -> Result<Option<PlanningPurpose>> {
    let Some((payload, raw)) = bounded_row(db, PURPOSE, id)? else {
        let marked:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversations WHERE id=?1 AND json_type(payload,'$.planning') IS NOT NULL) OR EXISTS(SELECT 1 FROM idempotency WHERE scope LIKE 'goal-planning-start:%' AND result=?1) OR EXISTS(SELECT 1 FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='conversation.planning_bound')",[id],|r|r.get(0)).map_err(db_error)?;
        if marked {
            return Err(changed());
        }
        return Ok(None);
    };
    let p: PlanningPurpose = decode(&raw)?;
    if p.conversation_id != id
        || payload != encode(&p.request)?
        || p.request.expected_digest != p.prepared.digest
        || p.prepared.digest != digest_prepared(&p.prepared)?
        || p.prepared.source.task_id != p.request.preparation.query.task_id
        || p.prepared.prompt.len() > 65_536
        || p.prepared.prompt_sha256 != format!("{:x}", Sha256::digest(p.prepared.prompt.as_bytes()))
    {
        return Err(changed());
    }
    let c = crate::conversations::read(db, id)?;
    if c.attempt_id != p.attempt_id
        || c.project_id != p.project_id
        || c.grant_id != p.grant_id
        || c.connection_id != p.prepared.binding.connection_id
        || c.workspace_id != p.prepared.binding.workspace_id
    {
        return Err(changed());
    }
    let binding:String=db.query_row("SELECT CASE WHEN octet_length(binding)<=16384 THEN binding ELSE NULL END FROM conversation_dispatches WHERE conversation_id=?1",[id],|r|r.get(0)).map_err(db_error)?;
    if encode(&decode::<ExecutionBinding>(&binding)?)? != encode(&p.prepared.binding)? {
        return Err(changed());
    }
    // Immutable creation provenance is separate from later progress/stop events.
    let origin:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='conversation.planning_bound' AND json_extract(document,'$.revision')=2 AND json_extract(document,'$.source')='conversation_core' AND json_extract(document,'$.payload.purpose_digest')=?2)",params![id,sha(&p)?],|r|r.get(0)).map_err(db_error)?;
    if !origin {
        return Err(changed());
    }
    let historical = stored_scope_view(db, &p.prepared.source.scope_id)?;
    if historical.task_id != p.prepared.source.task_id
        || historical.scope_digest != p.prepared.source.scope_digest
        || encode(&historical.binding)? != encode(&p.prepared.binding)?
        || historical.program_sha256 != p.prepared.program_sha256
    {
        return Err(changed());
    }
    Ok(Some(p))
}
pub(crate) fn bind(
    db: &Connection,
    c: &mut Conversation,
    r: &GoalPlanningStartRequest,
    p: GoalPlanningPrepared,
    request_key: &str,
    grant_id: &str,
) -> Result<()> {
    let purpose = PlanningPurpose {
        conversation_id: c.id.clone(),
        attempt_id: c.attempt_id.clone(),
        turn_id: Uuid::new_v4().to_string(),
        request_key: request_key.into(),
        request: r.clone(),
        prepared: p,
        project_id: c.project_id.clone(),
        grant_id: grant_id.into(),
    };
    db.execute(
        "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
        params![PURPOSE, c.id, encode(r)?, encode(&purpose)?],
    )
    .map_err(db_error)?;
    crate::conversations::save(
        db,
        c,
        "conversation.planning_bound",
        json!({"purpose_digest":sha(&purpose)?,"single_turn":true,"task_created":false}),
    )
}
pub(crate) fn complete(db: &Connection, p: &PlanningPurpose, c: &PlanningCompletion) -> Result<()> {
    db.execute(
        "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
        params![COMPLETION, p.conversation_id, sha(p)?, encode(c)?],
    )
    .map_err(db_error)?;
    Ok(())
}
fn run(db: &Connection, id: &str) -> Result<GoalPlanningRun> {
    let p = purpose(db, id)?.ok_or_else(|| err("NOT_FOUND", "Planning run not found"))?;
    let conversation = crate::conversations::read(db, id)?;
    let completion = bounded_row(db, COMPLETION, id)?
        .map(|(payload, raw)| -> Result<PlanningCompletion> {
            let c: PlanningCompletion = decode(&raw)?;
            if payload != sha(&p)? || c.purpose_digest != payload {
                return Err(changed());
            }
            Ok(c)
        })
        .transpose()?;
    if let Some(c) = &completion {
        let event:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source')='conversation_core' AND json_extract(document,'$.kind')='conversation.planning_settled' AND json_extract(document,'$.payload.completion_digest')=?2)",params![id,sha(c)?],|r|r.get(0)).map_err(db_error)?;
        let state: String = db
            .query_row(
                "SELECT state FROM conversation_dispatches WHERE conversation_id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let budget:String=db.query_row("SELECT CASE WHEN octet_length(document)<=16384 THEN document ELSE NULL END FROM conversation_budgets WHERE conversation_id=?1",[id],|r|r.get(0)).map_err(db_error)?;
        let budget: crate::conversations::ConversationBudget = decode(&budget)?;
        verify_accounting(db, &p, &budget)?;
        if !event
            || state != "stopped"
            || !budget.settled
            || (c.reply_complete && budget.actual_output_bytes != c.observed_output_bytes)
        {
            return Err(changed());
        }
        if c.reply_complete {
            let turn:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id=?1 AND id=?2 AND state='completed')",params![id,p.turn_id],|r|r.get(0)).map_err(db_error)?;
            if !turn
                || !c.process_reaped
                || !c.worker_joined
                || c.reply
                    .as_ref()
                    .is_none_or(|t| t.len() > p.prepared.response_bytes as usize)
                || c.observed_output_bytes
                    .is_none_or(|b| b > p.prepared.response_bytes)
            {
                return Err(changed());
            }
        } else if c.reply.is_some() {
            return Err(changed());
        }
    }
    let mut result = GoalPlanningRun {
        conversation_id: id.into(),
        conversation,
        source: p.prepared.source,
        preparation_digest: p.prepared.digest,
        prompt_sha256: p.prepared.prompt_sha256,
        runtime_ms: p.prepared.runtime_ms,
        response_bytes: p.prepared.response_bytes,
        request_key: p.request_key,
        reply: None,
        reply_complete: false,
        process_reaped: false,
        worker_joined: false,
        observed_output_bytes: None,
    };
    if let Some(c) = completion {
        result.process_reaped = c.process_reaped;
        result.worker_joined = c.worker_joined;
        result.observed_output_bytes = c.observed_output_bytes;
        if c.reply_complete
            && c.process_reaped
            && c.worker_joined
            && result.conversation.state == "stopped"
            && result.conversation.turn_state.as_deref() == Some("completed")
        {
            result.reply = c.reply;
            result.reply_complete = result.reply.is_some();
        }
    }
    Ok(result)
}
impl Store {
    pub fn prepare_goal_planning(
        &mut self,
        r: &GoalPlanningPrepareRequest,
    ) -> Result<GoalPlanningPrepared> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let (p, _, _) = prepare_from(&tx, r)?;
        tx.commit().map_err(db_error)?;
        Ok(p)
    }
    pub fn goal_planning_start_replay(
        &self,
        r: &GoalPlanningStartRequest,
        key_value: &str,
    ) -> Result<Option<GoalPlanningRun>> {
        key(key_value)?;
        self.goal_planning_start_replay_inner(r, key_value)
            .map_err(|mut e| {
                if e.side_effect_state != "confirmed" {
                    e.side_effect_state = "unknown".into();
                }
                e
            })
    }
    fn goal_planning_start_replay_inner(
        &self,
        r: &GoalPlanningStartRequest,
        key_value: &str,
    ) -> Result<Option<GoalPlanningRun>> {
        let marker = format!("goal-planning-start:{}", r.preparation.query.task_id);
        let Some((payload, id)) = bounded_row(&self.conn, &marker, key_value)? else {
            let orphan:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM idempotency WHERE scope=?1 AND octet_length(result)<=524288 AND json_extract(result,'$.request_key')=?2 AND json_extract(result,'$.prepared.source.task_id')=?3)",params![PURPOSE,key_value,r.preparation.query.task_id],|r|r.get(0)).map_err(db_error)?;
            if orphan {
                let mut e = changed();
                e.side_effect_state = "unknown".into();
                return Err(e);
            }
            return Ok(None);
        };
        if payload != encode(r)? {
            return Err(err(
                "CONFIG_CONFLICT",
                "Planning key belongs to another request",
            ));
        }
        let result = run(&self.conn, &id).map_err(|mut e| {
            e.side_effect_state = "confirmed".into();
            e
        })?;
        if result.request_key != key_value {
            return Err(changed());
        }
        Ok(Some(result))
    }
    pub fn start_goal_planning(
        &mut self,
        r: &GoalPlanningStartRequest,
        revision: u64,
        key_value: &str,
    ) -> Result<(GoalPlanningRun, bool)> {
        if revision != r.preparation.query.expected_task_revision {
            return Err(err("STALE_REVISION", "Planning source revision differs"));
        }
        if let Some(v) = self.goal_planning_start_replay(r, key_value)? {
            return Ok((v, false));
        }
        let (p, project, grant) = prepare_from(&self.conn, &r.preparation)?;
        if p.digest != r.expected_digest {
            return Err(changed());
        }
        let c = self.create_planning_conversation(
            &project,
            &p.binding.workspace_id,
            &p.binding.connection_id,
            &grant,
            key_value,
            r,
        )?;
        let result = run(&self.conn, &c.id).map_err(|mut e| {
            e.side_effect_state = "confirmed".into();
            e.details_ref = Some(format!("local-planning-recorded:{}", c.id));
            e
        })?;
        Ok((result, true))
    }
    pub fn get_goal_planning(&self, id: &str) -> Result<GoalPlanningRun> {
        key(id)?;
        run(&self.conn, id)
    }
    pub fn list_goal_planning(&self, task_id: &str) -> Result<Vec<GoalPlanningRun>> {
        key(task_id)?;
        let scope = format!("goal-planning-start:{task_id}");
        let mut q=self.conn.prepare("SELECT CASE WHEN octet_length(result)<=128 THEN result ELSE NULL END FROM idempotency WHERE scope=?1 ORDER BY rowid DESC LIMIT 16").map_err(db_error)?;
        let ids = q
            .query_map([scope], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        let mut total = 0usize;
        let mut runs = Vec::new();
        for id in ids {
            key(&id)?;
            let r = run(&self.conn, &id)?;
            total = total.checked_add(encode(&r)?.len()).ok_or_else(limited)?;
            if total > 4 * 1024 * 1024 {
                return Err(limited());
            }
            runs.push(r)
        }
        Ok(runs)
    }
    pub fn cancel_goal_planning(
        &mut self,
        id: &str,
        revision: u64,
        key_value: &str,
    ) -> Result<GoalPlanningRun> {
        self.get_goal_planning(id)?;
        self.cancel_conversation(id, revision, key_value)?;
        self.get_goal_planning(id).map_err(|mut e| {
            e.side_effect_state = "confirmed".into();
            e
        })
    }
}
pub(crate) fn start_marker(db: &Connection, p: &PlanningPurpose) -> Result<()> {
    db.execute(
        "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
        params![
            format!("goal-planning-start:{}", p.prepared.source.task_id),
            p.request_key,
            encode(&p.request)?,
            p.conversation_id
        ],
    )
    .map_err(db_error)?;
    Ok(())
}

pub(crate) fn purpose_digest(p: &PlanningPurpose) -> Result<String> {
    sha(p)
}
pub(crate) fn completion_digest(p: &PlanningCompletion) -> Result<String> {
    sha(p)
}
pub(crate) fn task_cancelled(db: &Connection, id: &str) -> Result<bool> {
    let Some(p) = purpose(db, id)? else {
        return Ok(false);
    };
    let t: Task = raw(db, "tasks", &p.prepared.source.task_id)?;
    Ok(matches!(
        t.state,
        TaskState::CancelRequested | TaskState::Cancelled
    ))
}
pub(crate) fn has_active_goal_planning(db: &Connection, task_id: &str) -> Result<bool> {
    // Search the durable Conversation intent as well as the purpose rows. Missing
    // purpose provenance is an error, never permission to treat a writer as absent.
    let mut q=db.prepare("SELECT c.id FROM conversations c JOIN conversation_dispatches d ON d.conversation_id=c.id WHERE (c.state NOT IN('stopped','failed') OR d.state!='stopped') LIMIT 1001").map_err(db_error)?;
    let ids = q
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    if ids.len() > 1000 {
        return Err(limited());
    }
    let mut active = false;
    for id in &ids {
        if let Some(p) = purpose(db, id)? {
            active |= p.prepared.source.task_id == task_id;
        }
    }
    Ok(active)
}

#[cfg(test)]
#[path = "goal_planning_tests.rs"]
mod tests;

pub(crate) fn validate_turn_source(db: &Connection, p: &PlanningPurpose) -> Result<()> {
    let scope = stored_scope_view(db, &p.prepared.source.scope_id)?;
    let task: Task = raw(db, "tasks", &p.prepared.source.task_id)?;
    if scope.revoked
        || scope.revision != p.prepared.source.scope_revision
        || scope.authority_revision != p.prepared.source.authority_revision
        || task.revision != p.prepared.source.task_revision
        || task.spec_revision != p.prepared.source.spec_revision
        || task.goal != p.prepared.source.task_goal
    {
        return Err(changed());
    }
    Ok(())
}

pub(crate) fn turn_source_changed(db: &Connection, id: &str) -> Result<bool> {
    let Some(p) = purpose(db, id)? else {
        return Ok(false);
    };
    Ok(validate_turn_source(db, &p).is_err() || task_cancelled(db, id)?)
}

/// Rebuilding shared ledgers must authenticate the private settlement as well as
/// its reservation. An observed partial byte count never becomes known usage.
pub(crate) fn verify_accounting(
    db: &Connection,
    p: &PlanningPurpose,
    b: &crate::conversations::ConversationBudget,
) -> Result<()> {
    if !b.settled {
        if b.actual_runtime_ms.is_some() || b.actual_output_bytes.is_some() {
            return Err(changed());
        }
        return Ok(());
    }
    let (payload, raw) = bounded_row(db, COMPLETION, &p.conversation_id)?.ok_or_else(changed)?;
    let c: PlanningCompletion = decode(&raw)?;
    if payload != sha(p)?
        || c.purpose_digest != payload
        || b.actual_output_bytes != c.accounted_output_bytes
        || b.actual_runtime_ms != Some(c.elapsed_ms)
    {
        return Err(changed());
    }
    let event:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source')='conversation_core' AND json_extract(document,'$.kind')='conversation.planning_settled' AND json_extract(document,'$.payload.completion_digest')=?2)",params![p.conversation_id,sha(&c)?],|r|r.get(0)).map_err(db_error)?;
    if !event {
        return Err(changed());
    }
    Ok(())
}
