use crate::{CoreError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt, path::Path};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

/// Bound journal decoding before constructing an IPC response. Count limits
/// alone do not bound memory when normalized Provider messages are large.
pub const MAX_EVENT_PAGE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Draft,
    Running,
    CancelRequested,
    Cancelled,
    OutcomeUnknown,
    Accepted,
}
fn initial_spec_revision() -> u64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub goal: String,
    #[serde(default = "initial_spec_revision")]
    pub spec_revision: u64,
    pub state: TaskState,
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_sequence: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub sequence: u64,
    pub event_id: String,
    pub aggregate_id: String,
    pub revision: u64,
    pub kind: String,
    pub occurred_at: String,
    pub payload: Value,
    pub source: String,
}
pub(crate) fn db_error(error: rusqlite::Error) -> CoreError {
    let mut e = CoreError::new(
        "INTERNAL_ERROR",
        "Local database operation failed; reconcile before retrying writes",
    );
    e.side_effect_state = "unknown".into();
    // Numeric diagnostics only; no SQL, paths, parameter values or engine text.
    if let rusqlite::Error::SqliteFailure(code, _) = error {
        e.details_ref = Some(format!("sqlite-code:{}", code.extended_code));
    }
    e
}
pub(crate) fn now() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| CoreError::new("INTERNAL_ERROR", "Clock formatting failed"))
}
pub(crate) fn encode<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value)
        .map_err(|_| CoreError::new("INTERNAL_ERROR", "Serialization failed"))
}
pub(crate) fn decode<T: for<'a> Deserialize<'a>>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|_| {
        CoreError::new(
            "INTERNAL_ERROR",
            "Stored record is invalid; restore or export the database",
        )
    })
}
pub struct Store {
    pub(crate) conn: Connection,
    // Ephemeral, exact-request authority; set only while dispatching a worker result.
    pub(crate) data_input: Option<crate::data_workspaces::DataInputWitness>,
    pub(crate) graph_input: Option<crate::goal_store::GraphReadWitness>,
    pub(crate) factory_read_control: Option<crate::factory::FactoryReadControl>,
    pub(crate) graph_capture_clocks: std::collections::HashMap<String, std::time::Instant>,
    // Timing notifications only, emitted after a verified new planner-recovery
    // transaction commits. They are neither persisted authority nor a refund.
    pub(crate) factory_recovery_queue_epochs: Vec<(String, u32, std::time::Instant)>,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let mut conn = Connection::open(path).map_err(db_error)?;
        conn.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(db_error)?;
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_error)?;
        if version > 15 {
            return Err(CoreError::new(
                "VERSION_UNSUPPORTED",
                "Database was created by a newer application",
            ));
        }
        if version > 0 && version < 15 {
            let backup = path.with_extension(format!("pre-v15-{}.sqlite3", Uuid::new_v4()));
            drop(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&backup)
                    .map_err(|_| {
                        CoreError::new(
                            "INTERNAL_ERROR",
                            "Cannot create child Goal authority migration backup",
                        )
                    })?,
            );
            conn.backup(rusqlite::MAIN_DB, &backup, None)
                .map_err(db_error)?;
        }
        if version > 0 && version < 14 {
            let backup = path.with_extension(format!("pre-v14-{}.sqlite3", Uuid::new_v4()));
            drop(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&backup)
                    .map_err(|_| {
                        CoreError::new(
                            "INTERNAL_ERROR",
                            "Cannot create Factory child-plan migration backup",
                        )
                    })?,
            );
            conn.backup(rusqlite::MAIN_DB, &backup, None)
                .map_err(db_error)?;
        }
        if version > 0 && version < 13 {
            let backup = path.with_extension(format!("pre-v13-{}.sqlite3", Uuid::new_v4()));
            drop(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&backup)
                    .map_err(|_| {
                        CoreError::new("INTERNAL_ERROR", "Cannot create Factory migration backup")
                    })?,
            );
            conn.backup(rusqlite::MAIN_DB, &backup, None)
                .map_err(db_error)?;
        }
        if version > 0 && version < 12 {
            let backup = path.with_extension(format!("pre-v12-{}.sqlite3", Uuid::new_v4()));
            drop(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&backup)
                    .map_err(|_| {
                        CoreError::new(
                            "INTERNAL_ERROR",
                            "Cannot create Goal graph migration backup",
                        )
                    })?,
            );
            conn.backup(rusqlite::MAIN_DB, &backup, None)
                .map_err(db_error)?;
        }
        if version > 0 && version < 11 {
            let backup = path.with_extension(format!("pre-v11-{}.sqlite3", Uuid::new_v4()));
            drop(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&backup)
                    .map_err(|_| {
                        CoreError::new(
                            "INTERNAL_ERROR",
                            "Cannot create Goal runtime migration backup",
                        )
                    })?,
            );
            conn.backup(rusqlite::MAIN_DB, &backup, None)
                .map_err(db_error)?;
        }
        if version > 0 && version < 10 {
            let backup = path.with_extension(format!("pre-v10-{}.sqlite3", Uuid::new_v4()));
            drop(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&backup)
                    .map_err(|_| {
                        CoreError::new(
                            "INTERNAL_ERROR",
                            "Cannot create data-workspace migration backup",
                        )
                    })?,
            );
            conn.backup(rusqlite::MAIN_DB, &backup, None)
                .map_err(db_error)?;
        }
        if version == 0 {
            let tables:u32=conn.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0)).map_err(db_error)?;
            if tables != 0 {
                return Err(CoreError::new(
                    "VERSION_UNSUPPORTED",
                    "Unrecognized database; refusing an in-place migration",
                ));
            }
            let tx = conn.transaction().map_err(db_error)?;
            tx.execute_batch("CREATE TABLE tasks(id TEXT PRIMARY KEY, document TEXT NOT NULL); CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT, document TEXT NOT NULL); CREATE TABLE idempotency(scope TEXT NOT NULL, key TEXT NOT NULL, payload TEXT NOT NULL, result TEXT NOT NULL, PRIMARY KEY(scope,key)); PRAGMA user_version=1;").map_err(db_error)?;
            tx.commit().map_err(db_error)?;
        }
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(db_error)?;
        if version > 0 && version < 7 {
            let backup = path.with_extension(format!("pre-v7-{}.sqlite3", Uuid::new_v4()));
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&backup)
                .map_err(|_| {
                    CoreError::new("INTERNAL_ERROR", "Cannot create pre-migration backup")
                })?;
            conn.backup(rusqlite::MAIN_DB, &backup, None)
                .map_err(db_error)?;
        }
        crate::execution::migrate(&mut conn, version)?;
        migrate_registry(&mut conn, version)?;
        migrate_sessions(&mut conn, version)?;
        migrate_runtime_snapshots(&mut conn, version)?;
        migrate_validations(&mut conn, version)?;
        migrate_data_validations(&mut conn, version)?;
        if version > 0 && version < 8 {
            let backup = path.with_extension(format!("pre-v8-{}.sqlite3", Uuid::new_v4()));
            drop(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&backup)
                    .map_err(|_| {
                        CoreError::new("INTERNAL_ERROR", "Cannot create command-migration backup")
                    })?,
            );
            conn.backup(rusqlite::MAIN_DB, &backup, None)
                .map_err(db_error)?;
        }
        migrate_command_validations(&mut conn, version)?;
        if version > 0 && version < 9 {
            let backup = path.with_extension(format!("pre-v9-{}.sqlite3", Uuid::new_v4()));
            drop(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(&backup)
                    .map_err(|_| {
                        CoreError::new("INTERNAL_ERROR", "Cannot create Goal-migration backup")
                    })?,
            );
            conn.backup(rusqlite::MAIN_DB, &backup, None)
                .map_err(db_error)?;
        }
        crate::goal_store::migrate(&mut conn, version)?;
        crate::data_workspaces::migrate(&mut conn, version)?;
        crate::goal_store::migrate_runtime(&mut conn, version)?;
        crate::goal_store::migrate_graph(&mut conn, version)?;
        crate::factory::migrate(&mut conn, version)?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )
        .map_err(db_error)?;
        let mut store = Self {
            conn,
            data_input: None,
            graph_input: None,
            factory_read_control: None,
            graph_capture_clocks: Default::default(),
            factory_recovery_queue_epochs: Vec::new(),
        };
        store.recover_dispatches()?;
        store.recover_workspace_creations()?;
        store.recover_sessions()?;
        store.recover_command_validations()?;
        store.recover_data_workspace_operations()?;
        store.recover_factory_runs()?;
        store.recover_graph_jobs()?;
        store.recover_goal_runs()?;
        Ok(store)
    }
    /// Authenticated entry point: new tasks require an active registered project or legacy local scope.
    /// Replay returns its original result even after archive, avoiding ambiguity about an earlier write.
    pub fn create_registered(&mut self, project_id: &str, goal: &str, key: &str) -> Result<Task> {
        if project_id != "local" {
            let scope = format!("local:{project_id}:tasks.create");
            let old: Option<(String, String)> = self
                .conn
                .query_row(
                    "SELECT payload,result FROM idempotency WHERE scope=?1 AND key=?2",
                    params![scope, key],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(db_error)?;
            if let Some((payload, result)) = old {
                if payload != encode(&json!({"project_id":project_id,"goal":goal}))? {
                    return Err(CoreError::new(
                        "CONFIG_CONFLICT",
                        "Idempotency key already used with a different request",
                    ));
                }
                return decode(&result);
            }
            let project = self.get_project(project_id)?;
            if project.archived {
                return Err(CoreError::new(
                    "PERMISSION_DENIED",
                    "Archived project cannot accept new tasks",
                ));
            }
        }
        self.create(project_id, goal, key)
    }
    pub fn create(&mut self, project_id: &str, goal: &str, key: &str) -> Result<Task> {
        if goal.trim().is_empty()
            || goal.len() > 16384
            || key.is_empty()
            || key.len() > 256
            || project_id.is_empty()
            || project_id.len() > 128
        {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "Goal, project and idempotency key are required and must fit their size limits",
            ));
        }
        let payload = encode(&json!({"project_id":project_id,"goal":goal}))?;
        // There is one authenticated local user; keys are additionally scoped by project and method.
        let scope = format!("local:{project_id}:tasks.create");
        let tx = self.conn.transaction().map_err(db_error)?;
        let old: Option<(String, String)> = tx
            .query_row(
                "SELECT payload,result FROM idempotency WHERE scope=?1 AND key=?2",
                params![scope, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        if let Some((old_payload, result)) = old {
            if old_payload != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Idempotency key already used with a different request",
                ));
            }
            return decode(&result);
        }
        let timestamp = now()?;
        let task = Task {
            id: Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            goal: goal.into(),
            spec_revision: 1,
            state: TaskState::Draft,
            revision: 1,
            acceptance_ref: None,
            acceptance_sequence: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        tx.execute(
            "INSERT INTO tasks VALUES(?1,?2)",
            params![task.id, encode(&task)?],
        )
        .map_err(db_error)?;
        append_event(&tx, &task, "task.created", json!({"goal":task.goal}))?;
        tx.execute(
            "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
            params![scope, key, payload, encode(&task)?],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(task)
    }
    pub fn get(&self, id: &str) -> Result<Task> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT document FROM tasks WHERE id=?1", [id], |r| r.get(0))
            .optional()
            .map_err(db_error)?;
        let task: Task =
            decode(&raw.ok_or_else(|| CoreError::new("NOT_FOUND", "Task does not exist"))?)?;
        crate::goal_store::verify_task_acceptance_state(&self.conn, &task)?;
        Ok(task)
    }
    pub fn list(&self) -> Result<Vec<Task>> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM tasks", [], |r| r.get(0))
            .map_err(db_error)?;
        if count > 1000 {
            return Err(CoreError::new("RESOURCE_LIMIT","Task listing exceeds 1000 records; use tasks.get until paginated listing is available"));
        }
        let mut stmt = self
            .conn
            .prepare("SELECT document FROM tasks ORDER BY rowid DESC LIMIT 1000")
            .map_err(db_error)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?;
        rows.map(|r| {
            let task: Task = decode(&r.map_err(db_error)?)?;
            crate::goal_store::verify_task_acceptance_state(&self.conn, &task)?;
            Ok(task)
        })
        .collect()
    }
    pub fn cancel(&mut self, id: &str, expected_revision: u64) -> Result<Task> {
        let mut task = self.get(id)?;
        if task.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Task changed; refresh before cancelling",
            ));
        }
        if matches!(
            task.state,
            TaskState::Accepted | TaskState::Cancelled | TaskState::CancelRequested
        ) {
            return Ok(task);
        }
        let active:i64=self.conn.query_row("SELECT count(*) FROM attempts a JOIN dispatch_intents d ON d.attempt_id=a.id WHERE a.task_id=?1 AND d.state!='stopped'",[id],|r|r.get(0)).map_err(db_error)?;
        let command_active:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM command_validation_runs WHERE task_id=?1 AND state IN('prepared','running','cancel_requested','outcome_unknown'))",[id],|r|r.get(0)).map_err(db_error)?;
        let no_attempt = task.state == TaskState::Draft && active == 0 && !command_active;
        task.state = if no_attempt {
            TaskState::Cancelled
        } else {
            TaskState::CancelRequested
        };
        task.revision += 1;
        task.updated_at = now()?;
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![encode(&task)?, id],
        )
        .map_err(db_error)?;
        append_event(
            &tx,
            &task,
            if no_attempt {
                "task.cancelled"
            } else {
                "task.cancel_requested"
            },
            json!({"stop_evidence":if no_attempt {Some("no_attempt_dispatched")} else {None}}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(task)
    }
    pub fn sequence(&self) -> Result<u64> {
        self.conn
            .query_row("SELECT COALESCE(MAX(sequence),0) FROM events", [], |r| {
                r.get::<_, i64>(0)
            })
            .map(|v| v as u64)
            .map_err(db_error)
    }
    pub fn events(&self, cursor: u64, limit: u32) -> Result<Value> {
        let sequence = self.sequence()?;
        if cursor > sequence {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Event cursor is ahead of this journal; obtain a snapshot",
            ));
        }
        if limit == 0 || limit > 500 {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "Event batch limit must be 1 through 500",
            ));
        }
        let mut stmt = self
            .conn
            .prepare(
                "SELECT sequence,octet_length(document) FROM events WHERE sequence>?1 ORDER BY sequence LIMIT ?2",
            )
            .map_err(db_error)?;
        let rows = stmt
            .query_map(params![cursor as i64, limit], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)?))
            })
            .map_err(db_error)?;
        let mut events = Vec::new();
        let mut raw_bytes = 0usize;
        let mut byte_limited = false;
        let mut documents = self
            .conn
            .prepare_cached("SELECT document FROM events WHERE sequence=?1")
            .map_err(db_error)?;
        for row in rows {
            let (seq, bytes) = row.map_err(db_error)?;
            let bytes = usize::try_from(bytes)
                .map_err(|_| CoreError::new("INTERNAL_ERROR", "Invalid journal record length"))?;
            if bytes > MAX_EVENT_PAGE_BYTES.saturating_sub(raw_bytes) {
                if events.is_empty() {
                    let mut error = CoreError::new(
                        "RESOURCE_LIMIT",
                        "One journal record exceeds the bounded event page; it was not skipped",
                    );
                    error.details_ref = Some(format!("event-sequence:{seq}"));
                    return Err(error);
                }
                byte_limited = true;
                break;
            }
            let sequence_id = i64::try_from(seq)
                .map_err(|_| CoreError::new("INTERNAL_ERROR", "Invalid journal sequence"))?;
            let raw: String = documents
                .query_row([sequence_id], |row| row.get(0))
                .map_err(db_error)?;
            // Same connection/read snapshot, with a defensive post-read check.
            if raw.len() != bytes {
                return Err(CoreError::new(
                    "INPUT_CHANGED",
                    "Journal record changed during bounded pagination",
                ));
            }
            let mut event: Event = decode(&raw)?;
            event.sequence = seq;
            events.push(event);
            raw_bytes += bytes;
        }
        let next_cursor = events.last().map_or(cursor, |e| e.sequence);
        Ok(
            json!({"events":events,"next_cursor":next_cursor,"sequence":sequence,"gap":false,"page_bytes":raw_bytes,"byte_limited":byte_limited}),
        )
    }
}
pub(crate) fn append_event(
    tx: &rusqlite::Transaction<'_>,
    task: &Task,
    kind: &str,
    payload: Value,
) -> Result<()> {
    let event = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: task.id.clone(),
        revision: task.revision,
        kind: kind.into(),
        occurred_at: task.updated_at.clone(),
        payload,
        source: "user_action".into(),
    };
    tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&event)?])
        .map_err(db_error)?;
    Ok(())
}

