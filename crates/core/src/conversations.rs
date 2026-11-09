//! Taskless, owner-authorized local conversations. No Task, Goal, acceptance or
//! integration authority is created here. Unknown writers retain their fences.
use crate::sessions::{Outcome, Work, WorkerLaunch, WorkerReceipt};
use crate::{
    storage::{db_error, decode, encode, now, Event, Store},
    CoreError, Result,
};
use agentbaton_config::{runtime::RuntimeRequirements, Ownership};
use agentbaton_providers::{
    connections::ProtocolRoute,
    installation::{verify_executable, BinaryDigest},
    transport::ApprovedInstallation,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::os::unix::fs::MetadataExt;
use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conversation {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub workspace_id: String,
    pub connection_id: String,
    pub grant_id: String,
    pub state: String,
    pub revision: u64,
    pub native_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_error: Option<String>,
    pub turn_state: Option<String>,
    pub runtime_snapshot: Option<crate::RuntimeBinding>,
    pub attempt_id: String,
    pub fencing_token: u64,
    pub git_common_device: u64,
    pub git_common_inode: u64,
}
fn error(code: &str, message: &str) -> CoreError {
    CoreError::new(code, message)
}
fn key_valid(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 256 {
        Err(error(
            "INVALID_REQUEST",
            "A bounded idempotency key is required",
        ))
    } else {
        Ok(())
    }
}
fn read(db: &Connection, id: &str) -> Result<Conversation> {
    let raw:Option<(Option<String>,String,String,String,String,String)>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END,project_id,workspace_id,connection_id,grant_id,state FROM conversations WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional().map_err(db_error)?;
    let (raw, p, w, connection, grant, state) =
        raw.ok_or_else(|| error("NOT_FOUND", "Conversation does not exist"))?;
    let c: Conversation = decode(
        &raw.ok_or_else(|| error("RESOURCE_LIMIT", "Conversation record exceeds bounded read"))?,
    )?;
    if c.id != id
        || c.project_id != p
        || c.workspace_id != w
        || c.connection_id != connection
        || c.grant_id != grant
        || c.state != state
    {
        return Err(error("INPUT_CHANGED", "Conversation owner mirrors changed"));
    }
    Ok(c)
}
fn event(db: &Connection, c: &Conversation, kind: &str, payload: Value) -> Result<()> {
    let e = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: c.id.clone(),
        revision: c.revision,
        kind: kind.into(),
        occurred_at: now()?,
        payload,
        source: "conversation_core".into(),
    };
    db.execute("INSERT INTO events(document) VALUES(?1)", [encode(&e)?])
        .map_err(db_error)?;
    Ok(())
}
fn save(db: &Connection, c: &mut Conversation, kind: &str, payload: Value) -> Result<()> {
    c.revision = c
        .revision
        .checked_add(1)
        .ok_or_else(|| error("RESOURCE_LIMIT", "Revision exhausted"))?;
    c.updated_at = now()?;
    db.execute(
        "UPDATE conversations SET document=?1,state=?2 WHERE id=?3",
        params![encode(c)?, c.state, c.id],
    )
    .map_err(db_error)?;
    event(db, c, kind, payload)
}
const SCHEMA:&str="CREATE TABLE conversations(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),workspace_id TEXT NOT NULL REFERENCES managed_workspaces(id),connection_id TEXT NOT NULL REFERENCES connections(id),grant_id TEXT NOT NULL REFERENCES execution_grants(id),root_grant_id TEXT NOT NULL REFERENCES execution_grants(id),state TEXT NOT NULL,document TEXT NOT NULL,idempotency_key TEXT NOT NULL,payload TEXT NOT NULL,UNIQUE(project_id,idempotency_key));
 CREATE TABLE conversation_dispatches(conversation_id TEXT PRIMARY KEY REFERENCES conversations(id),state TEXT NOT NULL CHECK(state IN('prepared','sent','outcome_unknown','stopped')),binding TEXT NOT NULL);
 CREATE TABLE conversation_turns(id TEXT PRIMARY KEY,conversation_id TEXT NOT NULL REFERENCES conversations(id),state TEXT NOT NULL,document TEXT NOT NULL,idempotency_key TEXT NOT NULL,payload TEXT NOT NULL,UNIQUE(conversation_id,idempotency_key));
 CREATE TABLE conversation_budgets(conversation_id TEXT PRIMARY KEY REFERENCES conversations(id),project_id TEXT NOT NULL,root_grant_id TEXT NOT NULL,document TEXT NOT NULL);
 PRAGMA user_version=18;";
