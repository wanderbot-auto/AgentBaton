//! Exercises the real batond binary and OS signals. The provider is a generated
//! local fixture, never a vendor CLI, private identity, network or model call.
use agentbaton_core::{
    ipc::call,
    storage::{Store, TaskState},
};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct OwnedDaemon {
    child: Child,
    runtime: PathBuf,
}
impl OwnedDaemon {
    fn start(root: &Path) -> Self {
        let runtime = root.join("runtime");
        let home = root.join("test-home");
        fs::create_dir_all(&home).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_batond"))
            .current_dir(root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", home)
            .env("BATON_RUNTIME_DIR", &runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut daemon = Self { child, runtime };
        let began = Instant::now();
        loop {
            if let Ok(value) = call(
                &daemon.runtime,
                json!({"method":"system.snapshot","params":{}}),
            ) {
                if value.get("result").is_some() {
                    break;
                }
            }
            assert!(
                daemon.child.try_wait().unwrap().is_none(),
                "batond exited before ready"
            );
            assert!(
                began.elapsed() < Duration::from_secs(10),
                "batond readiness timed out"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        daemon
    }
    fn request(&self, command: Value) -> Value {
        let response = call(&self.runtime, command).unwrap();
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }
    fn signal_and_wait(&mut self, signal: i32) {
        // Our unreaped child exclusively owns this PID until we wait; never signal
        // a discovered PID or a previously reaped child.
        assert!(self.child.try_wait().unwrap().is_none());
        assert_eq!(
            unsafe { libc::kill(self.child.id() as libc::pid_t, signal) },
            0
        );
        let began = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "Signal must reach graceful library cleanup, got {status}"
                );
                break;
            }
            assert!(
                began.elapsed() < Duration::from_secs(10),
                "batond did not complete bounded shutdown"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !self.runtime.join("batond.sock").exists(),
            "Daemon Drop must remove its socket"
        );
    }
}
impl Drop for OwnedDaemon {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn generated_fixture(root: &Path, slow_probe: bool) -> PathBuf {
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let path = root.join("generated-fixture");
    let code = r#"
import json,sys,time
if '--version' in sys.argv:
 if SLOW:time.sleep(30)
 print('AgentBaton owned fixture 1.0');sys.exit(0)
for line in sys.stdin:
 r=json.loads(line);m=r.get('method');i=r.get('id')
 if m=='initialize':print(json.dumps({'jsonrpc':'2.0','id':i,'result':{'protocolVersion':1,'agentCapabilities':{}}}),flush=True)
 elif m=='session/new':print(json.dumps({'jsonrpc':'2.0','id':i,'result':{'sessionId':'fixture'}}),flush=True)
 elif m=='session/prompt':pass
"#;
    fs::write(
        &path,
        format!(
            "#!{}\nSLOW={}\n{}",
            String::from_utf8(python.stdout).unwrap().trim(),
            if slow_probe { "True" } else { "False" },
            code
        ),
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}
fn approve(daemon: &OwnedDaemon, proposal: Value) -> Value {
    daemon.request(json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":format!("proposal:{}",proposal["id"])}))
}
fn wait(
    daemon: &OwnedDaemon,
    method: &str,
    params: Value,
    condition: impl Fn(&Value) -> bool,
) -> Value {
    let begin = Instant::now();
    loop {
        let value = daemon.request(json!({"method":method,"params":params}));
        if condition(&value) {
            return value;
        }
        assert!(
            begin.elapsed() < Duration::from_secs(10),
            "timed out {method}: {value}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn sigterm_reaches_supervisor_stop_receipts_and_sigint_allows_clean_restart() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    git(&source, &["init", "-q"]);
    fs::write(source.join("file"), "baseline").unwrap();
    git(&source, &["add", "file"]);
    git(&source, &["commit", "-qm", "fixture"]);
    let head = git(&source, &["rev-parse", "HEAD"]);
    let fixture = generated_fixture(&root, false);
    let mut daemon = OwnedDaemon::start(&root);
    let project=daemon.request(json!({"method":"projects.import","params":{"path":source,"name":"Signal fixture"},"idempotency_key":"project"}));
    let workspace=daemon.request(json!({"method":"workspaces.create","params":{"project_id":project["id"],"expected_head":head},"expected_revision":1,"idempotency_key":"workspace"}));
    let p=daemon.request(json!({"method":"connections.propose","params":{"name":"Generated signal fixture","provider":"custom_acp","executable":fixture,"arguments":[]},"idempotency_key":"connection"}));
    let connection = approve(&daemon, p);
    wait(&daemon, "connections.list", json!({}), |v| {
        v.as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == connection["id"] && c["probe_state"] == "ready")
    });
    let p=daemon.request(json!({"method":"grants.propose","params":{"project_id":project["id"],"workspace_id":workspace["id"],"connection_id":connection["id"],"max_attempts":2,"max_concurrency":1,"max_duration_ms":15_000},"idempotency_key":"grant"}));
    let grant = approve(&daemon, p);
    let task=daemon.request(json!({"method":"tasks.create","params":{"project_id":project["id"],"goal":"Owned generated signal fixture"},"idempotency_key":"task"}));
    let session=daemon.request(json!({"method":"sessions.create","params":{"task_id":task["id"],"workspace_id":workspace["id"],"connection_id":connection["id"],"grant_id":grant["id"]},"expected_revision":1,"idempotency_key":"session"}));
    let ready = wait(
        &daemon,
        "sessions.get",
        json!({"session_id":session["id"]}),
        |v| v["state"] == "ready",
    );
    daemon.request(json!({"method":"sessions.send","params":{"session_id":session["id"],"text":"wait until stopped"},"expected_revision":ready["revision"],"idempotency_key":"prompt"}));
    daemon.signal_and_wait(libc::SIGTERM);
    let store = Store::open(&daemon.runtime.join("baton.sqlite3")).unwrap();
    let saved = store.get_session(session["id"].as_str().unwrap()).unwrap();
    assert_eq!(saved.state, "stopped");
    assert_eq!(
        store.get(task["id"].as_str().unwrap()).unwrap().state,
        TaskState::Cancelled
    );
    assert_eq!(
        store
            .workspace_lease(workspace["id"].as_str().unwrap())
            .unwrap()
            .unwrap()
            .holder,
        ""
    );
    let journal = store.events(0, 500).unwrap();
    assert!(journal["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["aggregate_id"] == session["id"]
            && event["kind"] == "session.stopped"
            && event["payload"]["process_reaped"] == true));
    drop(store);
    let mut restarted = OwnedDaemon::start(&root);
    assert_eq!(
        restarted
            .request(json!({"method":"tasks.list","params":{}}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    restarted.signal_and_wait(libc::SIGINT);
}
#[test]
fn sigterm_waits_for_bounded_owned_version_probe_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let fixture = generated_fixture(&root, true);
    let mut daemon = OwnedDaemon::start(&root);
    let proposal=daemon.request(json!({"method":"connections.propose","params":{"name":"Generated slow version fixture","provider":"custom_acp","executable":fixture,"arguments":[]},"idempotency_key":"proposal"}));
    let connection = approve(&daemon, proposal);
    daemon.signal_and_wait(libc::SIGTERM);
    let store = Store::open(&daemon.runtime.join("baton.sqlite3")).unwrap();
    let saved = store
        .get_connection(connection["id"].as_str().unwrap())
        .unwrap();
    assert_eq!(saved.probe_state, "failed");
    let evidence = saved.version_evidence.unwrap();
    assert!(evidence.process_reaped);
    assert_eq!(serde_json::to_value(evidence.state).unwrap(), "timed_out");
}