fn migrate_registry(conn: &mut Connection, original_version: u32) -> Result<()> {
    if original_version < 3 {
        let tx = conn.transaction().map_err(db_error)?;
        tx.execute_batch("CREATE TABLE projects(id TEXT PRIMARY KEY,document TEXT NOT NULL,device TEXT NOT NULL,inode TEXT NOT NULL,root TEXT UNIQUE NOT NULL,UNIQUE(device,inode));
        CREATE TABLE managed_workspaces(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),document TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN('creating','ready','outcome_unknown')),idempotency_key TEXT NOT NULL,payload TEXT NOT NULL,root TEXT UNIQUE NOT NULL,UNIQUE(project_id,idempotency_key));
        CREATE TABLE workspace_leases(resource TEXT PRIMARY KEY REFERENCES managed_workspaces(id),holder TEXT NOT NULL,fencing_token INTEGER NOT NULL,expires_at INTEGER NOT NULL);
        PRAGMA user_version=3;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}

fn migrate_sessions(conn: &mut Connection, version: u32) -> Result<()> {
    if version < 4 {
        let tx = conn.transaction().map_err(db_error)?;
        tx.execute_batch("CREATE TABLE approval_proposals(id TEXT PRIMARY KEY,document TEXT NOT NULL,result TEXT);
        CREATE TABLE connections(id TEXT PRIMARY KEY,document TEXT NOT NULL);
        CREATE TABLE execution_grants(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),parent_id TEXT REFERENCES execution_grants(id),document TEXT NOT NULL);
        CREATE TABLE sessions(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),project_id TEXT NOT NULL REFERENCES projects(id),workspace_id TEXT NOT NULL REFERENCES managed_workspaces(id),connection_id TEXT NOT NULL REFERENCES connections(id),grant_id TEXT NOT NULL REFERENCES execution_grants(id),root_grant_id TEXT NOT NULL REFERENCES execution_grants(id),state TEXT NOT NULL,document TEXT NOT NULL,idempotency_key TEXT NOT NULL,payload TEXT NOT NULL,UNIQUE(task_id,idempotency_key));
        CREATE TABLE session_turns(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id),state TEXT NOT NULL,document TEXT NOT NULL,idempotency_key TEXT NOT NULL,payload TEXT NOT NULL,UNIQUE(session_id,idempotency_key));
        PRAGMA user_version=4;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}

