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
    #[serde(default)]
    handoff_results: Vec<FactoryTaskResult>,
    #[serde(default)]
    handoff_digest: Option<String>,
}
pub(crate) struct FactoryDispatchContext {
    scope: FactoryScope,
    run: FactoryRun,
    task: TaskBinding,
    queued_ms: u64,
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
    // Original Grant authority is indivisible: unselected recipients/workspaces
    // remain pinned, including every ancestor's current underlying bindings.
    crate::policy::validate_grant_live_from_with_data(
        db,
        &s.root_grant_id,
        &s.project_id,
        &s.selection.workspace_ids[0],
        &s.selection.connection_ids[0],
        None,
    )?;
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
    if project.archived || encode(&project)? != encode(&s.bindings.project)? {
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
            handoff_results: vec![],
            handoff_digest: None,
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
        if request
            .input_artifact_ids
            .iter()
            .any(|id| !caller.task.request.input_artifact_ids.contains(id))
            || request
                .output_artifact_ids
                .iter()
                .any(|id| !caller.task.request.output_artifact_ids.contains(id))
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
            handoff_results: vec![],
            handoff_digest: None,
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
        let mut result = b.result.clone().unwrap_or_else(|| FactoryTaskResult {
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
        result.task = b.handle.clone();
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

impl FactoryDispatchContext {
    pub(crate) fn set_queued_ms(&mut self, elapsed: u64) {
        self.queued_ms = elapsed;
    }
    pub(crate) fn root_task_id(&self) -> &str {
        &self.scope.root_task_id
    }
    pub(crate) fn request(&self) -> (&str, &str, &str, &str) {
        (
            &self.task.handle.task_id,
            &self.task.request.workspace_id,
            &self.task.request.connection_id,
            &self.scope.root_grant_id,
        )
    }
}
pub(crate) fn reserve_factory_session(
    db: &Connection,
    c: &FactoryDispatchContext,
    s: &ManagedSession,
    binding: &crate::execution::ExecutionBinding,
) -> Result<()> {
    live(db, &c.scope)?;
    let mut b = self::binding(db, &c.run.id, &s.task_id)?;
    let mut r = run(db, &c.scope.id)?;
    if r.cancel_requested
        || r.revision != c.run.revision
        || b.handle.revision != c.task.handle.revision
        || !matches!(b.handle.state.as_str(), "queued" | "resuming")
        || b.session_id.is_some()
        || s.task_id != c.task.handle.task_id
        || s.workspace_id != b.request.workspace_id
        || s.connection_id != b.request.connection_id
        || binding.policy_revision
            != format!("{}:{}", c.scope.root_grant_id, c.scope.root_grant_revision)
    {
        return Err(denied());
    }
    let ancestors = ancestry(db, &s.task_id)?;
    let output_bytes = b
        .request
        .output_artifact_ids
        .iter()
        .try_fold(0u64, |n, id| {
            let a = c
                .scope
                .selection
                .outputs
                .iter()
                .find(|a| &a.artifact_id == id)
                .ok_or_else(denied)?;
            n.checked_add(a.max_bytes).ok_or_else(limited)
        })?;
    let mut record = FactoryReservation {
        authority_kind: "factory_task".into(),
        scope_id: Some(c.scope.id.clone()),
        run_id: Some(r.id.clone()),
        root_task_id: c.scope.root_task_id.clone(),
        root_grant_id: c.scope.root_grant_id.clone(),
        project_id: s.project_id.clone(),
        task_id: s.task_id.clone(),
        session_id: s.id.clone(),
        attempt_id: s.attempt_id.clone(),
        ancestors: ancestors.clone(),
        queued_ms: c.queued_ms,
        reservation: crate::goal_plan::AttemptReservation {
            runtime_ms: b
                .request
                .budget
                .runtime_ms
                .min(c.scope.bindings.approved_grant.scope.max_duration_ms),
            output_bytes,
            tokens: None,
            cost_microunits: None,
        },
        settled: false,
        actual_runtime_ms: None,
        actual_output_bytes: None,
        measurement_scope: "selected_artifacts_no_validator_streams".into(),
        filesystem_total_bytes: None,
        captured_provider_message_bytes: None,
        provider_message_capture_complete: false,
    };
    // Every ancestor bound is cumulative over actual attempts, not a fresh child budget.
    for ancestor in &ancestors {
        let a = self::binding(db, &r.id, ancestor)?;
        let old = super::budget::budget_records(db, &c.scope.root_grant_id, &s.project_id)?;
        let same = old
            .iter()
            .filter(|x| {
                x.scope_id.as_deref() == Some(c.scope.id.as_str()) && x.ancestors.contains(ancestor)
            })
            .collect::<Vec<_>>();
        if same.len() >= a.request.budget.attempts as usize
            || same.iter().filter(|x| !x.settled).count() >= a.request.budget.parallel as usize
        {
            return Err(limited());
        }
        let spent = same.iter().try_fold(0u64, |n, old| {
            n.checked_add(if old.settled {
                old.actual_runtime_ms.ok_or_else(|| {
                    CoreError::new("BUDGET_UNKNOWN", "An ancestor has unknown runtime")
                })?
            } else {
                old.reservation.runtime_ms
            })
            .ok_or_else(limited)
        })?;
        let remaining = a
            .request
            .budget
            .runtime_ms
            .checked_sub(spent)
            .ok_or_else(limited)?;
        record.reservation.runtime_ms = record.reservation.runtime_ms.min(remaining);
        if record.reservation.runtime_ms == 0 {
            return Err(limited());
        }
        let mut runtime = record.reservation.runtime_ms;
        let mut output = record.reservation.output_bytes;
        for old in same {
            runtime = runtime
                .checked_add(if old.settled {
                    old.actual_runtime_ms.ok_or_else(|| {
                        CoreError::new(
                            "BUDGET_UNKNOWN",
                            "An ancestor execution has unknown runtime",
                        )
                    })?
                } else {
                    old.reservation.runtime_ms
                })
                .ok_or_else(limited)?;
            output = output
                .checked_add(if old.settled {
                    old.actual_output_bytes.ok_or_else(|| {
                        CoreError::new(
                            "BUDGET_UNKNOWN",
                            "An ancestor execution has unknown selected artifact bytes",
                        )
                    })?
                } else {
                    old.reservation.output_bytes
                })
                .ok_or_else(limited)?;
        }
        if runtime > a.request.budget.runtime_ms || output > a.request.budget.output_bytes {
            return Err(limited());
        }
    }
    if record.queued_ms >= record.reservation.runtime_ms {
        return Err(limited());
    }
    db.execute("INSERT INTO factory_attempts(session_id,attempt_id,scope_id,run_id,task_id,root_task_id,reservation,state,document) VALUES(?1,?2,?3,?4,?5,?6,?7,'reserved',?8)",params![s.id,s.attempt_id,c.scope.id,r.id,s.task_id,c.scope.root_task_id,encode(&record.reservation)?,encode(&json!({"session_id":s.id,"attempt_id":s.attempt_id,"scope_id":c.scope.id,"run_id":r.id,"task_id":s.task_id,"root_task_id":c.scope.root_task_id,"stop_confirmed":false,"never_started":false,"stop_reason":null}))?]).map_err(db_error)?;
    Store::reserve_shared_execution_meter(
        db,
        &record,
        &c.scope.selection.root_budget,
        &c.scope.selection.project_budget,
    )?;
    db.execute("INSERT INTO factory_budget_reservations(attempt_id,scope_id,root_task_id,project_id,document) VALUES(?1,?2,?3,?4,?5)",params![s.attempt_id,c.scope.id,c.scope.root_task_id,s.project_id,encode(&record)?]).map_err(db_error)?;
    b.session_id = Some(s.id.clone());
    b.handle.attempt_id = Some(s.attempt_id.clone());
    b.handle.state = "starting".into();
    b.handle.confirmed_stop = false;
    b.turn_sent = false;
    b.stop_reason = None;
    b.handle.revision += 1;
    save_binding(db, &mut b)?;
    r.active_task_id = Some(s.task_id.clone());
    r.active_session_id = Some(s.id.clone());
    r.state = "running".into();
    save_run(
        db,
        &mut r,
        "factory.attempt_reserved",
        json!({"session_id":s.id,"attempt_id":s.attempt_id,"task_id":s.task_id,"root_task_id":c.scope.root_task_id,"canonical_root":c.scope.bindings.workspaces[0].root,"reservation":record.reservation,"stop_confirmed":false}),
    )?;
    Ok(())
}

pub(crate) enum FactoryAction {
    Launch(FactoryDispatchContext),
    Prompt { session_id: String, text: String },
    Stop(String),
}
impl Store {
    pub(crate) fn next_factory_action(&mut self) -> Result<Option<FactoryAction>> {
        let mut q=self.conn.prepare("SELECT scope_id FROM factory_runs WHERE state NOT IN('completed','cancelled','outcome_unknown','blocked') ORDER BY rowid LIMIT 257").map_err(db_error)?;
        let ids = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        drop(q);
        if ids.len() > 256 {
            return Err(limited());
        }
        for id in ids {
            let s = scope(&self.conn, &id)?;
            let mut r = run(&self.conn, &id)?;
            let authorized = live(&self.conn, &s).is_ok();
            let all = bindings(&self.conn, &r)?;
            if r.cancel_requested || !authorized {
                let tx = self.conn.transaction().map_err(db_error)?;
                r.cancel_requested = true;
                let mut active = None;
                for mut b in all {
                    b.handle.cancel_requested = true;
                    if let Some(session) = &b.session_id {
                        let current: ManagedSession = decode(
                            &tx.query_row(
                                "SELECT document FROM sessions WHERE id=?1",
                                [session],
                                |r| r.get::<_, String>(0),
                            )
                            .map_err(db_error)?,
                        )?;
                        if !matches!(current.state.as_str(), "stopped" | "failed") {
                            active = Some(session.clone());
                            b.handle.state = "cancel_requested".into();
                        }
                    } else {
                        b.handle.state = "cancelled".into();
                        b.handle.confirmed_stop = true;
                    }
                    b.handle.revision += 1;
                    save_binding(&tx, &mut b)?;
                }
                r.state = if active.is_some() {
                    "cancel_requested"
                } else {
                    "cancelled"
                }
                .into();
                save_run(
                    &tx,
                    &mut r,
                    "factory.run_stop_requested",
                    json!({"authorized":authorized,"stop_confirmed":active.is_none()}),
                )?;
                tx.commit().map_err(db_error)?;
                if let Some(id) = active {
                    return Ok(Some(FactoryAction::Stop(id)));
                }
                continue;
            }
            if let Some(session_id) = r.active_session_id.clone() {
                let session = self.get_session(&session_id)?;
                let mut b = binding(&self.conn, &r.id, &session.task_id)?;
                if b.handle.cancel_requested {
                    return Ok(Some(FactoryAction::Stop(session_id)));
                }
                if session.state == "ready" && !b.turn_sent {
                    let text = task_prompt(&s, &b)?;
                    let tx = self.conn.transaction().map_err(db_error)?;
                    b.turn_sent = true;
                    b.handle.state = "running".into();
                    b.handle.revision += 1;
                    save_binding(&tx, &mut b)?;
                    event(
                        &tx,
                        &session.id,
                        session.revision,
                        "factory.turn_dispatch_prepared",
                        json!({"task_id":session.task_id,"request_digest":sha(&text)?}),
                        "factory_core",
                    )?;
                    tx.commit().map_err(db_error)?;
                    return Ok(Some(FactoryAction::Prompt { session_id, text }));
                }
                if session.turn_state.as_deref() == Some("completed") && b.stop_reason.is_none() {
                    let tx = self.conn.transaction().map_err(db_error)?;
                    b.stop_reason = Some("completed_turn_handoff".into());
                    b.handle.state = "stopping_for_handoff".into();
                    b.handle.revision += 1;
                    save_binding(&tx, &mut b)?;
                    event(
                        &tx,
                        &session.id,
                        session.revision,
                        "factory.internal_stop_prepared",
                        json!({"attempt_id":session.attempt_id,"turn_state":"completed","task_accepted":false}),
                        "factory_core",
                    )?;
                    tx.commit().map_err(db_error)?;
                    return Ok(Some(FactoryAction::Stop(session_id)));
                }
                if matches!(session.state.as_str(), "failed" | "outcome_unknown")
                    || matches!(
                        session.turn_state.as_deref(),
                        Some("cancelled" | "interrupted" | "outcome_unknown")
                    )
                {
                    let tx = self.conn.transaction().map_err(db_error)?;
                    r.state = "outcome_unknown".into();
                    save_run(
                        &tx,
                        &mut r,
                        "factory.run_unknown",
                        json!({"session_id":session_id,"automatic_retry":false}),
                    )?;
                    tx.commit().map_err(db_error)?;
                    return Ok(Some(FactoryAction::Stop(session_id)));
                }
                continue;
            }
            // No live parent: queued children can now acquire the same workspace.
            if let Some(b) = all.iter().find(|b| {
                matches!(
                    b.handle.state.as_str(),
                    "queued" | "blocked_on_live_parent" | "resuming"
                ) && b.session_id.is_none()
                    && !b.handle.cancel_requested
            }) {
                let mut b = b.clone();
                if b.handle.state == "blocked_on_live_parent" {
                    b.handle.state = "queued".into();
                    let tx = self.conn.transaction().map_err(db_error)?;
                    save_binding(&tx, &mut b)?;
                    tx.commit().map_err(db_error)?;
                }
                return Ok(Some(FactoryAction::Launch(FactoryDispatchContext {
                    scope: s,
                    run: r,
                    task: b,
                    queued_ms: 0,
                })));
            }
        }
        Ok(None)
    }
    pub(crate) fn factory_internal_stop_is_current(&self, id: &str) -> Result<bool> {
        if !owns_session(&self.conn, id)? {
            return Ok(false);
        }
        let session = self.get_session(id)?;
        let scope_id: String = self
            .conn
            .query_row(
                "SELECT scope_id FROM factory_attempts WHERE session_id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let s = scope(&self.conn, &scope_id)?;
        let r = run(&self.conn, &scope_id)?;
        let b = binding(&self.conn, &r.id, &session.task_id)?;
        let receipt:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='factory.internal_stop_prepared' AND json_extract(document,'$.source')='factory_core' AND json_extract(document,'$.payload.attempt_id')=?2 AND json_extract(document,'$.payload.turn_state')='completed')",params![id,session.attempt_id],|r|r.get(0)).map_err(db_error)?;
        Ok(receipt
            && live(&self.conn, &s).is_ok()
            && !r.cancel_requested
            && !b.handle.cancel_requested
            && b.session_id.as_deref() == Some(id)
            && b.stop_reason.as_deref() == Some("completed_turn_handoff")
            && session.turn_state.as_deref() == Some("completed"))
    }
    pub(crate) fn settle_factory_worker(
        &mut self,
        id: &str,
        reaped: bool,
        never_started: bool,
        reason: &str,
        capture: Option<&FactoryCapture>,
    ) -> Result<()> {
        if !owns_session(&self.conn, id)? {
            return Ok(());
        }
        let session = self.get_session(id)?;
        let scope_id: String = self
            .conn
            .query_row(
                "SELECT scope_id FROM factory_attempts WHERE session_id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let _s = scope(&self.conn, &scope_id)?;
        let mut r = run(&self.conn, &scope_id)?;
        let mut b = binding(&self.conn, &r.id, &session.task_id)?;
        let normal = reaped
            && !never_started
            && reason == "cancelled"
            && self.factory_internal_stop_is_current(id)?
            && capture.is_some_and(|c| c.inputs_current && c.recheck().is_ok())
            && self.shared_observation_within_reservation(id)?;
        let cancelled = r.cancel_requested
            || b.handle.cancel_requested
            || matches!(
                self.get(&session.task_id)?.state,
                TaskState::CancelRequested | TaskState::Cancelled
            );
        let artifacts = capture.map(|c| c.artifacts.clone()).unwrap_or_default();
        let missing = !capture.is_some_and(|c| c.outputs_complete);
        let tx = self.conn.transaction().map_err(db_error)?;
        b.handle.confirmed_stop = reaped || never_started;
        b.handle.revision += 1;
        let children: i64 = tx
            .query_row(
                "SELECT count(*) FROM factory_handoffs WHERE run_id=?1 AND parent_session_id=?2",
                params![r.id, session.id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        let can_wait = normal && children > 0;
        if normal && (!missing || can_wait) {
            b.handle.state = if can_wait {
                "waiting_child"
            } else {
                "result_available"
            }
            .into();
            b.handle.result_available = !can_wait;
            let result = FactoryTaskResult {
                task: b.handle.clone(),
                artifacts,
                validation_refs: vec![],
                acceptance_state: "not_established".into(),
                context_continuation: "new_attempt_explicit_handoff_only".into(),
                handoff_digest: None,
            };
            b.result = Some(result);
            r.active_session_id = None;
            r.active_task_id = None;
            r.state = if can_wait { "waiting_child" } else { "running" }.into();
            if !can_wait && b.handle.parent_task_id.is_none() {
                r.state = "completed".into();
            }
        } else {
            b.handle.state = if (reaped || never_started) && cancelled {
                "cancelled"
            } else {
                "outcome_unknown"
            }
            .into();
            r.state = if (reaped || never_started) && cancelled {
                "cancelled"
            } else {
                "outcome_unknown"
            }
            .into();
            r.active_session_id = None;
            r.active_task_id = None;
        }
        save_binding(&tx, &mut b)?;
        tx.execute("UPDATE factory_attempts SET state=?1,document=json_set(document,'$.stop_confirmed',?2,'$.never_started',?3,'$.stop_reason',?4) WHERE session_id=?5",params![b.handle.state,reaped||never_started,never_started,reason,id]).map_err(db_error)?;
        tx.execute(
            "UPDATE factory_scoped_handles SET revoked=1 WHERE session_id=?1",
            [id],
        )
        .map_err(db_error)?;
        save_run(
            &tx,
            &mut r,
            "factory.attempt_stopped",
            json!({"session_id":id,"attempt_id":session.attempt_id,"task_id":session.task_id,"owned_reaped":reaped,"never_started":never_started,"normal_completed_turn":normal,"selected_outputs_missing":missing,"task_accepted":false}),
        )?;
        if normal {
            tx.execute("UPDATE factory_handoffs SET document=json_set(document,'$.parent_stop_confirmed',json('true')) WHERE run_id=?1 AND parent_session_id=?2",params![r.id,session.id]).map_err(db_error)?;
            if can_wait {
                join_parent(&tx, &mut r, &session.task_id)?;
            }
            if let Some(parent) = b.handle.parent_task_id.as_ref() {
                join_parent(&tx, &mut r, parent)?;
            }
        }
        tx.commit().map_err(db_error)?;
        Ok(())
    }
}
impl Store {
    pub(crate) fn factory_mcp_for_launch(
        &mut self,
        id: &str,
        runtime: &std::path::Path,
    ) -> Result<Option<FactoryMcpLaunch>> {
        use std::io::Write;
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
        if !owns_session(&self.conn, id)? {
            return Ok(None);
        }
        let session = self.get_session(id)?;
        let scope_id: String = self
            .conn
            .query_row(
                "SELECT scope_id FROM factory_attempts WHERE session_id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let s = scope(&self.conn, &scope_id)?;
        live(&self.conn, &s)?;
        let b = binding(
            &self.conn,
            &run(&self.conn, &scope_id)?.id,
            &session.task_id,
        )?;
        if b.handle.cancel_requested {
            return Err(denied());
        }
        let handoff = b
            .handoff_results
            .iter()
            .flat_map(|r| r.artifacts.clone())
            .collect();
        if session.state != "starting" || session.project_id != s.project_id {
            return Err(denied());
        }
        let dir = runtime.join("factory-scoped-handles");
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(_) => return Err(denied()),
        }
        let metadata = std::fs::symlink_metadata(&dir).map_err(|_| denied())?;
        if !metadata.is_dir()
            || metadata.mode() & 0o7777 != 0o700
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || dir.canonicalize().ok().as_deref() != Some(&dir)
        {
            return Err(denied());
        }
        let record = HandleRecord {
            id: Uuid::new_v4().to_string(),
            scope_id: s.id.clone(),
            run_id: run(&self.conn, &s.id)?.id,
            session_id: id.into(),
            attempt_id: session.attempt_id.clone(),
            task_id: session.task_id.clone(),
            root_task_id: s.root_task_id.clone(),
            project_id: s.project_id.clone(),
            fencing_token: session.fencing_token,
            expires_at_ms: s.expires_at_ms,
        };
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let file = dir.join(format!("{}.json", record.id));
        let bytes = encode(
            &json!({"schema_version":1,"handle_id":record.id,"socket_path":runtime.join("batond.sock"),"scoped_token":token}),
        )?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        live(&tx, &s)?;
        tx.execute("INSERT INTO factory_scoped_handles(id,scope_id,session_id,attempt_id,task_id,token_digest,expires_at_ms,revoked,fencing_token,document) VALUES(?1,?2,?3,?4,?5,?6,?7,0,?8,?9)",params![record.id,s.id,id,session.attempt_id,session.task_id,agentbaton_workspaces::validation::sha256(token.as_bytes()),record.expires_at_ms,i64::try_from(record.fencing_token).map_err(|_|limited())?,encode(&record)?]).map_err(db_error)?;
        event(
            &tx,
            id,
            session.revision,
            "factory.scoped_handle_prepared",
            json!({"handle_id":record.id,"attempt_id":session.attempt_id,"scope_id":s.id,"owner_token_available":false,"private_credential_file":true}),
            "factory_core",
        )?;
        tx.commit().map_err(db_error)?;
        let mut fd = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC)
                    .bits() as i32,
            )
            .open(&file)
            .map_err(|_| denied())?;
        fd.write_all(bytes.as_bytes())
            .and_then(|()| fd.sync_all())
            .map_err(|_| denied())?;
        let capture = FactoryCapturePlan::new(
            s.clone(),
            session.task_id.clone(),
            session.attempt_id.clone(),
            self.get(&session.task_id)?.spec_revision,
            b.request.input_artifact_ids.clone(),
            b.request.output_artifact_ids.clone(),
        );
        Ok(Some(FactoryMcpLaunch {
            scope: s,
            credential: file,
            handoff,
            capture: Some(capture),
        }))
    }
}
fn recheck_external(s: &FactoryScope) -> Result<crate::data_workspaces::DataArtifactPins> {
    if super::store::companion(&s.bindings.companion.executable)? != s.bindings.companion {
        return Err(changed());
    }
    let contracts = s
        .bindings
        .inputs
        .iter()
        .map(|a| a.contract.clone())
        .collect::<Vec<_>>();
    let pins = crate::data_workspaces::DataArtifactPins::capture(&contracts)?;
    for a in &s.bindings.inputs {
        agentbaton_workspaces::validation::BoundArtifact::from_verified_contract(&a.contract)
            .map_err(|_| changed())?;
        let bytes = agentbaton_workspaces::read_artifact(
            &s.bindings.workspaces[0].root,
            &a.contract.requirement.relative_path,
            a.contract.requirement.max_bytes,
        )
        .map_err(|_| changed())?;
        if bytes.len() as u64 != a.bytes
            || agentbaton_workspaces::validation::sha256(&bytes) != a.sha256
        {
            return Err(changed());
        }
    }
    pins.verify(true)?;
    Ok(pins)
}
pub(crate) fn is_factory_task(db: &Connection, id: &str) -> Result<bool> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM factory_task_lineage WHERE task_id=?1)",
        [id],
        |r| r.get(0),
    )
    .map_err(db_error)
}
pub(crate) fn permits_factory_dispatch(
    db: &Connection,
    i: &crate::execution::DispatchIntent,
) -> Result<bool> {
    let row:Option<String>=db.query_row("SELECT scope_id FROM factory_attempts WHERE attempt_id=?1 AND task_id=?2 AND root_task_id=?3 AND state='reserved'",params![i.attempt_id,i.task_id,i.root_task_id],|r|r.get(0)).optional().map_err(db_error)?;
    let Some(id) = row else { return Ok(false) };
    let s = scope(db, &id)?;
    live(db, &s)?;
    let r = run(db, &id)?;
    let b = binding(db, &r.id, &i.task_id)?;
    Ok(!r.cancel_requested
        && !b.handle.cancel_requested
        && b.handle.attempt_id.as_deref() == Some(&i.attempt_id)
        && b.handle.state == "starting"
        && r.active_task_id.as_deref() == Some(&i.task_id)
        && s.root_task_id == i.root_task_id)
}

impl Store {
    pub(crate) fn factory_launch_error(&mut self, id: &str, error: CoreError) -> Result<()> {
        let scope_id: String = self
            .conn
            .query_row(
                "SELECT scope_id FROM factory_attempts WHERE session_id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let mut r = run(&self.conn, &scope_id)?;
        r.last_error = Some(error);
        let tx = self.conn.transaction().map_err(db_error)?;
        let payload = json!({"session_id":id,"error":r.last_error.clone()});
        save_run(&tx, &mut r, "factory.launch_rejected", payload)?;
        tx.commit().map_err(db_error)?;
        Ok(())
    }
}

pub(crate) struct FactoryMcpLaunch {
    scope: FactoryScope,
    credential: std::path::PathBuf,
    handoff: Vec<FactoryArtifactReference>,
    capture: Option<FactoryCapturePlan>,
}
impl FactoryMcpLaunch {
    pub(crate) fn take_capture_plan(&mut self) -> Option<FactoryCapturePlan> {
        self.capture.take()
    }
    pub(crate) fn authorize(self) -> Result<agentbaton_providers::ApprovedFactoryMcp> {
        if self.scope.expires_at_ms <= ms()? {
            return Err(denied());
        }
        let input_pins = recheck_external(&self.scope)?;
        let mut unique = std::collections::BTreeMap::new();
        for artifact in &self.handoff {
            if let Some(old) = unique.insert(
                artifact.relative_path.clone(),
                (artifact.sha256.clone(), artifact.bytes),
            ) {
                if old != (artifact.sha256.clone(), artifact.bytes) {
                    return Err(changed());
                }
            }
        }
        for (path, (sha, size)) in unique {
            let artifact = self
                .handoff
                .iter()
                .find(|a| a.relative_path == path && a.sha256 == sha && a.bytes == size)
                .ok_or_else(changed)?;
            let approved = self
                .scope
                .selection
                .outputs
                .iter()
                .find(|a| {
                    a.artifact_id == artifact.artifact_id
                        && a.relative_path == artifact.relative_path
                })
                .ok_or_else(denied)?;
            let bytes = agentbaton_workspaces::read_artifact(
                &self.scope.bindings.workspaces[0].root,
                &artifact.relative_path,
                approved.max_bytes,
            )
            .map_err(|_| changed())?;
            if bytes.len() as u64 != artifact.bytes
                || agentbaton_workspaces::validation::sha256(&bytes) != artifact.sha256
            {
                return Err(changed());
            }
        }
        if self.scope.expires_at_ms <= ms()? {
            return Err(denied());
        }
        let digest = agentbaton_providers::installation::BinaryDigest::parse(
            &self.scope.bindings.companion.executable_sha256,
        )
        .map_err(|_| changed())?;
        let approved = agentbaton_providers::ApprovedFactoryMcp::from_verified_host(
            &self.scope.bindings.companion.executable,
            &digest,
            &self.credential,
            &self.scope.approval_id,
        )
        .map_err(|_| changed())?;
        input_pins.verify(true)?;
        Ok(approved)
    }
}
impl Store {
    pub(crate) fn factory_session_policy_current(&self, id: &str) -> Result<bool> {
        if !owns_session(&self.conn, id)? {
            return Ok(true);
        }
        let sid: String = self
            .conn
            .query_row(
                "SELECT scope_id FROM factory_attempts WHERE session_id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let s = scope(&self.conn, &sid)?;
        let r = run(&self.conn, &sid)?;
        Ok(live(&self.conn, &s).is_ok() && !r.cancel_requested)
    }
}

impl Store {
    pub(crate) fn block_factory_run(&mut self, id: &str, error: CoreError) -> Result<()> {
        let mut r = run(&self.conn, id)?;
        r.state = "blocked".into();
        r.last_error = Some(error);
        let payload = json!({"last_error":r.last_error,"automatic_retry":false});
        let tx = self.conn.transaction().map_err(db_error)?;
        save_run(&tx, &mut r, "factory.run_blocked", payload)?;
        tx.commit().map_err(db_error)?;
        Ok(())
    }
}
impl Store {
    pub(crate) fn factory_scope_for_task(&self, id: &str) -> Result<String> {
        Ok(lineage(&self.conn, id)?.factory_scope_id)
    }
}

impl Store {
    pub(crate) fn factory_start_replay(
        &self,
        id: &str,
        rev: u64,
        key: &str,
    ) -> Result<Option<FactoryRun>> {
        replay(
            &self.conn,
            &format!("factory:{id}:start"),
            key,
            &encode(&json!({"scope_id":id,"revision":rev}))?,
        )
    }
}
fn task_prompt(s: &FactoryScope, b: &TaskBinding) -> Result<String> {
    let singleton = if b.handoff_results.len() == 1 {
        let mut child = b.handoff_results[0].clone();
        child.handoff_digest = b.handoff_digest.clone();
        Some(child)
    } else {
        None
    };
    let text = encode(
        &json!({"schema_version":1,"kind":"agentbaton_factory_task_request","task_id":b.handle.task_id,"root_task_id":s.root_task_id,"parent_task_id":b.handle.parent_task_id,"goal":b.request.goal,"input_artifacts":s.bindings.inputs.iter().filter(|a|b.request.input_artifact_ids.contains(&a.contract.requirement.artifact_id)).collect::<Vec<_>>(),"output_artifacts":s.bindings.outputs.iter().filter(|a|b.request.output_artifact_ids.contains(&a.requirement.artifact_id)).collect::<Vec<_>>(),"handoff_result":singleton,"handoff_results":b.handoff_results,"handoff_digest":b.handoff_digest,"context_continuation":"new_attempt_explicit_handoff_only","acceptance_state":"not_established"}),
    )?;
    // Preserve the existing Provider text bound; additionally check the complete
    // worst-escaped Provider wire. A large result set blocks; it is not truncated.
    if text.len() > agentbaton_providers::MAX_FRAME_BYTES / 8 {
        return Err(limited());
    }
    check_wire(
        &json!({"jsonrpc":"2.0","id":"\0".repeat(128),"method":"session/prompt","params":{"sessionId":"\0".repeat(256),"prompt":[{"type":"text","text":text}]}}),
    )?;
    Ok(text)
}
fn join_parent(db: &Connection, r: &mut FactoryRun, parent_id: &str) -> Result<()> {
    let mut parent = binding(db, &r.id, parent_id)?;
    if parent.handle.state != "waiting_child" || !parent.handle.confirmed_stop {
        return Ok(());
    }
    let sid = parent.session_id.as_deref().ok_or_else(changed)?;
    let proof:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE json_extract(document,'$.kind')='factory.attempt_stopped' AND json_extract(document,'$.source')='factory_core' AND json_extract(document,'$.payload.session_id')=?1 AND json_extract(document,'$.payload.normal_completed_turn')=1 AND json_extract(document,'$.payload.owned_reaped')=1)",[sid],|row|row.get(0)).map_err(db_error)?;
    if !proof {
        return Err(denied());
    }
    let(count,bytes):(i64,i64)=db.query_row("SELECT count(*),COALESCE(sum(octet_length(document)),0) FROM factory_handoffs WHERE run_id=?1 AND parent_session_id=?2",params![r.id,sid],|row|Ok((row.get(0)?,row.get(1)?))).map_err(db_error)?;
    if count > 1000 || bytes > MAX_FACTORY_BYTES as i64 {
        return Err(limited());
    }
    let mut query=db.prepare("SELECT child_task_id FROM factory_handoffs WHERE run_id=?1 AND parent_session_id=?2 ORDER BY child_task_id").map_err(db_error)?;
    let ids = query
        .query_map(params![r.id, sid], |row| row.get::<_, String>(0))
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    drop(query);
    if ids.is_empty() {
        return Ok(());
    }
    let mut results = Vec::new();
    for id in &ids {
        let child = binding(db, &r.id, id)?;
        if child.handle.state == "cancelled" && child.handle.confirmed_stop {
            parent.handle.state = "blocked".into();
            save_binding(db, &mut parent)?;
            r.state = "blocked".into();
            let mut error=CoreError::new("CHILD_CANCELLED","A required child was cancelled; no automatic replacement or parent retry was authorized");
            error.side_effect_state = "confirmed".into();
            r.last_error = Some(error);
            return save_run(
                db,
                r,
                "factory.join_blocked",
                json!({"parent_task_id":parent_id,"child_task_id":id,"reason":"CHILD_CANCELLED","automatic_retry":false}),
            );
        }
        if child.handle.state == "outcome_unknown" {
            r.state = "outcome_unknown".into();
            return save_run(
                db,
                r,
                "factory.join_unknown",
                json!({"parent_task_id":parent_id,"child_task_id":id,"automatic_retry":false}),
            );
        }
        if !child.handle.result_available || !child.handle.confirmed_stop {
            return Ok(());
        }
        let mut result = child.result.ok_or_else(changed)?;
        if result.task.task_id != *id || result.task.root_task_id != r.root_task_id {
            return Err(changed());
        }
        result.task = child.handle;
        results.push(result);
    }
    let digest = sha(
        &json!({"scope_id":r.scope_id,"run_id":r.id,"parent_task_id":parent_id,"parent_session_id":sid,"children":results}),
    )?;
    let old_session = sid.to_owned();
    parent.handoff_results = results;
    parent.handoff_digest = Some(digest.clone());
    let s = scope(db, &r.scope_id)?;
    if let Err(mut error) = task_prompt(&s, &parent) {
        error.side_effect_state = "confirmed".into();
        parent.handle.state = "blocked".into();
        save_binding(db, &mut parent)?;
        r.state = "blocked".into();
        r.last_error = Some(error);
        return save_run(
            db,
            r,
            "factory.join_bound_exceeded",
            json!({"parent_task_id":parent_id,"handoff_digest":digest,"silent_truncation":false,"automatic_retry":false}),
        );
    }
    // All exact sibling receipts are fixed before consuming a new parent attempt.
    live(db, &s)?;
    parent.session_id = None;
    parent.turn_sent = false;
    parent.stop_reason = None;
    parent.round = parent.round.checked_add(1).ok_or_else(limited)?;
    parent.handle.state = "resuming".into();
    save_binding(db, &mut parent)?;
    for result in &parent.handoff_results {
        db.execute("UPDATE factory_handoffs SET state='ready',child_attempt_id=?1,document=json_set(document,'$.parent_stop_confirmed',json('true'),'$.handoff_digest',?2) WHERE run_id=?3 AND parent_session_id=?4 AND child_task_id=?5",params![result.task.attempt_id,digest,r.id,old_session,result.task.task_id]).map_err(db_error)?;
    }
    r.state = "resuming".into();
    save_run(
        db,
        r,
        "factory.handoff_prepared",
        json!({"parent_task_id":parent_id,"parent_session_id":old_session,"child_task_ids":ids,"handoff_digest":digest,"handoff_results":parent.handoff_results,"context_continuation":"new_attempt_explicit_handoff_only","task_accepted":false}),
    )
}
