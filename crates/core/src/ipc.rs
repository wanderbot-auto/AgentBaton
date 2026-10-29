//! Versioned, authenticated local IPC. No TCP listener, shell, SQL, or arbitrary file API.
use crate::{sessions::Supervisor, storage::Store, CoreError, Result};
use fs2::FileExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    time::{Duration, Instant},
};
use uuid::Uuid;
#[cfg(debug_assertions)]
#[path = "goal_runtime_diagnostics.rs"]
pub(crate) mod goal_diagnostics;

pub const API_VERSION: u32 = 1;
const MAX_FRAME: u64 = 1_048_576;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    api_version: u32,
    request_id: String,
    token: String,
    method: String,
    #[serde(default)]
    params: Value,
    expected_revision: Option<u64>,
    idempotency_key: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalScopeId {
    scope_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    goal: String,
    #[serde(default = "default_project")]
    project_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportProject {
    path: String,
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataPreviewId {
    preview_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataCreationId {
    creation_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectId {
    project_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReassociateProject {
    project_id: String,
    path: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateWorkspace {
    project_id: String,
    expected_head: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterConnection {
    name: String,
    provider: String,
    executable: String,
    #[serde(default)]
    arguments: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateGrant {
    project_id: String,
    workspace_id: String,
    connection_id: String,
    max_attempts: u32,
    max_concurrency: u32,
    max_duration_ms: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantId {
    grant_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalId {
    proposal_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidationTaskId {
    task_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidationPolicyId {
    policy_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSession {
    task_id: String,
    workspace_id: String,
    connection_id: String,
    grant_id: String,
    #[serde(default)]
    require_enforcement: Option<RequiredEnforcement>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequiredEnforcement {
    #[serde(default)]
    filesystem: bool,
    #[serde(default)]
    network: bool,
    #[serde(default)]
    identity: bool,
    #[serde(default)]
    configuration: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionId {
    session_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SendSession {
    session_id: String,
    text: String,
}
fn default_project() -> String {
    "local".into()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskId {
    task_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Events {
    #[serde(default)]
    cursor: u64,
    #[serde(default = "default_limit")]
    limit: u32,
}
fn default_limit() -> u32 {
    100
}
fn invalid() -> CoreError {
    CoreError::new("INVALID_REQUEST", "Malformed request or unknown fields")
}
fn params<T: for<'de> Deserialize<'de>>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|_| invalid())
}
fn empty(v: &Value) -> Result<()> {
    if v.as_object().is_some_and(|o| o.is_empty()) || v.is_null() {
        Ok(())
    } else {
        Err(invalid())
    }
}

pub fn default_runtime_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("BATON_RUNTIME_DIR") {
        return Ok(path.into());
    }
    let home = std::env::var_os("HOME").ok_or_else(|| {
        CoreError::new("INVALID_REQUEST", "HOME or BATON_RUNTIME_DIR is required")
    })?;
    #[cfg(target_os = "macos")]
    {
        Ok(PathBuf::from(home).join("Library/Application Support/dev.agentbaton/runtime"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(home).join(".local/share"));
        Ok(data.join("agentbaton/runtime"))
    }
}
fn no_symlinks(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(CoreError::new(
            "PERMISSION_DENIED",
            "Runtime paths must be absolute and contain no parent traversal",
        ));
    }
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component);
        match fs::symlink_metadata(&prefix) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(CoreError::new(
                    "PERMISSION_DENIED",
                    "Runtime paths must not contain symbolic links",
                ))
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(CoreError::new(
                    "PERMISSION_DENIED",
                    "Runtime path is inaccessible",
                ))
            }
            _ => {}
        }
    }
    Ok(())
}
fn check_private(path: &Path, directory: bool) -> Result<()> {
    no_symlinks(path)?;
    let m = fs::symlink_metadata(path).map_err(|_| {
        CoreError::new(
            "PERMISSION_DENIED",
            "Runtime state is missing or inaccessible",
        )
    })?;
    if m.uid() != rustix::process::geteuid().as_raw()
        || m.mode() & 0o077 != 0
        || (directory && !m.is_dir())
        || (!directory && !m.is_file())
    {
        return Err(CoreError::new("PERMISSION_DENIED","Runtime state must be owned by this user and private; existing permissions were not changed"));
    }
    Ok(())
}
fn ensure_runtime(path: &Path) -> Result<()> {
    no_symlinks(path)?;
    if !path.exists() {
        let parent = path.parent().ok_or_else(invalid)?;
        fs::create_dir_all(parent)
            .map_err(|_| CoreError::new("PERMISSION_DENIED", "Cannot create runtime parent"))?;
        let mut builder = fs::DirBuilder::new();
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700).create(path).map_err(|_| {
            CoreError::new(
                "PERMISSION_DENIED",
                "Cannot create private runtime directory",
            )
        })?;
    }
    check_private(path, true)
}
fn private_file(path: &Path) -> Result<File> {
    no_symlinks(path)?;
    if path.exists() {
        check_private(path, false)?;
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
        .map_err(|_| CoreError::new("PERMISSION_DENIED", "Cannot open private runtime state"))
}
pub(crate) struct ReadyRequest {
    pub(crate) raw: String,
    pub(crate) reply: SyncSender<Value>,
    pub(crate) deadline: Instant,
    pub(crate) factory_dispatch: Option<crate::factory::FactoryDispatchTicket>,
}
fn configure_accepted_stream(stream: &UnixStream) -> std::io::Result<()> {
    // BSD/Darwin can inherit O_NONBLOCK from the listener. Deadline-based frame
    // IO requires a blocking accepted descriptor; do not depend on accept flags.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))
}
fn socket_worker(mut stream: UnixStream, requests: SyncSender<ReadyRequest>) {
    if configure_accepted_stream(&stream).is_err() {
        return; // Configuration failure closes only this client worker.
    }
    let response = match read_frame(&stream, MAX_FRAME) {
        Ok(raw) => {
            let (reply, receive): (_, Receiver<Value>) = mpsc::sync_channel(1);
            let job = ReadyRequest {
                raw,
                reply,
                deadline: Instant::now() + Duration::from_secs(60),
                factory_dispatch: None,
            };
            if requests.try_send(job).is_err() {
                json!({"request_id":null,"error":CoreError::new("RESOURCE_LIMIT","Local request queue is unavailable"),"revision":0,"warnings":[]})
            } else {
                receive.recv_timeout(Duration::from_secs(60)).unwrap_or_else(|_|json!({"request_id":null,"error":CoreError::transport(),"revision":0,"warnings":[]}))
            }
        }
        Err(error) => json!({"request_id":null,"error":error,"revision":0,"warnings":[]}),
    };
    if let Ok(mut output) = serde_json::to_vec(&response) {
        if output.len() as u64 > 32 * MAX_FRAME {
            output=serde_json::to_vec(&json!({"request_id":response["request_id"],"error":CoreError::new("RESOURCE_LIMIT","Response exceeds local IPC limit; request a smaller batch"),"revision":0,"warnings":[]})).unwrap_or_default();
        }
        output.push(b'\n');
        let _ = stream.write_all(&output);
    }
}
pub struct Daemon {
    listener: UnixListener,
    store: Store,
    token: String,
    _lock: File,
    socket: PathBuf,
    managed_root: PathBuf,
    supervisor: Supervisor,
    command_workers: crate::command_workers::CommandWorkers,
    data_workers: crate::data_workspace_workers::DataWorkspaceWorkers,
    goal_pending: Option<(String, Receiver<Value>)>,
    factory_queue_clocks: std::collections::HashMap<String, Instant>,
    factory_pending: Option<(String, Receiver<Value>)>,
    factory_fatal_error: Option<CoreError>,
    graph_workers: crate::goal_store::GraphWorkers,
}
impl Daemon {
    pub fn bind(runtime_dir: &Path) -> Result<Self> {
        ensure_runtime(runtime_dir)?;
        let data_workers = crate::data_workspace_workers::DataWorkspaceWorkers::new(runtime_dir)?;
        let lock = private_file(&runtime_dir.join("writer.lock"))?;
        FileExt::try_lock_exclusive(&lock)
            .map_err(|_| CoreError::new("WORKSPACE_BUSY", "A daemon already owns this runtime"))?;
        let db = runtime_dir.join("baton.sqlite3");
        // Closing another fd for this inode after SQLite opens can release process-scoped
        // POSIX locks. Finish permission preparation before SQLite owns any connection.
        drop(private_file(&db)?);
        let store = Store::open(&db)?;
        let managed_root = runtime_dir.join("workspaces");
        ensure_runtime(&managed_root)?;
        let socket = runtime_dir.join("batond.sock");
        if socket.exists() {
            let meta = fs::symlink_metadata(&socket)
                .map_err(|_| CoreError::new("PERMISSION_DENIED", "Cannot inspect old socket"))?;
            use std::os::unix::fs::FileTypeExt;
            if !meta.file_type().is_socket() {
                return Err(CoreError::new(
                    "PERMISSION_DENIED",
                    "Socket path contains an unexpected file",
                ));
            }
            fs::remove_file(&socket)
                .map_err(|_| CoreError::new("PERMISSION_DENIED", "Cannot remove stale socket"))?;
        }
        let listener = UnixListener::bind(&socket)
            .map_err(|_| CoreError::new("INTERNAL_ERROR", "Cannot bind private daemon socket"))?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
            .map_err(|_| CoreError::new("PERMISSION_DENIED", "Cannot secure socket"))?;
        listener
            .set_nonblocking(true)
            .map_err(|_| CoreError::new("INTERNAL_ERROR", "Cannot configure socket"))?;
        let token = Uuid::new_v4().to_string() + &Uuid::new_v4().to_string();
        let mut token_file = private_file(&runtime_dir.join("session.token"))?;
        token_file
            .set_len(0)
            .map_err(|_| CoreError::new("INTERNAL_ERROR", "Cannot initialize authentication"))?;
        token_file
            .write_all(token.as_bytes())
            .and_then(|()| token_file.sync_all())
            .map_err(|_| CoreError::new("INTERNAL_ERROR", "Cannot initialize authentication"))?;
        Ok(Self {
            listener,
            store,
            token,
            _lock: lock,
            socket,
            managed_root,
            supervisor: Supervisor::new(runtime_dir),
            command_workers: Default::default(),
            data_workers,
            goal_pending: None,
            factory_queue_clocks: Default::default(),
            factory_pending: None,
            factory_fatal_error: None,
            graph_workers: Default::default(),
        })
    }
    pub fn run(mut self, stop: Arc<AtomicBool>) -> Result<()> {
        #[cfg(debug_assertions)]
        let _diagnostic_flush = goal_diagnostics::FlushOnDrop;
        let (requests, ready) = mpsc::sync_channel::<ReadyRequest>(16);
        let mut readers: Vec<std::thread::JoinHandle<()>> = vec![];
        while !stop.load(Ordering::Relaxed) {
            self.supervisor.tick(&mut self.store)?;
            self.graph_workers.tick(&mut self.store)?;
            self.command_workers.tick(&mut self.store)?;
            self.data_workers.tick(&mut self.store)?;
            self.finish_data_admission();
            if let Some(error) = self.factory_fatal_error.take() {
                return Err(error);
            }
            #[cfg(debug_assertions)]
            let factory_clock = Instant::now();
            #[cfg(debug_assertions)]
            self.trace_goal_stage("factory_tick_begin", None, 0, None);
            self.tick_factory()?;
            #[cfg(debug_assertions)]
            self.trace_goal_stage("factory_tick_end", Some(factory_clock.elapsed()), 0, None);
            #[cfg(debug_assertions)]
            let goal_clock = Instant::now();
            #[cfg(debug_assertions)]
            self.trace_goal_stage("goal_tick_begin", None, 0, None);
            self.tick_goals()?;
            #[cfg(debug_assertions)]
            self.trace_goal_stage("goal_tick_end", Some(goal_clock.elapsed()), 0, None);
            for _ in 0..16 {
                let Ok(job) = ready.try_recv() else {
                    break;
                };
                self.handle_ready_request(job);
                self.supervisor.tick(&mut self.store)?;
                self.command_workers.tick(&mut self.store)?;
                self.data_workers.tick(&mut self.store)?;
                self.finish_data_admission();
            }
            readers.retain(|reader| !reader.is_finished());
            match self.listener.accept() {
                Ok((stream, _)) if readers.len() < 8 => {
                    let sender = requests.clone();
                    if let Ok(reader) = std::thread::Builder::new()
                        .name("baton-local-client".into())
                        .spawn(move || socket_worker(stream, sender))
                    {
                        readers.push(reader);
                    }
                }
                Ok((stream, _)) => drop(stream),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(_) => return Err(CoreError::new("INTERNAL_ERROR", "Daemon socket failed")),
            }
        }
        drop(ready);
        drop(requests);
        self.data_workers.shutdown(&mut self.store)?;
        self.command_workers.shutdown(&mut self.store)?;
        self.supervisor.shutdown(&mut self.store)?;
        self.graph_workers.shutdown(&mut self.store)?;
        for reader in readers {
            let _ = reader.join();
        }
        Ok(())
    }
    fn handle(&mut self, raw: &str) -> Value {
        #[cfg(debug_assertions)]
        let diagnostic_clock = Instant::now();
        let request = serde_json::from_str::<Request>(raw);
        let id = request.as_ref().ok().map(|r| r.request_id.clone());
        let authenticated = request.as_ref().is_ok_and(|r| r.token == self.token);
        let result = request
            .map_err(|_| invalid())
            .and_then(|r| self.dispatch(r));
        let revision = if authenticated {
            self.store.sequence().unwrap_or(0)
        } else {
            0
        };
        let response = match result {
            Ok(value) => json!({"request_id":id,"result":value,"revision":revision,"warnings":[]}),
            Err(error) => json!({"request_id":id,"error":error,"revision":revision,"warnings":[]}),
        };
        #[cfg(debug_assertions)]
        self.trace_goal_stage(
            "request_handled",
            Some(diagnostic_clock.elapsed()),
            goal_diagnostics::method_tag(raw),
            response["error"]["code"].as_str(),
        );
        response
    }
    fn dispatch(&mut self, r: Request) -> Result<Value> {
        if r.token != self.token {
            return Err(CoreError::new(
                "AUTH_REQUIRED",
                "Local authentication failed",
            ));
        }
        if r.api_version != API_VERSION {
            return Err(CoreError::new(
                "VERSION_UNSUPPORTED",
                "Unsupported local API version",
            ));
        }
        if r.request_id.is_empty() || r.request_id.len() > 128 {
            return Err(invalid());
        }
        if (self.supervisor.has_workers()
            || self.command_workers.has_workers()
            || self.data_workers.has_workers()
            || self.graph_workers.has_workers())
            && matches!(
                r.method.as_str(),
                "projects.import"
                    | "projects.reassociate"
                    | "workspaces.create"
                    | "connections.propose"
                    | "grants.propose"
                    | "goal_scopes.propose"
                    | "factory_scopes.propose"
                    | "factory_runs.start"
                    | "approvals.finalize"
                    | "sessions.create"
                    | "command_validation_policies.propose"
            )
        {
            let replay=match r.method.as_str(){
                "factory_runs.start"=>{let p:FactoryScopeId=params(r.params.clone())?;self.store.factory_start_replay(&p.scope_id,r.expected_revision.ok_or_else(invalid)?,r.idempotency_key.as_deref().ok_or_else(invalid)?)?.is_some()},

                "sessions.create"=>r.idempotency_key.as_ref().is_some_and(|key|self.store.conn.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE task_id=?1 AND idempotency_key=?2)",rusqlite::params![r.params["task_id"].as_str().unwrap_or(""),key],|row|row.get::<_,bool>(0)).unwrap_or(false)),
                "approvals.finalize"=>self.store.conn.query_row("SELECT result IS NOT NULL FROM approval_proposals WHERE id=?1",[r.params["proposal_id"].as_str().unwrap_or("")],|row|row.get::<_,bool>(0)).unwrap_or(false),
                _=>false,
            };
            let fixed_goal_execution = r.method == "approvals.finalize"
                && self.store.pending_goal_execution_approval(
                    r.params["proposal_id"].as_str().unwrap_or(""),
                )?;
            if !replay && !fixed_goal_execution {
                return Err(CoreError::new("WORKSPACE_BUSY","Stop active sessions before adding projects, workspaces, connections, grants or sessions. Concurrent setup is not yet supported"));
            }
        }
        match r.method.as_str() {
            "data_workspace_previews.start" => {
                let selection:crate::data_workspaces::DataWorkspaceSelection=params(r.params)?;
                Ok(json!(self.data_workers.preview(&mut self.store,&selection,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?))
            }
            "data_workspace_previews.get" => {let p:DataPreviewId=params(r.params)?;Ok(json!(self.store.get_data_workspace_preview(&p.preview_id)?))}
            "data_workspace_previews.list" => {let p:ProjectId=params(r.params)?;Ok(json!(self.store.list_data_workspace_previews(&p.project_id)?))}
            "data_workspace_previews.cancel" => {let p:DataPreviewId=params(r.params)?;let record=self.store.cancel_data_workspace_preview(&p.preview_id,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?;self.data_workers.cancel(&p.preview_id);Ok(json!(record))}
            "data_workspaces.propose" => {let p:DataPreviewId=params(r.params)?;Ok(json!(self.store.propose_data_workspace(&p.preview_id,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?,&self.managed_root)?))}
            "data_workspaces.get" => {let p:DataCreationId=params(r.params)?;Ok(json!(self.store.get_data_workspace_creation(&p.creation_id)?))}
            "data_workspaces.list" => {let p:ProjectId=params(r.params)?;Ok(json!(self.store.list_data_workspace_creations(&p.project_id)?))}
            "data_workspaces.cancel" => {let p:DataCreationId=params(r.params)?;let record=self.store.cancel_data_workspace_creation(&p.creation_id,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?;self.data_workers.cancel(&p.creation_id);Ok(json!(record))}
            "data_workspaces.create" | "data_workspaces.approve" => Err(CoreError::new("AUTH_REQUIRED","A server-fixed preview and native data_workspace owner confirmation are required before copying")),
            "goal_node_approvals.propose"=>{let p:crate::goal_store::GraphNodeApprovalSelection=params(r.params)?;Ok(json!(self.store.propose_goal_node_acceptance(&p,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?))}
            "approvals.decline"=>{let p:ProposalId=params(r.params)?;self.store.decide_goal_node(&p.proposal_id,r.expected_revision.ok_or_else(invalid)?,false)}
            "goal_graph_runs.cancel"=>{let p:GoalRunId=params(r.params)?;let (receipt,stops)=self.store.cancel_goal_graph_with_stops(&p.run_id,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?;for id in stops{self.supervisor.cancel(&id);}Ok(json!(receipt))}
            "goal_graph_runs.advance_plan"=>{let p:crate::goal_store::GoalPlanAdvance=params(r.params)?;Ok(json!(self.store.advance_goal_graph_plan(&p,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?))}
            "goal_graph_runs.propose"=>{let p:crate::goal_store::GraphRunSelection=params(r.params)?;Ok(json!(self.store.propose_goal_graph_run(&p,&r.idempotency_key.ok_or_else(invalid)?)?))}
            "goal_graph_runs.get"=>{let p:GoalRunId=params(r.params)?;Ok(json!(self.store.goal_graph_details(&p.run_id)?))}
            "goal_graph_runs.list"=>{let p:TaskId=params(r.params)?;Ok(json!(self.store.list_goal_graph_runs(&p.task_id)?))}
            "goal_runs.propose"=>{let p:crate::goal_store::GoalRunSelection=params(r.params)?;Ok(json!(self.store.propose_goal_run(&p,&r.idempotency_key.ok_or_else(invalid)?)?))}
            "goal_runs.get"=>{let p:GoalRunId=params(r.params)?;self.store.goal_run_details(&p.run_id)}
            "goal_runs.list"=>{let p:TaskId=params(r.params)?;Ok(json!(self.store.list_goal_runs(&p.task_id)?))}
            "goal_runs.cancel"=>{let p:GoalRunId=params(r.params)?;let (run,stops)=self.store.cancel_goal_with_stops(&p.run_id,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?;for id in stops {self.supervisor.cancel(&id);}Ok(json!(run))}
            "goal_runs.advance_plan"=>{let p:crate::goal_store::GoalPlanAdvance=params(r.params)?;Ok(json!(self.store.advance_goal_plan(&p,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?))}
            "goal_runs.dispatch" | "goal_graph_runs.dispatch"=>{
                let mut p:crate::goal_store::GoalRunDispatch=params(r.params)?;let revision=r.expected_revision.ok_or_else(invalid)?;let key=r.idempotency_key.ok_or_else(invalid)?;
                let (receipt,new)=if r.method=="goal_graph_runs.dispatch"{self.store.admit_goal_graph_advance(&p,revision,&key)?}else{self.store.admit_goal_advance(&p,revision,&key)?};if !new{return Ok(receipt);}
                p.expected_ledger_revision=self.store.sequence()?;
                let started=Instant::now();
                let action=(||->Result<()> {
                    if r.method=="goal_graph_runs.dispatch" {if let Some(prepared)=self.store.prepare_graph_validation(&p,revision)? {self.graph_workers.start_validation(&mut self.store,prepared)?;return Ok(());}}
                    if let Some(session)=self.store.goal_dispatch_session(&p,revision)? {
                        if let Err(error)=self.supervisor.launch(&mut self.store,&session.id){self.supervisor.record_local_launch_failure(&mut self.store,&session.id)?;self.store.settle_goal_worker(&session.id,u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),true,true)?;return Err(error);}
                        return Ok(());
                    }
                    if let Some((id,text))=self.store.goal_prompt(&p,revision)?{if let Err(error)=self.supervisor.send(&id,&text){self.store.record_turn_queue_failure(&id)?;self.supervisor.cancel(&id);return Err(error);}}Ok(())
                })();
                if let Err(mut error)=action {if error.side_effect_state=="not_started"{error.side_effect_state="confirmed".into();}error.details_ref=receipt["intent_id"].as_str().map(|id|format!("recorded-goal-advance:{id}"));return Err(error);}Ok(receipt)
            }
            "factory_runs.start" => {
                let p: FactoryScopeId = params(r.params)?;
                let revision=r.expected_revision.ok_or_else(invalid)?;
                let key=r.idempotency_key.ok_or_else(invalid)?;
                let replay=self.store.factory_start_replay(&p.scope_id,revision,&key)?.is_some();
                let run=self.store.start_factory_run(&p.scope_id,revision,&key)?;
                if !replay && run.state=="queued" {self.factory_queue_clocks.entry(run.root_task_id.clone()).or_insert_with(Instant::now);}
                Ok(json!(run))
            }
            "factory_runs.get" => {
                let p: FactoryScopeId = params(r.params)?;
                Ok(json!(self.store.get_factory_run(&p.scope_id)?))
            }
            "factory_runs.cancel" => {
                let p: FactoryScopeId = params(r.params)?;
                let record=self.store.cancel_factory_run(&p.scope_id,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?;
                self.data_workers.cancel_factory(&p.scope_id);
                Ok(json!(record))
            }
            "factory_scopes.propose"=>{let p:crate::factory::FactoryScopeSelection=params(r.params)?;let path=crate::factory::companion_path()?;Ok(json!(self.store.propose_factory_scope(&p,&r.idempotency_key.ok_or_else(invalid)?,&path)?))}
            "factory_child_plans.context"=>{if r.expected_revision.is_some()||r.idempotency_key.is_some(){return Err(invalid())}let p:crate::factory::FactoryChildPlanQuery=params(r.params)?;Ok(json!(self.store.factory_child_planning_context(&p)?))}
            "factory_child_plans.get"=>{if r.expected_revision.is_some()||r.idempotency_key.is_some(){return Err(invalid())}let p:crate::factory::FactoryChildPlanQuery=params(r.params)?;Ok(json!(self.store.get_factory_child_plan(&p)?))}
            "factory_child_plans.list"=>{if r.expected_revision.is_some()||r.idempotency_key.is_some(){return Err(invalid())}let p:crate::factory::FactoryChildPlanQuery=params(r.params)?;Ok(json!(self.store.list_factory_child_plans(&p)?))}
            "factory_child_plans.submit"=>{let p:crate::factory::FactoryChildPlanUpdate=params(r.params)?;Ok(json!(self.store.submit_factory_child_plan(&p,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?))}
            "factory_child_goals.start"=>{let p:crate::factory::FactoryChildGoalStart=params(r.params)?;Ok(json!(self.store.start_factory_child_goal(&p,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?))}
            "factory_child_goals.get"=>{if r.expected_revision.is_some()||r.idempotency_key.is_some(){return Err(invalid())}let p:crate::factory::FactoryChildGoalQuery=params(r.params)?;Ok(json!(self.store.get_factory_child_goal(&p)?))}
            "factory_child_goals.list"=>{if r.expected_revision.is_some()||r.idempotency_key.is_some(){return Err(invalid())}let p:crate::factory::FactoryChildGoalQuery=params(r.params)?;Ok(json!(self.store.list_factory_child_goals(&p)?))}
            "factory_child_goals.advance_plan"=>{let p:crate::factory::FactoryChildGoalAdvance=params(r.params)?;Ok(json!(self.store.advance_factory_child_goal(&p,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?))}
            "factory_child_goals.cancel"=>{let p:crate::factory::FactoryChildGoalCancel=params(r.params)?;let (receipt,stops)=self.store.cancel_factory_child_goal_with_stops(&p,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?;for id in stops{self.supervisor.cancel(&id);}Ok(json!(receipt))}
            "factory_scopes.get"=>{let p:FactoryScopeId=params(r.params)?;Ok(json!(self.store.get_factory_scope(&p.scope_id)?))}
            "factory_scopes.list"=>{let p:FactoryRootTaskId=params(r.params)?;Ok(json!(self.store.list_factory_scopes(&p.root_task_id)?))}
            "factory_scopes.revoke"=>{let p:FactoryScopeId=params(r.params)?;let record=self.store.revoke_factory_scope(&p.scope_id,r.expected_revision.ok_or_else(invalid)?,&r.idempotency_key.ok_or_else(invalid)?)?;self.data_workers.cancel_factory(&p.scope_id);Ok(json!(record))}
            "goal_scopes.propose" => {
                let selection:crate::goal_store::GoalScopeSelection=params(r.params)?;
                let key=r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self.store.propose_goal_scope(&selection,&key)?))
            }
            "goal_scopes.list" => {let p:TaskId=params(r.params)?;Ok(json!(self.store.list_goal_scopes(&p.task_id)?))}
            "goal_scopes.get" => {let p:GoalScopeId=params(r.params)?;Ok(json!(self.store.get_goal_scope(&p.scope_id)?))}
            "goal_scopes.revoke" => {let p:GoalScopeId=params(r.params)?;let key=r.idempotency_key.ok_or_else(invalid)?;Ok(json!(self.store.revoke_goal_scope(&p.scope_id,r.expected_revision.ok_or_else(invalid)?,&key)?))}
            "goal_plans.propose" | "goal_plans.revise" => {
                let p:crate::goal_store::GoalPlanRawSubmission=params(r.params)?;
                let key=r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self.store.submit_goal_plan_json(&p,&key,r.expected_revision.ok_or_else(invalid)?)?))
            }
            "goal_plans.get" => {let p:GoalScopeId=params(r.params)?;Ok(json!(self.store.get_goal_plan(&p.scope_id)?))}
            "goal_plans.list" => {let p:GoalScopeId=params(r.params)?;Ok(json!(self.store.list_goal_plan_revisions(&p.scope_id)?))}
            "goal_plans.ready_preview" => {let p:GoalScopeId=params(r.params)?;Ok(json!(self.store.preview_goal_ready(&p.scope_id,r.expected_revision.ok_or_else(invalid)?)?))}
            "goal_scopes.approve" | "goal_plans.dispatch" | "goal_plans.reserve" | "goal_plans.accept" => Err(CoreError::new("AUTH_REQUIRED","Renderer approval flags, dispatch/reservation and Task acceptance are not Goal preview methods")),
            "connections.propose" => {
                let p: RegisterConnection = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self.store.propose_connection(
                    &p.name,
                    &p.provider,
                    Path::new(&p.executable),
                    &p.arguments,
                    &key
                )?))
            }
            "approvals.finalize" => {
                let p: ProposalId = params(r.params)?;
                let revision = r.expected_revision.ok_or_else(invalid)?;
                let already_confirmed = self.store.conn.query_row("SELECT result IS NOT NULL FROM approval_proposals WHERE id=?1",[&p.proposal_id],|row|row.get::<_,bool>(0)).unwrap_or(false);
                let result = self.store.finalize_proposal(&p.proposal_id, revision)?;
                if !already_confirmed && result.get("dataset_revision").is_some() && result.get("preview_id").is_some() {
                    if let Some(id)=result["id"].as_str() {
                        if let Err(mut error)=self.data_workers.copy(&mut self.store,id) {
                            // The native owner receipt and creation intent have
                            // committed, even when no filesystem worker started.
                            // Keep unknown when a later local write is uncertain.
                            if error.side_effect_state=="not_started" {error.side_effect_state="confirmed".into();}
                            error.details_ref=Some(format!("data-copy-intent:{id}"));
                            error.message=format!("{}; owner approval and copy intent are recorded. Read the existing creation; this does not confirm filesystem copy or Ready",error.message);
                            return Err(error);
                        }
                    }
                }
                if result.get("provider").is_some() {
                    if let Some(id) = result["id"].as_str() {
                        if let Err(mut error) = self.supervisor.probe(&self.store, id) {
                            // The owner-approved connection and confirmation
                            // receipt already committed; only probe scheduling
                            // failed. Never report the approval as not started.
                            error.side_effect_state = "confirmed".into();
                            error.details_ref =
                                Some(format!("connection-approved-probe-not-scheduled:{id}"));
                            error.message = format!(
                                "{}; connection approval is recorded, but its version probe was not scheduled. Reconcile using the same proposal and existing connection",
                                error.message
                            );
                            return Err(error);
                        }
                    }
                }
                Ok(result)
            }
            "connections.register" | "grants.create" => Err(CoreError::new(
                "AUTH_REQUIRED",
                "Server proposal and trusted native owner confirmation are required",
            )),
            "connections.list" => {
                empty(&r.params)?;
                Ok(json!(self.store.list_connections()?))
            }
            "grants.propose" => {
                let p: CreateGrant = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self.store.propose_grant(
                    &p.project_id,
                    &p.workspace_id,
                    &p.connection_id,
                    p.max_attempts,
                    p.max_concurrency,
                    p.max_duration_ms,
                    &key
                )?))
            }
            "grants.list" => {
                let p: ProjectId = params(r.params)?;
                Ok(json!(self.store.list_grants(&p.project_id)?))
            }
            "grants.revoke" => {
                let p: GrantId = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                let revision = r.expected_revision.ok_or_else(invalid)?;
                Ok(json!(self.store.revoke_grant(
                    &p.grant_id,
                    revision,
                    &key
                )?))
            }
            "sessions.create" => {
                let p: CreateSession = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                let revision = r.expected_revision.ok_or_else(invalid)?;
                if let Some(required) = p.require_enforcement {
                    let grant = self.store.get_grant(&p.grant_id)?;
                    crate::policy::require_enforcement(
                        &grant,
                        required.filesystem,
                        required.network,
                        required.identity,
                    )?;
                    if required.configuration {
                        return Err(CoreError::new(
                            "ENFORCEMENT_UNAVAILABLE",
                            "Native configuration isolation has not been verified",
                        ));
                    }
                }
                let s = self.store.create_session(
                    &p.task_id,
                    &p.workspace_id,
                    &p.connection_id,
                    &p.grant_id,
                    revision,
                    &key,
                )?;
                if let Err(mut error) = self.supervisor.launch(&mut self.store, &s.id) {
                    self.supervisor
                        .record_local_launch_failure(&mut self.store, &s.id)?;
                    error.side_effect_state = "confirmed".into();
                    error.details_ref =
                        Some(format!("local-session-recorded-no-provider-start:{}", s.id));
                    return Err(error);
                }
                Ok(json!(s))
            }
            "command_validation_policies.propose" => {
                let selection: crate::command_validation::CommandValidationSelection =
                    params(r.params)?;
                Ok(json!(self.store.propose_command_validation_policy(
                    &selection,
                    &r.idempotency_key.ok_or_else(invalid)?
                )?))
            }
            "command_validation_policies.list" => {
                let p: ValidationTaskId = params(r.params)?;
                Ok(json!(self
                    .store
                    .list_command_validation_policies(&p.task_id)?))
            }
            "command_validation_policies.revoke" => {
                let p: ValidationPolicyId = params(r.params)?;
                let policy = self.store.revoke_command_validation_policy(
                    &p.policy_id,
                    r.expected_revision.ok_or_else(invalid)?,
                    &r.idempotency_key.ok_or_else(invalid)?,
                )?;
                self.command_workers.cancel_policy(&p.policy_id);
                Ok(json!(policy))
            }
            "command_validations.list" => {
                let p: ValidationTaskId = params(r.params)?;
                Ok(json!(self
                    .store
                    .list_command_validation_records(&p.task_id)?))
            }
            "command_validations.get" => {
                let p: CommandRunId = params(r.params)?;
                Ok(json!(self
                    .store
                    .get_command_validation_record(&p.run_id)?))
            }
            "command_validations.cancel" => {
                let p: CommandRunId = params(r.params)?;
                let record = self.store.request_command_cancel(
                    &p.run_id,
                    r.expected_revision.ok_or_else(invalid)?,
                    &r.idempotency_key.ok_or_else(invalid)?,
                    self.command_workers.owns(&p.run_id),
                )?;
                self.command_workers.cancel(&p.run_id);
                Ok(json!(record))
            }
            "command_validations.run" => {
                let p: ValidationPolicyId = params(r.params)?;
                let revision = r.expected_revision.ok_or_else(invalid)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                if self.supervisor.has_owned_io() {
                    if let Some(replay) = self.store.command_replay(&p.policy_id, revision, &key)? {
                        return Ok(json!(replay));
                    }
                    return Err(CoreError::new(
                        "WORKSPACE_BUSY",
                        "Stop owned Provider sessions before executing a fixed validator",
                    ));
                }
                Ok(json!(self.command_workers.start(
                    &mut self.store,
                    &p.policy_id,
                    revision,
                    &key
                )?))
            }
            "data_validation_policies.propose" => {
                let selection: crate::data_validation::DataValidationSelection = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self
                    .store
                    .propose_data_validation_policy(&selection, &key)?))
            }
            "data_validation_policies.list" => {
                let p: ValidationTaskId = params(r.params)?;
                Ok(json!(self
                    .store
                    .list_data_validation_policies(&p.task_id)?))
            }
            "data_validation_policies.revoke" => {
                let p: ValidationPolicyId = params(r.params)?;
                let policy=self.store.revoke_data_validation_policy(&p.policy_id,r.expected_revision.ok_or_else(invalid)?)?;
                self.data_workers.cancel_validation_policy(&p.policy_id);
                Ok(json!(policy))
            }
            "data_validations.list" => {
                let p: ValidationTaskId = params(r.params)?;
                Ok(json!(self
                    .store
                    .list_data_validation_records(&p.task_id)?))
            }
            "data_validations.run" => {
                let p: ValidationPolicyId = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                if self.supervisor.has_workers() || self.command_workers.has_workers() {
                    return Err(CoreError::new(
                        "WORKSPACE_BUSY",
                        "Stop active owned sessions before this bounded data-validation operation",
                    ));
                }
                Ok(json!(self.store.run_data_validation(
                    &p.policy_id,
                    r.expected_revision.ok_or_else(invalid)?,
                    &key
                )?))
            }
            "validation_policies.propose" => {
                let selection: crate::validation::ValidationSelection = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self
                    .store
                    .propose_validation_policy(&selection, &key)?))
            }
            "validation_policies.list" => {
                let p: ValidationTaskId = params(r.params)?;
                Ok(json!(self.store.list_validation_policies(&p.task_id)?))
            }
            "validation_policies.revoke" => {
                let p: ValidationPolicyId = params(r.params)?;
                let record=self.store.revoke_validation_policy(&p.policy_id,r.expected_revision.ok_or_else(invalid)?)?;
                self.data_workers.cancel_validation_policy(&p.policy_id);
                Ok(json!(record))
            }
            "validations.list" => {
                let p: ValidationTaskId = params(r.params)?;
                Ok(json!(self.store.list_validation_records(&p.task_id)?))
            }
            "validations.run" => {
                let p: ValidationPolicyId = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                if self.supervisor.has_workers() || self.command_workers.has_workers() {
                    return Err(CoreError::new(
                        "WORKSPACE_BUSY",
                        "Stop active owned sessions before this bounded file-validation operation",
                    ));
                }
                Ok(json!(self.store.run_artifact_validation(
                    &p.policy_id,
                    r.expected_revision.ok_or_else(invalid)?,
                    &key
                )?))
            }
            "sessions.list" => {
                empty(&r.params)?;
                Ok(json!(self.store.list_sessions()?))
            }
            "sessions.get" => {
                let p: SessionId = params(r.params)?;
                Ok(json!(self.store.get_session(&p.session_id)?))
            }
            "sessions.send" => {
                let p: SendSession = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                let revision = r.expected_revision.ok_or_else(invalid)?;
                let (s, dispatch) =
                    self.store
                        .send_session(&p.session_id, &p.text, revision, &key)?;
                if dispatch {
                    if let Err(error) = self.supervisor.send(&s.id, &p.text) {
                        self.supervisor.cancel(&s.id);
                        self.store.record_turn_queue_failure(&s.id)?;
                        return Err(error);
                    }
                }
                Ok(json!(s))
            }
            "sessions.cancel" => {
                let p: SessionId = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                let revision = r.expected_revision.ok_or_else(invalid)?;
                let s = self.store.cancel_session(
                    &p.session_id,
                    revision,
                    &key,
                    self.supervisor.owns(&p.session_id),
                )?;
                self.supervisor.cancel(&s.id);
                Ok(json!(s))
            }
            "projects.import" => {
                let p: ImportProject = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self.store.import_project(
                    Path::new(&p.path),
                    &p.name,
                    &key
                )?))
            }
            "projects.list" => {
                empty(&r.params)?;
                Ok(json!(self.store.list_projects()?))
            }
            "projects.archive" => {
                let p: ProjectId = params(r.params)?;
                let rev = r.expected_revision.ok_or_else(invalid)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self.store.archive_project(
                    &p.project_id,
                    rev,
                    &key
                )?))
            }
            "projects.reassociate" => {
                let p: ReassociateProject = params(r.params)?;
                let rev = r.expected_revision.ok_or_else(invalid)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self.store.reassociate_project(
                    &p.project_id,
                    Path::new(&p.path),
                    rev,
                    &key
                )?))
            }
            "workspaces.create" => {
                let p: CreateWorkspace = params(r.params)?;
                let rev = r.expected_revision.ok_or_else(invalid)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self.store.create_workspace(
                    &p.project_id,
                    rev,
                    &p.expected_head,
                    &self.managed_root,
                    &key
                )?))
            }
            "workspaces.list" => {
                let p: ProjectId = params(r.params)?;
                Ok(json!(self.store.list_workspaces(&p.project_id)?))
            }
            "tasks.create" => {
                let p: Create = params(r.params)?;
                let key = r.idempotency_key.ok_or_else(invalid)?;
                Ok(json!(self.store.create_registered(
                    &p.project_id,
                    &p.goal,
                    &key
                )?))
            }
            "tasks.list" => {
                empty(&r.params)?;
                Ok(json!(self.store.list()?))
            }
            "tasks.get" => {
                let p: TaskId = params(r.params)?;
                Ok(json!(self.store.get(&p.task_id)?))
            }
            "tasks.cancel" => {
                let p: TaskId = params(r.params)?;
                let rev = r.expected_revision.ok_or_else(invalid)?;
                let task = self.store.cancel(&p.task_id, rev)?;
                self.command_workers.cancel_task(&task.id);
                self.data_workers.cancel_task(&task.id);
                Ok(json!(task))
            }
            "events.subscribe" => {
                let p: Events = params(r.params)?;
                self.store.events(p.cursor, p.limit)
            }
            "system.snapshot" => {
                empty(&r.params)?;
                Ok(
                    json!({"tasks":self.store.list()?,"sequence":self.store.sequence()?,"api_version":API_VERSION}),
                )
            }
            _ => Err(CoreError::new(
                "CAPABILITY_UNSUPPORTED",
                "Unknown or unavailable method",
            )),
        }
    }
}
impl Drop for Daemon {
    fn drop(&mut self) {
        // Error/unwind paths also retain authority until every owned worker is
        // cancelled and joined. Unpersisted outcomes are reconciled on restart.
        self.graph_workers.reap_owned_workers();
        self.data_workers.reap_owned_workers();
        self.command_workers.reap_owned_workers();
        self.supervisor.reap_owned_workers();
        let _ = fs::remove_file(&self.socket);
        // `run` has finished before self is dropped. Release this ownership
        // explicitly: duplicated/inherited descriptors can outlive this File.
        // Unlink first so a successor's socket is never removed after unlock.
        let _ = FileExt::unlock(&self._lock);
    }
}
fn read_frame(stream: &UnixStream, max_frame: u64) -> Result<String> {
    use std::io::Read;
    let mut reader = BufReader::new(stream).take(max_frame + 1);
    let mut bytes = Vec::new();
    reader
        .read_until(b'\n', &mut bytes)
        .map_err(|_| CoreError::transport())?;
    if bytes.len() as u64 > max_frame || !bytes.ends_with(b"\n") {
        return Err(CoreError::new(
            "RESOURCE_LIMIT",
            "IPC frame exceeds limit or is incomplete",
        ));
    }
    String::from_utf8(bytes).map_err(|_| invalid())
}
/// Host-selected runtime only. Reads private session authentication and applies the version envelope.
pub fn call(runtime_dir: &Path, command: Value) -> Result<Value> {
    check_private(runtime_dir, true)?;
    check_private(&runtime_dir.join("session.token"), false)?;
    let token = fs::read_to_string(runtime_dir.join("session.token"))
        .map_err(|_| CoreError::transport())?;
    let mut object = command.as_object().cloned().ok_or_else(invalid)?;
    let read_timeout = if matches!(
        object.get("method").and_then(Value::as_str),
        Some(
            "projects.import"
                | "projects.reassociate"
                | "workspaces.create"
                | "validations.run"
                | "data_validations.run"
        )
    ) {
        Duration::from_secs(60)
    } else {
        Duration::from_secs(5)
    };
    object.insert("api_version".into(), json!(API_VERSION));
    object.insert("request_id".into(), json!(Uuid::new_v4().to_string()));
    object.insert("token".into(), json!(token));
    let mut stream =
        UnixStream::connect(runtime_dir.join("batond.sock")).map_err(|_| CoreError::transport())?;
    stream
        .set_read_timeout(Some(read_timeout))
        .map_err(|_| CoreError::transport())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| CoreError::transport())?;
    serde_json::to_writer(&mut stream, &object).map_err(|_| CoreError::transport())?;
    stream
        .write_all(b"\n")
        .map_err(|_| CoreError::transport())?;
    let raw = read_frame(&stream, 32 * MAX_FRAME)?;
    serde_json::from_str(&raw).map_err(|_| CoreError::transport())
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    #[test]
    fn inherited_nonblocking_stream_waits_for_the_delayed_complete_first_frame() {
        let (receiver, mut sender) = UnixStream::pair().unwrap();
        receiver.set_nonblocking(true).unwrap();
        let (requests, ready) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || socket_worker(receiver, requests));
        std::thread::sleep(Duration::from_millis(30));
        sender.write_all(b"{}\n").unwrap();
        let request = ready.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(request.raw, "{}\n");
        request
            .reply
            .send(json!({"result":"delayed-frame-read"}))
            .unwrap();
        let response: Value =
            serde_json::from_str(&read_frame(&sender, MAX_FRAME).unwrap()).unwrap();
        assert_eq!(response["result"], "delayed-frame-read");
        worker.join().unwrap();
    }

    #[test]
    fn daemon_drop_releases_its_lock_even_with_an_old_duplicate_descriptor() {
        let directory = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let runtime = directory.path().canonicalize().unwrap().join("runtime");
        let original = Daemon::bind(&runtime).unwrap();
        let old_duplicate = original._lock.try_clone().unwrap();
        assert_eq!(Daemon::bind(&runtime).err().unwrap().code, "WORKSPACE_BUSY");
        drop(original);
        assert!(!runtime.join("batond.sock").exists());
        // No retry or waiting: an old open-file-description duplicate must not
        // retain a stopped daemon's single-writer ownership.
        let replacement = Daemon::bind(&runtime).unwrap();
        assert_eq!(Daemon::bind(&runtime).err().unwrap().code, "WORKSPACE_BUSY");
        drop(old_duplicate);
        // Closing that old description must not release the new owner's lock.
        assert_eq!(Daemon::bind(&runtime).err().unwrap().code, "WORKSPACE_BUSY");
        assert!(runtime.join("batond.sock").exists());
        drop(replacement);
        let final_owner = Daemon::bind(&runtime).unwrap();
        drop(final_owner);
    }

    #[test]
    fn daemon_retains_its_writer_lock_until_owned_worker_cleanup_finishes() {
        let directory = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let runtime = directory.path().canonicalize().unwrap().join("runtime");
        let mut original = Daemon::bind(&runtime).unwrap();
        let old_duplicate = original._lock.try_clone().unwrap();
        let checked = Arc::new(AtomicBool::new(false));
        let worker_checked = checked.clone();
        let worker_runtime = runtime.clone();
        original.supervisor.install_drop_fixture(move || {
            assert_eq!(
                Daemon::bind(&worker_runtime).err().unwrap().code,
                "WORKSPACE_BUSY"
            );
            assert!(worker_runtime.join("batond.sock").exists());
            worker_checked.store(true, Ordering::SeqCst);
        });
        drop(original);
        assert!(checked.load(Ordering::SeqCst));
        let replacement = Daemon::bind(&runtime).unwrap();
        drop(old_duplicate);
        assert_eq!(Daemon::bind(&runtime).err().unwrap().code, "WORKSPACE_BUSY");
        drop(replacement);
    }

    #[test]
    fn fatal_actor_error_cancels_and_joins_owned_graph_reader_before_writer_lock_handoff() {
        let directory = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let runtime = directory.path().canonicalize().unwrap().join("runtime");
        let mut daemon = Daemon::bind(&runtime).unwrap();
        let old_duplicate = daemon._lock.try_clone().unwrap();
        let checked = Arc::new(AtomicBool::new(false));
        let observed = checked.clone();
        let worker_runtime = runtime.clone();
        daemon
            .graph_workers
            .install_drop_fixture(runtime.clone(), move || {
                assert_eq!(
                    Daemon::bind(&worker_runtime).err().unwrap().code,
                    "WORKSPACE_BUSY"
                );
                assert!(worker_runtime.join("batond.sock").exists());
                observed.store(true, Ordering::SeqCst);
            });
        // The synthetic reader has no authorized run: tick returns a real actor
        // error before run's normal shutdown branch. Drop must still reap it.
        let error = daemon.run(Arc::new(AtomicBool::new(false))).unwrap_err();
        assert_eq!(error.code, "NOT_FOUND");
        assert!(checked.load(Ordering::SeqCst));
        assert!(!runtime.join("batond.sock").exists());
        let successor = Daemon::bind(&runtime).unwrap();
        drop(old_duplicate);
        assert_eq!(Daemon::bind(&runtime).err().unwrap().code, "WORKSPACE_BUSY");
        drop(successor);
    }

    #[test]
    fn data_validation_run_refuses_owned_workers_before_any_file_validation() {
        let directory = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let runtime = directory.path().canonicalize().unwrap().join("runtime");
        let mut daemon = Daemon::bind(&runtime).unwrap();
        // Synthetic cancellation-observing owned worker, not a Provider session.
        daemon.supervisor.install_drop_fixture(|| {});
        let request = json!({"api_version":API_VERSION,"request_id":"busy-data","token":daemon.token,"method":"data_validations.run","params":{"policy_id":"must-not-be-looked-up"},"expected_revision":1,"idempotency_key":"busy"}).to_string();
        let response = daemon.handle(&request);
        assert_eq!(response["error"]["code"], "WORKSPACE_BUSY");
        assert_eq!(daemon.store.sequence().unwrap(), 0);
    }

    #[test]
    fn oversized_parser_returns_resource_limit_without_relying_on_error_delivery() {
        let (receiver, mut sender) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || {
            // The parser may reject before the writer completes; that socket
            // scheduling detail does not change the parser's required error.
            let result = sender.write_all(&[b'x'; 1025]);
            if let Err(error) = result {
                assert!(matches!(
                    error.kind(),
                    std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::NotConnected
                ));
            }
        });
        let error = read_frame(&receiver, 1024).unwrap_err();
        assert_eq!(error.code, "RESOURCE_LIMIT");
        drop(receiver);
        writer.join().unwrap();
    }
    #[test]
    fn nonblocking_accepted_stream_waits_for_a_delayed_authenticated_first_frame() {
        use std::sync::mpsc;
        let dir = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let runtime = dir.path().canonicalize().unwrap().join("runtime");
        let mut daemon = Daemon::bind(&runtime).unwrap();
        let mut client = UnixStream::connect(runtime.join("batond.sock")).unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let (accepted, _) = daemon.listener.accept().unwrap();
        // Linux accept does not inherit this flag; explicitly simulate the BSD
        // accepted descriptor state to make the portability regression deterministic.
        accepted.set_nonblocking(true).unwrap();
        let (ready, receive) = mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            configure_accepted_stream(&accepted).unwrap();
            ready.send(()).unwrap();
            let result = read_frame(&accepted, MAX_FRAME);
            (result, accepted)
        });
        receive.recv_timeout(Duration::from_secs(2)).unwrap();
        // Connection setup is complete before the peer starts sending its first
        // request; a nonblocking read must not fabricate a transport disconnect.
        std::thread::sleep(Duration::from_millis(50));
        let token = fs::read_to_string(runtime.join("session.token")).unwrap();
        let mut request=serde_json::to_vec(&json!({"api_version":API_VERSION,"request_id":"delayed","token":token,"method":"tasks.create","params":{"goal":"unknown project","project_id":"not-registered"},"idempotency_key":"delayed"})).unwrap();
        request.push(b'\n');
        client.write_all(&request).unwrap();
        let (frame, _accepted) = reader.join().unwrap();
        let response = daemon.handle(&frame.unwrap());
        assert_eq!(response["error"]["code"], "NOT_FOUND");
        assert_eq!(daemon.store.sequence().unwrap(), 0);
        assert!(
            matches!(daemon.listener.accept(),Err(error) if error.kind()==std::io::ErrorKind::WouldBlock)
        );
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRunId {
    run_id: String,
}

