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
struct ReadyRequest {
    raw: String,
    reply: SyncSender<Value>,
    deadline: Instant,
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
}
impl Daemon {
    pub fn bind(runtime_dir: &Path) -> Result<Self> {
        ensure_runtime(runtime_dir)?;
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
        })
    }
    pub fn run(mut self, stop: Arc<AtomicBool>) -> Result<()> {
        let (requests, ready) = mpsc::sync_channel::<ReadyRequest>(16);
        let mut readers: Vec<std::thread::JoinHandle<()>> = vec![];
        while !stop.load(Ordering::Relaxed) {
            self.supervisor.tick(&mut self.store)?;
            for _ in 0..16 {
                let Ok(job) = ready.try_recv() else {
                    break;
                };
                let response = if Instant::now() > job.deadline {
                    json!({"request_id":null,"error":CoreError::new("RESOURCE_LIMIT","Queued request expired before execution"),"revision":0,"warnings":[]})
                } else {
                    self.handle(&job.raw)
                };
                let _ = job.reply.try_send(response);
                self.supervisor.tick(&mut self.store)?;
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
        self.supervisor.shutdown(&mut self.store)?;
        for reader in readers {
            let _ = reader.join();
        }
        Ok(())
    }
    fn handle(&mut self, raw: &str) -> Value {
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
        match result {
            Ok(value) => json!({"request_id":id,"result":value,"revision":revision,"warnings":[]}),
            Err(error) => json!({"request_id":id,"error":error,"revision":revision,"warnings":[]}),
        }
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
        if self.supervisor.has_workers()
            && matches!(
                r.method.as_str(),
                "projects.import"
                    | "projects.reassociate"
                    | "workspaces.create"
                    | "connections.propose"
                    | "grants.propose"
                    | "approvals.finalize"
                    | "sessions.create"
            )
        {
            let replay=match r.method.as_str(){
                "sessions.create"=>r.idempotency_key.as_ref().is_some_and(|key|self.store.conn.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE task_id=?1 AND idempotency_key=?2)",rusqlite::params![r.params["task_id"].as_str().unwrap_or(""),key],|row|row.get::<_,bool>(0)).unwrap_or(false)),
                "approvals.finalize"=>self.store.conn.query_row("SELECT result IS NOT NULL FROM approval_proposals WHERE id=?1",[r.params["proposal_id"].as_str().unwrap_or("")],|row|row.get::<_,bool>(0)).unwrap_or(false),
                _=>false,
            };
            if !replay {
                return Err(CoreError::new("WORKSPACE_BUSY","Stop active sessions before adding projects, workspaces, connections, grants or sessions. Concurrent setup is not yet supported"));
            }
        }
        match r.method.as_str() {
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
                let result = self.store.finalize_proposal(&p.proposal_id, revision)?;
                if result.get("provider").is_some() {
                    if let Some(id) = result["id"].as_str() {
                        self.supervisor.probe(&self.store, id)?;
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
                Ok(json!(self.store.cancel(&p.task_id, rev)?))
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
        let _ = fs::remove_file(&self.socket);
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
        Some("projects.import" | "projects.reassociate" | "workspaces.create")
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
