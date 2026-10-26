use crate::{CoreError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Draft,
    Running,
    CancelRequested,
    Cancelled,
    OutcomeUnknown,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub goal: String,
    pub state: TaskState,
    pub revision: u64,
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
    // Numeric diagnostics only: never expose SQL, paths or bound values.
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
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let mut conn = Connection::open(path).map_err(db_error)?;
        conn.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(db_error)?;
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_error)?;
        if version > 2 {
            return Err(CoreError::new(
                "VERSION_UNSUPPORTED",
                "Database was created by a newer application",
            ));
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
        crate::execution::migrate(&mut conn, path, version)?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )
        .map_err(db_error)?;
        let mut store = Self { conn };
        store.recover_dispatches()?;
        Ok(store)
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
            state: TaskState::Draft,
            revision: 1,
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
        decode(&raw.ok_or_else(|| CoreError::new("NOT_FOUND", "Task does not exist"))?)
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
        rows.map(|r| decode(&r.map_err(db_error)?)).collect()
    }
    pub fn cancel(&mut self, id: &str, expected_revision: u64) -> Result<Task> {
        let mut task = self.get(id)?;
        if task.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Task changed; refresh before cancelling",
            ));
        }
        if task.state == TaskState::Cancelled || task.state == TaskState::CancelRequested {
            return Ok(task);
        }
        let active:i64=self.conn.query_row("SELECT count(*) FROM attempts a JOIN dispatch_intents d ON d.attempt_id=a.id WHERE a.task_id=?1 AND d.state!='stopped'",[id],|r|r.get(0)).map_err(db_error)?;
        let no_attempt = task.state == TaskState::Draft && active == 0;
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
                "SELECT sequence,document FROM events WHERE sequence>?1 ORDER BY sequence LIMIT ?2",
            )
            .map_err(db_error)?;
        let rows = stmt
            .query_map(params![cursor as i64, limit], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?))
            })
            .map_err(db_error)?;
        let mut events = Vec::new();
        for row in rows {
            let (seq, raw) = row.map_err(db_error)?;
            let mut event: Event = decode(&raw)?;
            event.sequence = seq;
            events.push(event);
        }
        let next_cursor = events.last().map_or(cursor, |e| e.sequence);
        Ok(json!({"events":events,"next_cursor":next_cursor,"sequence":sequence,"gap":false}))
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
