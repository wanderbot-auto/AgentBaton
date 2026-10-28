//! Durable dispatch boundaries. These trusted core methods are deliberately not IPC methods:
//! policy resolution and native executor identity verification must precede their use.
use crate::{
    storage::{append_event, db_error, decode, encode, now, Store, Task, TaskState},
    CoreError, Result,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionBinding {
    pub connection_id: String,
    pub environment_id: String,
    pub identity_ref: String,
    pub host_id: String,
    pub workspace_id: String,
    pub config_revision: String,
    pub input_revision: String,
    pub capability_snapshot_id: String,
    pub policy_revision: String,
}
impl ExecutionBinding {
    fn validate(&self) -> Result<()> {
        let fields = [
            &self.connection_id,
            &self.environment_id,
            &self.identity_ref,
            &self.host_id,
            &self.workspace_id,
            &self.config_revision,
            &self.input_revision,
            &self.capability_snapshot_id,
            &self.policy_revision,
        ];
        if fields.iter().any(|s| s.trim().is_empty() || s.len() > 512) {
            return Err(CoreError::new("INVALID_REQUEST","Execution requires fixed connection, identity, host, workspace, configuration, input, capability and policy references"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentState {
    Prepared,
    Sent,
    OutcomeUnknown,
    Stopped,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchIntent {
    pub id: String,
    pub attempt_id: String,
    pub task_id: String,
    pub root_task_id: String,
    pub state: IntentState,
    pub executor_instance: Option<String>,
    pub binding: ExecutionBinding,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub total_limit: u32,
    pub parallel_limit: u32,
    pub consumed: u32,
    pub active: u32,
}
/// Evidence is accepted only from a trusted supervisor; not from renderer or provider text.
pub enum StopEvidence {
    NeverDispatched,
    ConfirmedNotStarted {
        executor_instance: String,
        receipt_ref: String,
    },
    ConfirmedStopped {
        executor_instance: String,
        receipt_ref: String,
    },
}
fn budget_key(project: bool, id: &str) -> String {
    format!("{}:{id}", if project { "project" } else { "root" })
}

pub(crate) fn migrate(conn: &mut Connection, original_version: u32) -> Result<()> {
    if original_version < 2 {
        let tx = conn.transaction().map_err(db_error)?;
        tx.execute_batch("CREATE TABLE budgets(scope TEXT PRIMARY KEY,total_limit INTEGER NOT NULL CHECK(total_limit>0),parallel_limit INTEGER NOT NULL CHECK(parallel_limit>0),consumed INTEGER NOT NULL DEFAULT 0,active INTEGER NOT NULL DEFAULT 0);
        CREATE TABLE attempts(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),root_task_id TEXT NOT NULL REFERENCES tasks(id),binding TEXT NOT NULL,idempotency_key TEXT NOT NULL,UNIQUE(task_id,idempotency_key));
        CREATE TABLE dispatch_intents(id TEXT PRIMARY KEY,attempt_id TEXT NOT NULL UNIQUE REFERENCES attempts(id),state TEXT NOT NULL CHECK(state IN('prepared','sent','outcome_unknown','stopped')),executor_instance TEXT,document TEXT NOT NULL);
        PRAGMA user_version=2;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}
impl Store {
    /// Server-owned budget policy. Existing policy cannot be silently widened or reset.
    pub fn configure_budget(
        &mut self,
        project: bool,
        id: &str,
        total_limit: u32,
        parallel_limit: u32,
    ) -> Result<()> {
        if id.is_empty() || total_limit == 0 || parallel_limit == 0 || parallel_limit > total_limit
        {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "Budgets require positive total and bounded parallel limits",
            ));
        }
        let key = budget_key(project, id);
        let existing = self.budget(project, id)?;
        if let Some(b) = existing {
            if b.total_limit == total_limit && b.parallel_limit == parallel_limit {
                return Ok(());
            }
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Budget policy changes require a new approved policy revision",
            ));
        }
        self.conn
            .execute(
                "INSERT INTO budgets(scope,total_limit,parallel_limit) VALUES(?1,?2,?3)",
                params![key, total_limit, parallel_limit],
            )
            .map_err(db_error)?;
        Ok(())
    }
    pub fn budget(&self, project: bool, id: &str) -> Result<Option<BudgetStatus>> {
        self.conn
            .query_row(
                "SELECT total_limit,parallel_limit,consumed,active FROM budgets WHERE scope=?1",
                [budget_key(project, id)],
                |r| {
                    Ok(BudgetStatus {
                        total_limit: r.get(0)?,
                        parallel_limit: r.get(1)?,
                        consumed: r.get(2)?,
                        active: r.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(db_error)
    }
    /// Persist immutable binding + two budget reservations before crossing the dispatch boundary.
    /// This initial API supports root tasks; child delegation must supply independently checked lineage.
    pub fn prepare_attempt(
        &mut self,
        task_id: &str,
        expected_revision: u64,
        binding: ExecutionBinding,
        key: &str,
    ) -> Result<DispatchIntent> {
        binding.validate()?;
        if key.is_empty() || key.len() > 256 {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "Attempt idempotency key is required",
            ));
        }
        let task = self.get(task_id)?;
        let old:Option<String>=self.conn.query_row("SELECT d.document FROM attempts a JOIN dispatch_intents d ON d.attempt_id=a.id WHERE a.task_id=?1 AND a.idempotency_key=?2",params![task_id,key],|r|r.get(0)).optional().map_err(db_error)?;
        if let Some(raw) = old {
            let intent: DispatchIntent = decode(&raw)?;
            if intent.binding != binding {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Attempt key already belongs to a different execution binding",
                ));
            }
            return Ok(intent);
        }
        if task.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Task changed before attempt preparation",
            ));
        }
        if task.state != TaskState::Draft {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Task is not ready for a new attempt",
            ));
        }
        let tx = self.conn.transaction().map_err(db_error)?;
        crate::goal_store::ordinary_session_budget_gate(&tx, &task.project_id)?;
        let active:i64=tx.query_row("SELECT count(*) FROM attempts a JOIN dispatch_intents d ON d.attempt_id=a.id WHERE a.task_id=?1 AND d.state!='stopped'",[task_id],|r|r.get(0)).map_err(db_error)?;
        if active != 0 {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "Task already has an unresolved attempt",
            ));
        }
        for scope in [
            budget_key(true, &task.project_id),
            budget_key(false, &task.id),
        ] {
            let changed=tx.execute("UPDATE budgets SET consumed=consumed+1,active=active+1 WHERE scope=?1 AND consumed<total_limit AND active<parallel_limit",[scope]).map_err(db_error)?;
            if changed != 1 {
                return Err(CoreError::new(
                    "RESOURCE_LIMIT",
                    "Project or root task budget is missing or exhausted",
                ));
            }
        }
        let intent = DispatchIntent {
            id: Uuid::new_v4().to_string(),
            attempt_id: Uuid::new_v4().to_string(),
            task_id: task.id.clone(),
            root_task_id: task.id.clone(),
            state: IntentState::Prepared,
            executor_instance: None,
            binding,
        };
        tx.execute(
            "INSERT INTO attempts VALUES(?1,?2,?3,?4,?5)",
            params![
                intent.attempt_id,
                intent.task_id,
                intent.root_task_id,
                encode(&intent.binding)?,
                key
            ],
        )
        .map_err(db_error)?;
        tx.execute(
            "INSERT INTO dispatch_intents VALUES(?1,?2,'prepared',NULL,?3)",
            params![intent.id, intent.attempt_id, encode(&intent)?],
        )
        .map_err(db_error)?;
        let mut changed = task;
        changed.revision += 1;
        changed.updated_at = now()?;
        tx.execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![encode(&changed)?, changed.id],
        )
        .map_err(db_error)?;
        append_event(
            &tx,
            &changed,
            "attempt.prepared",
            json!({"attempt_id":intent.attempt_id,"intent_id":intent.id,"binding":intent.binding}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(intent)
    }
    pub fn intent(&self, id: &str) -> Result<DispatchIntent> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT document FROM dispatch_intents WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?;
        decode(&raw.ok_or_else(|| CoreError::new("NOT_FOUND", "Dispatch intent does not exist"))?)
    }
    /// Commit 'sent' before touching an external executor. A crash beyond this point needs reconciliation.
    pub fn mark_dispatch_started(
        &mut self,
        id: &str,
        executor_instance: &str,
    ) -> Result<DispatchIntent> {
        if executor_instance.trim().is_empty() || executor_instance.len() > 512 {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "A supervisor-owned executor instance identity is required",
            ));
        }
        let mut intent = self.intent(id)?;
        if intent.state != IntentState::Prepared {
            return Err(CoreError::new(
                "OUTCOME_UNKNOWN",
                "Intent cannot be dispatched again; reconcile its existing outcome",
            ));
        }
        let mut task = self.get(&intent.task_id)?;
        if task.state != TaskState::Draft
            && !(task.state == TaskState::Running
                && crate::goal_store::permits_continuing_task(&self.conn, &intent)?)
        {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Task no longer permits dispatch",
            ));
        }
        intent.state = IntentState::Sent;
        intent.executor_instance = Some(executor_instance.into());
        task.state = TaskState::Running;
        self.persist_intent(
            &intent,
            &mut task,
            "attempt.dispatch_started",
            json!({"intent_id":id}),
        )?;
        Ok(intent)
    }
    pub fn confirm_stop(&mut self, id: &str, evidence: StopEvidence) -> Result<DispatchIntent> {
        let mut intent = self.intent(id)?;
        if intent.state == IntentState::Stopped {
            return Ok(intent);
        }
        let receipt = match evidence {
            StopEvidence::NeverDispatched if intent.state == IntentState::Prepared => {
                json!({"kind":"never_dispatched"})
            }
            StopEvidence::ConfirmedNotStarted {
                executor_instance,
                receipt_ref,
            } if matches!(
                intent.state,
                IntentState::Sent | IntentState::OutcomeUnknown
            ) && intent.executor_instance.as_deref() == Some(&executor_instance)
                && !receipt_ref.trim().is_empty()
                && receipt_ref.len() <= 512 =>
            {
                json!({"kind":"executor_not_started","executor_instance":executor_instance,"receipt_ref":receipt_ref})
            }
            StopEvidence::ConfirmedStopped {
                executor_instance,
                receipt_ref,
            } if matches!(
                intent.state,
                IntentState::Sent | IntentState::OutcomeUnknown
            ) && intent.executor_instance.as_deref() == Some(&executor_instance)
                && !receipt_ref.trim().is_empty()
                && receipt_ref.len() <= 512 =>
            {
                json!({"kind":"executor_stop","executor_instance":executor_instance,"receipt_ref":receipt_ref})
            }
            _ => {
                return Err(CoreError::new(
                    "PERMISSION_DENIED",
                    "Stop evidence does not match the recorded dispatch identity",
                ))
            }
        };
        let mut task = self.get(&intent.task_id)?;
        let tx = self.conn.transaction().map_err(db_error)?;
        for scope in [
            budget_key(true, &task.project_id),
            budget_key(false, &intent.root_task_id),
        ] {
            let changed = tx
                .execute(
                    "UPDATE budgets SET active=active-1 WHERE scope=?1 AND active>0",
                    [scope],
                )
                .map_err(db_error)?;
            if changed != 1 {
                return Err(CoreError::new(
                    "INTERNAL_ERROR",
                    "Budget reservation invariant failed",
                ));
            }
        }
        intent.state = IntentState::Stopped;
        task.state = if task.state == TaskState::CancelRequested {
            TaskState::Cancelled
        } else if crate::goal_store::permits_continuing_task(&tx, &intent)? {
            TaskState::Running
        } else if receipt["kind"] == "executor_not_started" || receipt["kind"] == "never_dispatched"
        {
            TaskState::Draft
        } else {
            TaskState::OutcomeUnknown
        };
        task.revision += 1;
        task.updated_at = now()?;
        tx.execute(
            "UPDATE dispatch_intents SET state='stopped',document=?1 WHERE id=?2",
            params![encode(&intent)?, id],
        )
        .map_err(db_error)?;
        tx.execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![encode(&task)?, task.id],
        )
        .map_err(db_error)?;
        append_event(
            &tx,
            &task,
            "attempt.stopped",
            json!({"intent_id":id,"stop_evidence":receipt}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(intent)
    }
    pub(crate) fn recover_dispatches(&mut self) -> Result<()> {
        let ids: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT id FROM dispatch_intents WHERE state='sent'")
                .map_err(db_error)?;
            let values = stmt
                .query_map([], |r| r.get(0))
                .map_err(db_error)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_error)?;
            values
        };
        for id in ids {
            let mut intent = self.intent(&id)?;
            intent.state = IntentState::OutcomeUnknown;
            let mut task = self.get(&intent.task_id)?;
            task.state = TaskState::OutcomeUnknown;
            self.persist_intent(
                &intent,
                &mut task,
                "attempt.outcome_unknown",
                json!({"intent_id":id,"reason":"daemon_restart_requires_reconciliation"}),
            )?;
        }
        Ok(())
    }
    fn persist_intent(
        &mut self,
        intent: &DispatchIntent,
        task: &mut Task,
        kind: &str,
        payload: serde_json::Value,
    ) -> Result<()> {
        task.revision += 1;
        task.updated_at = now()?;
        let tx = self.conn.transaction().map_err(db_error)?;
        let state = match intent.state {
            IntentState::Prepared => "prepared",
            IntentState::Sent => "sent",
            IntentState::OutcomeUnknown => "outcome_unknown",
            IntentState::Stopped => "stopped",
        };
        tx.execute(
            "UPDATE dispatch_intents SET state=?1,executor_instance=?2,document=?3 WHERE id=?4",
            params![state, intent.executor_instance, encode(intent)?, intent.id],
        )
        .map_err(db_error)?;
        tx.execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![encode(task)?, task.id],
        )
        .map_err(db_error)?;
        append_event(&tx, task, kind, payload)?;
        tx.commit().map_err(db_error)?;
        Ok(())
    }
}
