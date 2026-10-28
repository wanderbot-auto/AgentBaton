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
