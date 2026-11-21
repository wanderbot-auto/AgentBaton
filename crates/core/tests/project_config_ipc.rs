//! Real Unix IPC, SQLite and generated local process fixtures only.
use agentbaton_config::runtime::{MaterializedKind, RuntimeRequirements, RuntimeRoot};
use agentbaton_core::ipc::{call, Daemon, API_VERSION};
use rusqlite::Connection;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const FIRST: &str = "---\nname: review\ndescription: Review fixture\n---\nReview version one.\n";
const SECOND: &str = "---\nname: review\ndescription: Review fixture\n---\nReview version two.\n";
const UNCLASSIFIED: &str =
    "---\nname: review\ndescription: Review fixture\n---\nNever auto-admit this change.\n";

enum DaemonOwner {
    Thread(Arc<AtomicBool>, JoinHandle<()>),
    Process(Child),
}

struct Server {
    runtime: PathBuf,
    owner: Option<DaemonOwner>,
}

impl Server {
    fn start(runtime: &Path) -> Self {
        let daemon = Daemon::bind(runtime).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let thread = thread::spawn(move || daemon.run(signal).unwrap());
        Self {
            runtime: runtime.into(),
            owner: Some(DaemonOwner::Thread(stop, thread)),
        }
    }

    fn start_product(runtime: &Path) -> Self {
        let tools = PathBuf::from(
            std::env::var_os("AGENTBATON_FACTORY_TEST_TOOLS")
                .expect("Prepare current Factory test tools before starting the product daemon"),
        )
        .canonicalize()
        .unwrap();
        let child = Command::new(tools.join("batond"))
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("BATON_RUNTIME_DIR", runtime)
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        let mut server = Self {
            runtime: runtime.into(),
            owner: Some(DaemonOwner::Process(child)),
        };
        let until = Instant::now() + Duration::from_secs(5);
        let request = json!({"method":"system.snapshot","params":{}});
        loop {
            if runtime.join("session.token").is_file()
                && call(runtime, request.clone())
                    .is_ok_and(|reply| reply["result"]["api_version"] == API_VERSION)
            {
                return server;
            }
            let Some(DaemonOwner::Process(child)) = server.owner.as_mut() else {
                unreachable!();
            };
            assert!(
                child.try_wait().unwrap().is_none(),
                "Fixture batond exited during startup"
            );
            assert!(
                Instant::now() < until,
                "Fixture batond did not publish its private IPC endpoint"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn request(&self, request: Value) -> Value {
        call(&self.runtime, request).unwrap()
    }

    fn result(&self, request: Value) -> Value {
        let response = self.request(request);
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }

    fn await_result(&self, request: Value, ready: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let value = self.result(request.clone());
            if ready(&value) {
                return value;
            }
            assert!(Instant::now() < deadline, "Timed out at {value}");
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn send_without_reading(&self, request: &Value) -> UnixStream {
        let mut request = request.clone();
        request["request_id"] = json!("lost-response");
        request["api_version"] = json!(API_VERSION);
        request["token"] = json!(fs::read_to_string(self.runtime.join("session.token")).unwrap());
        let mut socket = UnixStream::connect(self.runtime.join("batond.sock")).unwrap();
        serde_json::to_writer(&mut socket, &request).unwrap();
        socket.write_all(b"\n").unwrap();
        socket.shutdown(std::net::Shutdown::Write).unwrap();
        socket
    }

    fn db(&self) -> Connection {
        Connection::open(self.runtime.join("baton.sqlite3")).unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        match self.owner.take().unwrap() {
            DaemonOwner::Thread(stop, thread) => {
                stop.store(true, Ordering::Relaxed);
                thread.join().unwrap();
            }
            DaemonOwner::Process(mut child) => {
                let outcome = stop_product(&mut child);
                if !thread::panicking() {
                    assert!(
                        outcome.is_some_and(|status| status.success()),
                        "Fixture batond did not shut down cleanly"
                    );
                }
            }
        }
    }
}

fn stop_product(child: &mut Child) -> Option<std::process::ExitStatus> {
    if let Ok(Some(status)) = child.try_wait() {
        return Some(status);
    }
    // Until wait reaps it, this PID belongs to the original Child.
    let pid = rustix::process::Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
    let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
    let until = Instant::now() + Duration::from_secs(5);
    while Instant::now() < until {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

fn git(source: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(source)
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
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().into()
}

struct Fixture {
    _private: tempfile::TempDir,
    runtime: PathBuf,
    root: PathBuf,
    source: PathBuf,
    skill: PathBuf,
    project: Value,
    head: String,
}

impl Fixture {
    fn start() -> (Self, Server) {
        Self::start_with(Server::start)
    }

    fn start_with(start_server: fn(&Path) -> Server) -> (Self, Server) {
        let private = tempfile::tempdir().unwrap();
        let root = private.path().canonicalize().unwrap();
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "-q"]);
        fs::write(source.join("source.txt"), "immutable fixture input").unwrap();
        fs::write(source.join(".gitignore"), "reports/\n").unwrap();
        git(&source, &["add", "source.txt", ".gitignore"]);
        git(&source, &["commit", "-qm", "fixture"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let skill = source.join(".agents/skills/review/SKILL.md");
        fs::create_dir_all(skill.parent().unwrap()).unwrap();
        fs::write(&skill, FIRST).unwrap();
        let runtime = root.join("runtime");
        let server = start_server(&runtime);
        let project = server.result(json!({
            "method":"projects.import","params":{"path":source,"name":"Config fixture"},
            "idempotency_key":"import",
        }))["id"]
            .clone();
        (
            Self {
                _private: private,
                runtime,
                root,
                source,
                skill,
                project,
                head,
            },
            server,
        )
    }

    fn get(&self) -> Value {
        json!({"method":"projects.config.get","params":{"project_id":self.project}})
    }

    fn preview(&self, server: &Server) -> Value {
        server.result(json!({
            "method":"projects.config.preview","params":{"project_id":self.project,"provider":"codex"},
        }))
    }

    fn proposal(&self, server: &Server, revision: u64, approved: &[&str], key: &str) -> Value {
        let preview = self.preview(server);
        let candidate = &preview["inspection"]["candidates"][0];
        assert_eq!(candidate["admission"], "reviewed_instruction");
        assert!(!preview.to_string().contains("Review version"));
        server.result(json!({
            "method":"projects.config.propose",
            "expected_revision":1,
            "idempotency_key":key,
            "params":{
                "project_id":self.project,"provider":"codex","expected_config_revision":revision,
                "instructions":[{"key":candidate["key"],"sha256":candidate["source_sha256"],
                    "approved_update_sha256": approved.iter().map(|text| format!("{:x}", Sha256::digest(text.as_bytes()))).collect::<Vec<_>>()}],
            },
        }))
    }

    fn refresh(&self, revision: u64, key: &str) -> Value {
        json!({"method":"projects.config.refresh","params":{"project_id":self.project,"expected_config_revision":revision},"expected_revision":1,"idempotency_key":key})
    }

    fn task(&self, server: &Server, key: &str) -> Value {
        server.result(json!({"method":"tasks.create","params":{"project_id":self.project,"goal":"Use generated fixture only"},"idempotency_key":key}))
    }

    fn assert_no_execution(&self, server: &Server) {
        let db = server.db();
        for table in [
            "connections",
            "execution_grants",
            "attempts",
            "sessions",
            "dispatch_intents",
        ] {
            // Closed list of fixture tables.
            assert_eq!(
                db.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                    .get::<_, u32>(0))
                    .unwrap(),
                0,
                "{table}"
            );
        }
    }
}

fn approve(proposal: &Value) -> Value {
    json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"]})
}

#[test]
fn approval_allowlist_and_review_required_are_real_ipc() {
    let (fixture, server) = Fixture::start();
    let before = fixture.preview(&server);
    assert_eq!(before["configuration"]["config_revision"], 0);
    assert!(!fixture.runtime.join("project-config").exists());

    let proposal = fixture.proposal(&server, 0, &[SECOND], "first-proposal");
    assert_eq!(proposal["kind"], "project_config");
    assert_eq!(
        fixture.proposal(&server, 0, &[SECOND], "first-proposal"),
        proposal
    );
    assert_eq!(server.result(fixture.get())["active"], Value::Null);
    fixture.assert_no_execution(&server);

    let mut stale = approve(&proposal);
    stale["expected_revision"] = json!(2);
    assert_eq!(server.request(stale)["error"]["code"], "STALE_REVISION");
    let applied = server.result(approve(&proposal));
    assert_eq!(applied["status"], "applied");
    assert_eq!(
        applied["configuration"]["active"]["update_rule_bound"],
        true
    );
    assert_eq!(server.result(approve(&proposal)), applied);
    let previous = fixture.task(&server, "old-task");
    assert_eq!(
        server.result(fixture.refresh(1, "before-drift"))["status"],
        "unchanged"
    );

    fs::write(&fixture.skill, SECOND).unwrap();
    let mut forged_classification = fixture.refresh(1, "forged");
    forged_classification["params"]["classified_instructions"] = json!({});
    assert_eq!(
        server.request(forged_classification)["error"]["code"],
        "INVALID_REQUEST"
    );
    let updated = server.result(fixture.refresh(1, "approved-drift"));
    assert_eq!(updated["status"], "applied");
    assert_eq!(updated["configuration"]["config_revision"], 2);
    assert_ne!(
        updated["configuration"]["active"]["snapshot_sha256"],
        applied["configuration"]["active"]["snapshot_sha256"]
    );
    assert_eq!(
        updated["configuration"]["active"]["approval_id"],
        proposal["id"]
    );
    assert_eq!(server.result(fixture.refresh(1, "approved-drift")), updated);
    assert_eq!(fixture.task(&server, "old-task"), previous);
    let fresh = fixture.task(&server, "new-task");
    let db = server.db();
    for (task, revision) in [(previous, 1), (fresh, 2)] {
        let pinned: i64 = db.query_row(
            "SELECT json_extract(result,'$.revision') FROM idempotency WHERE scope='local:project-config:task-pins' AND key=?1",
            [task["id"].as_str().unwrap()],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(pinned, revision);
    }
    drop(db);

    fs::write(&fixture.skill, UNCLASSIFIED).unwrap();
    let review = server.result(fixture.refresh(2, "unclassified"));
    assert_eq!(review["status"], "review_required");
    assert_eq!(
        review["review"]["reason"],
        "instruction_classification_required"
    );
    assert_eq!(server.result(fixture.get())["config_revision"], 2);

    let native = fixture.source.join(".codex");
    fs::create_dir(&native).unwrap();
    fs::write(
        native.join("config.toml"),
        "[mcp_servers.unreviewed]\ncommand = \"/never/executed\"\n",
    )
    .unwrap();
    let review = server.result(fixture.refresh(2, "unreviewed-mcp"));
    assert_eq!(review["status"], "review_required");
    assert!(!review.to_string().contains("/never/executed"));
    assert_eq!(server.result(fixture.get())["config_revision"], 2);
    fixture.assert_no_execution(&server);
}

#[test]
fn interrupted_settlement_reconciles_one_publication_after_restart() {
    let (fixture, server) = Fixture::start();
    let proposal = fixture.proposal(&server, 0, &[SECOND], "interrupted-proposal");
    let db = server.db();
    db.execute_batch(
        "
        CREATE TRIGGER stop_config_settlement BEFORE INSERT ON idempotency
        WHEN NEW.scope LIKE 'local:project-config:state:%'
          AND json_type(NEW.result,'$.pending')='null'
        BEGIN SELECT RAISE(ABORT,'fixture stops only final config settlement'); END;
    ",
    )
    .unwrap();
    let unread_socket = server.send_without_reading(&approve(&proposal));
    let pending = server.await_result(fixture.get(), |view| {
        view["pending_operation_id"].is_string()
    });
    assert_eq!(pending["config_revision"], 0);
    drop(unread_socket);
    let operation = &pending["pending_operation_id"];
    let store_dir = fixture
        .runtime
        .join("project-config")
        .join(fixture.project.as_str().unwrap())
        .join(operation.as_str().unwrap());
    let published = store_dir.join("project-config.json");
    let bytes = fs::read(&published).unwrap();
    let inode = fs::metadata(&published).unwrap().ino();
    let entries = || -> std::collections::BTreeSet<_> {
        fs::read_dir(&store_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect()
    };
    let before = entries();
    assert_eq!(
        server.request(approve(&proposal))["error"]["code"],
        "OUTCOME_UNKNOWN"
    );
    assert_eq!(server.request(json!({"method":"tasks.create","params":{"project_id":fixture.project,"goal":"Cannot bind during unknown outcome"},"idempotency_key":"pending-task"}))["error"]["code"], "OUTCOME_UNKNOWN");
    fixture.assert_no_execution(&server);
    drop(server);
    db.execute_batch("DROP TRIGGER stop_config_settlement;")
        .unwrap();
    drop(db);

    let server = Server::start(&fixture.runtime);
    let reconciled = server.result(json!({"method":"projects.config.reconcile","params":{"project_id":fixture.project,"operation_id":operation}}));
    assert_eq!(reconciled["status"], "applied");
    assert_eq!(reconciled["configuration"]["config_revision"], 1);
    assert_eq!(server.result(approve(&proposal)), reconciled);
    assert_eq!(fs::read(&published).unwrap(), bytes);
    assert_eq!(fs::metadata(&published).unwrap().ino(), inode);
    assert_eq!(entries(), before);
    fixture.assert_no_execution(&server);
}

#[test]
fn approval_refuses_changed_source_without_publication() {
    let (fixture, server) = Fixture::start();
    let proposal = fixture.proposal(&server, 0, &[SECOND], "stale-source");
    fs::write(&fixture.skill, SECOND).unwrap();
    let response = server.request(approve(&proposal));
    assert!(response["error"].is_object(), "{response}");
    let state = server.result(fixture.get());
    assert_eq!(state["config_revision"], 0);
    assert_eq!(state["pending_operation_id"], Value::Null);
    fixture.assert_no_execution(&server);
}

fn fixture_codex(root: &Path) -> PathBuf {
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let python = String::from_utf8(python.stdout).unwrap();
    let executable = root.join("generated-codex");
    let script = r#"
import json, os, sys
if '--version' in sys.argv:
    print('Generated local fixture 1.0')
    sys.exit(0)
assert sys.argv[1:] == ['app-server', '--listen', 'stdio://']
def reply(message, result):
    print(json.dumps({'id':message['id'], 'result':result}), flush=True)
for line in sys.stdin:
    message = json.loads(line)
    if message['method'] == 'initialize':
        reply(message, {'codexHome':os.environ['CODEX_HOME'], 'platformFamily':'unix', 'platformOs':'fixture', 'userAgent':'generated project-config fixture'})
    elif message['method'] == 'initialized':
        continue
    elif message['method'] == 'thread/start':
        cwd = os.getcwd()
        thread = {'id':'fixture-thread','cliVersion':'fixture','createdAt':1,'updatedAt':1,'cwd':cwd,'ephemeral':True,'modelProvider':'fixture','preview':'','sessionId':'fixture-session','source':'appServer','status':{'type':'idle'}}
        reply(message, {'thread':thread,'cwd':cwd,'approvalPolicy':'on-request','approvalsReviewer':'user','model':'fixture','modelProvider':'fixture','sandbox':{'type':'workspaceWrite','networkAccess':False,'writableRoots':[cwd]}})
    else:
        raise AssertionError(message['method'])
"#;
    fs::write(&executable, format!("#!{}\n{script}", python.trim())).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    executable
}

#[test]
fn running_session_and_new_task_use_their_own_verified_snapshots() {
    let (fixture, server) = Fixture::start();
    let workspace = server.result(json!({"method":"workspaces.create","params":{"project_id":fixture.project,"expected_head":fixture.head},"expected_revision":1,"idempotency_key":"workspace"}));
    let draft = server.result(json!({"method":"connections.propose","params":{"name":"Generated fixture","provider":"codex","executable":fixture_codex(&fixture.root),"arguments":[]},"idempotency_key":"connection"}));
    let connection = server.result(approve(&draft));
    server.await_result(json!({"method":"connections.list","params":{}}), |list| {
        list.as_array()
            .unwrap()
            .iter()
            .any(|value| value["id"] == connection["id"] && value["probe_state"] == "ready")
    });
    let draft = server.result(json!({"method":"grants.propose","params":{"project_id":fixture.project,"workspace_id":workspace["id"],"connection_id":connection["id"],"max_attempts":3,"max_concurrency":1,"max_duration_ms":20000},"idempotency_key":"grant"}));
    let grant = server.result(approve(&draft));
    let no_config = fixture.task(&server, "pre-config-task");
    let approval = fixture.proposal(&server, 0, &[SECOND], "start-config");
    assert_eq!(server.result(approve(&approval))["status"], "applied");
    let first_task = fixture.task(&server, "first-task");

    let start = |task: &Value, key: &str| {
        let starting = server.result(json!({"method":"sessions.create","params":{"task_id":task["id"],"workspace_id":workspace["id"],"connection_id":connection["id"],"grant_id":grant["id"]},"expected_revision":1,"idempotency_key":key}));
        server.await_result(
            json!({"method":"sessions.get","params":{"session_id":starting["id"]}}),
            |session| session["state"] == "ready",
        )
    };
    let stop = |session: &Value, key: &str| {
        server.result(json!({"method":"sessions.cancel","params":{"session_id":session["id"]},"expected_revision":session["revision"],"idempotency_key":key}));
        server.await_result(
            json!({"method":"sessions.get","params":{"session_id":session["id"]}}),
            |session| session["state"] == "stopped",
        );
    };
    let assert_materialized = |session: &Value, expected: Option<&str>| {
        let binding = &session["runtime_snapshot"];
        let descriptor = serde_json::from_value(binding["snapshot"].clone()).unwrap();
        let root = RuntimeRoot::open(&fixture.runtime.join("runtime-config")).unwrap();
        let snapshot = root
            .verify_before_launch(&descriptor, RuntimeRequirements::default())
            .unwrap();
        assert_eq!(binding["plan_digest"], snapshot.plan_digest());
        assert_eq!(
            binding["connection_config_revision"],
            connection["config_revision"]
        );
        let instructions: Vec<_> = snapshot
            .manifest()
            .files
            .iter()
            .filter(|entry| entry.kind == MaterializedKind::SkillInstruction)
            .collect();
        if let Some(expected) = expected {
            assert_eq!(instructions.len(), 1);
            assert_eq!(
                fs::read_to_string(snapshot.directory().join(&instructions[0].relative_path))
                    .unwrap(),
                expected
            );
        } else {
            assert!(instructions.is_empty());
        }
    };
    let old = start(&no_config, "old-session");
    assert_materialized(&old, None);
    stop(&old, "cancel-old");

    let first = start(&first_task, "first-session");
    assert_materialized(&first, Some(FIRST));
    fs::write(&fixture.skill, SECOND).unwrap();
    assert_eq!(
        server.result(fixture.refresh(1, "active-drift"))["status"],
        "applied"
    );
    let still_running =
        server.result(json!({"method":"sessions.get","params":{"session_id":first["id"]}}));
    assert_eq!(still_running["state"], "ready");
    assert_eq!(still_running["runtime_snapshot"], first["runtime_snapshot"]);
    assert_materialized(&still_running, Some(FIRST));
    let second_task = fixture.task(&server, "second-task");
    stop(&first, "cancel-first");

    let second = start(&second_task, "second-session");
    assert_materialized(&second, Some(SECOND));
    assert_ne!(
        second["runtime_snapshot"]["plan_digest"],
        first["runtime_snapshot"]["plan_digest"]
    );
    stop(&second, "cancel-second");
}

fn factory_provider(root: &Path) -> PathBuf {
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let python = String::from_utf8(python.stdout).unwrap();
    let executable = root.join("generated-factory-acp");
    let script = r#"
import json, subprocess, sys, time
from pathlib import Path
if '--version' in sys.argv:
    print('Generated Factory fixture 1')
    sys.exit(0)
def reply(request, result):
    print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result}), flush=True)
def rpc(method, params):
    global sequence
    sequence += 1
    mcp.stdin.write(json.dumps({'jsonrpc':'2.0','id':sequence,'method':method,'params':params})+'\n')
    mcp.stdin.flush()
    value = json.loads(mcp.stdout.readline())
    assert 'error' not in value, value
    return value['result']
def tool(name, arguments):
    value = json.loads(rpc('tools/call',{'name':name,'arguments':arguments})['content'][0]['text'])
    assert 'error' not in value, value
    return value['result']
reports = Path('reports')
reports.mkdir(exist_ok=True)
sequence = 0
with (reports / 'generated-factory-stderr.txt').open('a') as diagnostic:
    sys.stderr = diagnostic
    for line in sys.stdin:
        request = json.loads(line)
        method = request['method']
        if method == 'initialize':
            reply(request, {'protocolVersion':1,'agentCapabilities':{}})
        elif method == 'session/new':
            server, = request['params']['mcpServers']
            assert server['name'] == 'agentbaton-factory'
            mcp = subprocess.Popen([server['command'],*server['args']], stdin=subprocess.PIPE, stdout=subprocess.PIPE,text=True)
            rpc('initialize', {'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'fixture','version':'1'}})
            reply(request, {'sessionId':'generated-factory'})
        elif method == 'session/prompt':
            task = json.loads(request['params']['prompt'][0]['text'])
            assert task['kind'] == 'agentbaton_factory_task_request'
            status = tool('get_task_status', {'task_id':task['task_id']})
            assert status['task_id'] == task['task_id']
            if not task['parent_task_id'] and not task['handoff_result']:
                (reports / 'before-delegation').touch()
                deadline = time.monotonic() + 8
                while not (reports / 'allow-delegation').exists():
                    assert time.monotonic() < deadline, 'coordinator did not complete drift'
                    time.sleep(.02)
                route = json.loads((reports / 'fixture-route.json').read_text())
                child = tool('delegate_task', {
                    'goal':'Produce the generated local report','connection_id':route['connection_id'],
                    'workspace_id':route['workspace_id'],'input_artifact_ids':['input'],
                    'output_artifact_ids':['report'],
                    'budget':{'attempts':1,'parallel':1,'runtime_ms':10000,'output_bytes':4096,'children':0,'reworks':0,'tokens':None,'cost_microunits':None},
                    'wait_for_child':True,'idempotency_key':'generated-child'})
                assert child['state'] == 'blocked_on_live_parent'
                (reports / 'child-task-id').write_text(child['task_id'])
            else:
                output = Path(task['output_artifacts'][0]['requirement']['relative_path'])
                output.write_text('Generated child report\n' if task['parent_task_id'] else 'Generated parent report\n')
            reply(request, {'stopReason':'end_turn'})
        elif method == 'session/cancel':
            mcp.stdin.close()
            mcp.wait(timeout=2)
"#;
    fs::write(&executable, format!("#!{}\n{script}", python.trim())).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    executable
}

fn factory_child_keeps_root_pin(pinned: bool) {
    let (fixture, server) = Fixture::start_with(Server::start_product);
    let pre_config = (!pinned).then(|| fixture.task(&server, "factory-root"));
    let approval = fixture.proposal(&server, 0, &[SECOND], "project-config");
    let first = server.result(approve(&approval));
    let root = pre_config.unwrap_or_else(|| fixture.task(&server, "factory-root"));
    let workspace = server.result(json!({"method":"workspaces.create","params":{"project_id":fixture.project,"expected_head":fixture.head},"expected_revision":1,"idempotency_key":"factory-workspace"}));
    let draft = server.result(json!({"method":"connections.propose","params":{"name":"Generated Factory fixture","provider":"custom_acp","executable":factory_provider(&fixture.root),"arguments":[]},"idempotency_key":"factory-connection"}));
    let connection = server.result(approve(&draft));
    server.await_result(json!({"method":"connections.list","params":{}}), |list| {
        list.as_array()
            .unwrap()
            .iter()
            .any(|value| value["id"] == connection["id"] && value["probe_state"] == "ready")
    });
    let draft = server.result(json!({"method":"grants.propose","params":{"project_id":fixture.project,"workspace_id":workspace["id"],"connection_id":connection["id"],"max_attempts":4,"max_concurrency":1,"max_duration_ms":30000},"idempotency_key":"factory-grant"}));
    let grant = server.result(approve(&draft));
    let budget = json!({"attempts":4,"parallel":1,"runtime_ms":30000,"output_bytes":16384,"children":1,"reworks":0,"tokens":null,"cost_microunits":null});
    let proposal = server.result(json!({
        "method":"factory_scopes.propose","idempotency_key":"factory-scope",
        "params":{
            "root_task_id":root["id"],"expected_task_revision":root["revision"],
            "grant_id":grant["id"],"expected_grant_revision":grant["revision"],
            "connection_ids":[connection["id"]],"workspace_ids":[workspace["id"]],
            "inputs":[{"artifact_id":"input","relative_path":"source.txt","max_bytes":1024,"expected_sha256":format!("{:x}",Sha256::digest(b"immutable fixture input"))}],
            "outputs":[{"artifact_id":"report","relative_path":"reports/report.txt","max_bytes":4096,"expected_sha256":null}],
            "recipients":[],"max_depth":1,"max_children":1,"max_total_nodes":2,
            "root_budget":budget,"project_budget":budget,"ttl_ms":30000,
            "capabilities":{"delegate_task":true,"resume_parent_as_new_attempt":true,"child_planning":false,"child_goal_execution":false,"create_workspace":false}
        }
    }));
    let scope = server.result(approve(&proposal));
    let reports = Path::new(workspace["root"].as_str().unwrap()).join("reports");
    fs::create_dir_all(&reports).unwrap();
    fs::write(
        reports.join("fixture-route.json"),
        json!({"connection_id":connection["id"],"workspace_id":workspace["id"]}).to_string(),
    )
    .unwrap();
    server.result(json!({"method":"factory_runs.start","params":{"scope_id":scope["id"]},"expected_revision":scope["revision"],"idempotency_key":"factory-start"}));
    let get = json!({"method":"factory_runs.get","params":{"scope_id":scope["id"]}});
    let terminal = |value: &Value| {
        matches!(
            value["run"]["state"].as_str(),
            Some("completed" | "cancelled" | "blocked" | "outcome_unknown")
        )
    };
    server.await_result(get.clone(), |status| {
        assert!(
            !terminal(status),
            "{status}\n{}",
            fs::read_to_string(reports.join("generated-factory-stderr.txt")).unwrap_or_default()
        );
        reports.join("before-delegation").exists()
    });
    fs::write(&fixture.skill, SECOND).unwrap();
    let latest = server.result(fixture.refresh(1, "while-parent-live"));
    assert_eq!(latest["configuration"]["config_revision"], 2);
    let first_sha = first["configuration"]["active"]["snapshot_sha256"]
        .as_str()
        .unwrap();
    assert_ne!(
        latest["configuration"]["active"]["snapshot_sha256"],
        first_sha
    );
    fs::write(reports.join("allow-delegation"), "v2 is active").unwrap();
    let finished = server.await_result(get, terminal);
    assert_eq!(
        finished["run"]["state"],
        "completed",
        "{finished}\n{}",
        fs::read_to_string(reports.join("generated-factory-stderr.txt")).unwrap_or_default()
    );

    let child = fs::read_to_string(reports.join("child-task-id")).unwrap();
    let db = server.db();
    let (parent_id, root_id): (String, String) = db
        .query_row(
            "SELECT parent_task_id,root_task_id FROM factory_task_lineage WHERE task_id=?1",
            [&child],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(parent_id, root["id"]);
    assert_eq!(root_id, root["id"]);
    let runtime = RuntimeRoot::open(&fixture.runtime.join("runtime-config")).unwrap();
    let mut snapshots = db.prepare("SELECT s.task_id,r.document FROM sessions s JOIN session_runtime_snapshots r ON r.session_id=s.id WHERE s.task_id IN(?1,?2)").unwrap();
    let bindings = snapshots
        .query_map([root_id.as_str(), child.as_str()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(
        bindings.iter().any(|(task, _)| task == &child),
        "The real subprocess must have started on the child path"
    );
    assert!(bindings.iter().any(|(task, _)| task == &root_id));
    for (_, raw) in bindings {
        let binding: Value = serde_json::from_str(&raw).unwrap();
        let descriptor = serde_json::from_value(binding["snapshot"].clone()).unwrap();
        let snapshot = runtime
            .verify_before_launch(&descriptor, RuntimeRequirements::default())
            .unwrap();
        assert_eq!(binding["plan_digest"], snapshot.plan_digest());
        let refs: Vec<_> = snapshot
            .manifest()
            .source_refs
            .iter()
            .filter(|entry| entry.source_ref == "baton-project-config")
            .collect();
        if pinned {
            assert_eq!(refs.len(), 1);
            assert_eq!(refs[0].revision, "1");
            assert_eq!(refs[0].sha256, first_sha);
        } else {
            assert!(refs.is_empty());
        }
        assert!(snapshot
            .manifest()
            .files
            .iter()
            .all(|file| file.kind != MaterializedKind::SkillInstruction));
    }
}

#[test]
fn actual_factory_child_preserves_the_root_project_configuration_pin() {
    factory_child_keeps_root_pin(true);
}

#[test]
fn actual_factory_child_of_preconfig_root_does_not_take_the_new_project_version() {
    factory_child_keeps_root_pin(false);
}
