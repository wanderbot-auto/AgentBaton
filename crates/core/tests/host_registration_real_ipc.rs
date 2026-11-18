//! Ordinary daemon and production Store over private Unix sockets.
use agentbaton_core::ipc::{call, Daemon, API_VERSION};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

const PAYLOAD: &str = include_str!("fixtures/host_registration/payload-v1.json");
const MISSING_ID: &str = "11111111-1111-4111-8111-111111111111";

struct Server {
    runtime: PathBuf,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<agentbaton_core::Result<()>>>,
}

impl Server {
    fn start(runtime: &Path) -> Self {
        let daemon = Daemon::bind(runtime).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        Self {
            runtime: runtime.into(),
            stop,
            worker: Some(std::thread::spawn(move || daemon.run(flag))),
        }
    }

    fn request(&self, command: Value) -> Value {
        call(&self.runtime, command).unwrap()
    }

    fn finish(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let joined = worker.join();
            if !std::thread::panicking() {
                joined.unwrap().unwrap();
                assert!(!self.runtime.join("batond.sock").exists());
                println!("ordinary daemon thread stopped and joined");
            }
        }
    }

    fn denied_registration(&self, params: Value) {
        let mut wire = UnixStream::connect(self.runtime.join("batond.sock")).unwrap();
        wire.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        wire.set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        serde_json::to_writer(
            &mut wire,
            &json!({
                "api_version": API_VERSION, "request_id": "invalid-owner",
                "token": "not-the-owner-token", "method": "hosts.register",
                "params": params, "idempotency_key": "denied-host",
            }),
        )
        .unwrap();
        wire.write_all(b"\n").unwrap();
        let mut line = String::new();
        BufReader::new(wire)
            .take(524_289)
            .read_line(&mut line)
            .unwrap();
        assert!(line.ends_with('\n') && line.len() <= 524_288);
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["request_id"], "invalid-owner");
        refusal(reply, "AUTH_REQUIRED");
    }

    fn db(&self, flags: OpenFlags) -> Connection {
        let db = Connection::open_with_flags(
            self.runtime.join("baton.sqlite3"),
            flags | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .unwrap();
        db.busy_timeout(Duration::from_secs(3)).unwrap();
        db
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.finish();
    }
}

fn private_runtime() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("ab19-wire-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in("/tmp")
        .unwrap()
}

fn result(reply: Value) -> Value {
    assert!(reply.get("error").is_none(), "{reply}");
    reply.get("result").unwrap().clone()
}

fn refusal(reply: Value, code: &str) {
    assert!(reply.get("result").is_none(), "{reply}");
    assert_eq!(reply["error"]["code"], code, "{reply}");
    assert_eq!(reply["error"]["side_effect_state"], "not_started");
}

fn background(server: &Server) -> Value {
    result(server.request(json!({"method":"system.background","params":{}})))
}

fn snapshot(server: &Server) -> Value {
    result(server.request(json!({"method":"system.snapshot","params":{}})))
}

fn register(server: &Server, payload: &Value, key: &str) -> Value {
    result(server.request(json!({
        "method":"hosts.register","params":payload,"idempotency_key":key
    })))
}

fn find(server: &Server, key: &str) -> Value {
    let lookup = result(server.request(json!({
        "method":"hosts.find_registration","params":{"key":key}
    })));
    assert_eq!(lookup["lookup_version"], 1);
    assert_eq!(lookup["key"], key);
    lookup["receipt"].clone()
}