pub(crate) fn migrate(db: &mut Connection, version: u32) -> Result<()> {
    if version >= 18 {
        return Ok(());
    }
    let tx = db.transaction().map_err(db_error)?;
    tx.execute_batch(SCHEMA).map_err(db_error)?;
    let(count,bytes):(i64,i64)=tx.query_row("SELECT count(*),COALESCE(sum(octet_length(type)+octet_length(name)+octet_length(tbl_name)+COALESCE(octet_length(sql),0)),0) FROM sqlite_schema",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    if !(1..=256).contains(&count) || !(1..=1048576).contains(&bytes) {
        return Err(error(
            "INPUT_CHANGED",
            "Conversation migration exceeds fixed schema bounds",
        ));
    }
    validate_schema(&tx)?;
    tx.commit().map_err(db_error)
}
/// Shared admission queries use this view of both owner kinds, never invent Task rows.
pub(crate) const OWNERS:&str="(SELECT grant_id,root_grant_id,project_id,state FROM sessions UNION ALL SELECT grant_id,root_grant_id,project_id,state FROM conversations)";
impl Store {
    pub fn conversation_create_replay(&self, project_id: &str, key: &str) -> Result<bool> {
        self.conn.query_row("SELECT EXISTS(SELECT 1 FROM conversations WHERE project_id=?1 AND idempotency_key=?2)",params![project_id,key],|r|r.get(0)).map_err(db_error)
    }
    pub fn get_conversation(&self, id: &str) -> Result<Conversation> {
        read(&self.conn, id)
    }
    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let bytes:i64=self.conn.query_row("SELECT COALESCE(sum(octet_length(document)),0) FROM (SELECT document FROM conversations ORDER BY rowid DESC LIMIT 1001)",[],|r|r.get(0)).map_err(db_error)?;
        if bytes > 4 * 1024 * 1024 {
            return Err(error(
                "RESOURCE_LIMIT",
                "Conversation list exceeds bounded response",
            ));
        }
        let mut q = self
            .conn
            .prepare("SELECT id FROM conversations ORDER BY rowid DESC LIMIT 1001")
            .map_err(db_error)?;
        let raw = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        if raw.len() > 1000 {
            return Err(error(
                "RESOURCE_LIMIT",
                "Conversation list exceeds bounded page",
            ));
        }
        raw.iter().map(|id| read(&self.conn, id)).collect()
    }
    pub fn create_conversation(
        &mut self,
        project_id: &str,
        title: &str,
        workspace_id: &str,
        connection_id: &str,
        grant_id: &str,
        key: &str,
    ) -> Result<Conversation> {
        key_valid(key)?;
        if title.trim().is_empty() || title.len() > 512 {
            return Err(error(
                "INVALID_REQUEST",
                "Conversation title must be nonempty and at most 512 bytes",
            ));
        }
        let payload = encode(
            &json!({"project_id":project_id,"title":title,"workspace_id":workspace_id,"connection_id":connection_id,"grant_id":grant_id}),
        )?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let old:Option<(String,String)>=tx.query_row("SELECT payload,document FROM conversations WHERE project_id=?1 AND idempotency_key=?2",params![project_id,key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
        if let Some((p, d)) = old {
            if p != payload {
                return Err(error(
                    "CONFIG_CONFLICT",
                    "Conversation key was used for another intent",
                ));
            }
            return decode(&d);
        }
        let grant = crate::policy::validate_grant_from(
            &tx,
            grant_id,
            project_id,
            workspace_id,
            connection_id,
        )?;
        let raw: String = tx
            .query_row(
                "SELECT document FROM managed_workspaces WHERE id=?1",
                [workspace_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let workspace: crate::registry::ManagedWorkspace = decode(&raw)?;
        if !matches!(
            workspace.identity.as_ref().map(|i| &i.kind),
            Some(agentbaton_workspaces::WorkspaceKind::Git { .. })
        ) {
            return Err(error(
                "CAPABILITY_UNSUPPORTED",
                "Independent conversations currently require a managed Git workspace",
            ));
        }
        let common = match &workspace.identity.as_ref().expect("checked identity").kind {
            agentbaton_workspaces::WorkspaceKind::Git { common_dir, .. } => {
                std::fs::symlink_metadata(common_dir)
                    .map_err(|_| error("INPUT_CHANGED", "Git common directory unavailable"))?
            }
            _ => unreachable!(),
        };
        if !common.is_dir() || common.file_type().is_symlink() {
            return Err(error(
                "INPUT_CHANGED",
                "Git common directory is not a trusted directory",
            ));
        }
        let c: crate::sessions::ProviderConnection = decode(
            &tx.query_row(
                "SELECT document FROM connections WHERE id=?1",
                [connection_id],
                |r| r.get::<_, String>(0),
            )
            .map_err(db_error)?,
        )?;
        verify_executable(
            &c.executable,
            &BinaryDigest::parse(&c.executable_sha256)
                .map_err(|_| error("INPUT_CHANGED", "Connection digest changed"))?,
        )
        .map_err(|_| error("INPUT_CHANGED", "Approved executable changed"))?;
        let fenced: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id=?1)",
                [workspace_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let lease: Option<(String, i64)> = tx
            .query_row(
                "SELECT holder,fencing_token FROM workspace_leases WHERE resource=?1",
                [workspace_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        let active_writer:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE workspace_id=?1 AND state NOT IN('stopped','failed')) OR EXISTS(SELECT 1 FROM conversations WHERE workspace_id=?1 AND state NOT IN('stopped','failed'))",[workspace_id],|r|r.get(0)).map_err(db_error)?;
        if fenced || active_writer || lease.as_ref().is_some_and(|(h, _)| !h.is_empty()) {
            return Err(error(
                "WORKSPACE_BUSY",
                "Workspace has an unreconciled writer or read fence",
            ));
        }
        crate::policy::validate_ancestor_grant_budgets_from(&tx, grant_id)?;
        let active: u32 = tx
            .query_row(
                &format!("SELECT count(*) FROM {OWNERS} WHERE state NOT IN('stopped','failed')"),
                [],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if active >= 4 {
            return Err(error(
                "RESOURCE_LIMIT",
                "Local supervisor concurrency limit reached",
            ));
        }
        let root = crate::policy::grant_from_for_supervisor(&tx, &grant.root_grant_id)?;
        let scope = format!("project:{project_id}");
        tx.execute(
            "INSERT OR IGNORE INTO budgets(scope,total_limit,parallel_limit) VALUES(?1,?2,?3)",
            params![scope, root.scope.max_attempts, root.scope.max_concurrency],
        )
        .map_err(db_error)?;
        if tx.execute("UPDATE budgets SET consumed=consumed+1,active=active+1 WHERE scope=?1 AND consumed<total_limit AND active<parallel_limit",[&scope]).map_err(db_error)?!=1{return Err(error("RESOURCE_LIMIT","Shared project budget exhausted"))}
        let id = Uuid::new_v4().to_string();
        let epoch = lease
            .map_or(0, |(_, e)| e)
            .checked_add(1)
            .ok_or_else(|| error("RESOURCE_LIMIT", "Writer generation exhausted"))?;
        let timestamp = now()?;
        let conversation = Conversation {
            id: id.clone(),
            project_id: project_id.into(),
            title: title.into(),
            workspace_id: workspace_id.into(),
            connection_id: connection_id.into(),
            grant_id: grant_id.into(),
            state: "starting".into(),
            revision: 1,
            native_session_id: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            last_error: None,
            turn_state: None,
            runtime_snapshot: None,
            attempt_id: Uuid::new_v4().to_string(),
            fencing_token: u64::try_from(epoch)
                .map_err(|_| error("INPUT_CHANGED", "Invalid writer generation"))?,
            git_common_device: common.dev(),
            git_common_inode: common.ino(),
        };
        let binding = crate::execution::ExecutionBinding {
            connection_id: connection_id.into(),
            environment_id: c.environment_id,
            identity_ref: c.identity_ref,
            host_id: c.host_id,
            workspace_id: workspace_id.into(),
            config_revision: c.config_revision,
            input_revision: workspace.base_revision,
            capability_snapshot_id: grant.connection_bindings[connection_id]
                .capability_snapshot_id
                .clone(),
            policy_revision: format!("{}:{}", grant.id, grant.revision),
        };
        tx.execute(
            "INSERT INTO conversations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                id,
                project_id,
                workspace_id,
                connection_id,
                grant_id,
                grant.root_grant_id,
                conversation.state,
                encode(&conversation)?,
                key,
                payload
            ],
        )
        .map_err(db_error)?;
        tx.execute(
            "INSERT INTO conversation_dispatches VALUES(?1,'prepared',?2)",
            params![id, encode(&binding)?],
        )
        .map_err(db_error)?;
        tx.execute("INSERT INTO workspace_leases VALUES(?1,?2,?3,0) ON CONFLICT(resource) DO UPDATE SET holder=excluded.holder,fencing_token=excluded.fencing_token,expires_at=0",params![workspace_id,id,epoch]).map_err(db_error)?;
        reserve_budget(&tx, &conversation, &grant)?;
        event(
            &tx,
            &conversation,
            "conversation.launch_reserved",
            json!({"attempt_id":conversation.attempt_id,"binding":binding,"fencing_token":epoch,"task_created":false,"automatic_replay":false}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(conversation)
    }
    pub fn send_conversation(
        &mut self,
        id: &str,
        revision: u64,
        text: &str,
        key: &str,
    ) -> Result<(Conversation, bool)> {
        key_valid(key)?;
        if text.trim().is_empty() || text.len() > 65536 {
            return Err(error(
                "INVALID_REQUEST",
                "Message must be nonempty and at most 64 KiB",
            ));
        }
        let payload = encode(&json!({"text":text,"expected_revision":revision}))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let old:Option<String>=tx.query_row("SELECT payload FROM conversation_turns WHERE conversation_id=?1 AND idempotency_key=?2",params![id,key],|r|r.get(0)).optional().map_err(db_error)?;
        if let Some(p) = old {
            if p != payload {
                return Err(error(
                    "CONFIG_CONFLICT",
                    "Turn key already used for another message",
                ));
            }
            return Ok((read(&tx, id)?, false));
        }
        let mut c = read(&tx, id)?;
        if c.revision != revision {
            return Err(error(
                "STALE_REVISION",
                "Conversation changed; refresh before sending",
            ));
        }
        if c.state != "ready" {
            return Err(error(
                "PERMISSION_DENIED",
                "Conversation is not ready; unknown turns are never replayed",
            ));
        }
        validate_live(&tx, &c)?;
        let turn_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO conversation_turns VALUES(?1,?2,'pending',?3,?4,?5)",
            params![
                turn_id,
                id,
                encode(&json!({"turn_id":turn_id,"text":text}))?,
                key,
                payload
            ],
        )
        .map_err(db_error)?;
        c.state = "running".into();
        c.turn_state = Some("running".into());
        save(
            &tx,
            &mut c,
            "conversation.message_queued",
            json!({"turn_id":turn_id,"text":text}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok((c, true))
    }
    pub fn pending_conversation_turn(&self, id: &str) -> Result<Option<String>> {
        self.conn.query_row("SELECT json_extract(document,'$.text') FROM conversation_turns WHERE conversation_id=?1 AND state='pending'",[id],|r|r.get(0)).optional().map_err(db_error)
    }
    pub fn cancel_conversation(
        &mut self,
        id: &str,
        revision: u64,
        key: &str,
    ) -> Result<Conversation> {
        key_valid(key)?;
        let scope = format!("conversation:{id}:cancel");
        let payload = encode(&json!({"revision":revision}))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let old: Option<String> = tx
            .query_row(
                "SELECT payload FROM idempotency WHERE scope=?1 AND key=?2",
                params![scope, key],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if let Some(p) = old {
            if p != payload {
                return Err(error("CONFIG_CONFLICT", "Cancel key was reused"));
            }
            return read(&tx, id);
        }
        let mut c = read(&tx, id)?;
        if revision == 0 || revision > c.revision {
            return Err(error(
                "STALE_REVISION",
                "Cancel revision is ahead of this conversation",
            ));
        }
        if c.state == "outcome_unknown" {
            return Err(error(
                "OUTCOME_UNKNOWN",
                "Unknown process ownership cannot be cancelled by database state",
            ));
        }
        if c.state != "stopped" && c.state != "failed" {
            c.state = "cancel_requested".into();
            save(&tx, &mut c, "conversation.cancel_requested", json!({}))?;
        }
        tx.execute(
            "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
            params![scope, key, payload, encode(&c)?],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(c)
    }
    pub fn conversation_failure(&mut self, id: &str, reason: &str) -> Result<()> {
        let tx = self.conn.transaction().map_err(db_error)?;
        let mut c = read(&tx, id)?;
        if c.state == "stopped" || c.state == "failed" {
            return Ok(());
        }
        c.state = "outcome_unknown".into();
        c.last_error = Some(reason.into());
        c.turn_state = Some("outcome_unknown".into());
        tx.execute("UPDATE conversation_dispatches SET state='outcome_unknown' WHERE conversation_id=?1 AND state!='stopped'",[id]).map_err(db_error)?;
        tx.execute("UPDATE conversation_turns SET state='outcome_unknown' WHERE conversation_id=?1 AND state='pending'",[id]).map_err(db_error)?;
        save(
            &tx,
            &mut c,
            "conversation.outcome_unknown",
            json!({"reason":reason,"automatic_replay":false}),
        )?;
        tx.commit().map_err(db_error)
    }
    pub(crate) fn recover_conversations(&mut self) -> Result<()> {
        let ids = {
            let mut q=self.conn.prepare("SELECT id FROM conversations WHERE state NOT IN('stopped','failed','outcome_unknown')").map_err(db_error)?;
            let ids = q
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(db_error)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_error)?;
            ids
        };
        for id in ids {
            self.conversation_failure(&id, "daemon_restart_requires_reconciliation")?
        }
        Ok(())
    }
}
fn validate_live(db: &Connection, c: &Conversation) -> Result<crate::policy::ExecutionGrant> {
    let raw: String = db
        .query_row(
            "SELECT document FROM managed_workspaces WHERE id=?1",
            [&c.workspace_id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    let workspace: crate::registry::ManagedWorkspace = decode(&raw)?;
    let pin = workspace
        .identity
        .as_ref()
        .ok_or_else(|| error("INPUT_CHANGED", "Workspace identity unavailable"))?;
    let agentbaton_workspaces::WorkspaceKind::Git { common_dir, .. } = &pin.kind else {
        return Err(error(
            "INPUT_CHANGED",
            "Conversation workspace kind changed",
        ));
    };
    let common = std::fs::symlink_metadata(common_dir)
        .map_err(|_| error("INPUT_CHANGED", "Git common directory unavailable"))?;
    if !common.is_dir()
        || common.file_type().is_symlink()
        || common.dev() != c.git_common_device
        || common.ino() != c.git_common_inode
    {
        return Err(error("INPUT_CHANGED", "Physical Git repository changed"));
    }
    let grant = crate::policy::validate_grant_live_from_with_data(
        db,
        &c.grant_id,
        &c.project_id,
        &c.workspace_id,
        &c.connection_id,
        None,
    )?;
    let lease: Option<(String, i64)> = db
        .query_row(
            "SELECT holder,fencing_token FROM workspace_leases WHERE resource=?1",
            [&c.workspace_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(db_error)?;
    if lease
        != Some((
            c.id.clone(),
            i64::try_from(c.fencing_token)
                .map_err(|_| error("INPUT_CHANGED", "Invalid writer generation"))?,
        ))
    {
        return Err(error("WORKSPACE_BUSY", "Conversation writer fence changed"));
    }
    Ok(grant)
}

struct ConversationWorker {
    commands: SyncSender<Work>,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<WorkerReceipt>,
}
pub struct ConversationSupervisor {
    runtime: PathBuf,
    workers: HashMap<String, ConversationWorker>,
    receipts: HashMap<String, WorkerReceipt>,
    no_start: HashMap<String, Instant>,
    events: Receiver<Outcome>,
    sender: SyncSender<Outcome>,
}
impl ConversationSupervisor {
    pub fn new(runtime: &Path) -> Self {
        let (sender, events) = mpsc::sync_channel(64);
        Self {
            runtime: runtime.into(),
            workers: HashMap::new(),
            receipts: HashMap::new(),
            no_start: HashMap::new(),
            events,
            sender,
        }
    }
    pub fn owns(&self, id: &str) -> bool {
        self.workers.contains_key(id) || self.receipts.contains_key(id)
    }
    pub fn has_workers(&self) -> bool {
        !self.workers.is_empty() || !self.receipts.is_empty()
    }
    pub fn cancel(&self, id: &str) {
        if let Some(w) = self.workers.get(id) {
            w.cancel.store(true, Ordering::Relaxed)
        }
    }
    pub fn send(&mut self, id: &str, text: &str) -> Result<()> {
        self.workers
            .get(id)
            .ok_or_else(|| error("OUTCOME_UNKNOWN", "No owned conversation worker exists"))?
            .commands
            .try_send(Work::Prompt(text.into()))
            .map_err(|_| {
                error(
                    "OUTCOME_UNKNOWN",
                    "Conversation queue unavailable; do not resend",
                )
            })
    }
    pub fn launch(&mut self, store: &mut Store, id: &str) -> Result<()> {
        if self.owns(id) {
            return Ok(());
        }
        let started = Instant::now();
        let mut c = store.get_conversation(id)?;
        if c.state != "starting" {
            return Ok(());
        }
        let state: String = store
            .conn
            .query_row(
                "SELECT state FROM conversation_dispatches WHERE conversation_id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if state != "prepared" {
            return Err(error(
                "OUTCOME_UNKNOWN",
                "Conversation launch already dispatched; never replay",
            ));
        }
        self.no_start.insert(id.into(), started);
        let grant = validate_live(&store.conn, &c)?;
        let connection = store.get_connection(&c.connection_id)?;
        let plan = crate::runtime_config::approved_plan(&connection)?;
        let workspace = store
            .list_workspaces(&c.project_id)?
            .into_iter()
            .find(|w| w.id == c.workspace_id)
            .ok_or_else(|| error("INPUT_CHANGED", "Managed workspace is unavailable"))?;
        let root = crate::sessions::Supervisor::new(&self.runtime).configuration_root()?;
        let snapshot = if let Some(b) = &c.runtime_snapshot {
            if b.session_id != c.id
                || b.attempt_id != c.attempt_id
                || b.connection_config_revision != connection.config_revision
            {
                return Err(error(
                    "INPUT_CHANGED",
                    "Conversation runtime binding changed",
                ));
            }
            root.verify_before_launch(&b.snapshot, RuntimeRequirements::default())
                .map_err(crate::runtime_config::config_error)?
        } else {
            let snapshot = root
                .materialize(plan.clone(), Ownership::BatonManaged)
                .map_err(crate::runtime_config::config_error)?;
            c.runtime_snapshot = Some(crate::RuntimeBinding {
                session_id: c.id.clone(),
                attempt_id: c.attempt_id.clone(),
                connection_config_revision: connection.config_revision.clone(),
                plan_digest: snapshot.plan_digest().into(),
                snapshot: snapshot.descriptor(),
            });
            let tx = store.conn.transaction().map_err(db_error)?;
            save(
                &tx,
                &mut c,
                "conversation.runtime_bound",
                json!({"plan_digest":snapshot.plan_digest()}),
            )?;
            tx.commit().map_err(db_error)?;
            snapshot
        };
        if snapshot.plan_digest()
            != plan
                .plan_digest()
                .map_err(crate::runtime_config::config_error)?
        {
            return Err(error("INPUT_CHANGED", "Runtime differs from approved plan"));
        }
        let snapshot = root
            .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
            .map_err(crate::runtime_config::config_error)?;
        let configuration = Some(
            if connection.protocol_route == ProtocolRoute::CodexAppServer {
                crate::runtime_config::codex_snapshot_configuration(&snapshot)?
            } else {
                snapshot
                    .acp_configuration()
                    .map_err(crate::runtime_config::config_error)?
            },
        );
        let arguments: Vec<OsString> = match connection.protocol_route {
            ProtocolRoute::CustomAcp => connection.arguments.iter().map(OsString::from).collect(),
            ProtocolRoute::CodexAppServer => ["app-server", "--listen", "stdio://"]
                .into_iter()
                .map(OsString::from)
                .collect(),
            ProtocolRoute::QoderNativeAcp => vec![
                "--acp".into(),
                "--config-dir".into(),
                snapshot.provider_configuration_root().into_os_string(),
            ],
            ProtocolRoute::ClaudeAgentAcp => vec![],
            ProtocolRoute::CodexAcpAdapter => {
                return Err(error("CAPABILITY_UNSUPPORTED", "Unapproved adapter route"))
            }
        };
        verify_executable(
            &connection.executable,
            &BinaryDigest::parse(&connection.executable_sha256)
                .map_err(|_| error("INPUT_CHANGED", "Connection digest changed"))?,
        )
        .map_err(|_| error("INPUT_CHANGED", "Approved executable changed before launch"))?;
        let installation = ApprovedInstallation {
            executable: connection.executable,
            expected_sha256: BinaryDigest::parse(&connection.executable_sha256)
                .map_err(|_| error("INPUT_CHANGED", "Connection digest changed"))?,
            arguments,
            workspace: workspace.root,
            approval_reference: grant.approval_id,
            environment: snapshot.environment(),
        };
        let duration = conversation_duration(&store.conn, id)?.min(grant.scope.max_duration_ms);
        let prelaunch_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let remaining = duration.saturating_sub(prelaunch_ms);
        if remaining == 0 {
            return Err(error(
                "RESOURCE_LIMIT",
                "Configuration preparation exhausted runtime budget",
            ));
        }
        validate_live(&store.conn, &c)?;
        if store.conn.execute("UPDATE conversation_dispatches SET state='sent' WHERE conversation_id=?1 AND state='prepared'",[id]).map_err(db_error)?!=1{return Err(error("OUTCOME_UNKNOWN","Conversation launch intent changed"))}
        let (commands, receiver) = mpsc::sync_channel(8);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let sender = self.sender.clone();
        let worker_id = id.to_owned();
        let route = connection.protocol_route;
        let thread = std::thread::Builder::new()
            .name("baton-conversation".into())
            .spawn(move || {
                crate::sessions::run_worker(
                    worker_id,
                    WorkerLaunch::conversation(
                        route,
                        installation,
                        configuration,
                        prelaunch_ms,
                        Duration::from_millis(remaining),
                    ),
                    receiver,
                    worker_cancel,
                    sender,
                )
            })
            .map_err(|_| error("RESOURCE_LIMIT", "Cannot start owned conversation worker"))?;
        self.workers.insert(
            id.into(),
            ConversationWorker {
                commands,
                cancel,
                thread,
            },
        );
        self.no_start.remove(id);
        Ok(())
    }
    pub fn record_local_launch_failure(&mut self, store: &mut Store, id: &str) -> Result<()> {
        if self.owns(id) || !self.no_start.contains_key(id) {
            return Err(error(
                "OUTCOME_UNKNOWN",
                "No local never-started proof exists",
            ));
        }
        let elapsed_ms = self
            .no_start
            .remove(id)
            .map(|clock| u64::try_from(clock.elapsed().as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        self.receipts
            .insert(id.into(), WorkerReceipt::conversation_no_start(elapsed_ms));
        self.flush(store)
    }
    fn collect_finished(&mut self) {
        let ids: Vec<_> = self
            .workers
            .iter()
            .filter(|(_, w)| w.thread.is_finished())
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if let Some(w) = self.workers.remove(&id) {
                let receipt = w
                    .thread
                    .join()
                    .unwrap_or(WorkerReceipt::conversation_panicked());
                self.receipts.insert(id, receipt);
            }
        }
    }
    fn flush(&mut self, store: &mut Store) -> Result<()> {
        let ids: Vec<_> = self.receipts.keys().cloned().collect();
        for id in ids {
            settle(store, &id, &self.receipts[&id])?;
            self.receipts.remove(&id);
        }
        Ok(())
    }
    pub fn tick(&mut self, store: &mut Store) -> Result<()> {
        for (id, w) in &self.workers {
            let c = store.get_conversation(id)?;
            if c.state == "cancel_requested"
                || c.state == "outcome_unknown"
                || validate_live(&store.conn, &c).is_err()
            {
                w.cancel.store(true, Ordering::Relaxed)
            }
        }
        for _ in 0..64 {
            let outcome = match self.events.try_recv() {
                Ok(o) => o,
                Err(_) => break,
            };
            match outcome {
                Outcome::Ready(id, native) => {
                    let mut c = store.get_conversation(&id)?;
                    if c.state == "starting" {
                        c.state = "ready".into()
                    }
                    c.native_session_id = Some(native);
                    let tx = store.conn.transaction().map_err(db_error)?;
                    save(
                        &tx,
                        &mut c,
                        "conversation.ready",
                        json!({"identity_status":"unknown","host_tools":false,"history_capture":{"normalized_updates":true,"raw_protocol":false,"completeness":"partial"}}),
                    )?;
                    tx.commit().map_err(db_error)?;
                }
                Outcome::Update(id, update) => {
                    let mut c = store.get_conversation(&id)?;
                    let tx = store.conn.transaction().map_err(db_error)?;
                    save(&tx, &mut c, "conversation.update", json!({"update":update}))?;
                    tx.commit().map_err(db_error)?;
                }
                Outcome::Turn(id, state) => {
                    let tx = store.conn.transaction().map_err(db_error)?;
                    let mut c = read(&tx, &id)?;
                    let turn_id:Option<String>=tx.query_row("SELECT id FROM conversation_turns WHERE conversation_id=?1 AND state='pending'",[&id],|r|r.get(0)).optional().map_err(db_error)?;
                    if let Some(turn_id) = turn_id {
                        tx.execute(
                            "UPDATE conversation_turns SET state=?1 WHERE id=?2",
                            params![state, turn_id],
                        )
                        .map_err(db_error)?;
                        if c.state == "running" {
                            c.state = if state == "completed" {
                                "ready"
                            } else {
                                "outcome_unknown"
                            }
                            .into()
                        }
                        c.turn_state = Some(state.clone());
                        save(
                            &tx,
                            &mut c,
                            "conversation.turn_completed",
                            json!({"turn_id":turn_id,"state":state}),
                        )?;
                    }
                    tx.commit().map_err(db_error)?;
                }
                Outcome::Failure(id, reason) => {
                    store.conversation_failure(&id, &reason)?;
                    self.cancel(&id)
                }
                Outcome::GoalTurn { .. } => {
                    return Err(error(
                        "INTERNAL_ERROR",
                        "Unexpected task authority on conversation channel",
                    ))
                }
            }
        }
        self.collect_finished();
        self.flush(store)
    }
    pub fn reap_owned_workers(&mut self) {
        for w in self.workers.values() {
            w.cancel.store(true, Ordering::Relaxed)
        }
        for (id, w) in std::mem::take(&mut self.workers) {
            let receipt = w
                .thread
                .join()
                .unwrap_or(WorkerReceipt::conversation_panicked());
            self.receipts.insert(id, receipt);
        }
    }
    pub fn shutdown(&mut self, store: &mut Store) -> Result<()> {
        for w in self.workers.values() {
            w.cancel.store(true, Ordering::Relaxed)
        }
        let workers = std::mem::take(&mut self.workers);
        for (id, w) in workers {
            let receipt = w
                .thread
                .join()
                .unwrap_or(WorkerReceipt::conversation_panicked());
            self.receipts.insert(id, receipt);
        }
        self.tick(store)
    }
}
impl Drop for ConversationSupervisor {
    fn drop(&mut self) {
        for w in self.workers.values() {
            w.cancel.store(true, Ordering::Relaxed)
        }
        for (_, w) in self.workers.drain() {
            let _ = w.thread.join();
        }
    }
}
fn settle(store: &mut Store, id: &str, r: &WorkerReceipt) -> Result<()> {
    if !r.process_reaped && !r.never_started {
        return store.conversation_failure(id, "owned_process_stop_unconfirmed");
    }
    let tx = store
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    let mut c = read(&tx, id)?;
    let state: String = tx
        .query_row(
            "SELECT state FROM conversation_dispatches WHERE conversation_id=?1",
            [id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    if state == "stopped" {
        return Ok(());
    }
    tx.execute("UPDATE conversation_turns SET state='outcome_unknown' WHERE conversation_id=?1 AND state='pending'",[id]).map_err(db_error)?;
    let unknown:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id=?1 AND state='outcome_unknown')",[id],|r|r.get(0)).map_err(db_error)?;
    c.state = if r.never_started { "failed" } else { "stopped" }.into();
    if unknown {
        c.turn_state = Some("outcome_unknown".into())
    }
    c.last_error = if r.reason == "cancelled" {
        None
    } else {
        Some(r.reason.clone())
    };
    tx.execute(
        "UPDATE conversation_dispatches SET state='stopped' WHERE conversation_id=?1",
        [id],
    )
    .map_err(db_error)?;
    tx.execute("UPDATE workspace_leases SET holder='' WHERE resource=?1 AND holder=?2 AND fencing_token=?3",params![c.workspace_id,id,i64::try_from(c.fencing_token).map_err(|_|error("INPUT_CHANGED","Invalid writer generation"))?]).map_err(db_error)?;
    tx.execute(
        "UPDATE budgets SET active=max(0,active-1) WHERE scope=?1",
        [format!("project:{}", c.project_id)],
    )
    .map_err(db_error)?;
    settle_budget(&tx, &c, r.elapsed_ms)?;
    save(
        &tx,
        &mut c,
        "conversation.stopped",
        json!({"never_started":r.never_started,"process_reaped":r.process_reaped,"reason":r.reason,"elapsed_ms":r.elapsed_ms,"task_accepted":false,"turn_outcome_unknown":unknown}),
    )?;
    tx.commit().map_err(db_error)
}

/// Exact new table definitions; rejecting missing/altered constraints is safer
/// than allowing a readonly snapshot to borrow the current version number.
pub(crate) fn validate_schema(db: &Connection) -> Result<()> {
    for statement in SCHEMA
        .split(';')
        .map(str::trim)
        .filter(|s| s.starts_with("CREATE TABLE"))
    {
        let name = statement
            .trim_start_matches("CREATE TABLE ")
            .split('(')
            .next()
            .unwrap_or("");
        let actual: Option<String> = db
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if actual.as_deref().map(str::trim) != Some(statement) {
            return Err(error(
                "INPUT_CHANGED",
                "Conversation schema constraints changed",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConversationBudget {
    pub conversation_id: String,
    pub attempt_id: String,
    pub project_id: String,
    pub root_grant_id: String,
    pub reservation: crate::goal_plan::AttemptReservation,
    pub metered: bool,
    pub settled: bool,
    pub actual_runtime_ms: Option<u64>,
    pub actual_output_bytes: Option<u64>,
}
fn reserve_budget(
    db: &Connection,
    c: &Conversation,
    grant: &crate::policy::ExecutionGrant,
) -> Result<()> {
    use crate::goal_plan::{AttemptReservation, BudgetLimits};
    let ledger: Option<String> = db
        .query_row(
            "SELECT document FROM goal_ledgers WHERE scope=?1",
            [format!("project:{}", c.project_id)],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let mut duration = grant.scope.max_duration_ms;
    let mut limits = None;
    if let Some(raw) = ledger {
        let value: Value = decode(&raw)?;
        let project: BudgetLimits = serde_json::from_value(value["limits"].clone())
            .map_err(|_| error("BUDGET_UNKNOWN", "Shared project ledger is invalid"))?;
        let root_row: Option<String> = db
            .query_row(
                "SELECT document FROM goal_ledgers WHERE scope=?1",
                [format!("root-grant:{}", grant.root_grant_id)],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?;
        let root = if let Some(raw) = root_row {
            let v: Value = decode(&raw)?;
            serde_json::from_value::<BudgetLimits>(v["limits"].clone())
                .map_err(|_| error("BUDGET_UNKNOWN", "Shared root ledger is invalid"))?
        } else {
            BudgetLimits {
                attempts: grant.scope.max_attempts.min(project.attempts),
                parallel: grant.scope.max_concurrency.min(project.parallel),
                runtime_ms: grant
                    .scope
                    .max_duration_ms
                    .saturating_mul(u64::from(grant.scope.max_attempts))
                    .min(project.runtime_ms),
                output_bytes: project.output_bytes,
                children: project.children,
                reworks: 0,
                tokens: None,
                cost_microunits: None,
            }
        };
        if project.tokens.is_some()
            || project.cost_microunits.is_some()
            || root.tokens.is_some()
            || root.cost_microunits.is_some()
        {
            return Err(error(
                "BUDGET_UNKNOWN",
                "Conversation Provider has no reliable token or cost meter",
            ));
        }
        duration = duration.min(project.runtime_ms).min(root.runtime_ms).min(
            Store::remaining_shared_runtime(db, &grant.root_grant_id, &c.project_id)?
                .unwrap_or(u64::MAX),
        );
        limits = Some((root, project));
    }
    if duration == 0 {
        return Err(error("RESOURCE_LIMIT", "Shared runtime budget exhausted"));
    }
    let record = ConversationBudget {
        conversation_id: c.id.clone(),
        attempt_id: c.attempt_id.clone(),
        project_id: c.project_id.clone(),
        root_grant_id: grant.root_grant_id.clone(),
        reservation: AttemptReservation {
            runtime_ms: duration,
            output_bytes: 0,
            tokens: None,
            cost_microunits: None,
        },
        metered: limits.is_some(),
        settled: false,
        actual_runtime_ms: None,
        actual_output_bytes: None,
    };
    if let Some((root, project)) = limits {
        crate::goal_store::reserve_conversation_meter(db, &record, &root, &project)?;
    }
    db.execute(
        "INSERT INTO conversation_budgets VALUES(?1,?2,?3,?4)",
        params![c.id, c.project_id, grant.root_grant_id, encode(&record)?],
    )
    .map_err(db_error)?;
    Ok(())
}
fn conversation_duration(db: &Connection, id: &str) -> Result<u64> {
    let raw: String = db
        .query_row(
            "SELECT document FROM conversation_budgets WHERE conversation_id=?1",
            [id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    let record: ConversationBudget = decode(&raw)?;
    Ok(record.reservation.runtime_ms)
}
fn settle_budget(db: &Connection, c: &Conversation, elapsed: u64) -> Result<()> {
    let raw: String = db
        .query_row(
            "SELECT document FROM conversation_budgets WHERE conversation_id=?1",
            [&c.id],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    let mut record: ConversationBudget = decode(&raw)?;
    if record.settled {
        return Ok(());
    }
    record.settled = true;
    record.actual_runtime_ms = Some(elapsed);
    record.actual_output_bytes = Some(0);
    if record.metered {
        crate::goal_store::settle_conversation_meter(
            db,
            &record.root_grant_id,
            &record.project_id,
            Some(elapsed),
            Some(0),
        )?;
    }
    db.execute(
        "UPDATE conversation_budgets SET document=?1 WHERE conversation_id=?2",
        params![encode(&record)?, c.id],
    )
    .map_err(db_error)?;
    Ok(())
}
pub(crate) fn budget_records(
    db: &Connection,
    root: &str,
    project: &str,
) -> Result<Vec<ConversationBudget>> {
    let(count,bytes):(i64,i64)=db.query_row("SELECT count(*),COALESCE(sum(octet_length(b.document)+COALESCE(octet_length(c.document),0)),0) FROM conversation_budgets b LEFT JOIN conversations c ON c.id=b.conversation_id WHERE b.project_id=?1 OR b.root_grant_id=?2",params![project,root],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    if !(0..=100000).contains(&count) || !(0..=1048576).contains(&bytes) {
        return Err(error(
            "RESOURCE_LIMIT",
            "Conversation accounting exceeds bounded read",
        ));
    }
    let mut q=db.prepare("SELECT b.document,c.document,c.root_grant_id,b.project_id,b.root_grant_id,b.conversation_id,d.state FROM conversation_budgets b JOIN conversations c ON c.id=b.conversation_id JOIN conversation_dispatches d ON d.conversation_id=c.id WHERE b.project_id=?1 OR b.root_grant_id=?2").map_err(db_error)?;
    let rows = q
        .query_map(params![project, root], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    if rows.len() as i64 != count {
        return Err(error("BUDGET_UNKNOWN", "Conversation budget owner missing"));
    }
    let mut out = vec![];
    for (raw, _owner, root, p, br, id, state) in rows {
        let r: ConversationBudget = decode(&raw)?;
        let c = read(db, &id)?;
        if r.conversation_id != id
            || c.id != id
            || r.attempt_id != c.attempt_id
            || r.project_id != c.project_id
            || r.project_id != p
            || r.root_grant_id != root
            || r.root_grant_id != br
            || r.settled != (state == "stopped")
            || r.reservation.output_bytes != 0
            || r.reservation.tokens.is_some()
            || r.reservation.cost_microunits.is_some()
        {
            return Err(error(
                "BUDGET_UNKNOWN",
                "Conversation accounting origin mismatch",
            ));
        }
        out.push(r)
    }
    Ok(out)
}

/// Historical writer provenance survives process stop and lease release. A later
/// conversation cannot lend its writes to a pre-existing Goal candidate.
pub(crate) fn has_writer_after(db: &Connection, workspace_id: &str, epoch: i64) -> Result<bool> {
    db.query_row("SELECT EXISTS(SELECT 1 FROM conversations WHERE workspace_id=?1 AND json_extract(document,'$.fencing_token')>?2)",params![workspace_id,epoch],|r|r.get(0)).map_err(db_error)
}