fn migrate_runtime_snapshots(conn: &mut Connection, version: u32) -> Result<()> {
    if version < 5 {
        let tx = conn.transaction().map_err(db_error)?;
        tx.execute_batch("CREATE TABLE session_runtime_snapshots(session_id TEXT PRIMARY KEY REFERENCES sessions(id),attempt_id TEXT NOT NULL UNIQUE REFERENCES attempts(id),config_revision TEXT NOT NULL,document TEXT NOT NULL);PRAGMA user_version=5;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}

// Validation policy approval is separate from execution authorization. A passed
// artifact-integrity record is not task acceptance or permission to integrate.
fn migrate_validations(conn: &mut Connection, version: u32) -> Result<()> {
    if version < 6 {
        let tx = conn.transaction().map_err(db_error)?;
        tx.execute_batch("CREATE TABLE validation_policies(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),session_id TEXT NOT NULL REFERENCES sessions(id),attempt_id TEXT NOT NULL REFERENCES attempts(id),workspace_id TEXT NOT NULL REFERENCES managed_workspaces(id),approval_id TEXT NOT NULL UNIQUE REFERENCES approval_proposals(id),document TEXT NOT NULL);
        CREATE TABLE validation_runs(id TEXT PRIMARY KEY,policy_id TEXT NOT NULL REFERENCES validation_policies(id),task_id TEXT NOT NULL REFERENCES tasks(id),state TEXT NOT NULL CHECK(state IN('integrity_passed','failed')),document TEXT NOT NULL,idempotency_key TEXT NOT NULL,payload TEXT NOT NULL,UNIQUE(policy_id,idempotency_key));
        PRAGMA user_version=6;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}

// Data contract policies and receipts never share the integrity-only namespace.
fn migrate_data_validations(conn: &mut Connection, version: u32) -> Result<()> {
    if version < 7 {
        let tx = conn.transaction().map_err(db_error)?;
        tx.execute_batch("CREATE TABLE data_validation_policies(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),session_id TEXT NOT NULL REFERENCES sessions(id),attempt_id TEXT NOT NULL REFERENCES attempts(id),workspace_id TEXT NOT NULL REFERENCES managed_workspaces(id),approval_id TEXT NOT NULL UNIQUE REFERENCES approval_proposals(id),document TEXT NOT NULL);
        CREATE TABLE data_validation_runs(id TEXT PRIMARY KEY,policy_id TEXT NOT NULL REFERENCES data_validation_policies(id),task_id TEXT NOT NULL REFERENCES tasks(id),state TEXT NOT NULL CHECK(state IN('contract_passed','contract_failed')),document TEXT NOT NULL,idempotency_key TEXT NOT NULL,payload TEXT NOT NULL,UNIQUE(policy_id,idempotency_key));
        PRAGMA user_version=7;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}

fn migrate_command_validations(conn: &mut Connection, version: u32) -> Result<()> {
    if version < 8 {
        let tx = conn.transaction().map_err(db_error)?;
        tx.execute_batch("CREATE TABLE command_validation_policies(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),session_id TEXT NOT NULL REFERENCES sessions(id),attempt_id TEXT NOT NULL REFERENCES attempts(id),workspace_id TEXT NOT NULL REFERENCES managed_workspaces(id),approval_id TEXT NOT NULL UNIQUE REFERENCES approval_proposals(id),document TEXT NOT NULL);
CREATE TABLE command_validation_runs(id TEXT PRIMARY KEY,policy_id TEXT NOT NULL REFERENCES command_validation_policies(id),task_id TEXT NOT NULL REFERENCES tasks(id),workspace_id TEXT NOT NULL REFERENCES managed_workspaces(id),state TEXT NOT NULL CHECK(state IN('prepared','running','cancel_requested','command_passed','command_failed','outcome_unknown')),document TEXT NOT NULL,idempotency_key TEXT NOT NULL,payload TEXT NOT NULL,UNIQUE(policy_id,idempotency_key));
CREATE TABLE command_workspace_fences(workspace_id TEXT PRIMARY KEY REFERENCES managed_workspaces(id),run_id TEXT NOT NULL UNIQUE REFERENCES command_validation_runs(id));
CREATE TRIGGER command_fence_attempt BEFORE INSERT ON attempts WHEN EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id=json_extract(NEW.binding,'$.workspace_id')) BEGIN SELECT RAISE(ABORT,'command workspace remains fenced'); END;
CREATE TRIGGER command_fence_lease_insert BEFORE INSERT ON workspace_leases WHEN EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id=NEW.resource) BEGIN SELECT RAISE(ABORT,'command workspace remains fenced'); END;
CREATE TRIGGER command_fence_lease_update BEFORE UPDATE ON workspace_leases WHEN EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id=NEW.resource) BEGIN SELECT RAISE(ABORT,'command workspace remains fenced'); END;
PRAGMA user_version=8;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}
