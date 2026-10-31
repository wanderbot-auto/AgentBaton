//! Real Owner Unix IPC directory reads and refusal/no-write controls. Git is
//! used only for a generated repository fixture; no Provider or terminal runs.
use agentbaton_core::ipc::{call, Daemon};
use agentbaton_core::{pin_open_context, WorkspaceOpenContext};
use agentbaton_workspaces::{WorkspaceIdentity, WorkspaceKind};
use rusqlite::{types::Value as SqlValue, Connection, OpenFlags};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{symlink, MetadataExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};
use uuid::Uuid;

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
        Self {
            runtime: runtime.into(),
            stop,
            thread: Some(std::thread::spawn(move || daemon.run(run).unwrap())),
        }
    }
    fn request(&self, frame: Value) -> Value {
        call(&self.runtime, frame).unwrap()
    }
    fn raw(&self, frame: Value) -> Value {
        self.raw_text(&frame.to_string())
    }
    fn raw_text(&self, frame: &str) -> Value {
        let mut socket = UnixStream::connect(self.runtime.join("batond.sock")).unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        socket.write_all(frame.as_bytes()).unwrap();
        socket.write_all(b"\n").unwrap();
        let mut response = String::new();
        BufReader::new(socket).read_line(&mut response).unwrap();
        serde_json::from_str(&response).unwrap()
    }
    fn db(&self, read_only: bool) -> Connection {
        Connection::open_with_flags(
            self.runtime.join("baton.sqlite3"),
            if read_only {
                OpenFlags::SQLITE_OPEN_READ_ONLY
            } else {
                OpenFlags::SQLITE_OPEN_READ_WRITE
            },
        )
        .unwrap()
    }
    fn unchanged(&self, frame: Value) -> Value {
        let before = image(&self.db(true));
        let counts = execution_counts(&self.db(true));
        let reply = self.request(frame);
        assert!(
            before == image(&self.db(true)),
            "Owner directory read/refusal changed a persisted row"
        );
        assert_eq!(counts, execution_counts(&self.db(true)));
        assert!(
            counts.iter().all(|(_, count)| *count == 0),
            "Fixture must have zero task-execution/approval domains"
        );
        reply
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let joined = thread.join();
            if !std::thread::panicking() {
                joined.unwrap();
            }
        }
    }
}
fn image(db: &Connection) -> Vec<(String, Vec<Vec<SqlValue>>)> {
    db.execute_batch("PRAGMA query_only=ON; BEGIN;").unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        17
    );
    let mut names = db
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap();
    let names = names
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    let mut image = vec![];
    for name in names {
        assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        let mut statement = db
            .prepare(&format!("SELECT * FROM {name} ORDER BY rowid"))
            .unwrap();
        let columns = statement.column_count();
        let rows = statement
            .query_map([], |row| {
                (0..columns)
                    .map(|i| row.get::<_, SqlValue>(i))
                    .collect::<std::result::Result<Vec<_>, _>>()
            })
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        image.push((name, rows));
    }
    image
}
fn execution_counts(db: &Connection) -> Vec<(&'static str, i64)> {
    [
        "tasks",
        "attempts",
        "sessions",
        "session_turns",
        "execution_grants",
        "approval_proposals",
        "connections",
        "factory_runs",
        "factory_attempts",
        "factory_task_bindings",
        "factory_budget_reservations",
    ]
    .into_iter()
    .map(|name| {
        (
            name,
            db.query_row(&format!("SELECT count(*) FROM {name}"), [], |r| r.get(0))
                .unwrap(),
        )
    })
    .collect()
}
fn result(reply: Value) -> Value {
    assert!(reply.get("error").is_none(), "Fixture operation failed");
    reply["result"].clone()
}
fn refusal(reply: &Value, code: &str) {
    assert_eq!(reply["error"]["code"], code);
    assert_eq!(reply["error"]["side_effect_state"], "not_started");
}
fn frame(selection: Value) -> Value {
    json!({"method":"workspace_open.context","params":{"selection":selection}})
}
fn git(root: &Path, args: &[&str]) {
    #[cfg(target_os = "macos")]
    let mut command = Command::new(agentbaton_workspaces::MACOS_SYSTEM_GIT);
    #[cfg(not(target_os = "macos"))]
    let mut command = Command::new("git");
    agentbaton_workspaces::configure_local_git_environment(&mut command);
    let output = command
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgSign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Generated local Git fixture failed"
    );
}
struct Fixture {
    server: Server,
    source: PathBuf,
    project: Value,
    root: tempfile::TempDir,
}
impl Fixture {
    fn new(repository: bool) -> Self {
        // Keep the real Unix socket within the Darwin pathname limit. Mac /var
        // or /tmp aliases are canonicalized in fixture construction, not guards.
        let root = tempfile::Builder::new()
            .prefix("wo-")
            .tempdir_in("/tmp")
            .unwrap();
        let base = root.path().canonicalize().unwrap();
        let container = base.join("project");
        fs::create_dir(&container).unwrap();
        let source = container.join("source 空间\n-directory");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("input"), b"generated\n").unwrap();
        if repository {
            git(&source, &["init", "--initial-branch=main"]);
            git(&source, &["add", "."]);
            git(&source, &["commit", "-m", "generated"]);
        }
        let server = Server::start(&base.join("runtime"));
        let project=result(server.request(json!({"method":"projects.import","params":{"path":source,"name":"Directory fixture"},"idempotency_key":"project"})));
        Self {
            server,
            source,
            project,
            root,
        }
    }
    fn source_selection(&self) -> Value {
        json!({"kind":"project_source","project_id":self.project["id"],"expected_project_revision":self.project["revision"]})
    }
    fn managed_selection(&self, workspace: &Value) -> Value {
        json!({"kind":"managed_workspace","project_id":self.project["id"],"expected_project_revision":self.project["revision"],"workspace_id":workspace["id"],"expected_workspace_revision":workspace["revision"]})
    }
    fn context(&self, selection: Value) -> WorkspaceOpenContext {
        let raw = result(self.server.unchanged(frame(selection.clone())));
        assert_eq!(raw["selection"], selection);
        assert!(raw["directory_identity"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["device"].is_string() && row["inode"].is_string()));
        let context: WorkspaceOpenContext = serde_json::from_value(raw).unwrap();
        pin_open_context(&context).unwrap().recheck().unwrap();
        context
    }
    fn directory_record(&self) -> Value {
        let path = self.root.path().canonicalize().unwrap().join("ready-data");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("editable-data"), b"local").unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let identity = WorkspaceIdentity {
            canonical_root: path.clone(),
            device: metadata.dev(),
            inode: metadata.ino(),
            kind: WorkspaceKind::Directory,
        };
        // A typed ready data registry fixture only. This exercises the real
        // Owner IPC read, not a claim that copying/approving data ran here.
        let value = json!({"id":Uuid::new_v4().to_string(),"project_id":self.project["id"],"identity":identity,"root":path,"base_revision":"fixture-data","created_at":self.project["created_at"],"state":"ready","revision":1});
        save_record(&self.server.db(false), &value);
        value
    }
}
fn save_record(db: &Connection, value: &Value) {
    db.execute("INSERT INTO managed_workspaces(id,project_id,document,state,idempotency_key,payload,root) VALUES(?1,?2,?3,?4,?1,'{}',?5) ON CONFLICT(id) DO UPDATE SET document=excluded.document,state=excluded.state,root=excluded.root",rusqlite::params![value["id"].as_str().unwrap(),value["project_id"].as_str().unwrap(),value.to_string(),value["state"].as_str().unwrap(),value["root"].as_str().unwrap()]).unwrap();
}