impl Daemon {
    fn data_contexts_for_request(
        &self,
        raw: &str,
    ) -> Result<Vec<crate::data_workspaces::DataInputContext>> {
        let request: Request = match serde_json::from_str(raw) {
            Ok(request) => request,
            Err(_) => return Ok(vec![]),
        };
        if request.token != self.token
            || request.api_version != API_VERSION
            || request.request_id.is_empty()
            || request.request_id.len() > 128
        {
            return Ok(vec![]);
        }
        let mut ids = Vec::new();
        match request.method.as_str() {
            "factory_child_goals.start" | "factory_child_goals.advance_plan" => {
                let (scope, task, namespace, payload) = if request.method
                    == "factory_child_goals.start"
                {
                    let p: crate::factory::FactoryChildGoalStart = params(request.params.clone())?;
                    (
                        p.scope_id.clone(),
                        p.request.task_id.clone(),
                        format!(
                            "factory:{}:owner:{}:child-goal:start",
                            p.scope_id, p.request.task_id
                        ),
                        serde_json::to_string(&(
                            request.expected_revision.ok_or_else(invalid)?,
                            &p,
                        ))
                        .map_err(|_| invalid())?,
                    )
                } else {
                    let p: crate::factory::FactoryChildGoalAdvance =
                        params(request.params.clone())?;
                    (
                        p.scope_id.clone(),
                        p.request.task_id.clone(),
                        format!(
                            "factory:{}:owner:{}:child-goal:advance",
                            p.scope_id, p.request.task_id
                        ),
                        serde_json::to_string(&(
                            request.expected_revision.ok_or_else(invalid)?,
                            &p,
                        ))
                        .map_err(|_| invalid())?,
                    )
                };
                if self
                    .store
                    .factory_child_goal_action_replay(
                        &namespace,
                        request.idempotency_key.as_deref().ok_or_else(invalid)?,
                        &payload,
                    )?
                    .is_some()
                {
                    return Ok(vec![]);
                }
                let _ = task;
                ids.extend(
                    self.store
                        .get_factory_scope(&scope)?
                        .selection
                        .workspace_ids,
                );
            }

            "factory_child_plans.context" | "factory_child_plans.submit" => {
                let scope_id = request
                    .params
                    .get("scope_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(invalid)?;
                let scope = self.store.get_factory_scope(scope_id)?;
                if request.method == "factory_child_plans.submit" {
                    let p: crate::factory::FactoryChildPlanUpdate = params(request.params.clone())?;
                    if self.store.factory_child_plan_owner_replay(
                        &p,
                        request.expected_revision.ok_or_else(invalid)?,
                        request.idempotency_key.as_deref().ok_or_else(invalid)?,
                    )? {
                        return Ok(vec![]);
                    }
                }
                if !scope.revoked
                    && i128::from(scope.expires_at_ms)
                        > time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000
                    && !(request.method == "factory_child_plans.context"
                        && matches!(
                            self.store.get_factory_run(scope_id)?.run.state.as_str(),
                            "cancelled" | "blocked" | "completed" | "outcome_unknown"
                        ))
                {
                    ids.extend(scope.selection.workspace_ids);
                }
            }
            "factory_scopes.propose" => {
                let p: crate::factory::FactoryScopeSelection = params(request.params)?;
                if self.store.factory_proposal_replay(
                    &p,
                    request.idempotency_key.as_deref().ok_or_else(invalid)?,
                )? {
                    return Ok(vec![]);
                }
                ids.extend(self.store.get_grant(&p.grant_id)?.scope.workspace_ids);
                ids.extend(p.workspace_ids);
            }
            "grants.propose" => {
                let p: CreateGrant = params(request.params)?;
                let intent = json!({"project_id":p.project_id,"workspace_id":p.workspace_id,"connection_id":p.connection_id,"max_attempts":p.max_attempts,"max_concurrency":p.max_concurrency,"max_duration_ms":p.max_duration_ms});
                if let Some(key) = request.idempotency_key.as_deref() {
                    if self
                        .store
                        .data_proposal_replay("grant", &intent, key)?
                        .is_some()
                    {
                        return Ok(vec![]);
                    }
                }
                ids.push(p.workspace_id);
            }
            "sessions.create" => {
                let p: CreateSession = params(request.params)?;
                if let Some(key) = request.idempotency_key.as_ref() {
                    use rusqlite::OptionalExtension;
                    let old: Option<String> = self
                        .store
                        .conn
                        .query_row(
                            "SELECT id FROM sessions WHERE task_id=?1 AND idempotency_key=?2",
                            rusqlite::params![p.task_id, key],
                            |r| r.get(0),
                        )
                        .optional()
                        .map_err(crate::storage::db_error)?;
                    if old.is_some() {
                        return Ok(vec![]);
                    }
                }
                ids.push(p.workspace_id);
            }
            "sessions.send" => {
                let p: SendSession = params(request.params)?;
                ids.push(self.store.get_session(&p.session_id)?.workspace_id);
            }
            "goal_plans.propose" | "goal_plans.revise" => {
                let p: crate::goal_store::GoalPlanRawSubmission = params(request.params)?;
                if let Some(key) = request.idempotency_key.as_deref() {
                    let scope = format!("goal:{}:plan", p.scope_id);
                    let old: bool = self
                        .store
                        .conn
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM idempotency WHERE scope=?1 AND key=?2)",
                            rusqlite::params![scope, key],
                            |r| r.get(0),
                        )
                        .map_err(crate::storage::db_error)?;
                    if old {
                        return Ok(vec![]);
                    }
                }
                ids.push(self.store.get_goal_scope(&p.scope_id)?.workspace_id);
            }
            "goal_node_approvals.propose" => {
                let p: crate::goal_store::GraphNodeApprovalSelection = params(request.params)?;
                if let (Some(revision), Some(key)) = (
                    request.expected_revision,
                    request.idempotency_key.as_deref(),
                ) {
                    if self
                        .store
                        .node_proposal_replay(&p, revision, key)?
                        .is_some()
                    {
                        return Ok(vec![]);
                    }
                }
                let run = self.store.goal_graph_summary(&p.run_id)?.run;
                ids.push(self.store.get_goal_scope(&run.scope_id)?.workspace_id);
            }
            "goal_graph_runs.propose" => {
                let p: crate::goal_store::GraphRunSelection = params(request.params)?;
                if let Some(key) = request.idempotency_key.as_deref() {
                    if self
                        .store
                        .graph_execution_proposal_replay(&p, key)?
                        .is_some()
                    {
                        return Ok(vec![]);
                    }
                }
                ids.push(self.store.get_goal_scope(&p.scope_id)?.workspace_id);
            }
            "goal_runs.propose" => {
                let p: crate::goal_store::GoalRunSelection = params(request.params)?;
                if let Some(key) = request.idempotency_key.as_deref() {
                    if self
                        .store
                        .goal_execution_proposal_replay(&p, key)?
                        .is_some()
                    {
                        return Ok(vec![]);
                    }
                }
                ids.push(self.store.get_goal_scope(&p.scope_id)?.workspace_id);
            }
            "goal_graph_runs.advance_plan" => {
                let p: crate::goal_store::GoalPlanAdvance = params(request.params)?;
                if let (Some(key), Some(rev)) = (
                    request.idempotency_key.as_deref(),
                    request.expected_revision,
                ) {
                    if self
                        .store
                        .graph_plan_advance_replay(&p, rev, key)?
                        .is_some()
                    {
                        return Ok(vec![]);
                    }
                }
                let r = self.store.goal_graph_summary(&p.run_id)?.run;
                ids.push(self.store.get_goal_scope(&r.scope_id)?.workspace_id);
            }
            "goal_runs.advance_plan" => {
                let p: crate::goal_store::GoalPlanAdvance = params(request.params)?;
                if let (Some(key), Some(revision)) = (
                    request.idempotency_key.as_deref(),
                    request.expected_revision,
                ) {
                    if self
                        .store
                        .goal_plan_advance_replay(&p, revision, key)?
                        .is_some()
                    {
                        return Ok(vec![]);
                    }
                }
                let run = self.store.get_goal_run(&p.run_id)?;
                ids.push(self.store.get_goal_scope(&run.scope_id)?.workspace_id);
            }
            "goal_runs.dispatch" | "goal_graph_runs.dispatch" => {
                let graph = request.method == "goal_graph_runs.dispatch";
                let p: crate::goal_store::GoalRunDispatch = params(request.params)?;
                if let (Some(key), Some(revision)) = (
                    request.idempotency_key.as_deref(),
                    request.expected_revision,
                ) {
                    if !graph && self.store.goal_advance_replay(&p, revision, key)?.is_some() {
                        return Ok(vec![]);
                    }
                }
                let run = if graph {
                    self.store.goal_graph_summary(&p.run_id)?.run
                } else {
                    self.store.get_goal_run(&p.run_id)?
                };
                ids.push(self.store.get_goal_scope(&run.scope_id)?.workspace_id);
            }
            "goal_plans.ready_preview" => {
                let p: GoalScopeId = params(request.params)?;
                ids.push(self.store.get_goal_scope(&p.scope_id)?.workspace_id);
            }
            "goal_scopes.propose" => {
                let p: crate::goal_store::GoalScopeSelection = params(request.params)?;
                if let Some(key) = request.idempotency_key.as_deref() {
                    if self
                        .store
                        .data_proposal_replay("goal", &json!(p), key)?
                        .is_some()
                    {
                        return Ok(vec![]);
                    }
                }
                ids.extend(self.store.get_grant(&p.grant_id)?.scope.workspace_ids);
            }
            "validation_policies.propose" => {
                let p: crate::validation::ValidationSelection = params(request.params)?;
                if let Some(key) = request.idempotency_key.as_deref() {
                    if self
                        .store
                        .data_proposal_replay("validation", &json!(p), key)?
                        .is_some()
                    {
                        return Ok(vec![]);
                    }
                }
                ids.push(self.store.get_session(&p.session_id)?.workspace_id);
            }
            "data_validation_policies.propose" => {
                let p: crate::data_validation::DataValidationSelection = params(request.params)?;
                if let Some(key) = request.idempotency_key.as_deref() {
                    if self
                        .store
                        .data_proposal_replay("data_validation", &json!(p), key)?
                        .is_some()
                    {
                        return Ok(vec![]);
                    }
                }
                ids.push(self.store.get_session(&p.session_id)?.workspace_id);
            }
            "command_validation_policies.propose" => {
                let p: crate::command_validation::CommandValidationSelection =
                    params(request.params)?;
                if let Some(key) = request.idempotency_key.as_deref() {
                    if self
                        .store
                        .data_proposal_replay("command_validation", &json!(p), key)?
                        .is_some()
                    {
                        return Ok(vec![]);
                    }
                }
                ids.push(self.store.get_session(&p.session_id)?.workspace_id);
            }
            "validations.run" | "data_validations.run" | "command_validations.run" => {
                let p: ValidationPolicyId = params(request.params)?;
                let table = match request.method.as_str() {
                    "validations.run" => "validation_runs",
                    "data_validations.run" => "data_validation_runs",
                    _ => "command_validation_runs",
                };
                if let Some(key) = request.idempotency_key.as_ref() {
                    let q=format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE policy_id=?1 AND idempotency_key=?2)");
                    if self
                        .store
                        .conn
                        .query_row(&q, rusqlite::params![p.policy_id, key], |r| {
                            r.get::<_, bool>(0)
                        })
                        .map_err(crate::storage::db_error)?
                    {
                        return Ok(vec![]);
                    }
                }
                let workspace = match request.method.as_str() {
                    "validations.run" => {
                        self.store.get_validation_policy(&p.policy_id)?.workspace_id
                    }
                    "data_validations.run" => {
                        self.store
                            .get_data_validation_policy(&p.policy_id)?
                            .workspace_id
                    }
                    _ => {
                        self.store
                            .get_command_validation_policy(&p.policy_id)?
                            .workspace_id
                    }
                };
                ids.push(workspace);
            }
            "factory_runs.start" => {
                let p: FactoryScopeId = params(request.params)?;
                let scope = self.store.get_factory_scope(&p.scope_id)?;
                if self
                    .store
                    .factory_start_replay(
                        &p.scope_id,
                        request.expected_revision.ok_or_else(invalid)?,
                        request.idempotency_key.as_deref().ok_or_else(invalid)?,
                    )?
                    .is_none()
                {
                    ids.extend(scope.bindings.approved_grant.scope.workspace_ids);
                }
            }
            "approvals.finalize" | "approvals.decline" => {
                let p: ProposalId = params(request.params)?;
                let raw: Option<String> = {
                    use rusqlite::OptionalExtension;
                    self.store.conn.query_row("SELECT document FROM approval_proposals WHERE id=?1 AND result IS NULL AND octet_length(document)<=4194304",[p.proposal_id],|r|r.get(0)).optional().map_err(crate::storage::db_error)?
                };
                if let Some(raw) = raw {
                    let proposal: crate::sessions::ApprovalProposal = crate::storage::decode(&raw)?;
                    match proposal.kind.as_str() {
                        "factory" => {
                            // Declining pending authority performs no input read
                            // and must remain available while its read is owned.
                            if request.method == "approvals.decline" {
                                return Ok(vec![]);
                            }
                            if let Some(id) = proposal.snapshot["selection"]["grant_id"].as_str() {
                                ids.extend(self.store.get_grant(id)?.scope.workspace_ids);
                            }
                            if let Some(workspaces) =
                                proposal.snapshot["bindings"]["workspaces"].as_array()
                            {
                                if workspaces.len() > 1 {
                                    return Err(invalid());
                                }
                                for workspace in workspaces {
                                    ids.push(
                                        workspace["id"].as_str().ok_or_else(invalid)?.to_owned(),
                                    );
                                }
                            }
                        }

                        "goal_execution" | "goal_graph_execution" => {
                            if let Some(id) = proposal.snapshot["intent"]["scope_id"].as_str() {
                                ids.push(self.store.get_goal_scope(id)?.workspace_id);
                            }
                        }
                        "goal_node_acceptance" => {
                            if let Some(id) =
                                proposal.snapshot["run_binding"]["workspace_id"].as_str()
                            {
                                ids.push(id.to_owned());
                            }
                        }
                        "goal" => {
                            if let Some(id) = proposal.snapshot["intent"]["grant_id"].as_str() {
                                ids.extend(self.store.get_grant(id)?.scope.workspace_ids);
                            }
                        }
                        "grant" => {
                            if let Some(id) = proposal.snapshot["intent"]["workspace_id"].as_str() {
                                ids.push(id.to_owned())
                            }
                        }
                        "validation" | "data_validation" | "command_validation" => {
                            if let Some(session) =
                                proposal.snapshot["intent"]["session_id"].as_str()
                            {
                                ids.push(self.store.get_session(session)?.workspace_id)
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        ids.sort();
        ids.dedup();
        self.store.prepare_data_input_contexts(&ids)
    }
    fn factory_context_for_request(
        &self,
        raw: &str,
    ) -> Result<Option<crate::factory::FactoryReadPlan>> {
        let request: Request = serde_json::from_str(raw).map_err(|_| invalid())?;
        if request.token != self.token || request.api_version != API_VERSION {
            return Ok(None);
        }
        let derived = self.store.prepare_factory_goal_read_for_request(
            &request.method,
            &request.params,
            request.expected_revision,
            request.idempotency_key.as_deref(),
            raw,
        )?;
        let plan = if derived.is_some() {
            derived
        } else if matches!(
            request.method.as_str(),
            "factory_scopes.propose"
                | "factory_runs.start"
                | "approvals.finalize"
                | "factory_child_plans.context"
                | "factory_child_plans.submit"
        ) {
            self.store.prepare_factory_owner_read(
                &request.method,
                &request.params,
                request.expected_revision,
                request.idempotency_key.as_deref(),
                raw,
                None,
            )?
        } else {
            None
        };
        if plan.is_some() {
            if (!plan.as_ref().is_some_and(|p| p.is_child_planning())
                && (self.supervisor.has_workers()
                    || self.command_workers.has_workers()
                    || self.graph_workers.has_workers()))
                || self.data_workers.has_workers()
            {
                return Err(CoreError::new("WORKSPACE_BUSY","Stop active workers before a new Factory owner read; historical exact-key replies remain available"));
            }
            self.data_workers.runtime_privacy_pin()?;
        }
        Ok(plan)
    }
    fn graph_context_for_request(
        &self,
        raw: &str,
    ) -> Result<Option<crate::goal_store::GraphReadContext>> {
        let request: Request = serde_json::from_str(raw).map_err(|_| invalid())?;
        if request.token != self.token || request.api_version != API_VERSION {
            return Ok(None);
        }
        self.store.graph_request_read_context(
            &request.method,
            &request.params,
            request.expected_revision,
            request.idempotency_key.as_deref(),
        )
    }
    fn ready_error(&self, raw: &str, error: CoreError) -> Value {
        #[cfg(debug_assertions)]
        self.trace_goal_stage(
            "ready_error",
            None,
            goal_diagnostics::method_tag(raw),
            Some(&error.code),
        );
        let request_id = serde_json::from_str::<Value>(raw)
            .ok()
            .and_then(|v| v["request_id"].as_str().map(str::to_owned));
        json!({"request_id":request_id,"error":error,"revision":self.store.sequence().unwrap_or(0),"warnings":[]})
    }
    fn handle_ready_request(&mut self, job: ReadyRequest) {
        if Instant::now() > job.deadline {
            if job.factory_dispatch.is_some() {
                self.finish_factory_ticket_error(
                    job,
                    CoreError::new(
                        "RESOURCE_LIMIT",
                        "Queued private Factory dispatch expired before admission",
                    ),
                );
                return;
            }
            let _ = job.reply.try_send(json!({"request_id":null,"error":CoreError::new("RESOURCE_LIMIT","Queued request expired before admission"),"revision":0,"warnings":[]}));
            return;
        }
        if let Some(ticket) = job.factory_dispatch.as_ref() {
            let watch = (ticket.scope_id().to_owned(), ticket.scope_revision());
            let contexts = {
                if !ticket.for_request(&job.raw) {
                    Err(CoreError::new(
                        "AUTH_REQUIRED",
                        "Private Factory ticket does not match its exact request",
                    ))
                } else {
                    self.store.factory_ticket_data_contexts(ticket)
                }
            };
            match contexts {
                Err(error) => self.finish_factory_ticket_error(job, error),
                Ok(contexts) if !contexts.is_empty() => {
                    if let Err(failure) = self.data_workers.defer(contexts, None, None, job) {
                        let (error, job) = *failure;
                        self.finish_factory_ticket_error(job, error);
                    } else {
                        self.data_workers.watch_factory(&watch.0, watch.1);
                    }
                }
                Ok(_) => self.execute_factory_ticket(job),
            }
            return;
        }
        if serde_json::from_str::<Value>(&job.raw)
            .ok()
            .is_some_and(|v| v.get("scoped_token").is_some())
        {
            let contexts = (|| {
                let request: FactoryScopedRequest =
                    params(serde_json::from_str(&job.raw).map_err(|_| invalid())?)?;
                if request.api_version != 1
                    || request.request_id.is_empty()
                    || request.request_id.len() > 128
                    || !matches!(
                        request.method.as_str(),
                        "delegate_task"
                            | "get_task_status"
                            | "get_task_result"
                            | "cancel_task"
                            | "submit_child_plan"
                            | "execute_child_goal"
                            | "advance_child_goal"
                    )
                {
                    return Err(CoreError::new(
                        "AUTH_REQUIRED",
                        "Only the fixed scoped Factory methods can request a data observation",
                    ));
                }
                let prepared = self
                    .store
                    .factory_scoped_data_contexts(&request.handle_id, &request.scoped_token)?;
                let plan = if request.method == "submit_child_plan" {
                    let p: crate::factory::FactoryChildPlanSubmission = params(request.params)?;
                    Some(self.store.prepare_factory_scoped_child_plan_read(
                        &request.handle_id,
                        &request.scoped_token,
                        &p,
                        &job.raw,
                    )?)
                } else if matches!(
                    request.method.as_str(),
                    "execute_child_goal" | "advance_child_goal"
                ) {
                    let task = if request.method == "execute_child_goal" {
                        params::<crate::factory::FactoryChildGoalSubmission>(request.params)?
                            .task_id
                    } else {
                        params::<crate::factory::FactoryChildGoalAdvanceSubmission>(request.params)?
                            .request
                            .task_id
                    };
                    Some(self.store.prepare_factory_scoped_child_goal_read(
                        &request.handle_id,
                        &request.scoped_token,
                        &task,
                        &job.raw,
                    )?)
                } else {
                    None
                };
                Ok((prepared, plan))
            })();
            match contexts {
                Err(_) => {
                    let response = self.handle_factory_scoped(&job.raw);
                    let _ = job.reply.try_send(response);
                }
                Ok((prepared, plan)) if !prepared.inputs.is_empty() || plan.is_some() => {
                    if let Err(failure) = self.data_workers.defer(prepared.inputs, None, plan, job)
                    {
                        let (error, job) = *failure;
                        let _ = job
                            .reply
                            .try_send(self.factory_ready_error(&job.raw, error));
                    } else {
                        self.data_workers
                            .watch_factory(&prepared.scope_id, prepared.revision);
                    }
                }
                Ok(_) => {
                    let response = self.handle_factory_scoped(&job.raw);
                    let _ = job.reply.try_send(response);
                }
            }
            return;
        }

        #[cfg(debug_assertions)]
        let factory_clock = Instant::now();
        let factory = match self.factory_context_for_request(&job.raw) {
            Ok(plan) => plan,
            Err(error) => {
                #[cfg(debug_assertions)]
                self.trace_goal_stage(
                    "factory_preflight",
                    Some(factory_clock.elapsed()),
                    goal_diagnostics::method_tag(&job.raw),
                    Some(&error.code),
                );
                let _ = job.reply.try_send(self.ready_error(&job.raw, error));
                return;
            }
        };
        #[cfg(debug_assertions)]
        self.trace_goal_stage(
            "factory_preflight",
            Some(factory_clock.elapsed()),
            goal_diagnostics::method_tag(&job.raw),
            None,
        );
        #[cfg(debug_assertions)]
        let graph_clock = Instant::now();
        let graph = match self.graph_context_for_request(&job.raw) {
            Ok(g) => g,
            Err(error) => {
                #[cfg(debug_assertions)]
                self.trace_goal_stage(
                    "graph_preflight",
                    Some(graph_clock.elapsed()),
                    goal_diagnostics::method_tag(&job.raw),
                    Some(&error.code),
                );
                let _ = job.reply.try_send(self.ready_error(&job.raw, error));
                return;
            }
        };
        #[cfg(debug_assertions)]
        self.trace_goal_stage(
            "graph_preflight",
            Some(graph_clock.elapsed()),
            goal_diagnostics::method_tag(&job.raw),
            None,
        );
        #[cfg(debug_assertions)]
        let data_clock = Instant::now();
        let contexts = self.data_contexts_for_request(&job.raw);
        #[cfg(debug_assertions)]
        self.trace_goal_stage(
            "data_context",
            Some(data_clock.elapsed()),
            goal_diagnostics::method_tag(&job.raw),
            contexts.as_ref().err().map(|e| e.code.as_str()),
        );
        match contexts {
            Err(error) => {
                let _ = job.reply.try_send(self.ready_error(&job.raw, error));
            }
            Ok(contexts) if !contexts.is_empty() || graph.is_some() || factory.is_some() => {
                if let Err(failure) = self.data_workers.defer(contexts, graph, factory, job) {
                    let (error, job) = *failure;
                    let _ = job.reply.try_send(self.ready_error(&job.raw, error));
                }
            }
            Ok(_) => {
                let response = self.handle(&job.raw);
                let _ = job.reply.try_send(response);
            }
        }
    }
    fn finish_data_admission(&mut self) {
        use crate::data_workspace_workers::CompletedDataRequest;
        let Some(completed) = self.data_workers.take_admission() else {
            return;
        };
        #[cfg(debug_assertions)]
        if let CompletedDataRequest::Input(job, result) = &completed {
            self.trace_goal_stage(
                "input_read_done",
                None,
                goal_diagnostics::method_tag(&job.raw),
                result.as_ref().as_ref().err().map(|e| e.code.as_str()),
            );
        }
        match completed {
            CompletedDataRequest::Input(job, result) => match *result {
                Err(error) => {
                    if job.factory_dispatch.is_some() {
                        self.finish_factory_ticket_error(job, error);
                    } else {
                        let _ = job
                            .reply
                            .try_send(self.factory_ready_error(&job.raw, error));
                    }
                }
                Ok(mut witness) => {
                    if Instant::now() > job.deadline || !witness.for_request(&job.raw) {
                        let error = CoreError::new(
                            "DEADLINE_EXCEEDED",
                            "Data observation expired or belongs to another exact request",
                        );
                        if job.factory_dispatch.is_some() {
                            self.finish_factory_ticket_error(job, error);
                        } else {
                            let _ = job
                                .reply
                                .try_send(self.factory_ready_error(&job.raw, error));
                        }
                        return;
                    }
                    debug_assert!(self.store.data_input.is_none());
                    self.store.graph_input = witness.graph.take();
                    self.store.data_input = Some(witness);
                    if job.factory_dispatch.is_some() {
                        self.execute_factory_ticket(job);
                        self.store.data_input = None;
                        self.store.graph_input = None;
                        return;
                    }
                    if serde_json::from_str::<Value>(&job.raw)
                        .ok()
                        .is_some_and(|v| v.get("scoped_token").is_some())
                    {
                        let response = self.handle_factory_scoped(&job.raw);
                        self.store.data_input = None;
                        self.store.graph_input = None;
                        let _ = job.reply.try_send(response);
                        return;
                    }
                    let request = serde_json::from_str::<Request>(&job.raw)
                        .expect("authenticated deferred request");
                    if request.method == "data_validations.run" {
                        let plan = (|| {
                            let p: ValidationPolicyId = params(request.params)?;
                            self.store.prepare_data_validation(
                                &p.policy_id,
                                request.expected_revision.ok_or_else(invalid)?,
                                &request.idempotency_key.ok_or_else(invalid)?,
                            )
                        })();
                        let witness = self.store.data_input.take().unwrap();
                        match plan {
                            Err(error) => {
                                let _ = job.reply.try_send(self.ready_error(&job.raw, error));
                            }
                            Ok(crate::data_validation::DataValidationPlan::Replay(record)) => {
                                let _ = job
                                    .reply
                                    .try_send(self.ready_success(&job.raw, json!(record)));
                            }
                            Ok(crate::data_validation::DataValidationPlan::Check(prepared)) => {
                                if let Err(failure) =
                                    self.data_workers.validate_data(prepared, witness, job)
                                {
                                    let (error, job) = *failure;
                                    let _ = job.reply.try_send(self.ready_error(&job.raw, error));
                                }
                            }
                        }
                    } else if request.method == "validations.run" {
                        let plan = (|| {
                            let p: ValidationPolicyId = params(request.params)?;
                            self.store.prepare_artifact_validation(
                                &p.policy_id,
                                request.expected_revision.ok_or_else(invalid)?,
                                &request.idempotency_key.ok_or_else(invalid)?,
                            )
                        })();
                        let witness = self.store.data_input.take().unwrap();
                        match plan {
                            Err(error) => {
                                let _ = job.reply.try_send(self.ready_error(&job.raw, error));
                            }
                            Ok(crate::validation::ArtifactValidationPlan::Replay(record)) => {
                                let _ = job
                                    .reply
                                    .try_send(self.ready_success(&job.raw, json!(record)));
                            }
                            Ok(crate::validation::ArtifactValidationPlan::Check(prepared)) => {
                                if let Err(failure) =
                                    self.data_workers.validate_integrity(prepared, witness, job)
                                {
                                    let (error, job) = *failure;
                                    let _ = job.reply.try_send(self.ready_error(&job.raw, error));
                                }
                            }
                        }
                    } else {
                        let response = self.handle(&job.raw);
                        self.store.data_input = None;
                        self.store.graph_input = None;
                        let _ = job.reply.try_send(response);
                    }
                }
            },
            CompletedDataRequest::Validation(job, result) => {
                match *result {
                    Err(error) => {
                        let _ = job.reply.try_send(self.ready_error(&job.raw, error));
                    }
                    Ok((checked, witness)) => {
                        if Instant::now() > job.deadline || !witness.for_request(&job.raw) {
                            let _=job.reply.try_send(self.ready_error(&job.raw,CoreError::new("DEADLINE_EXCEEDED","Data validation receipt arrived after its exact request deadline")));
                            return;
                        }
                        debug_assert!(self.store.data_input.is_none());
                        self.store.data_input = Some(witness);
                        let record = self.store.finish_data_validation(checked);
                        self.store.data_input = None;
                        let response = match record {
                            Ok(record) => self.ready_success(&job.raw, json!(record)),
                            Err(error) => self.ready_error(&job.raw, error),
                        };
                        let _ = job.reply.try_send(response);
                    }
                }
            }
            CompletedDataRequest::Integrity(job, result) => match *result {
                Err(error) => {
                    let _ = job.reply.try_send(self.ready_error(&job.raw, error));
                }
                Ok((checked, witness)) => {
                    if Instant::now() > job.deadline || !witness.for_request(&job.raw) {
                        let _ = job.reply.try_send(self.ready_error(
                            &job.raw,
                            CoreError::new(
                                "DEADLINE_EXCEEDED",
                                "Integrity receipt arrived after its exact request deadline",
                            ),
                        ));
                        return;
                    }
                    debug_assert!(self.store.data_input.is_none());
                    self.store.data_input = Some(witness);
                    let record = self.store.finish_artifact_validation(checked);
                    self.store.data_input = None;
                    let response = match record {
                        Ok(record) => self.ready_success(&job.raw, json!(record)),
                        Err(error) => self.ready_error(&job.raw, error),
                    };
                    let _ = job.reply.try_send(response);
                }
            },
        }
    }
    fn ready_success(&self, raw: &str, result: Value) -> Value {
        let request_id = serde_json::from_str::<Value>(raw)
            .ok()
            .and_then(|v| v["request_id"].as_str().map(str::to_owned));
        json!({"request_id":request_id,"result":result,"revision":self.store.sequence().unwrap_or(0),"warnings":[]})
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GoalRunId {
    run_id: String,
}
impl Daemon {
    #[cfg(debug_assertions)]
    fn trace_goal_stage(
        &self,
        stage: &'static str,
        elapsed: Option<Duration>,
        count: usize,
        code: Option<&str>,
    ) {
        let (worker, retained) = self.data_workers.diagnostic_worker_state();
        goal_diagnostics::emit(
            stage,
            elapsed,
            [
                worker,
                retained,
                self.graph_workers.has_workers(),
                self.goal_pending.is_some(),
                self.factory_pending.is_some(),
                self.supervisor.has_workers(),
            ],
            count,
            code,
        );
    }
    fn tick_goals(&mut self) -> Result<()> {
        for id in self.store.goal_stop_requests()? {
            self.supervisor.cancel(&id);
        }
        #[cfg(debug_assertions)]
        self.trace_goal_stage("goal_after_stops", None, 0, None);
        if let Some((id, receiver)) = self.goal_pending.take() {
            match receiver.try_recv() {
                Ok(response) => {
                    #[cfg(debug_assertions)]
                    self.trace_goal_stage(
                        "goal_pending_reply",
                        None,
                        0,
                        response["error"]["code"].as_str(),
                    );
                    if let Some(code) = response["error"]["code"].as_str() {
                        if !matches!(code, "STALE_REVISION" | "WORKSPACE_BUSY") {
                            self.store.block_goal_run(&id, code)?;
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.goal_pending = Some((id, receiver));
                    #[cfg(debug_assertions)]
                    self.trace_goal_stage("goal_pending_empty", None, 0, None);
                    return Ok(());
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    #[cfg(debug_assertions)]
                    self.trace_goal_stage(
                        "goal_pending_disconnect",
                        None,
                        0,
                        Some("INTERNAL_ADVANCE_INTERRUPTED"),
                    );
                    self.store
                        .block_goal_run(&id, "INTERNAL_ADVANCE_INTERRUPTED")?;
                }
            }
        }
        if self.data_workers.has_workers() || self.graph_workers.has_workers() {
            #[cfg(debug_assertions)]
            self.trace_goal_stage("goal_lane_busy", None, 0, None);
            return Ok(());
        }
        let mut next = None;
        for r in self.store.active_goal_runs()? {
            #[cfg(debug_assertions)]
            let ready_clock = Instant::now();
            let needs = self.store.goal_run_needs_advance(&r.id);
            #[cfg(debug_assertions)]
            self.trace_goal_stage(
                "goal_needs_advance",
                Some(ready_clock.elapsed()),
                usize::from(needs.as_ref().is_ok_and(|b| *b)),
                needs.as_ref().err().map(|e| e.code.as_str()),
            );
            if needs? {
                next = Some(r);
                break;
            }
        }
        let Some(run) = next else { return Ok(()) };
        #[cfg(debug_assertions)]
        self.trace_goal_stage("goal_selected", None, 1, None);
        let ledger = self.store.sequence()?;
        let method = if self.store.is_goal_graph_run(&run.id)? {
            "goal_graph_runs.dispatch"
        } else {
            "goal_runs.dispatch"
        };
        let raw=serde_json::to_string(&json!({"api_version":API_VERSION,"request_id":Uuid::new_v4().to_string(),"token":self.token,"method":method,"params":{"run_id":run.id,"expected_ledger_revision":ledger},"expected_revision":run.revision,"idempotency_key":format!("goal-advance:{}:{}:{}",run.id,run.revision,ledger)})).map_err(|_|CoreError::new("INTERNAL_ERROR","Cannot construct private Goal advancement"))?;
        let (reply, receive) = mpsc::sync_channel(1);
        self.goal_pending = Some((run.id, receive));
        self.handle_ready_request(ReadyRequest {
            raw,
            reply,
            deadline: Instant::now() + Duration::from_secs(60),
            factory_dispatch: None,
        });
        Ok(())
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FactoryScopeId {
    scope_id: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FactoryRootTaskId {
    root_task_id: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FactoryScopedRequest {
    api_version: u32,
    request_id: String,
    handle_id: String,
    scoped_token: String,
    method: String,
    params: Value,
    #[serde(default)]
    expected_revision: Option<u64>,
    #[serde(default)]
    idempotency_key: Option<String>,
}
impl Daemon {
    fn handle_factory_scoped(&mut self, raw: &str) -> Value {
        let request = match serde_json::from_str::<FactoryScopedRequest>(raw) {
            Ok(r) => r,
            Err(_) => {
                return json!({"request_id":null,"error":CoreError::new("AUTH_REQUIRED","Only the fixed scoped Factory envelope is accepted"),"revision":0,"warnings":[]})
            }
        };
        let id = request.request_id.clone();
        if request.api_version != 1
            || id.is_empty()
            || id.len() > 128
            || !matches!(
                request.method.as_str(),
                "delegate_task"
                    | "get_task_status"
                    | "get_task_result"
                    | "cancel_task"
                    | "submit_child_plan"
                    | "execute_child_goal"
                    | "advance_child_goal"
            )
        {
            return json!({"request_id":id,"error":CoreError::new("AUTH_REQUIRED","Scoped Factory callers cannot access owner approval, configuration or arbitrary IPC"),"revision":0,"warnings":[]});
        }
        let caller = match self
            .store
            .authenticate_factory_handle(&request.handle_id, &request.scoped_token)
        {
            Ok(c) => c,
            Err(_) => {
                return json!({"request_id":id,"error":CoreError::new("AUTH_REQUIRED","Factory scoped credential is unavailable, expired or does not own this active session"),"revision":0,"warnings":[]})
            }
        };
        let result = (|| match request.method.as_str() {
            "delegate_task" => {
                if request.expected_revision.is_some() {
                    return Err(invalid());
                }
                let p: crate::factory::FactoryDelegateRequest = params(request.params)?;
                self.store.factory_delegate(
                    caller,
                    &p,
                    &request.idempotency_key.ok_or_else(invalid)?,
                )
            }
            "get_task_status" => {
                if request.expected_revision.is_some() || request.idempotency_key.is_some() {
                    return Err(invalid());
                }
                let p: crate::factory::FactoryTaskRequest = params(request.params)?;
                self.store.factory_status(&caller, &p.task_id)
            }
            "get_task_result" => {
                if request.expected_revision.is_some() || request.idempotency_key.is_some() {
                    return Err(invalid());
                }
                let p: crate::factory::FactoryTaskRequest = params(request.params)?;
                self.store.factory_result(&caller, &p.task_id)
            }
            "submit_child_plan" => {
                if request.expected_revision.is_some() {
                    return Err(invalid());
                }
                let p: crate::factory::FactoryChildPlanSubmission = params(request.params)?;
                self.store.factory_submit_child_plan(
                    caller,
                    &p,
                    &request.idempotency_key.ok_or_else(invalid)?,
                )
            }
            "execute_child_goal" => {
                if request.expected_revision.is_some() {
                    return Err(invalid());
                }
                let p: crate::factory::FactoryChildGoalSubmission = params(request.params)?;
                self.store.factory_execute_child_goal(
                    caller,
                    &p,
                    &request.idempotency_key.ok_or_else(invalid)?,
                )
            }
            "advance_child_goal" => {
                if request.expected_revision.is_some() {
                    return Err(invalid());
                }
                let p: crate::factory::FactoryChildGoalAdvanceSubmission = params(request.params)?;
                self.store.factory_advance_child_goal(
                    caller,
                    &p,
                    &request.idempotency_key.ok_or_else(invalid)?,
                )
            }
            "cancel_task" => {
                let p: crate::factory::FactoryTaskRequest = params(request.params)?;
                self.store.factory_cancel_task(
                    &caller,
                    &p.task_id,
                    request.expected_revision.ok_or_else(invalid)?,
                    &request.idempotency_key.ok_or_else(invalid)?,
                )
            }
            _ => Err(invalid()),
        })();
        match result {
            Ok(outcome) => {
                if request.method == "delegate_task" {
                    if let Some(task) = outcome.result["task_id"].as_str() {
                        self.factory_queue_clocks
                            .entry(task.to_owned())
                            .or_insert_with(Instant::now);
                    }
                }
                for session in outcome.stops {
                    self.supervisor.cancel(&session);
                }
                json!({"request_id":id,"result":outcome.result,"revision":self.store.sequence().unwrap_or(0),"warnings":[]})
            }
            Err(error) => {
                json!({"request_id":id,"error":error,"revision":self.store.sequence().unwrap_or(0),"warnings":[]})
            }
        }
    }
}

impl Daemon {
    fn tick_factory(&mut self) -> Result<()> {
        if let Some((scope, receiver)) = self.factory_pending.take() {
            match receiver.try_recv() {
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => {
                    self.factory_pending = Some((scope, receiver));
                    return Ok(());
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.store.block_factory_run(
                        &scope,
                        CoreError::new(
                            "OUTCOME_UNKNOWN",
                            "Private Factory dispatch result was interrupted; no automatic replay",
                        ),
                    )?;
                }
            }
        }
        if self.data_workers.has_workers()
            || self.graph_workers.has_workers()
            || self.command_workers.has_workers()
        {
            return Ok(());
        }
        let Some(action) = self.store.next_factory_action()? else {
            return Ok(());
        };
        match action {
            crate::factory::FactoryAction::Stop(id) => self.supervisor.cancel(&id),
            crate::factory::FactoryAction::Prompt {
                session_id,
                text,
                context,
            } => {
                let (raw, ticket) = context.prompt_ticket(session_id, text)?;
                self.schedule_factory_ticket(raw, ticket);
            }
            crate::factory::FactoryAction::Launch(context) => {
                let task = context.request().0.to_owned();
                let queued = self
                    .factory_queue_clocks
                    .remove(&task)
                    .unwrap_or_else(Instant::now);
                let (raw, ticket) = context.ticket(queued)?;
                self.schedule_factory_ticket(raw, ticket);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod factory_queue_deadline_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn expired_scoped_queue_request_is_rejected_before_authentication_or_any_mutation() {
        let temp = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let runtime = temp.path().canonicalize().unwrap();
        let mut daemon = Daemon::bind(&runtime).unwrap();
        let sequence = daemon.store.sequence().unwrap();
        for method in [
            "delegate_task",
            "get_task_status",
            "get_task_result",
            "cancel_task",
        ] {
            let (send, recv) = std::sync::mpsc::sync_channel(1);
            let deadline = Instant::now();
            std::thread::sleep(Duration::from_millis(2));
            daemon.handle_ready_request(ReadyRequest{raw:json!({"api_version":1,"request_id":"generated-expired-scoped","handle_id":"missing-generated","scoped_token":"0".repeat(64),"method":method,"params":{},"expected_revision":1,"idempotency_key":"never-admit"}).to_string(),reply:send,deadline,factory_dispatch:None});
            let response = recv.recv_timeout(Duration::from_secs(1)).unwrap();
            assert_eq!(
                response["error"]["code"], "RESOURCE_LIMIT",
                "{method}: {response}"
            );
            assert_eq!(response["revision"], 0);
            assert_eq!(daemon.store.sequence().unwrap(), sequence);
            for table in [
                "factory_task_lineage",
                "factory_delegations",
                "factory_attempts",
                "factory_scoped_handles",
                "idempotency",
            ] {
                let count: i64 = daemon
                    .store
                    .conn
                    .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                    .unwrap();
                assert_eq!(count, 0, "{table}");
            }
        }
    }
}
impl Daemon {
    fn factory_ready_error(&self, raw: &str, error: CoreError) -> Value {
        if serde_json::from_str::<Value>(raw)
            .ok()
            .is_some_and(|v| v.get("scoped_token").is_some())
        {
            let id = serde_json::from_str::<Value>(raw)
                .ok()
                .and_then(|v| v["request_id"].as_str().map(str::to_owned));
            json!({"request_id":id,"error":error,"revision":0,"warnings":[]})
        } else {
            self.ready_error(raw, error)
        }
    }
    fn schedule_factory_ticket(
        &mut self,
        raw: String,
        ticket: crate::factory::FactoryDispatchTicket,
    ) {
        let scope = ticket.scope_id().to_owned();
        let (reply, receive) = mpsc::sync_channel(1);
        self.factory_pending = Some((scope, receive));
        self.handle_ready_request(ReadyRequest {
            raw,
            reply,
            deadline: Instant::now() + Duration::from_secs(60),
            factory_dispatch: Some(ticket),
        });
    }
    fn execute_factory_ticket(&mut self, mut job: ReadyRequest) {
        let Some(ticket) = job.factory_dispatch.take() else {
            return;
        };
        if Instant::now() > job.deadline || !ticket.for_request(&job.raw) {
            job.factory_dispatch = Some(ticket);
            self.finish_factory_ticket_error(
                job,
                CoreError::new(
                    "DEADLINE_EXCEEDED",
                    "Private Factory request expired before dispatch",
                ),
            );
            return;
        }
        let scope = ticket.scope_id().to_owned();
        let prompt_session = ticket.prompt().map(|(id, _)| id.to_owned());
        let result = (|| -> Result<Value> {
            if let Some((id, text)) = ticket.prompt() {
                self.store.factory_ticket_data_contexts(&ticket)?;
                let session = self.store.get_session(id)?;
                let (_, queued) = self.store.send_factory_session(
                    id,
                    text,
                    session.revision,
                    &format!("factory-turn:{}", session.attempt_id),
                )?;
                if queued {
                    self.supervisor.send(id, text)?;
                }
                return Ok(json!({"session_id":id,"turn_queued":queued,"task_accepted":false}));
            }
            let context = ticket.into_context();
            let (task, workspace, connection, grant) = context.request();
            let args = (
                task.to_owned(),
                workspace.to_owned(),
                connection.to_owned(),
                grant.to_owned(),
            );
            let rev = self.store.get(&args.0)?.revision;
            let s = self.store.create_session_for_factory(
                (
                    &args.0,
                    &args.1,
                    &args.2,
                    &args.3,
                    rev,
                    &format!("factory-attempt:{}:{}", args.0, rev),
                ),
                context,
            )?;
            if let Err(error) = self.supervisor.launch(&mut self.store, &s.id) {
                self.store.factory_launch_error(&s.id, error)?;
                self.supervisor
                    .record_local_launch_failure(&mut self.store, &s.id)?;
            }
            Ok(json!({"session_id":s.id,"attempt_id":s.attempt_id,"task_accepted":false}))
        })();
        let response = match result {
            Ok(value) => self.ready_success(&job.raw, value),
            Err(error) => {
                if let Some(id) = prompt_session.as_deref() {
                    self.supervisor.cancel(id);
                }
                if let Err(fatal) = self.store.block_factory_run(&scope, error.clone()) {
                    self.factory_fatal_error = Some(fatal);
                }
                self.ready_error(&job.raw, error)
            }
        };
        let _ = job.reply.try_send(response);
    }
    fn finish_factory_ticket_error(&mut self, mut job: ReadyRequest, error: CoreError) {
        if let Some(ticket) = job.factory_dispatch.take() {
            let result = if let Some((session, _)) = ticket.prompt() {
                self.supervisor.cancel(session);
                self.store
                    .block_factory_run(ticket.scope_id(), error.clone())
            } else {
                self.store.fail_factory_preflight(
                    ticket.scope_id(),
                    ticket.elapsed_ms(),
                    error.clone(),
                )
            };
            if let Err(fatal) = result {
                self.factory_fatal_error = Some(fatal);
            }
        }
        let _ = job
            .reply
            .try_send(self.factory_ready_error(&job.raw, error));
    }
}
