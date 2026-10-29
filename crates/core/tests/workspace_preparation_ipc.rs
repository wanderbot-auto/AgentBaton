//! Actual local Unix IPC with generated TempDir sources; no Provider or Native dialog.
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
        let root = tempfile::tempdir().unwrap();
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
    fn selection(&self) -> Value {
        json!({"task_id":self.task["id"],"expected_task_revision":self.task["revision"],"expected_project_revision":self.project["revision"],"host_id":"local"})
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
    assert_eq!(reply["error"]["side_effect_state"], "not_started");
    assert!(reply.get("result").is_none());
}

#[test]
fn real_git_directory_and_personal_reads_are_metadata_only_and_repeatable() {
    for repository in [false, true] {
        let f = Fixture::new(repository);
        let reply = f.read(f.selection());
        let value = result(reply.clone());
        assert_eq!(value["task_id"], f.task["id"]);
        assert_eq!(value["project_id"], f.project["id"]);
        assert_eq!(
            value["state"],
            if repository {
                "needs_git_workspace"
            } else {
                "needs_dataset_selection"
            }
        );
        assert_eq!(
            value["source"]["device"],
            f.project["identity"]["device"]
                .as_u64()
                .unwrap()
                .to_string()
        );
        assert_eq!(
            value["source"]["inode"],
            f.project["identity"]["inode"].as_u64().unwrap().to_string()
        );
        assert_eq!(value["execution_authorized"], false);
        assert_eq!(value["creates_workspace"], false);
        assert_eq!(value["candidates"], json!([]));
        assert_eq!(f.read(f.selection())["result"], value);
        assert_eq!(f.read(f.selection())["revision"], reply["revision"]);
        let task=result(f.server.request(json!({"method":"tasks.create","params":{"goal":"Personal draft"},"idempotency_key":"personal"})));
        let personal=result(f.read(json!({"task_id":task["id"],"expected_task_revision":task["revision"],"expected_project_revision":0,"host_id":"local"})));
        assert_eq!(personal["state"], "needs_registered_project");
        assert_eq!(personal["project_id"], "local");
        for field in [
            "source",
            "source_head",
            "project_revision",
            "preparation_key",
        ] {
            assert!(personal[field].is_null());
        }
    }
}

#[test]
fn stale_foreign_malformed_and_write_envelopes_cannot_change_any_rows() {
    let f = Fixture::new(false);
    for (field, value, code) in [
        ("task_id", json!("missing-generated-task"), "NOT_FOUND"),
        ("host_id", json!("other-host"), "CAPABILITY_UNSUPPORTED"),
        ("expected_task_revision", json!(999), "STALE_REVISION"),
        ("expected_project_revision", json!(999), "STALE_REVISION"),
        ("task_id", json!("x".repeat(129)), "INVALID_REQUEST"),
        ("expected_task_revision", json!(1.0), "INVALID_REQUEST"),
        ("expected_project_revision", json!(true), "INVALID_REQUEST"),
        ("host_id", Value::Null, "INVALID_REQUEST"),
    ] {
        let mut p = f.selection();
        p[field] = value;
        error(&f.read(p), code);
    }
    for field in [
        "path",
        "destination",
        "source",
        "grant_id",
        "execution_authorized",
    ] {
        let mut p = f.selection();
        p[field] = json!("not-authority");
        error(&f.read(p), "INVALID_REQUEST");
    }
    let other = result(f.server.request(json!({"method":"tasks.create","params":{"project_id":f.project["id"],"goal":"Another actual task"},"idempotency_key":"other-task"})));
    let cancelled = result(f.server.request(json!({"method":"tasks.cancel","params":{"task_id":other["id"]},"expected_revision":other["revision"]})));
    let mut foreign = f.selection();
    foreign["task_id"] = cancelled["id"].clone();
    error(&f.read(foreign), "STALE_REVISION");
    let mut missing = f.selection();
    missing.as_object_mut().unwrap().remove("host_id");
    error(&f.read(missing), "INVALID_REQUEST");
    for (field, value) in [
        ("expected_revision", json!(1)),
        ("idempotency_key", json!("forbidden")),
    ] {
        let mut c = json!({"method":"workspaces.resolve","params":f.selection()});
        c[field] = value;
        error(&f.unchanged(c), "INVALID_REQUEST");
    }
    let mut nulls = json!({"method":"workspaces.resolve","params":f.selection()});
    nulls["expected_revision"] = Value::Null;
    nulls["idempotency_key"] = Value::Null;
    assert!(f.unchanged(nulls).get("result").is_some());
    result(f.server.request(json!({"method":"projects.archive","params":{"project_id":f.project["id"]},"expected_revision":f.project["revision"],"idempotency_key":"archive"})));
    let mut archived = f.selection();
    archived["expected_project_revision"] = json!(f.project["revision"].as_u64().unwrap() + 1);
    error(&f.read(archived), "PERMISSION_DENIED");
}

#[test]
fn corrupt_project_pin_foreign_task_document_and_oversized_document_fail_closed() {
    for case in ["project_pin", "foreign_task_document", "oversized_project"] {
        let f = Fixture::new(false);
        let db = f.server.db(false);
        match case {
            "project_pin" => {
                db.execute(
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
            }
            "foreign_task_document" => {
                db.execute("UPDATE tasks SET document=json_set(document,'$.id','foreign-generated-task') WHERE id=?1",[f.task["id"].as_str().unwrap()]).unwrap();
            }
            _ => {
                db.execute(
                    "UPDATE projects SET document=?1 WHERE id=?2",
                    rusqlite::params!["x".repeat(262145), f.project["id"].as_str().unwrap()],
                )
                .unwrap();
            }
        }
        drop(db);
        let reply = f.read(f.selection());
        error(
            &reply,
            if case == "oversized_project" {
                "RESOURCE_LIMIT"
            } else {
                "INPUT_CHANGED"
            },
        );
        if case == "project_pin" {
            assert_eq!(reply["error"]["details_ref"], "code-workspace-project-pin");
        }
    }
}

#[test]
fn companion_role_and_wrong_owner_token_cannot_obtain_source_reads() {
    let f = Fixture::new(false);
    let before = image(&f.server.db(true));
    let source = files(&f.source);
    let managed = files(&f.server.runtime.join("workspaces"));
    let denied=f.server.raw(json!({"api_version":1,"request_id":"scoped-owner-read-denied","handle_id":"not-an-owner","scoped_token":"generated-not-a-credential","method":"workspaces.resolve","params":f.selection()}));
    error(&denied, "AUTH_REQUIRED");
    let owner_denied=f.server.raw(json!({"api_version":1,"request_id":"wrong-owner-token","token":"generated-invalid","method":"workspaces.resolve","params":f.selection()}));
    error(&owner_denied, "AUTH_REQUIRED");
    assert!(before == image(&f.server.db(true)));
    assert!(source == files(&f.source));
    assert!(managed == files(&f.server.runtime.join("workspaces")));
    // The method allowlist is checked before handle authentication. No handle/token
    // is created merely for this denial control; live scoped-handle tests remain separate.
}