#[test]
fn owner_unix_read_returns_full_opaque_identity_chain_without_any_writes() {
    let f = Fixture::new(false);
    let context = f.context(f.source_selection());
    assert_eq!(context.canonical_path, f.source.to_str().unwrap());
    assert_eq!(
        context.directory_identity.len(),
        f.source.components().count()
    );
    let metadata = fs::metadata(&f.source).unwrap();
    let leaf = context.directory_identity.last().unwrap();
    assert_eq!(leaf.device, metadata.dev().to_string());
    assert_eq!(leaf.inode, metadata.ino().to_string());
}

#[test]
fn owner_reads_real_ready_dirty_git_and_typed_ready_data_without_execution() {
    let f = Fixture::new(true);
    let workspace=result(f.server.request(json!({"method":"workspaces.create","params":{"project_id":f.project["id"],"expected_head":f.project["identity"]["kind"]["head"]},"expected_revision":f.project["revision"],"idempotency_key":"managed"})));
    fs::write(
        Path::new(workspace["root"].as_str().unwrap()).join("dirty edit"),
        b"uncommitted\n",
    )
    .unwrap();
    let context = f.context(f.managed_selection(&workspace));
    assert_eq!(context.canonical_path, workspace["root"].as_str().unwrap());
    let data = f.directory_record();
    let context = f.context(f.managed_selection(&data));
    assert_eq!(context.canonical_path, data["root"].as_str().unwrap());
    // Source dirtiness is also irrelevant to a directory-only operation.
    fs::write(f.source.join("input"), b"changed locally").unwrap();
    f.context(f.source_selection());
}