fn counts(server: &Server) -> (i64, i64, i64) {
    let db = server.db(OpenFlags::SQLITE_OPEN_READ_ONLY);
    db.query_row(
        "SELECT (SELECT count(*) FROM host_registrations),
                (SELECT count(*) FROM idempotency WHERE scope='hosts.register:v1'),
                (SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='host.registered')",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap()
}

#[test]
fn normal_schema19_background_and_host_lifecycle_over_real_socket() {
    let directory = private_runtime();
    let runtime = directory.path().canonicalize().unwrap();
    let mut server = Server::start(&runtime);
    let initial = background(&server);
    let before = snapshot(&server);
    assert_eq!(initial["database_schema_version"], 19);
    assert_eq!(initial["api_version"], 1);
    assert_eq!(initial["store_recovery"], "completed_on_start");
    assert_eq!(initial["owned_provider_workers"], false);
    assert_eq!(initial["sequence"], before["sequence"]);
    assert_eq!(before["database_schema_version"], 19);
    assert_eq!(before["api_version"], 1);
    assert_eq!(background(&server), initial);

    let metadata: Value = serde_json::from_str(PAYLOAD).unwrap();
    server.denied_registration(metadata.clone());
    assert!(find(&server, "denied-host").is_null());
    assert_eq!(counts(&server), (0, 0, 0));
    assert_eq!(snapshot(&server), before);

    let first = register(&server, &metadata, "first-host");
    let second = register(&server, &metadata, "second-host");
    assert_ne!(first["host_id"], second["host_id"]);
    assert_eq!(first["receipt_version"], 1);
    assert_eq!(first["revision"], 1);
    assert_eq!(find(&server, "first-host"), first);
    let get = result(server.request(json!({
        "method":"hosts.get","params":{"host_id":first["host_id"]}
    })));
    assert_eq!(get["record"]["id"], first["host_id"]);
    assert_eq!(get["record"]["created_sequence"], first["created_sequence"]);
    assert_eq!(get["record"]["creation_facts"]["execution_enabled"], false);
    assert_eq!(
        get["record"]["creation_facts"]["remote_connection"],
        "not_contacted"
    );

    let page = result(server.request(json!({"method":"hosts.list","params":{"limit":1}})));
    assert_eq!(page["hosts"], json!([get]));
    assert_eq!(page["next_created_sequence"], first["created_sequence"]);
    let last = result(server.request(json!({
        "method":"hosts.list","params":{"limit":1,"after_created_sequence":page["next_created_sequence"]}
    })));
    assert_eq!(last["hosts"].as_array().unwrap().len(), 1);
    assert_eq!(last["hosts"][0]["record"]["id"], second["host_id"]);
    assert!(last["next_created_sequence"].is_null());

    let after = snapshot(&server);
    assert_eq!(
        after["sequence"].as_u64().unwrap(),
        before["sequence"].as_u64().unwrap() + 2
    );
    assert_eq!(after["tasks"], before["tasks"]);
    assert_eq!(counts(&server), (2, 2, 2));
    server.finish();

    let mut reopened = Server::start(&runtime);
    let identity = background(&reopened);
    assert_ne!(identity["instance_id"], initial["instance_id"]);
    assert_eq!(identity["database_schema_version"], 19);
    assert_eq!(identity["sequence"], after["sequence"]);
    assert_eq!(snapshot(&reopened), after);
    assert_eq!(find(&reopened, "first-host"), first);
    assert_eq!(register(&reopened, &metadata, "first-host"), first);
    assert_eq!(counts(&reopened), (2, 2, 2));
    assert_eq!(snapshot(&reopened), after);
    reopened.finish();
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "user.name=Reader fixture",
            "-c",
            "user.email=reader@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generated Git command failed: {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn project_command(path: &Path, key: &str) -> Value {
    json!({"method":"projects.import","params":{"path":path,"name":"Generated Git project"},"idempotency_key":key})
}

#[test]
fn git_project_and_target_reads_keep_the_fence_fail_closed() {
    let directory = private_runtime();
    let runtime = directory.path().canonicalize().unwrap();
    let root = runtime.join("source");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "--initial-branch=main"]);
    fs::write(root.join("input"), b"committed\n").unwrap();
    git(&root, &["add", "input"]);
    git(&root, &["commit", "-qm", "baseline"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    let mut server = Server::start(&runtime);

    let project = result(server.request(project_command(&root, "import-git")));
    let task = result(server.request(json!({
        "method":"tasks.create","params":{"project_id":project["id"],"goal":"Read Git targets"},
        "idempotency_key":"create-task"
    })));
    let targets = json!({"method":"integration_targets.list","params":{"task_id":task["id"]}});
    refusal(server.request(targets.clone()), "CAPABILITY_UNSUPPORTED");
    let view = result(server.request(json!({
        "method":"workspaces.resolve","params":{
            "task_id":task["id"],"expected_task_revision":task["revision"],
            "expected_project_revision":project["revision"],"host_id":"local"
        }
    })));
    let ready = result(server.request(json!({
        "method":"workspaces.prepare_task_git","params":{
            "task_id":task["id"],"expected_spec_revision":view["spec_revision"],
            "expected_project_revision":view["project_revision"],"host_id":"local",
            "expected_source_pin":{
                "canonical_root":view["source"]["canonical_root"],
                "device":view["source"]["device"],"inode":view["source"]["inode"],
                "common_dir":view["source"]["kind"]["common_dir"],"head":view["source_head"]
            },
            "preparation_key":view["preparation_key"]
        },
        "expected_revision":task["revision"],"idempotency_key":"prepare-git"
    })));
    assert_eq!(ready["binding"]["state"], "ready");
    let before = snapshot(&server);
    let branches = result(server.request(targets));
    assert_eq!(branches.as_array().unwrap().len(), 1);
    assert_eq!(branches[0]["target_ref"], "refs/heads/main");
    assert_eq!(branches[0]["head"], head);
    assert_eq!(branches[0]["checked_out"], true);
    assert_eq!(snapshot(&server), before);
    let db = server.db(OpenFlags::SQLITE_OPEN_READ_WRITE);
    // Deliberate local corruption control. Only this fixture connection can
    // inject an orphan fence, after normal startup. The production reader must
    // authenticate stored fence rows before it can treat the repo as unfenced.
    db.execute_batch("PRAGMA foreign_keys=OFF;
        INSERT INTO integration_ref_fences VALUES('orphan','refs/heads/main','missing-run',1,'held','{}');").unwrap();
    refusal(
        server.request(project_command(&root, "orphan-control")),
        "NOT_FOUND",
    );
    assert_eq!(snapshot(&server), before);
    assert_eq!(
        db.query_row("SELECT count(*) FROM integration_ref_fences", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM idempotency WHERE key='orphan-control'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    db.execute(
        "DELETE FROM integration_ref_fences WHERE integration_id='missing-run'",
        [],
    )
    .unwrap();
    refusal(
        server.request(project_command(&root, "no-fence-duplicate")),
        "CONFIG_CONFLICT",
    );
    assert_eq!(snapshot(&server), before);
    server.finish();
}

#[test]
fn live_readers_reject_18_and_20_without_recovery_or_writes() {
    let directory = private_runtime();
    let runtime = directory.path().canonicalize().unwrap();
    let mut server = Server::start(&runtime);
    let task = json!({"method":"integration_targets.list","params":{"task_id":MISSING_ID}});
    let history = json!({"method":"factory_child_goals.list","params":{"scope_id":MISSING_ID,"task_id":MISSING_ID}});
    refusal(server.request(task.clone()), "NOT_FOUND");
    let before = snapshot(&server);
    let db = server.db(OpenFlags::SQLITE_OPEN_READ_WRITE);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        19
    );
    let schema_cookie: u32 = db
        .pragma_query_value(None, "schema_version", |r| r.get(0))
        .unwrap();
    // Inject only an unexpected header in an already-open private runtime.
    // This diagnoses live readers and does not simulate a historical producer.
    for version in [18, 20] {
        db.pragma_update(None, "user_version", version).unwrap();
        refusal(
            server.request(json!({"method":"system.background","params":{}})),
            "VERSION_UNSUPPORTED",
        );
        refusal(server.request(task.clone()), "INPUT_CHANGED");
        let error = server.request(history.clone());
        assert_eq!(
            error["error"]["message"],
            "Readonly history database schema changed"
        );
        refusal(error, "INPUT_CHANGED");
        let mut expected = before.clone();
        expected["database_schema_version"] = json!(version);
        assert_eq!(snapshot(&server), expected);
        assert_eq!(counts(&server), (0, 0, 0));
        assert_eq!(
            db.pragma_query_value(None, "schema_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            schema_cookie
        );
    }
    db.pragma_update(None, "user_version", 19).unwrap();
    assert_eq!(background(&server)["database_schema_version"], 19);
    assert_eq!(snapshot(&server), before);
    server.finish();
}
