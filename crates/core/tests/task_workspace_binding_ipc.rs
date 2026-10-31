//! Fixed Task binding Unix IPC; generated TempDir sources, no Provider or Native dialog.
use agentbaton_core::ipc::{call, Daemon};
use rusqlite::{types::Value as SqlValue, Connection, OpenFlags};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{ffi::OsStrExt, net::UnixStream},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

struct Server {
    runtime: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Server {
    fn start(runtime: &Path) -> Self {
        let daemon = Daemon::bind(runtime).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let run = stop.clone();
        let thread = std::thread::spawn(move || daemon.run(run).unwrap());
        Self {
            runtime: runtime.into(),
            stop,
            thread: Some(thread),
        }
    }
    fn request(&self, command: Value) -> Value {
        call(&self.runtime, command).unwrap()
    }
    fn raw(&self, value: Value) -> Value {
        let mut socket = UnixStream::connect(self.runtime.join("batond.sock")).unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        socket
            .write_all(&serde_json::to_vec(&value).unwrap())
            .unwrap();
        socket.write_all(b"\n").unwrap();
        let mut line = String::new();
        BufReader::new(socket).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    fn db(&self, readonly: bool) -> Connection {
        Connection::open_with_flags(
            self.runtime.join("baton.sqlite3"),
            if readonly {
                OpenFlags::SQLITE_OPEN_READ_ONLY
            } else {
                OpenFlags::SQLITE_OPEN_READ_WRITE
            },
        )
        .unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn result(reply: Value) -> Value {
    assert!(
        reply.get("error").is_none(),
        "generated fixture setup failed"
    );
    reply["result"].clone()
}
fn git(root: &Path, args: &[&str]) {
    let mut cmd = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.as_bytes().starts_with(b"GIT_") {
            cmd.env_remove(key);
        }
    }
    let output = cmd
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(output.status.success(), "own temporary Git fixture failed");
}
struct Fixture {
    server: Server,
    source: PathBuf,
    project: Value,
    task: Value,
    _root: tempfile::TempDir,
}
impl Fixture {
    fn new(repository: bool) -> Self {
        use std::os::unix::fs::PermissionsExt;
        // Darwin's default temporary parent can contain /var -> /private/var.
        // Keep the fixture on a short canonical parent; the production daemon
        // must continue to reject runtime paths containing symbolic links.
        let parent = Path::new("/tmp").canonicalize().unwrap();
        let root = tempfile::Builder::new()
            .prefix("ab-twbi-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(&parent)
            .unwrap();
        assert_eq!(root.path().parent(), Some(parent.as_path()));
        assert_eq!(root.path().canonicalize().unwrap().as_path(), root.path());
        assert_eq!(
            fs::symlink_metadata(root.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::create_dir(source.join("empty")).unwrap();
        fs::write(source.join("input"), "unchanged").unwrap();
        if repository {
            git(&source, &["init", "--initial-branch=main"]);
            git(&source, &["add", "."]);
            git(&source, &["commit", "-m", "generated"]);
        }
        let server = Server::start(&root.path().join("runtime"));
        let project=result(server.request(json!({"method":"projects.import","params":{"path":source,"name":"Generated source"},"idempotency_key":"import"})));
        let task=result(server.request(json!({"method":"tasks.create","params":{"project_id":project["id"],"goal":"Do not infer permission from this goal"},"idempotency_key":"task"})));
        Self {
            server,
            source,
            project,
            task,
            _root: root,
        }
    }
    fn read(&self, params: Value) -> Value {
        self.unchanged(json!({"method":"workspaces.resolve","params":params}))
    }
    fn unchanged(&self, command: Value) -> Value {
        let before = image(&self.server.db(true));
        let source = files(&self.source);
        let managed = files(&self.server.runtime.join("workspaces"));
        let reply = self.server.request(command);
        assert!(
            before == image(&self.server.db(true)),
            "read changed durable rows/sequence"
        );
        assert!(
            source == files(&self.source),
            "read changed source or empty directories"
        );
        assert!(
            managed == files(&self.server.runtime.join("workspaces")),
            "read created managed content"
        );
        reply
    }
}
type DatabaseImage = Vec<(String, Vec<Vec<SqlValue>>)>;
fn image(db: &Connection) -> DatabaseImage {
    let mut stmt=db.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_stat%' ORDER BY name").unwrap();
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    names
        .into_iter()
        .map(|name| {
            assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'));
            let mut stmt = db.prepare(&format!("SELECT * FROM {name}")).unwrap();
            let cols = stmt.column_count();
            let rows = stmt
                .query_map([], |r| {
                    (0..cols)
                        .map(|i| r.get::<_, SqlValue>(i))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            (name, rows)
        })
        .collect()
}
fn files(root: &Path) -> Vec<(PathBuf, bool, Vec<u8>)> {
    fn walk(root: &Path, current: &Path, out: &mut Vec<(PathBuf, bool, Vec<u8>)>) {
        let mut paths: Vec<_> = fs::read_dir(current)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        paths.sort();
        for p in paths {
            let m = fs::symlink_metadata(&p).unwrap();
            assert!(!m.file_type().is_symlink());
            let relative = p.strip_prefix(root).unwrap().into();
            if m.is_dir() {
                out.push((relative, true, vec![]));
                walk(root, &p, out);
            } else {
                out.push((relative, false, fs::read(p).unwrap()));
            }
        }
    }
    let mut out = vec![];
    walk(root, root, &mut out);
    out
}
fn error(reply: &Value, code: &str) {
    assert_eq!(reply["error"]["code"], code);
    let effect = if code == "OUTCOME_UNKNOWN" {
        "unknown"
    } else {
        "not_started"
    };
    assert_eq!(reply["error"]["side_effect_state"], effect);
    assert!(reply.get("result").is_none());
}

impl Fixture {
    fn preparation_frame(&self, task: &Value, key: &str) -> Value {
        let prep=result(self.read(json!({"task_id":task["id"],"expected_task_revision":task["revision"],"expected_project_revision":self.project["revision"],"host_id":"local"})));
        json!({"method":"workspaces.prepare_task_git","params":{"task_id":task["id"],"expected_spec_revision":prep["spec_revision"],"expected_project_revision":prep["project_revision"],"host_id":"local","expected_source_pin":{"canonical_root":prep["source"]["canonical_root"],"device":prep["source"]["device"],"inode":prep["source"]["inode"],"common_dir":prep["source"]["kind"]["common_dir"],"head":prep["source_head"]},"preparation_key":prep["preparation_key"]},"expected_revision":task["revision"],"idempotency_key":key})
    }
}
impl Server {
    fn send_without_reading_response(&self, mut command: Value) -> UnixStream {
        command["api_version"] = json!(1);
        command["request_id"] = json!("explicit-lost-preparation-response");
        command["token"] = json!(fs::read_to_string(self.runtime.join("session.token")).unwrap());
        let mut socket = UnixStream::connect(self.runtime.join("batond.sock")).unwrap();
        socket
            .write_all(&serde_json::to_vec(&command).unwrap())
            .unwrap();
        socket.write_all(b"\n").unwrap();
        socket.shutdown(std::net::Shutdown::Write).unwrap();
        // Keep the receive side alive until the separate durable-state read:
        // closing both sides now may discard an unread request on BSD sockets.
        // No reply is read, parsed or promoted to a successful result.
        socket
    }
}
fn no_execution(db: &Connection) {
    for name in [
        "execution_grants",
        "budgets",
        "attempts",
        "dispatch_intents",
        "sessions",
        "workspace_leases",
        "factory_runs",
    ] {
        let n: i64 = db
            .query_row(&format!("SELECT count(*) FROM {name}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(n, 0, "preparation minted execution authority");
    }
}

#[test]
fn two_tasks_get_distinct_real_worktrees_and_readonly_status_has_no_fresh_authority() {
    let f = Fixture::new(true);
    fs::write(f.source.join("input"), "dirty original\n").unwrap();
    let index = fs::read(f.source.join(".git/index")).unwrap();
    let original = f.preparation_frame(&f.task, "prepare");
    let first = result(f.server.request(original.clone()));
    assert_eq!(first["binding"]["state"], "ready");
    assert_eq!(first["identity_revalidated"], true);
    assert_eq!(first["execution_authorized"], false);
    assert!(first["original_receipt"]["source_pin"]["device"].is_string());
    let task2=result(f.server.request(json!({"method":"tasks.create","params":{"project_id":f.project["id"],"goal":"Second isolated task"},"idempotency_key":"task-2"})));
    let second = result(f.server.request(f.preparation_frame(&task2, "prepare")));
    assert_ne!(
        first["binding"]["receipt"]["workspace_id"],
        second["binding"]["receipt"]["workspace_id"]
    );
    let root1 = PathBuf::from(
        first["binding"]["managed_pin"]["canonical_root"]
            .as_str()
            .unwrap(),
    );
    let root2 = PathBuf::from(
        second["binding"]["managed_pin"]["canonical_root"]
            .as_str()
            .unwrap(),
    );
    fs::write(root1.join("output"), "first only").unwrap();
    assert!(!root2.join("output").exists());
    assert_eq!(
        fs::read_to_string(root2.join("input")).unwrap(),
        "unchanged"
    );
    assert_eq!(
        fs::read_to_string(f.source.join("input")).unwrap(),
        "dirty original\n"
    );
    assert_eq!(fs::read(f.source.join(".git/index")).unwrap(), index);
    let status =
        result(f.unchanged(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})));
    assert_eq!(
        status["code_workspace_binding"]["receipt"],
        first["original_receipt"]
    );
    assert!(status.get("identity_revalidated").is_none());
    assert!(status.get("execution_authorized").is_none());
    let replay = result(f.unchanged(original));
    assert_eq!(replay["original_receipt"], first["original_receipt"]);
    no_execution(&f.server.db(true));
}

#[test]
fn actual_dropped_write_reply_is_recovered_with_original_payload_after_task_revision_advanced() {
    let f = Fixture::new(true);
    let frame = f.preparation_frame(&f.task, "lost-key");
    let unread_response = f.server.send_without_reading_response(frame.clone());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let stored = loop {
        assert!(
            std::time::Instant::now() < deadline,
            "dropped write did not reach bounded durable state"
        );
        let task = result(
            f.server
                .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
        );
        if task.get("code_workspace_binding").is_some() {
            assert_eq!(task["code_workspace_binding"]["state"], "ready");
            break task;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert!(stored["revision"].as_u64().unwrap() > frame["expected_revision"].as_u64().unwrap());
    // Deliberately lose the original response before retrying its stable key.
    drop(unread_response);
    let recovered = result(f.unchanged(frame.clone()));
    assert_eq!(
        recovered["original_receipt"],
        stored["code_workspace_binding"]["receipt"]
    );
    assert_eq!(
        recovered["original_receipt"]["original_task_revision"],
        frame["expected_revision"]
    );
    let mut changed = frame.clone();
    changed["expected_revision"] = stored["revision"].clone();
    error(&f.unchanged(changed), "CONFIG_CONFLICT");
    let mut new_key = frame;
    new_key["idempotency_key"] = json!("another-key");
    error(&f.unchanged(new_key), "CONFIG_CONFLICT");
    no_execution(&f.server.db(true));
}

#[test]
fn malformed_stale_foreign_role_and_corrupt_source_requests_never_create_or_assign() {
    let f = Fixture::new(true);
    let original = f.preparation_frame(&f.task, "prepare");
    for field in [
        "expected_task_revision",
        "managed_root",
        "destination",
        "path",
        "grant_id",
        "execution_authorized",
    ] {
        let mut bad = original.clone();
        bad["params"][field] = json!("not-authority");
        error(&f.unchanged(bad), "INVALID_REQUEST");
    }
    for field in ["expected_revision", "idempotency_key"] {
        let mut bad = original.clone();
        bad.as_object_mut().unwrap().remove(field);
        error(&f.unchanged(bad), "INVALID_REQUEST");
        let mut bad = original.clone();
        bad[field] = Value::Null;
        error(&f.unchanged(bad), "INVALID_REQUEST");
    }
    for value in [json!(0), json!(1.0), json!(true)] {
        let mut bad = original.clone();
        bad["expected_revision"] = value;
        error(&f.unchanged(bad), "INVALID_REQUEST");
    }
    for value in [json!(1), json!("01"), json!("18446744073709551616")] {
        let mut bad = original.clone();
        bad["params"]["expected_source_pin"]["device"] = value;
        error(&f.unchanged(bad), "INVALID_REQUEST");
    }
    let mut stale = original.clone();
    stale["expected_revision"] = json!(99);
    error(&f.unchanged(stale), "STALE_REVISION");
    let mut stale = original.clone();
    stale["params"]["expected_spec_revision"] = json!(99);
    error(&f.unchanged(stale), "STALE_REVISION");
    let mut stale = original.clone();
    stale["params"]["expected_project_revision"] = json!(99);
    error(&f.unchanged(stale), "STALE_REVISION");
    let mut foreign = original.clone();
    foreign["params"]["host_id"] = json!("remote");
    error(&f.unchanged(foreign), "INVALID_REQUEST");
    let mut oversized = original.clone();
    oversized["params"]["task_id"] = json!("x".repeat(129));
    error(&f.unchanged(oversized), "INVALID_REQUEST");
    let mut oversized = original.clone();
    oversized["idempotency_key"] = json!("x".repeat(257));
    error(&f.unchanged(oversized), "INVALID_REQUEST");
    let other = result(f.server.request(json!({"method":"tasks.create","params":{"project_id":f.project["id"],"goal":"Other actual Task"},"idempotency_key":"other-task"})));
    let mut wrong_task = original.clone();
    wrong_task["params"]["task_id"] = other["id"].clone();
    error(&f.unchanged(wrong_task), "INPUT_CHANGED");
    let mut empty_key = original.clone();
    empty_key["idempotency_key"] = json!("");
    error(&f.unchanged(empty_key), "INVALID_REQUEST");
    let mut wide_key = original.clone();
    wide_key["idempotency_key"] = json!("界".repeat(100));
    error(&f.unchanged(wide_key), "INVALID_REQUEST");
    let before = image(&f.server.db(true));
    let source = files(&f.source);
    let managed = files(&f.server.runtime.join("workspaces"));
    for method in ["workspaces.prepare_task_git", "tasks.get"] {
        let denied=f.server.raw(json!({"api_version":1,"request_id":"scoped-cannot-prepare-or-read-owner","handle_id":"generated-not-owner","scoped_token":"generated-invalid","method":method,"params":original["params"],"expected_revision":1,"idempotency_key":"forbidden"}));
        error(&denied, "AUTH_REQUIRED");
    }
    let denied = f.server.raw(json!({"api_version":1,"request_id":"bad-owner-token","token":"generated-invalid","method":"workspaces.prepare_task_git","params":original["params"],"expected_revision":1,"idempotency_key":"prepare"}));
    error(&denied, "AUTH_REQUIRED");
    assert!(before == image(&f.server.db(true)));
    assert!(source == files(&f.source));
    assert!(managed == files(&f.server.runtime.join("workspaces")));
    f.server
        .db(false)
        .execute(
            "UPDATE projects SET inode=?1 WHERE id=?2",
            rusqlite::params![
                if f.project["identity"]["inode"] == 0 {
                    "1"
                } else {
                    "0"
                },
                f.project["id"].as_str().unwrap()
            ],
        )
        .unwrap();
    error(&f.unchanged(original), "INPUT_CHANGED");
    no_execution(&f.server.db(true));
    let archived = Fixture::new(true);
    let frame = archived.preparation_frame(&archived.task, "prepare");
    result(archived.server.request(json!({"method":"projects.archive","params":{"project_id":archived.project["id"]},"expected_revision":archived.project["revision"],"idempotency_key":"archive"})));
    error(&archived.unchanged(frame), "PERMISSION_DENIED");
    let directory = Fixture::new(false);
    let frame = json!({"method":"workspaces.prepare_task_git","params":{"task_id":directory.task["id"],"expected_spec_revision":directory.task["spec_revision"],"expected_project_revision":directory.project["revision"],"host_id":"local","expected_source_pin":{"canonical_root":directory.source,"device":directory.project["identity"]["device"].as_u64().unwrap().to_string(),"inode":directory.project["identity"]["inode"].as_u64().unwrap().to_string(),"common_dir":directory.source.join(".git"),"head":"a".repeat(40)},"preparation_key":"code-prep:".to_owned()+&"b".repeat(64)},"expected_revision":directory.task["revision"],"idempotency_key":"prepare"});
    error(&directory.unchanged(frame), "CAPABILITY_UNSUPPORTED");
    assert!(!directory.source.join(".git").exists());
}

#[test]
fn missing_ready_identity_and_restarted_unknown_intent_never_recheckout_or_report_fresh() {
    let mut f = Fixture::new(true);
    let original = f.preparation_frame(&f.task, "prepare");
    let view = result(f.server.request(original.clone()));
    let workspace = view["original_receipt"]["workspace_id"].as_str().unwrap();
    f.server.db(false).execute("UPDATE managed_workspaces SET document=json_set(document,'$.identity',json('null')) WHERE id=?1",[workspace]).unwrap();
    error(&f.unchanged(original.clone()), "OUTCOME_UNKNOWN");
    let status =
        result(f.unchanged(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})));
    assert_eq!(status["code_workspace_binding"]["state"], "ready");
    assert!(status.get("identity_revalidated").is_none());
    // Controlled raw corruption simulates a persisted unresolved marker; it is
    // never used to mint a successful receipt/candidate/stop or TaskAccepted.
    f.server.db(false).execute("UPDATE tasks SET document=json_set(document,'$.code_workspace_binding.state','outcome_unknown') WHERE id=?1",[f.task["id"].as_str().unwrap()]).unwrap();
    let runtime = f.server.runtime.clone();
    drop(f.server);
    f.server = Server::start(&runtime);
    error(&f.unchanged(original), "OUTCOME_UNKNOWN");
    let status =
        result(f.unchanged(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})));
    assert_eq!(status["code_workspace_binding"]["state"], "outcome_unknown");
    no_execution(&f.server.db(true));
}