#[test]
fn malformed_remote_cas_stale_missing_and_foreign_reads_are_refused_without_writes() {
    let f = Fixture::new(false);
    let original = f.source_selection();
    for (field, value) in [
        ("path", json!("/untrusted")),
        ("host_id", json!("remote")),
        ("grant_id", json!("untrusted")),
        ("terminal", json!("macos_terminal")),
        ("expected_project_revision", json!(1.0)),
        ("expected_project_revision", json!(0)),
        ("project_id", json!("local")),
    ] {
        let mut selected = original.clone();
        selected[field] = value;
        refusal(&f.server.unchanged(frame(selected)), "INVALID_REQUEST");
    }
    let mut extra = frame(original.clone());
    extra["params"]["path"] = json!("/untrusted");
    refusal(&f.server.unchanged(extra), "INVALID_REQUEST");
    for (field, value) in [
        ("expected_revision", json!(1)),
        ("idempotency_key", json!("no-writes")),
    ] {
        let mut extra = frame(original.clone());
        extra[field] = value;
        refusal(&f.server.unchanged(extra), "INVALID_REQUEST");
    }
    let mut stale = original.clone();
    stale["expected_project_revision"] = json!(2);
    refusal(&f.server.unchanged(frame(stale)), "STALE_REVISION");
    let mut absent = original;
    absent["project_id"] = json!(Uuid::new_v4().to_string());
    refusal(&f.server.unchanged(frame(absent)), "NOT_FOUND");
    let workspace = f.directory_record();
    let other = f.root.path().canonicalize().unwrap().join("other");
    fs::create_dir(&other).unwrap();
    let other=result(f.server.request(json!({"method":"projects.import","params":{"path":other,"name":"Other"},"idempotency_key":"other"})));
    let mut selected = f.managed_selection(&workspace);
    selected["project_id"] = other["id"].clone();
    selected["expected_project_revision"] = other["revision"].clone();
    refusal(&f.server.unchanged(frame(selected)), "PERMISSION_DENIED");
}

#[test]
fn owner_read_refuses_pending_unknown_missing_identity_and_changed_leaf() {
    let f = Fixture::new(false);
    let original = f.directory_record();
    let selected = f.managed_selection(&original);
    for state in ["creating", "outcome_unknown"] {
        let mut row = original.clone();
        row["state"] = json!(state);
        save_record(&f.server.db(false), &row);
        refusal(
            &f.server.unchanged(frame(selected.clone())),
            "INPUT_CHANGED",
        );
    }
    let mut row = original.clone();
    row["identity"] = Value::Null;
    save_record(&f.server.db(false), &row);
    refusal(
        &f.server.unchanged(frame(selected.clone())),
        "INPUT_CHANGED",
    );
    save_record(&f.server.db(false), &original);
    let mut stale = selected.clone();
    stale["expected_workspace_revision"] = json!(2);
    refusal(&f.server.unchanged(frame(stale)), "STALE_REVISION");
    let path = Path::new(original["root"].as_str().unwrap());
    let prior = path.with_file_name("old-data");
    fs::rename(path, &prior).unwrap();
    symlink(&prior, path).unwrap();
    refusal(
        &f.server.unchanged(frame(selected.clone())),
        "INPUT_CHANGED",
    );
    fs::remove_file(path).unwrap();
    fs::create_dir(path).unwrap();
    refusal(&f.server.unchanged(frame(selected)), "INPUT_CHANGED");
}

