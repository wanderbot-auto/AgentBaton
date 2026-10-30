//! Real private Unix IPC/generated Git previews and refusal controls. These do
//! not claim Native OS approval, accepted Graph execution or a merged CAS pass.
use agentbaton_core::ipc::{call, Daemon};
use rusqlite::{types::Value as SqlValue, Connection, OpenFlags};
use serde_json::{json, Value};
use std::{
    fs,
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
    thread: Option<JoinHandle<agentbaton_core::Result<()>>>,
}
impl Server {
    fn start(runtime: PathBuf) -> Self {
        let daemon = Daemon::bind(&runtime).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || daemon.run(flag));
        Self {
            runtime,
            stop,
            thread: Some(thread),
        }
    }
    fn request(&self, frame: Value) -> Value {
        call(&self.runtime, frame).unwrap()
    }
    fn image(&self) -> Vec<(String, Vec<Vec<SqlValue>>)> {
        let db = Connection::open_with_flags(
            self.runtime.join("baton.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        db.execute_batch("PRAGMA query_only=ON; BEGIN;").unwrap();
        let version: u32 = db
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 17);
        let mut result = vec![];
        // Every retained row, including approvals/events/counters, must remain
        // byte-identical for a preview/refusal. No synthetic authority writes.
        let mut tables = db
            .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
            .unwrap();
        let names = tables
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for name in names {
            assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'));
            let mut statement = db
                .prepare(&format!("SELECT * FROM {name} ORDER BY rowid"))
                .unwrap();
            let columns = statement.column_count();
            let rows = statement
                .query_map([], |r| {
                    (0..columns)
                        .map(|i| r.get::<_, SqlValue>(i))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            result.push((name, rows));
        }
        result
    }
    fn finish(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let outcome = thread.join();
            if !std::thread::panicking() {
                outcome.unwrap().unwrap();
            }
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.finish();
    }
}
fn result(reply: Value) -> Value {
    assert!(
        reply.get("error").is_none(),
        "unexpected IPC code: {}",
        reply["error"]["code"]
    );
    reply["result"].clone()
}
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "user.name=Generated fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "generated Git command failed");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
struct Fixture {
    _directory: tempfile::TempDir,
    source: PathBuf,
    server: Server,
    project: Value,
    task: Value,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        fs::write(source.join("input.txt"), b"original input\n").unwrap();
        git(&source, &["add", "input.txt"]);
        git(&source, &["commit", "-m", "generated base"]);
        git(&source, &["branch", "integration"]);
        git(
            &source,
            &["symbolic-ref", "refs/heads/alias", "refs/heads/integration"],
        );
        let server = Server::start(root.join("runtime"));
        let project = result(server.request(json!({"method":"projects.import","params":{"path":source,"name":"Generated integration source"},"idempotency_key":"project"})));
        let task = result(server.request(json!({"method":"tasks.create","params":{"project_id":project["id"],"goal":"Preview does not authorize adoption"},"idempotency_key":"task"})));
        Self {
            _directory: directory,
            source,
            server,
            project,
            task,
        }
    }
    fn prepare(&self) -> Value {
        let view = result(self.server.request(json!({"method":"workspaces.resolve","params":{"task_id":self.task["id"],"expected_task_revision":self.task["revision"],"expected_project_revision":self.project["revision"],"host_id":"local"}})));
        result(self.server.request(json!({"method":"workspaces.prepare_task_git","params":{
            "task_id":self.task["id"],"expected_spec_revision":view["spec_revision"],"expected_project_revision":view["project_revision"],"host_id":"local",
            "expected_source_pin":{"canonical_root":view["source"]["canonical_root"],"device":view["source"]["device"],"inode":view["source"]["inode"],"common_dir":view["source"]["kind"]["common_dir"],"head":view["source_head"]},
            "preparation_key":view["preparation_key"]},"expected_revision":self.task["revision"],"idempotency_key":"prepare"})))
    }
}

#[test]
fn actual_owner_targets_are_bounded_direct_refs_without_durable_or_source_changes() {
    let mut f = Fixture::new();
    let ready = f.prepare();
    assert_eq!(ready["binding"]["state"], "ready");
    let base = git(&f.source, &["rev-parse", "HEAD"]);
    fs::write(f.source.join("input.txt"), b"unselected owner edit\n").unwrap();
    let index = fs::read(f.source.join(".git/index")).unwrap();
    let before = f.server.image();
    let first = result(f.server.request(
        json!({"method":"integration_targets.list","params":{"task_id":f.task["id"],"limit":1}}),
    ));
    assert_eq!(first.as_array().unwrap().len(), 1);
    assert_eq!(first[0]["target_ref"], "refs/heads/integration");
    assert_eq!(first[0]["head"], base);
    assert_eq!(first[0]["checked_out"], false);
    let second = result(f.server.request(json!({"method":"integration_targets.list","params":{"task_id":f.task["id"],"after_ref":first[0]["target_ref"],"limit":100}})));
    assert_eq!(second.as_array().unwrap().len(), 1);
    assert_eq!(second[0]["target_ref"], "refs/heads/main");
    assert_eq!(second[0]["checked_out"], true);
    assert_eq!(f.server.image(), before);
    assert_eq!(
        git(&f.source, &["symbolic-ref", "refs/heads/alias"]),
        "refs/heads/integration"
    );
    assert_eq!(git(&f.source, &["rev-parse", "HEAD"]), base);
    assert_eq!(fs::read(f.source.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read(f.source.join("input.txt")).unwrap(),
        b"unselected owner edit\n"
    );
    f.server.finish();
}

#[test]
fn unmanaged_targets_and_forged_start_parameters_cannot_create_integration_authority() {
    let mut f = Fixture::new();
    let before = f.server.image();
    let unsupported = f
        .server
        .request(json!({"method":"integration_targets.list","params":{"task_id":f.task["id"]}}));
    assert_eq!(unsupported["error"]["code"], "CAPABILITY_UNSUPPORTED");
    assert_eq!(unsupported["error"]["side_effect_state"], "not_started");
    let forged = f.server.request(json!({"method":"integrations.start","params":{"policy_id":"not-an-approved-policy","accepted":true},"expected_revision":1,"idempotency_key":"forged-authority"}));
    assert_eq!(forged["error"]["code"], "INVALID_REQUEST");
    assert_eq!(forged["error"]["side_effect_state"], "not_started");
    assert_eq!(f.server.image(), before);
    f.server.finish();
}
