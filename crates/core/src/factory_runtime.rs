//! Durable bounded Task decomposition; every context below comes from Store,
//! never Deserialize or a renderer/Provider claim of caller identity.
use super::store::*;
use super::*;
use crate::{
    sessions::ManagedSession,
    storage::{db_error, decode, encode, now, Store, TaskState},
    Result,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use uuid::Uuid;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskBinding {
    handle: FactoryTaskHandle,
    scope_id: String,
    run_id: String,
    grant_id: String,
    request: FactoryDelegateRequest,
    round: u32,
    session_id: Option<String>,
    turn_sent: bool,
    stop_reason: Option<String>,
    result: Option<FactoryTaskResult>,
}
pub(crate) struct FactoryDispatchContext {
    scope: FactoryScope,
    run: FactoryRun,
    task: TaskBinding,
}
pub(crate) struct FactoryCaller {
    scope: FactoryScope,
    run: FactoryRun,
    session: ManagedSession,
    task: TaskBinding,
}
fn run(db: &Connection, scope_id: &str) -> Result<FactoryRun> {
    key(scope_id)?;
    let row:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM factory_runs WHERE scope_id=?1",[scope_id],|r|r.get(0)).optional().map_err(db_error)?;
    let r: FactoryRun = decode(
        &row.ok_or_else(|| CoreError::new("NOT_FOUND", "Factory run has not been started"))?
            .ok_or_else(limited)?,
    )?;
    if r.scope_id != scope_id {
        return Err(changed());
    }
    let fields: (String, String, String, i64, String) = db
        .query_row(
            "SELECT id,root_task_id,project_id,revision,state FROM factory_runs WHERE scope_id=?1",
            [scope_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(db_error)?;
    if fields
        != (
            r.id.clone(),
            r.root_task_id.clone(),
            r.project_id.clone(),
            i64::try_from(r.revision).map_err(|_| changed())?,
            r.state.clone(),
        )
    {
        return Err(changed());
    }
    Ok(r)
}
fn binding(db: &Connection, run_id: &str, task_id: &str) -> Result<TaskBinding> {
    key(run_id)?;
    key(task_id)?;
    let row:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM factory_task_bindings WHERE run_id=?1 AND task_id=?2",params![run_id,task_id],|r|r.get(0)).optional().map_err(db_error)?;
    let b: TaskBinding = decode(
        &row.ok_or_else(|| CoreError::new("NOT_FOUND", "Task is not in this Factory run"))?
            .ok_or_else(limited)?,
    )?;
    let columns:(String,String,String)=db.query_row("SELECT scope_id,grant_id,state FROM factory_task_bindings WHERE run_id=?1 AND task_id=?2",params![run_id,task_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(db_error)?;
    if b.run_id != run_id
        || b.handle.task_id != task_id
        || b.scope_id != b.handle.factory_scope_id
        || columns
            != (
                b.scope_id.clone(),
                b.grant_id.clone(),
                b.handle.state.clone(),
            )
    {
        return Err(changed());
    }
    Ok(b)
}
fn bindings(db: &Connection, r: &FactoryRun) -> Result<Vec<TaskBinding>> {
    let(c,n):(i64,i64)=db.query_row("SELECT count(*),COALESCE(sum(octet_length(document)),0) FROM factory_task_bindings WHERE run_id=?1",[&r.id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(db_error)?;
    if c > 1000 || n > MAX_FACTORY_BYTES as i64 {
        return Err(limited());
    }
    let mut q = db
        .prepare("SELECT task_id FROM factory_task_bindings WHERE run_id=?1 ORDER BY rowid")
        .map_err(db_error)?;
    let ids = q
        .query_map([&r.id], |row| row.get::<_, String>(0))
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    ids.iter().map(|id| binding(db, &r.id, id)).collect()
}
fn save_run(db: &Connection, r: &mut FactoryRun, kind: &str, payload: Value) -> Result<()> {
    r.revision = r.revision.checked_add(1).ok_or_else(limited)?;
    r.updated_at = now()?;
    check_wire(
        &json!({"request_id":"\0".repeat(128),"result":r,"revision":u64::MAX,"warnings":[]}),
    )?;
    db.execute(
        "UPDATE factory_runs SET document=?1,state=?2,revision=?3 WHERE id=?4",
        params![
            encode(r)?,
            r.state,
            i64::try_from(r.revision).map_err(|_| limited())?,
            r.id
        ],
    )
    .map_err(db_error)?;
    event(db, &r.id, r.revision, kind, payload, "factory_core")?;
    Ok(())
}
fn save_binding(db: &Connection, b: &mut TaskBinding) -> Result<()> {
    b.handle.revision = b.handle.revision.checked_add(1).ok_or_else(limited)?;
    check_wire(b)?;
    db.execute(
        "UPDATE factory_task_bindings SET state=?1,document=?2 WHERE run_id=?3 AND task_id=?4",
        params![b.handle.state, encode(b)?, b.run_id, b.handle.task_id],
    )
    .map_err(db_error)?;
    Ok(())
}
pub(crate) fn task_budget_root(db: &Connection, task_id: &str) -> Result<String> {
    key(task_id)?;
    let row: Option<String> = db
        .query_row(
            "SELECT root_task_id FROM factory_task_lineage WHERE task_id=?1",
            [task_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    Ok(row.unwrap_or_else(|| task_id.into()))
}
pub(crate) fn owns_session(db: &Connection, id: &str) -> Result<bool> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM factory_attempts WHERE session_id=?1)",
        [id],
        |r| r.get(0),
    )
    .map_err(db_error)
}
fn live(db: &Connection, s: &FactoryScope) -> Result<()> {
    let current = scope(db, &s.id)?;
    if current.revoked
        || current.revision != s.revision
        || current.expires_at_ms <= ms()?
        || current.contract_digest != s.contract_digest
    {
        return Err(denied());
    }
    let grant = crate::policy::checked_lineage(db, &s.root_grant_id)?;
    if encode(&grant)? != encode(&s.bindings.approved_grant)? {
        return Err(changed());
    }
    for original in &s.bindings.connections {
        let row:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM connections WHERE id=?1",[&original.id],|r|r.get(0)).optional().map_err(db_error)?;
        let current: crate::sessions::ProviderConnection =
            decode(&row.flatten().ok_or_else(changed)?)?;
        if encode(&current)? != encode(original)? {
            return Err(changed());
        }
    }
    for original in &s.bindings.workspaces {
        let row:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM managed_workspaces WHERE id=?1",[&original.id],|r|r.get(0)).optional().map_err(db_error)?;
        let current: crate::registry::ManagedWorkspace =
            decode(&row.flatten().ok_or_else(changed)?)?;
        if encode(&current)? != encode(original)? {
            return Err(changed());
        }
    }
    let project: Option<String> = db
        .query_row(
            "SELECT document FROM projects WHERE id=?1 AND octet_length(document)<=1048576",
            [&s.project_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let project: crate::registry::Project = decode(&project.ok_or_else(changed)?)?;
    if project.archived {
        return Err(denied());
    }
    let task: Option<String> = db
        .query_row(
            "SELECT document FROM tasks WHERE id=?1 AND octet_length(document)<=1048576",
            [&s.root_task_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let task: crate::storage::Task = decode(&task.ok_or_else(changed)?)?;
    if task.goal != s.bindings.root_task.goal
        || task.spec_revision != s.bindings.root_task.spec_revision
        || task.project_id != s.project_id
        || matches!(
            task.state,
            TaskState::CancelRequested | TaskState::Cancelled | TaskState::Accepted
        )
    {
        return Err(denied());
    }
    Ok(())
}
impl Store {
    pub fn get_factory_run(&self, scope_id: &str) -> Result<FactoryRunDetails> {
        let _ = scope(&self.conn, scope_id)?;
        let r = run(&self.conn, scope_id)?;
        let tasks = bindings(&self.conn, &r)?
            .into_iter()
            .map(|b| b.handle)
            .collect();
        let(c,n):(i64,i64)=self.conn.query_row("SELECT count(*),COALESCE(sum(octet_length(document)),0) FROM factory_handoffs WHERE run_id=?1",[&r.id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(db_error)?;
        if c > 1000 || n > MAX_FACTORY_BYTES as i64 {
            return Err(limited());
        }
        let mut q = self
            .conn
            .prepare("SELECT document FROM factory_handoffs WHERE run_id=?1 ORDER BY rowid")
            .map_err(db_error)?;
        let rows = q
            .query_map([&r.id], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        let waiting_relations = rows
            .iter()
            .map(|raw| decode(raw))
            .collect::<Result<Vec<_>>>()?;
        let details = FactoryRunDetails {
            run: r,
            tasks,
            waiting_relations,
        };
        check_wire(
            &json!({"request_id":"\0".repeat(128),"result":details,"revision":u64::MAX,"warnings":[]}),
        )?;
        Ok(details)
    }
    pub fn start_factory_run(
        &mut self,
        scope_id: &str,
        revision: u64,
        key_id: &str,
    ) -> Result<FactoryRun> {
        key(scope_id)?;
        let payload = encode(&json!({"scope_id":scope_id,"revision":revision}))?;
        let idem = format!("factory:{scope_id}:start");
        if let Some(old) = replay(&self.conn, &idem, key_id, &payload)? {
            return Ok(old);
        }
        let s = scope(&self.conn, scope_id)?;
        live(&self.conn, &s)?;
        if s.revision != revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Factory scope changed before start",
            ));
        }
        let existing: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM factory_runs WHERE scope_id=?1)",
                [scope_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if existing {
            return Err(CoreError::new("CONFIG_CONFLICT","This Factory scope has one durable run; reconcile its original request rather than creating a new-key run"));
        }
        let task = self.get(&s.root_task_id)?;
        if task.state != TaskState::Draft {
            return Err(CoreError::new(
                "OUTCOME_UNKNOWN",
                "A new Factory scope cannot replay an earlier unknown or terminal task",
            ));
        }
        let observed =
            self.resolve_factory_bindings(&s.selection, &s.bindings.companion.executable)?;
        if encode(&observed)? != encode(&s.bindings)? {
            return Err(changed());
        }
        let timestamp = now()?;
        let r = FactoryRun {
            id: Uuid::new_v4().to_string(),
            scope_id: s.id.clone(),
            project_id: s.project_id.clone(),
            root_task_id: s.root_task_id.clone(),
            revision: 1,
            state: "queued".into(),
            active_task_id: None,
            active_session_id: None,
            cancel_requested: false,
            context_continuation: "new_attempt_explicit_handoff_only".into(),
            last_error: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        let lineage = FactoryTaskLineage {
            task_id: task.id.clone(),
            root_task_id: task.id.clone(),
            parent_task_id: None,
            factory_scope_id: s.id.clone(),
            delegation_id: None,
            depth: 0,
        };
        let b = TaskBinding {
            handle: FactoryTaskHandle {
                task_id: task.id.clone(),
                parent_task_id: None,
                root_task_id: task.id.clone(),
                delegation_id: format!("root:{}", task.id),
                factory_scope_id: s.id.clone(),
                revision: 1,
                state: "queued".into(),
                attempt_id: None,
                cancel_requested: false,
                confirmed_stop: false,
                result_available: false,
            },
            scope_id: s.id.clone(),
            run_id: r.id.clone(),
            grant_id: s.root_grant_id.clone(),
            request: FactoryDelegateRequest {
                goal: task.goal.clone(),
                connection_id: s.selection.connection_ids[0].clone(),
                workspace_id: s.selection.workspace_ids[0].clone(),
                input_artifact_ids: s
                    .selection
                    .inputs
                    .iter()
                    .map(|a| a.artifact_id.clone())
                    .collect(),
                output_artifact_ids: s
                    .selection
                    .outputs
                    .iter()
                    .map(|a| a.artifact_id.clone())
                    .collect(),
                budget: s.selection.root_budget.clone(),
                wait_for_child: false,
            },
            round: 0,
            session_id: None,
            turn_sent: false,
            stop_reason: None,
            result: None,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        live(&tx, &s)?;
        check_admission_db(&tx, &s.bindings)?;
        tx.execute("INSERT INTO factory_runs(id,scope_id,root_task_id,project_id,revision,state,document) VALUES(?1,?2,?3,?4,1,'queued',?5)",params![r.id,r.scope_id,r.root_task_id,r.project_id,encode(&r)?]).map_err(db_error)?;
        tx.execute("INSERT OR IGNORE INTO factory_task_lineage(task_id,root_task_id,parent_task_id,scope_id,delegation_id,depth,document) VALUES(?1,?1,NULL,?2,NULL,0,?3)",params![task.id,s.id,encode(&lineage)?]).map_err(db_error)?;
        if task_budget_root(&tx, &task.id)? != task.id {
            return Err(changed());
        }
        tx.execute("INSERT INTO factory_task_bindings(task_id,scope_id,run_id,grant_id,state,document) VALUES(?1,?2,?3,?4,'queued',?5)",params![task.id,s.id,r.id,s.root_grant_id,encode(&b)?]).map_err(db_error)?;
        event(
            &tx,
            &r.id,
            1,
            "factory.run_started",
            json!({"scope_id":s.id,"root_task_id":task.id,"contract_digest":s.contract_digest,"provider_started":false,"context_continuation":"new_attempt_explicit_handoff_only"}),
            "factory_core",
        )?;
        remember(&tx, &idem, key_id, &payload, &r)?;
        check_admission_db(&tx, &s.bindings)?;
        tx.commit().map_err(db_error)?;
        Ok(r)
    }
    pub fn cancel_factory_run(
        &mut self,
        scope_id: &str,
        revision: u64,
        key_id: &str,
    ) -> Result<FactoryRun> {
        key(scope_id)?;
        let payload = encode(&json!({"scope_id":scope_id,"revision":revision}))?;
        let idem = format!("factory:{scope_id}:cancel");
        if let Some(old) = replay(&self.conn, &idem, key_id, &payload)? {
            return Ok(old);
        }
        let mut r = run(&self.conn, scope_id)?;
        if revision == 0 || revision > r.revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Factory run observation is unavailable",
            ));
        }
        if !matches!(r.state.as_str(), "cancelled" | "completed") {
            r.cancel_requested = true;
            if r.state != "outcome_unknown" {
                r.state = "cancel_requested".into();
            }
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute(
            "UPDATE factory_scoped_handles SET revoked=1 WHERE scope_id=?1",
            [scope_id],
        )
        .map_err(db_error)?;
        save_run(
            &tx,
            &mut r,
            "factory.cancel_requested",
            json!({"confirmed_stop":false,"no_new_children":true}),
        )?;
        remember(&tx, &idem, key_id, &payload, &r)?;
        tx.commit().map_err(db_error)?;
        Ok(r)
    }
    pub(crate) fn recover_factory_runs(&mut self) -> Result<()> {
        let ids = {
            let mut q=self.conn.prepare("SELECT scope_id FROM factory_runs WHERE state IN('queued','starting','running','waiting_child','resuming','cancel_requested') LIMIT 257").map_err(db_error)?;
            let rows = q
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(db_error)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_error)?;
            if rows.len() > 256 {
                return Err(limited());
            }
            rows
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute(
            "UPDATE factory_scoped_handles SET revoked=1 WHERE revoked=0",
            [],
        )
        .map_err(db_error)?;
        for id in ids {
            let mut r = run(&tx, &id)?;
            r.state = "outcome_unknown".into();
            r.last_error=Some(CoreError{side_effect_state:"unknown".into(),..CoreError::new("OUTCOME_UNKNOWN","Factory execution was interrupted; old handles are revoked and no action is replayed")});
            save_run(
                &tx,
                &mut r,
                "factory.restart_requires_reconciliation",
                json!({"automatic_retry":false,"confirmed_stop":false}),
            )?;
        }
        tx.commit().map_err(db_error)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandleRecord {
    id: String,
    scope_id: String,
    run_id: String,
    session_id: String,
    attempt_id: String,
    task_id: String,
    root_task_id: String,
    project_id: String,
    fencing_token: u64,
    expires_at_ms: i64,
}
/// A reply and already-resolved stop targets, all derived before committing a
/// control mutation. Dispatching it never performs a fallible post-commit lookup.
pub(crate) struct FactoryToolOutcome {
    pub result: Value,
    pub stops: Vec<String>,
}
fn lineage(db: &Connection, task_id: &str) -> Result<FactoryTaskLineage> {
    let row:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=65536 THEN document END FROM factory_task_lineage WHERE task_id=?1",[task_id],|r|r.get(0)).optional().map_err(db_error)?;
    let l: FactoryTaskLineage = decode(&row.ok_or_else(denied)?.ok_or_else(limited)?)?;
    let columns:(String,Option<String>,String,Option<String>,u32)=db.query_row("SELECT root_task_id,parent_task_id,scope_id,delegation_id,depth FROM factory_task_lineage WHERE task_id=?1",[task_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(db_error)?;
    if l.task_id != task_id
        || columns
            != (
                l.root_task_id.clone(),
                l.parent_task_id.clone(),
                l.factory_scope_id.clone(),
                l.delegation_id.clone(),
                l.depth,
            )
    {
        return Err(changed());
    }
    Ok(l)
}
fn ancestry(db: &Connection, task_id: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut id = task_id.to_owned();
    let mut root = None;
    loop {
        if out.contains(&id) || out.len() > 32 {
            return Err(denied());
        }
        let l = lineage(db, &id)?;
        if root.as_ref().is_some_and(|r| r != &l.root_task_id) {
            return Err(denied());
        }
        root = Some(l.root_task_id.clone());
        out.push(id.clone());
        match l.parent_task_id {
            Some(parent) => {
                let p = lineage(db, &parent)?;
                if p.depth.checked_add(1) != Some(l.depth) {
                    return Err(denied());
                }
                id = parent;
            }
            None => {
                if id != l.root_task_id || l.depth != 0 {
                    return Err(denied());
                }
                break;
            }
        }
    }
    Ok(out)
}
fn authorized_target(db: &Connection, caller: &FactoryCaller, target: &str) -> Result<TaskBinding> {
    key(target)?;
    let b = binding(db, &caller.run.id, target)?;
    if b.scope_id != caller.scope.id
        || b.handle.root_task_id != caller.scope.root_task_id
        || !ancestry(db, target)?.contains(&caller.task.handle.task_id)
    {
        return Err(denied());
    }
    Ok(b)
}
impl Store {
    pub(crate) fn authenticate_factory_handle(
        &self,
        handle_id: &str,
        token: &str,
    ) -> Result<FactoryCaller> {
        key(handle_id)?;
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(denied());
        }
        let digest = agentbaton_workspaces::validation::sha256(token.as_bytes());
        let row:Option<(Option<String>,bool,i64,String,String,String,u64)>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=8192 THEN document END,revoked,expires_at_ms,scope_id,session_id,attempt_id,fencing_token FROM factory_scoped_handles WHERE id=?1 AND token_digest=?2",params![handle_id,digest],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get::<_,i64>(6)? as u64))).optional().map_err(db_error)?;
        let (raw, revoked, expiry, scope_id, session_id, attempt_id, fence) =
            row.ok_or_else(denied)?;
        if revoked || expiry <= ms()? {
            return Err(denied());
        }
        let h: HandleRecord = decode(&raw.ok_or_else(limited)?)?;
        if h.id != handle_id
            || h.scope_id != scope_id
            || h.session_id != session_id
            || h.attempt_id != attempt_id
            || h.fencing_token != fence
            || h.expires_at_ms != expiry
        {
            return Err(denied());
        }
        let scope = scope(&self.conn, &scope_id)?;
        live(&self.conn, &scope)?;
        let run = run(&self.conn, &scope_id)?;
        let session = self.get_session(&session_id)?;
        let task = binding(&self.conn, &run.id, &h.task_id)?;
        let owns:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM factory_attempts WHERE session_id=?1 AND attempt_id=?2 AND scope_id=?3 AND run_id=?4 AND task_id=?5 AND root_task_id=?6)",params![session_id,attempt_id,scope_id,run.id,h.task_id,h.root_task_id],|r|r.get(0)).map_err(db_error)?;
        if !owns
            || run.cancel_requested
            || !matches!(
                run.state.as_str(),
                "running" | "waiting_child" | "resuming" | "starting"
            )
            || !matches!(session.state.as_str(), "running" | "ready")
            || session.attempt_id != h.attempt_id
            || session.task_id != h.task_id
            || session.project_id != h.project_id
            || session.fencing_token != h.fencing_token
            || h.project_id != scope.project_id
            || h.root_task_id != scope.root_task_id
            || task.session_id.as_deref() != Some(session.id.as_str())
            || task.handle.attempt_id.as_deref() != Some(session.attempt_id.as_str())
        {
            return Err(denied());
        }
        let lease = self
            .workspace_lease(&session.workspace_id)?
            .ok_or_else(denied)?;
        if lease.holder != session.id || lease.fencing_token != session.fencing_token {
            return Err(denied());
        }
        Ok(FactoryCaller {
            scope,
            run,
            session,
            task,
        })
    }
    pub(crate) fn factory_delegate(
        &mut self,
        caller: FactoryCaller,
        request: &FactoryDelegateRequest,
        key_id: &str,
    ) -> Result<FactoryToolOutcome> {
        key(key_id)?;
        if caller.task.handle.cancel_requested || caller.run.cancel_requested {
            return Err(denied());
        }
        if request.goal.trim().is_empty()
            || request.goal.len() > 4096
            || request.input_artifact_ids.len() > 16
            || request.output_artifact_ids.is_empty()
            || request.output_artifact_ids.len() > 16
            || !request.budget.narrower(&caller.task.request.budget)
            || request.budget.reworks != 0
            || request.budget.tokens.is_some()
            || request.budget.cost_microunits.is_some()
        {
            return Err(invalid());
        }
        if !caller
            .scope
            .selection
            .connection_ids
            .contains(&request.connection_id)
            || !caller
                .scope
                .selection
                .workspace_ids
                .contains(&request.workspace_id)
        {
            return Err(denied());
        }
        for (selected, allowed) in [
            (
                &request.input_artifact_ids,
                caller
                    .scope
                    .selection
                    .inputs
                    .iter()
                    .map(|a| &a.artifact_id)
                    .collect::<Vec<_>>(),
            ),
            (
                &request.output_artifact_ids,
                caller
                    .scope
                    .selection
                    .outputs
                    .iter()
                    .map(|a| &a.artifact_id)
                    .collect::<Vec<_>>(),
            ),
        ] {
            let mut seen = std::collections::BTreeSet::new();
            for id in selected {
                key(id)?;
                if !seen.insert(id) || !allowed.contains(&id) {
                    return Err(denied());
                }
            }
        }
        let payload = encode(request)?;
        let idem = format!(
            "factory:{}:{}:{}:delegate",
            caller.scope.project_id, caller.scope.id, caller.session.id
        );
        if let Some(old) = replay::<FactoryTaskHandle>(&self.conn, &idem, key_id, &payload)? {
            return Ok(FactoryToolOutcome {
                result: json!(old),
                stops: vec![],
            });
        }
        let l = lineage(&self.conn, &caller.task.handle.task_id)?;
        if l.depth >= caller.scope.selection.max_depth {
            return Err(limited());
        }
        let current: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM factory_task_lineage WHERE root_task_id=?1",
                [&caller.scope.root_task_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let siblings: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM factory_task_lineage WHERE parent_task_id=?1",
                [&l.task_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if current >= i64::from(caller.scope.selection.max_total_nodes)
            || siblings >= i64::from(caller.scope.selection.max_children)
        {
            return Err(limited());
        }
        let ancestor_ids = ancestry(&self.conn, &l.task_id)?;
        for id in &ancestor_ids {
            let parent = binding(&self.conn, &caller.run.id, id)?;
            let descendants:i64=self.conn.query_row("WITH RECURSIVE descendants(id) AS(SELECT task_id FROM factory_task_lineage WHERE parent_task_id=?1 UNION SELECT l.task_id FROM factory_task_lineage l JOIN descendants d ON l.parent_task_id=d.id) SELECT count(*) FROM descendants",[id],|r|r.get(0)).map_err(db_error)?;
            if descendants >= i64::from(parent.request.budget.children) {
                return Err(limited());
            }
        }
        let mut r = caller.run.clone();
        let mut parent = caller.task.clone();
        let child_id = Uuid::new_v4().to_string();
        let delegation_id = Uuid::new_v4().to_string();
        let timestamp = now()?;
        let task = crate::storage::Task {
            id: child_id.clone(),
            project_id: caller.scope.project_id.clone(),
            goal: request.goal.clone(),
            spec_revision: 1,
            state: TaskState::Draft,
            revision: 1,
            acceptance_ref: None,
            acceptance_sequence: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        let h = FactoryTaskHandle {
            task_id: child_id.clone(),
            parent_task_id: Some(parent.handle.task_id.clone()),
            root_task_id: caller.scope.root_task_id.clone(),
            delegation_id: delegation_id.clone(),
            factory_scope_id: caller.scope.id.clone(),
            revision: 1,
            state: "blocked_on_live_parent".into(),
            attempt_id: None,
            cancel_requested: false,
            confirmed_stop: false,
            result_available: false,
        };
        let child = TaskBinding {
            handle: h.clone(),
            scope_id: caller.scope.id.clone(),
            run_id: r.id.clone(),
            grant_id: caller.scope.root_grant_id.clone(),
            request: request.clone(),
            round: 0,
            session_id: None,
            turn_sent: false,
            stop_reason: None,
            result: None,
        };
        let line = FactoryTaskLineage {
            task_id: child_id.clone(),
            root_task_id: h.root_task_id.clone(),
            parent_task_id: h.parent_task_id.clone(),
            factory_scope_id: caller.scope.id.clone(),
            delegation_id: Some(delegation_id.clone()),
            depth: l.depth + 1,
        };
        let wait = FactoryWaitingRelation {
            parent_task_id: parent.handle.task_id.clone(),
            child_task_id: child_id.clone(),
            parent_session_id: caller.session.id.clone(),
            parent_stop_confirmed: false,
            handoff_digest: None,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        live(&tx, &caller.scope)?;
        tx.execute(
            "INSERT INTO tasks(id,document) VALUES(?1,?2)",
            params![task.id, encode(&task)?],
        )
        .map_err(db_error)?;
        tx.execute("INSERT INTO factory_task_lineage(task_id,root_task_id,parent_task_id,scope_id,delegation_id,depth,document) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![line.task_id,line.root_task_id,line.parent_task_id,line.factory_scope_id,line.delegation_id,line.depth,encode(&line)?]).map_err(db_error)?;
        tx.execute("INSERT INTO factory_task_bindings(task_id,scope_id,run_id,grant_id,state,document) VALUES(?1,?2,?3,?4,?5,?6)",params![task.id,caller.scope.id,r.id,child.grant_id,h.state,encode(&child)?]).map_err(db_error)?;
        tx.execute("INSERT INTO factory_delegations(id,scope_id,run_id,parent_task_id,child_task_id,grant_id,state,document) VALUES(?1,?2,?3,?4,?5,?6,'queued',?7)",params![delegation_id,caller.scope.id,r.id,parent.handle.task_id,child_id,child.grant_id,encode(&json!({"request":request,"caller_session_id":caller.session.id,"caller_attempt_id":caller.session.attempt_id,"caller_fencing_token":caller.session.fencing_token,"child":h,"scope_digest":caller.scope.contract_digest}))?]).map_err(db_error)?;
        tx.execute("INSERT INTO factory_handoffs(id,run_id,parent_task_id,child_task_id,parent_session_id,child_attempt_id,state,document) VALUES(?1,?2,?3,?4,?5,NULL,'waiting_parent_stop',?6)",params![Uuid::new_v4().to_string(),r.id,parent.handle.task_id,child_id,caller.session.id,encode(&wait)?]).map_err(db_error)?;
        if request.wait_for_child {
            parent.handle.state = "waiting_child".into();
            save_binding(&tx, &mut parent)?;
            r.state = "waiting_child".into();
        }
        save_run(
            &tx,
            &mut r,
            "factory.child_queued",
            json!({"child_task_id":child_id,"parent_task_id":parent.handle.task_id,"caller_session_id":caller.session.id,"caller_attempt_id":caller.session.attempt_id,"caller_fencing_token":caller.session.fencing_token,"parent_still_owned":true,"resource_released":false,"wait_for_child":request.wait_for_child,"new_grant":false}),
        )?;
        remember(&tx, &idem, key_id, &payload, &h)?;
        tx.commit().map_err(db_error)?;
        Ok(FactoryToolOutcome {
            result: json!(h),
            stops: vec![],
        })
    }
    pub(crate) fn factory_status(
        &self,
        caller: &FactoryCaller,
        id: &str,
    ) -> Result<FactoryToolOutcome> {
        let b = authorized_target(&self.conn, caller, id)?;
        Ok(FactoryToolOutcome {
            result: json!(b.handle),
            stops: vec![],
        })
    }
    pub(crate) fn factory_result(
        &self,
        caller: &FactoryCaller,
        id: &str,
    ) -> Result<FactoryToolOutcome> {
        let b = authorized_target(&self.conn, caller, id)?;
        let result = b.result.unwrap_or_else(|| FactoryTaskResult {
            task: b.handle.clone(),
            artifacts: vec![],
            validation_refs: vec![],
            acceptance_state: "not_established".into(),
            context_continuation: "new_attempt_explicit_handoff_only".into(),
            handoff_digest: None,
        });
        check_wire(
            &json!({"request_id":"\0".repeat(128),"result":result,"revision":u64::MAX,"warnings":[]}),
        )?;
        Ok(FactoryToolOutcome {
            result: json!(result),
            stops: vec![],
        })
    }
}
impl Store {
    pub(crate) fn factory_cancel_task(
        &mut self,
        caller: &FactoryCaller,
        id: &str,
        revision: u64,
        key_id: &str,
    ) -> Result<FactoryToolOutcome> {
        let target = authorized_target(&self.conn, caller, id)?;
        let payload = encode(&json!({"task_id":id,"revision":revision}))?;
        let idem = format!(
            "factory:{}:{}:{}:cancel",
            caller.scope.project_id, caller.scope.id, caller.session.id
        );
        if let Some(old) = replay::<FactoryTaskHandle>(&self.conn, &idem, key_id, &payload)? {
            return Ok(FactoryToolOutcome {
                result: json!(old),
                stops: vec![],
            });
        }
        if revision == 0 || revision > target.handle.revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Factory task observation is unavailable",
            ));
        }
        let all = bindings(&self.conn, &caller.run)?;
        let mut affected = Vec::new();
        for b in all {
            if ancestry(&self.conn, &b.handle.task_id)?
                .iter()
                .any(|a| a == id)
            {
                affected.push(b);
            }
        }
        let mut stops = Vec::new();
        let mut returned = target.handle.clone();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        for mut b in affected {
            if b.handle.result_available || b.handle.state == "cancelled" {
                continue;
            }
            b.handle.cancel_requested = true;
            if b.handle.attempt_id.is_none() {
                b.handle.state = "cancelled".into();
                b.handle.confirmed_stop = true;
                let raw: String = tx
                    .query_row(
                        "SELECT document FROM tasks WHERE id=?1",
                        [&b.handle.task_id],
                        |r| r.get(0),
                    )
                    .map_err(db_error)?;
                let mut task: crate::storage::Task = decode(&raw)?;
                if task.state == TaskState::Draft {
                    task.state = TaskState::Cancelled;
                    task.revision += 1;
                    task.updated_at = now()?;
                    tx.execute(
                        "UPDATE tasks SET document=?1 WHERE id=?2",
                        params![encode(&task)?, task.id],
                    )
                    .map_err(db_error)?;
                }
            } else if let Some(session_id) = &b.session_id {
                let row: Option<String> = tx
                    .query_row(
                        "SELECT state FROM sessions WHERE id=?1",
                        [session_id],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(db_error)?;
                b.handle.state = if row.as_deref() == Some("outcome_unknown") {
                    "outcome_unknown"
                } else {
                    "cancel_requested"
                }
                .into();
                if row.as_deref() != Some("stopped") {
                    stops.push(session_id.clone());
                }
            }
            save_binding(&tx, &mut b)?;
            if b.handle.task_id == id {
                returned = b.handle.clone();
            }
            tx.execute(
                "UPDATE factory_scoped_handles SET revoked=1 WHERE scope_id=?1 AND task_id=?2",
                params![caller.scope.id, b.handle.task_id],
            )
            .map_err(db_error)?;
            event(
                &tx,
                &b.handle.task_id,
                b.handle.revision,
                "factory.task_cancel_requested",
                json!({"scope_id":caller.scope.id,"caller_session_id":caller.session.id,"root_task_id":caller.scope.root_task_id,"never_dispatched":b.handle.attempt_id.is_none(),"confirmed_stop":b.handle.confirmed_stop,"no_new_children":true}),
                "factory_core",
            )?;
        }
        remember(&tx, &idem, key_id, &payload, &returned)?;
        tx.commit().map_err(db_error)?;
        Ok(FactoryToolOutcome {
            result: json!(returned),
            stops,
        })
    }
}