#[test]
fn owner_read_refuses_an_ancestor_alias_and_a_replaced_source_without_mutation() {
    let f = Fixture::new(false);
    let selected = f.source_selection();
    let base = f.source.parent().unwrap();
    let moved = base.join("moved");
    fs::rename(&f.source, &moved).unwrap();
    symlink(&moved, &f.source).unwrap();
    refusal(
        &f.server.unchanged(frame(selected.clone())),
        "INPUT_CHANGED",
    );
    fs::remove_file(&f.source).unwrap();
    fs::create_dir(&f.source).unwrap();
    refusal(&f.server.unchanged(frame(selected)), "INPUT_CHANGED");
    let f = Fixture::new(false);
    let selected = f.source_selection();
    let base = f.source.parent().unwrap();
    let old = base.with_file_name("old-project");
    fs::rename(base, &old).unwrap();
    symlink(&old, base).unwrap();
    // The runtime is a sibling outside the changed project ancestor. This is
    // the actual Core NOFOLLOW refusal, not a broken client/socket fixture.
    refusal(&f.server.unchanged(frame(selected)), "INPUT_CHANGED");
}

#[test]
fn scoped_companion_and_wrong_owner_token_cannot_obtain_directory_context() {
    let f = Fixture::new(false);
    let before = image(&f.server.db(true));
    let counts = execution_counts(&f.server.db(true));
    let denied=f.server.raw(json!({"api_version":1,"request_id":"scoped-directory-denied","handle_id":"generated-not-owner","scoped_token":"generated-not-owner","method":"workspace_open.context","params":{"selection":f.source_selection()}}));
    refusal(&denied, "AUTH_REQUIRED");
    let denied=f.server.raw(json!({"api_version":1,"request_id":"owner-token-denied","token":"generated-not-owner","method":"workspace_open.context","params":{"selection":f.source_selection()}}));
    refusal(&denied, "AUTH_REQUIRED");
    assert!(before == image(&f.server.db(true)));
    assert_eq!(counts, execution_counts(&f.server.db(true)));
    assert!(counts.iter().all(|(_, count)| *count == 0));
}

#[test]
fn raw_owner_duplicate_keys_at_all_three_levels_and_null_write_fields_are_refused() {
    let f = Fixture::new(false);
    let token =
        json!(fs::read_to_string(f.server.runtime.join("session.token")).unwrap()).to_string();
    let project = f.project["id"].to_string();
    let selected = f.source_selection().to_string();
    let prefix=format!("{{\"api_version\":1,\"request_id\":\"duplicate-refusal\",\"token\":{token},\"method\":\"workspace_open.context\",");
    let frames=[
        format!("{prefix}\"params\":{{\"selection\":{selected}}},\"params\":{{\"selection\":{selected}}}}}"),
        format!("{prefix}\"params\":{{\"selection\":{selected},\"selection\":{selected}}}}}"),
        format!("{prefix}\"params\":{{\"selection\":{{\"kind\":\"project_source\",\"project_id\":{project},\"project_id\":{project},\"expected_project_revision\":1}}}}}}"),
        format!("{prefix}\"params\":{{\"selection\":{{\"kind\":\"project_source\",\"project_id\":{project},\"expected_project_revision\":1,\"expected_project_revision\":1}}}}}}"),
        format!("{prefix}\"params\":{{\"selection\":{selected}}},\"expected_revision\":null}}"),
        format!("{prefix}\"params\":{{\"selection\":{selected}}},\"idempotency_key\":null}}"),
    ];
    let before = image(&f.server.db(true));
    let counts = execution_counts(&f.server.db(true));
    for raw in frames {
        refusal(&f.server.raw_text(&raw), "INVALID_REQUEST");
    }
    assert!(before == image(&f.server.db(true)));
    assert_eq!(counts, execution_counts(&f.server.db(true)));
    assert!(counts.iter().all(|(_, count)| *count == 0));
    f.context(f.source_selection());
}
