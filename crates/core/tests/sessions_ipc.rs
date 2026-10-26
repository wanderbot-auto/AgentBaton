//! Real generated fixture child only. No vendor executable, identity, model or network is invoked.
use agentbaton_core::ipc::{call, Daemon};
use serde_json::{json, Value};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
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
    fn request(&self, v: Value) -> Value {
        call(&self.runtime, v).unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn result(v: Value) -> Value {
    assert!(v.get("error").is_none(), "{v}");
    v["result"].clone()
}
fn git(path: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(path)
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
fn fixture(root: &Path) -> PathBuf {
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let python = String::from_utf8(python.stdout).unwrap();
    let script = root.join("fixture-acp");
    let body = r#"
import json,sys,os,time
if '--version' in sys.argv:
 print('AgentBaton generated fixture 1.0');sys.exit(0)
def send(v): print(json.dumps(v),flush=True)
open('fixture-started','w').write('owned fixture')
pending=None
native_turn=0
native_pending=None
for line in sys.stdin:
 r=json.loads(line);m=r.get('method');i=r.get('id')
 if r.get('jsonrpc') is None:
  if m=='initialize':
   assert sys.argv[1:]==['app-server','--listen','stdio://']
   send({'id':i,'result':{'codexHome':os.environ['CODEX_HOME'],'platformFamily':'unix','platformOs':'fixture','userAgent':'generated core native fixture'}})
  elif m=='initialized':continue
  elif m=='thread/start':
   assert r['params']=={'cwd':os.getcwd(),'approvalPolicy':'on-request','sandbox':'workspace-write','ephemeral':True}
   native_thread={'id':'thread-fixture','cliVersion':'fixture','createdAt':1,'updatedAt':1,'cwd':os.getcwd(),'ephemeral':True,'modelProvider':'fixture','preview':'','projectId':None,'sessionId':'native-fixture','source':'appServer','status':{'type':'idle'},'turns':[]}
   send({'id':i,'result':{'thread':native_thread,'cwd':os.getcwd(),'approvalPolicy':'on-request','approvalsReviewer':'user','model':'fixture','modelProvider':'fixture','sandbox':{'type':'workspaceWrite','networkAccess':False,'writableRoots':[os.getcwd()]}}})
  elif m=='turn/start':
   native_turn+=1;native_pending='turn-'+str(native_turn)
   assert r['params']['threadId']=='thread-fixture'
   text=r['params']['input'][0]['text']
   send({'id':i,'result':{'turn':{'id':native_pending,'items':[],'status':'inProgress'}}})
   if text in ('hang','ack_only'):continue
   send({'method':'item/agentMessage/delta','params':{'threadId':'thread-fixture','turnId':native_pending,'itemId':'message-fixture','delta':'native-fixture:'+text}})
   send({'method':'turn/completed','params':{'threadId':'thread-fixture','turn':{'id':native_pending,'items':[],'status':'completed'}}})
  elif m=='turn/interrupt':
   send({'id':i,'result':{}})
   send({'method':'turn/completed','params':{'threadId':'thread-fixture','turn':{'id':native_pending,'items':[],'status':'interrupted'}}})
  continue
 if m=='initialize':
  if '--acp' in sys.argv:
   assert len(sys.argv)==4 and sys.argv[1:3]==['--acp','--config-dir'] and os.path.isdir(sys.argv[3])
  send({'jsonrpc':'2.0','id':i,'result':{'protocolVersion':1,'agentCapabilities':{}}})
 elif m=='session/new':
  assert r['params']['cwd']==os.getcwd() and r['params']['mcpServers']==[]
  if 'CLAUDE_CONFIG_DIR' in os.environ:
   assert os.path.isdir(os.environ['CLAUDE_CONFIG_DIR'])
   options=r['params']['_meta']['claudeCode']['options']
   assert options['settingSources']==[] and options['allowDangerouslySkipPermissions'] is False and options['strictMcpConfig'] is True
   assert options['settings']['enableAllProjectMcpServers'] is False
  send({'jsonrpc':'2.0','id':i,'result':{'sessionId':'native-fixture'}})
 elif m=='session/prompt':
  text=r['params']['prompt'][0]['text'];pending=i
  if text=='hang':open('fixture-hang','w').write(str(os.getpid()));continue
  if text=='malformed':print('invalid-json',flush=True);continue
  if text=='stream':
   for chunk in range(200):
    send({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'native-fixture','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'chunk-'+str(chunk)}}}});time.sleep(.005)
   continue
  if text=='edit':open('file','w').write('fixture writer changed file')
  output=','.join(sorted(os.environ)) if text=='env' else 'fixture:'+text
  send({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'native-fixture','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':output}}}})
  send({'jsonrpc':'2.0','id':i,'result':{'stopReason':'end_turn'}});pending=None
 elif m=='session/cancel' and pending is not None:
  send({'jsonrpc':'2.0','id':pending,'result':{'stopReason':'cancelled'}});pending=None
"#;
    std::fs::write(&script, format!("#!{}\n{}", python.trim(), body)).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    script
}
fn wait(server: &Server, method: &str, params: Value, check: impl Fn(&Value) -> bool) -> Value {
    let start = Instant::now();
    loop {
        let value = result(server.request(json!({"method":method,"params":params})));
        if check(&value) {
            return value;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out {method}: {value}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn approve(server: &Server, proposal: Value) -> Value {
    result(server.request(json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":format!("proposal:{}",proposal["id"])})))
}
struct Scenario {
    _dir: tempfile::TempDir,
    root: PathBuf,
    server: Server,
    project: Value,
    workspace: Value,
    connection: Value,
    grant: Value,
    task: Value,
}
impl Scenario {
    fn new(duration: u64) -> Self {
        Self::with_provider(duration, "custom_acp")
    }
    fn with_provider(duration: u64, provider: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("source");
        std::fs::create_dir(&source).unwrap();
        git(&source, &["init", "-q"]);
        std::fs::write(source.join("file"), "baseline").unwrap();
        git(&source, &["add", "file"]);
        git(&source, &["commit", "-qm", "baseline"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let script = fixture(&root);
        let server = Server::start(&root.join("runtime"));
        let project=result(server.request(json!({"method":"projects.import","params":{"path":source,"name":"Fixture Project"},"idempotency_key":"project"})));
        let workspace=result(server.request(json!({"method":"workspaces.create","params":{"project_id":project["id"],"expected_head":head},"expected_revision":1,"idempotency_key":"workspace"})));
        let p=result(server.request(json!({"method":"connections.propose","params":{"name":"Generated fixture only","provider":provider,"executable":script,"arguments":[]},"idempotency_key":"connection-proposal"})));
        assert_eq!(p["snapshot"]["scope"], "version_probe_only");
        let connection = approve(&server, p);
        let connections = wait(&server, "connections.list", json!({}), |v| {
            v.as_array()
                .unwrap()
                .iter()
                .any(|c| c["id"] == connection["id"] && c["probe_state"] == "ready")
        });
        let connection = connections
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == connection["id"])
            .unwrap()
            .clone();
        let gp=result(server.request(json!({"method":"grants.propose","params":{"project_id":project["id"],"workspace_id":workspace["id"],"connection_id":connection["id"],"max_attempts":2,"max_concurrency":1,"max_duration_ms":duration},"idempotency_key":"grant-proposal"})));
        let grant = approve(&server, gp);
        let task=result(server.request(json!({"method":"tasks.create","params":{"project_id":project["id"],"goal":"Run generated local fixture only"},"idempotency_key":"task"})));
        Self {
            _dir: dir,
            root,
            server,
            project,
            workspace,
            connection,
            grant,
            task,
        }
    }
    fn create(&self) -> Value {
        result(self.server.request(json!({"method":"sessions.create","params":{"task_id":self.task["id"],"workspace_id":self.workspace["id"],"connection_id":self.connection["id"],"grant_id":self.grant["id"]},"expected_revision":1,"idempotency_key":"session"})))
    }
    fn ready(&self, id: &Value) -> Value {
        wait(
            &self.server,
            "sessions.get",
            json!({"session_id":id}),
            |s| s["state"] == "ready",
        )
    }
    fn send(&self, session: &Value, text: &str, key: &str) -> Value {
        result(self.server.request(json!({"method":"sessions.send","params":{"session_id":session["id"],"text":text},"expected_revision":session["revision"],"idempotency_key":key})))
    }
    fn cancel(&self, session: &Value) -> Value {
        result(self.server.request(json!({"method":"sessions.cancel","params":{"session_id":session["id"]},"expected_revision":session["revision"],"idempotency_key":"cancel"})))
    }
}
#[test]
fn real_session_turns_survive_ui_requests_and_never_imply_acceptance() {
    let f = Scenario::new(10_000);
    assert_eq!(f.connection["identity_status"], "unknown");
    let s = f.create();
    let ready = f.ready(&s["id"]);
    let sent = f.send(&ready, "edit", "turn-1");
    assert_eq!(sent["state"], "running");
    let ready = f.ready(&s["id"]);
    assert_eq!(ready["turn_state"], "completed");
    let path = PathBuf::from(f.workspace["root"].as_str().unwrap());
    assert_eq!(
        std::fs::read_to_string(path.join("file")).unwrap(),
        "fixture writer changed file"
    );
    // Further turns accept this same writer's edits; clean initial admission checks are not rerun.
    f.send(&ready, "env", "turn-2");
    let ready = f.ready(&s["id"]);
    let events = result(
        f.server
            .request(json!({"method":"events.subscribe","params":{"cursor":0,"limit":500}})),
    );
    let encoded = events.to_string();
    assert!(encoded.contains("fixture:edit"));
    assert!(!encoded.contains("GH_TOKEN"));
    assert!(!encoded.contains("OPENAI_API_KEY"));
    let task = result(
        f.server
            .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
    );
    assert_ne!(task["state"], "accepted");
    f.cancel(&ready);
    let stopped = wait(
        &f.server,
        "sessions.get",
        json!({"session_id":s["id"]}),
        |s| s["state"] == "stopped",
    );
    assert_eq!(stopped["last_error"], Value::Null);
}
#[test]
fn cancel_live_prompt_does_not_block_daemon_or_claim_stop_early() {
    let f = Scenario::new(10_000);
    let s = f.create();
    let ready = f.ready(&s["id"]);
    let running = f.send(&ready, "hang", "hang");
    let before = Instant::now();
    let requested = f.cancel(&running);
    assert_eq!(requested["state"], "cancel_requested");
    assert!(before.elapsed() < Duration::from_secs(2));
    result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    );
    wait(
        &f.server,
        "sessions.get",
        json!({"session_id":s["id"]}),
        |s| s["state"] == "stopped",
    );
}
#[test]
fn runtime_timeout_and_revocation_stop_only_owned_process() {
    use agentbaton_core::{sessions::Supervisor, storage::Store};

    struct Unrelated(std::process::Child);
    impl Drop for Unrelated {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut unrelated = Unrelated(Command::new("/bin/sleep").arg("60").spawn().unwrap());
    let hang_pid = |workspace: &Path| {
        let start = Instant::now();
        loop {
            if let Ok(pid) = std::fs::read_to_string(workspace.join("fixture-hang")) {
                if let Ok(pid) = pid.parse::<libc::pid_t>() {
                    return pid;
                }
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "hang not received"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    let reaped = |pid| {
        let start = Instant::now();
        loop {
            // This is the generated child's actual PID, not a session projection.
            if unsafe { libc::kill(pid, 0) } == -1 {
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::ESRCH)
                );
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "owned child alive"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    let receipt = |events: Value, id: &Value| {
        let event = events["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|event| event["aggregate_id"] == *id && event["kind"] == "session.stopped")
            .cloned()
            .expect("durable supervisor stop receipt");
        assert!(
            event["payload"]["process_reaped"] == true
                || event["payload"]["no_process_started"] == true
        );
        assert_eq!(event["payload"]["task_accepted"], false);
        event["payload"].clone()
    };

    // The absolute grant deadline starts at approval, before session admission
    // and Provider initialization. A late observer need not ever see ready.
    let f = Scenario::new(1200);
    let s = f.create();
    wait(
        &f.server,
        "sessions.get",
        json!({"session_id":s["id"]}),
        |s| s["state"] == "stopped",
    );
    // Deliberately begin observation only after real deadline enforcement.
    let stopped = wait(
        &f.server,
        "sessions.get",
        json!({"session_id":s["id"]}),
        |s| s["state"] == "ready" || s["state"] == "stopped",
    );
    assert_eq!(stopped["state"], "stopped");
    assert!(stopped["last_error"].is_string());
    assert!(matches!(
        stopped["last_error"].as_str(),
        Some("grant_revoked_or_expired" | "launch_cancelled_or_timeout" | "duration_limit")
    ));
    receipt(
        result(
            f.server
                .request(json!({"method":"events.subscribe","params":{"cursor":0,"limit":500}})),
        ),
        &s["id"],
    );
    assert!(unrelated.0.try_wait().unwrap().is_none());

    // Separately prove a running, hung worker enforces its own runtime budget.
    // Drive the public Supervisor directly, so no policy tick can cancel it
    // for the absolute grant deadline while waiting for the actual child exit.
    // The normal 10s fixture budget and all 10s wait bounds remain unchanged.
    let Scenario {
        _dir,
        root,
        server,
        workspace,
        connection,
        grant,
        task,
        ..
    } = Scenario::new(10_000);
    drop(server);
    let runtime = root.join("runtime");
    let mut store = Store::open(&runtime.join("baton.sqlite3")).unwrap();
    let session = store
        .create_session(
            task["id"].as_str().unwrap(),
            workspace["id"].as_str().unwrap(),
            connection["id"].as_str().unwrap(),
            grant["id"].as_str().unwrap(),
            1,
            "runtime-budget",
        )
        .unwrap();
    let mut supervisor = Supervisor::new(&runtime);
    let worker_started = Instant::now();
    supervisor.launch(&mut store, &session.id).unwrap();
    let start = Instant::now();
    let ready = loop {
        supervisor.tick(&mut store).unwrap();
        let current = store.get_session(&session.id).unwrap();
        if current.state == "ready" {
            break current;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "not ready: {current:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let (running, fresh) = store
        .send_session(&ready.id, "hang", ready.revision, "hang-runtime")
        .unwrap();
    assert!(fresh);
    assert_eq!(running.state, "running");
    supervisor.send(&ready.id, "hang").unwrap();
    let pid = hang_pid(Path::new(workspace["root"].as_str().unwrap()));
    reaped(pid); // No Supervisor::tick/cancel/revoke until the child is reaped.
    assert!(worker_started.elapsed() >= Duration::from_millis(10_000));
    let start = Instant::now();
    let stopped = loop {
        supervisor.tick(&mut store).unwrap();
        let current = store.get_session(&session.id).unwrap();
        if current.state == "stopped" {
            break current;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "not stopped: {current:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(stopped.last_error.is_some());
    // The remaining-runtime turn deadline and outer worker budget expire at
    // the same boundary; either existing timeout diagnostic is valid here.
    assert!(matches!(
        stopped.last_error.as_deref(),
        Some("duration_limit" | "provider_protocol_or_timeout_failure")
    ));
    let proof = receipt(store.events(0, 500).unwrap(), &json!(session.id));
    assert_eq!(proof["process_reaped"], true);
    assert_eq!(proof["no_process_started"], false);
    assert!(!supervisor.owns(&session.id));
    assert!(unrelated.0.try_wait().unwrap().is_none());

    // Revocation still starts from an actually ready worker which consumed hang.
    let f = Scenario::new(10_000);
    let s = f.create();
    let ready = f.ready(&s["id"]);
    let running = f.send(&ready, "hang", "hang");
    assert_eq!(running["state"], "running");
    let pid = hang_pid(Path::new(f.workspace["root"].as_str().unwrap()));
    result(f.server.request(json!({"method":"grants.revoke","params":{"grant_id":f.grant["id"]},"expected_revision":f.grant["revision"],"idempotency_key":"revoke"})));
    let stopped = wait(
        &f.server,
        "sessions.get",
        json!({"session_id":s["id"]}),
        |s| s["state"] == "stopped",
    );
    assert_eq!(stopped["last_error"], "grant_revoked_or_expired");
    let proof = receipt(
        result(
            f.server
                .request(json!({"method":"events.subscribe","params":{"cursor":0,"limit":500}})),
        ),
        &s["id"],
    );
    assert_eq!(proof["process_reaped"], true);
    assert_eq!(proof["no_process_started"], false);
    reaped(pid);
    assert!(unrelated.0.try_wait().unwrap().is_none());
}
#[test]
fn binary_drift_unknown_grant_and_unknown_provider_label_never_spawn() {
    let f = Scenario::new(10_000);
    let response=f.server.request(json!({"method":"sessions.create","params":{"task_id":f.task["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"grant_id":"missing"},"expected_revision":1,"idempotency_key":"no-grant"}));
    assert!(response["error"].is_object());
    let executable = PathBuf::from(f.connection["executable"].as_str().unwrap());
    std::fs::write(&executable, "#!/bin/sh\nprintf changed\n").unwrap();
    let response=f.server.request(json!({"method":"sessions.create","params":{"task_id":f.task["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"grant_id":f.grant["id"]},"expected_revision":1,"idempotency_key":"changed"}));
    assert_eq!(response["error"]["code"], "INPUT_CHANGED");
    assert!(!Path::new(f.workspace["root"].as_str().unwrap())
        .join("fixture-started")
        .exists());
    assert_eq!(f.server.request(json!({"method":"connections.propose","params":{"name":"No fallback","provider":"codex_app_server","executable":executable,"arguments":[]},"idempotency_key":"native"}))["error"]["code"],"CAPABILITY_UNSUPPORTED");
}
#[test]
fn owner_proposal_rejects_changed_binary_and_direct_registration() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let script = fixture(&root);
    let server = Server::start(&root.join("runtime"));
    let params =
        json!({"name":"Fixture","provider":"custom_acp","executable":script,"arguments":[]});
    assert_eq!(
        server.request(
            json!({"method":"connections.register","params":params,"idempotency_key":"bypass"})
        )["error"]["code"],
        "AUTH_REQUIRED"
    );
    let p = result(server.request(
        json!({"method":"connections.propose","params":params,"idempotency_key":"proposal"}),
    ));
    std::fs::write(&script, "#!/bin/sh\necho changed\n").unwrap();
    assert_eq!(server.request(json!({"method":"approvals.finalize","params":{"proposal_id":p["id"]},"expected_revision":1}))["error"]["code"],"INPUT_CHANGED");
    assert!(
        result(server.request(json!({"method":"connections.list","params":{}})))
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn cancellation_uses_immutable_session_identity_despite_stream_revision_growth() {
    let f = Scenario::new(10_000);
    let s = f.create();
    let old = f.ready(&s["id"]);
    f.send(&old, "stream", "stream");
    let changed = wait(
        &f.server,
        "sessions.get",
        json!({"session_id":s["id"]}),
        |v| {
            v["revision"].as_u64().unwrap() > old["revision"].as_u64().unwrap() + 2
                || v["state"] == "stopped"
        },
    );
    assert!(changed["revision"].as_u64().unwrap() > old["revision"].as_u64().unwrap());
    let cancelled = f.cancel(&old);
    assert!(cancelled["state"] == "cancel_requested" || cancelled["state"] == "stopped");
    wait(
        &f.server,
        "sessions.get",
        json!({"session_id":s["id"]}),
        |v| v["state"] == "stopped",
    );
}
#[test]
fn malformed_provider_is_reaped_and_records_failure_without_fabricating_success() {
    let f = Scenario::new(10_000);
    let s = f.create();
    let ready = f.ready(&s["id"]);
    f.send(&ready, "malformed", "malformed");
    let stopped = wait(
        &f.server,
        "sessions.get",
        json!({"session_id":s["id"]}),
        |v| v["state"] == "stopped",
    );
    assert_eq!(
        stopped["last_error"],
        "provider_protocol_or_timeout_failure"
    );
    let task = result(
        f.server
            .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
    );
    assert_eq!(task["state"], "outcome_unknown");
}
#[test]
fn another_named_connection_cannot_borrow_account_config_or_grant() {
    let f = Scenario::new(10_000);
    let proposal=result(f.server.request(json!({"method":"connections.propose","params":{"name":"Other account label","provider":"custom_acp","executable":f.connection["executable"],"arguments":["--account-B-fixture"]},"idempotency_key":"other-connection"})));
    let other = approve(&f.server, proposal);
    let list = wait(&f.server, "connections.list", json!({}), |v| {
        v.as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == other["id"] && c["probe_state"] == "ready")
    });
    let other = list
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == other["id"])
        .unwrap();
    assert_ne!(other["environment_id"], f.connection["environment_id"]);
    assert_ne!(other["identity_ref"], f.connection["identity_ref"]);
    assert_eq!(other["identity_status"], "unknown");
    let response=f.server.request(json!({"method":"sessions.create","params":{"task_id":f.task["id"],"workspace_id":f.workspace["id"],"connection_id":other["id"],"grant_id":f.grant["id"]},"expected_revision":1,"idempotency_key":"wrong-connection"}));
    assert_eq!(response["error"]["code"], "PERMISSION_DENIED");
    assert!(!Path::new(f.workspace["root"].as_str().unwrap())
        .join("fixture-started")
        .exists());
}
#[test]
fn crash_window_persistence_is_unknown_and_expired_lease_does_not_allow_replay() {
    use agentbaton_core::storage::Store;
    let f = Scenario::new(10_000);
    let Scenario {
        _dir,
        root,
        server,
        project,
        workspace,
        connection,
        grant,
        task,
    } = f;
    drop(server);
    let runtime = root.join("runtime");
    let db = runtime.join("baton.sqlite3");
    let mut store = Store::open(&db).unwrap();
    let session = store
        .create_session(
            task["id"].as_str().unwrap(),
            workspace["id"].as_str().unwrap(),
            connection["id"].as_str().unwrap(),
            grant["id"].as_str().unwrap(),
            1,
            "crash-window",
        )
        .unwrap();
    store
        .mark_dispatch_started(&session.intent_id, &session.id)
        .unwrap();
    drop(store);
    // Simulates the durable sent marker immediately before a crash; there is no provider child to replay.
    let server = Server::start(&runtime);
    let unknown =
        result(server.request(json!({"method":"sessions.get","params":{"session_id":session.id}})));
    assert_eq!(unknown["state"], "outcome_unknown");
    let response=server.request(json!({"method":"sessions.cancel","params":{"session_id":session.id},"expected_revision":unknown["revision"],"idempotency_key":"cancel-unknown"}));
    assert_eq!(response["error"]["code"], "OUTCOME_UNKNOWN");
    assert_eq!(
        result(server.request(json!({"method":"sessions.get","params":{"session_id":session.id}})))
            ["state"],
        "outcome_unknown"
    );
    let c = rusqlite::Connection::open(&db).unwrap();
    c.execute(
        "UPDATE workspace_leases SET expires_at=0 WHERE resource=?1",
        [workspace["id"].as_str().unwrap()],
    )
    .unwrap();
    drop(c);
    let other=result(server.request(json!({"method":"tasks.create","params":{"project_id":project["id"],"goal":"must not race unknown writer"},"idempotency_key":"other-task"})));
    let response=server.request(json!({"method":"sessions.create","params":{"task_id":other["id"],"workspace_id":workspace["id"],"connection_id":connection["id"],"grant_id":grant["id"]},"expected_revision":1,"idempotency_key":"must-not-replay"}));
    assert_eq!(response["error"]["code"], "WORKSPACE_BUSY");
    assert!(!Path::new(workspace["root"].as_str().unwrap())
        .join("fixture-started")
        .exists());
    drop(server);
    drop(_dir);
}
#[test]
fn slow_socket_and_heavy_registry_request_cannot_starve_owned_session_cancellation() {
    use std::{io::Write, os::unix::net::UnixStream};
    let f = Scenario::new(10_000);
    let s = f.create();
    let ready = f.ready(&s["id"]);
    let running = f.send(&ready, "hang", "hang");
    let mut slow = UnixStream::connect(f.server.runtime.join("batond.sock")).unwrap();
    slow.write_all(b"{").unwrap();
    let before = Instant::now();
    let refused=f.server.request(json!({"method":"projects.import","params":{"path":f.root.join("source"),"name":"would require Git"},"idempotency_key":"heavy"}));
    assert_eq!(refused["error"]["code"], "WORKSPACE_BUSY");
    let requested = f.cancel(&running);
    assert_eq!(requested["state"], "cancel_requested");
    assert!(
        before.elapsed() < Duration::from_secs(2),
        "Slow peer or Git admission must not block cancellation actor"
    );
    drop(slow);
    wait(
        &f.server,
        "sessions.get",
        json!({"session_id":s["id"]}),
        |s| s["state"] == "stopped",
    );
}
#[test]
fn strict_native_enforcement_requires_evidence_and_no_process_is_launched() {
    let f = Scenario::new(10_000);
    let response=f.server.request(json!({"method":"sessions.create","params":{"task_id":f.task["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"grant_id":f.grant["id"],"require_enforcement":{"identity":true}},"expected_revision":1,"idempotency_key":"strict-identity"}));
    assert_eq!(response["error"]["code"], "ENFORCEMENT_UNAVAILABLE");
    assert!(!Path::new(f.workspace["root"].as_str().unwrap())
        .join("fixture-started")
        .exists());
}
#[test]
fn proven_no_process_started_releases_reservations_without_replaying_an_unknown_effect() {
    let f = Scenario::new(10_000);
    let exe = Path::new(f.connection["executable"].as_str().unwrap());
    std::fs::set_permissions(exe, std::fs::Permissions::from_mode(0o600)).unwrap();
    let s = f.create();
    let stopped = wait(
        &f.server,
        "sessions.get",
        json!({"session_id":s["id"]}),
        |v| v["state"] == "stopped",
    );
    assert_eq!(stopped["last_error"], "initialization_failed");
    let task = result(
        f.server
            .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
    );
    assert_eq!(task["state"], "draft");
    let events = result(
        f.server
            .request(json!({"method":"events.subscribe","params":{"cursor":0,"limit":500}})),
    );
    assert!(events["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["aggregate_id"] == s["id"]
            && e["kind"] == "session.stopped"
            && e["payload"]["no_process_started"] == true));
    std::fs::set_permissions(exe, std::fs::Permissions::from_mode(0o700)).unwrap();
    let retry=result(f.server.request(json!({"method":"sessions.create","params":{"task_id":f.task["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"grant_id":f.grant["id"]},"expected_revision":task["revision"],"idempotency_key":"explicit-new-attempt"})));
    let ready = f.ready(&retry["id"]);
    f.cancel(&ready);
    wait(
        &f.server,
        "sessions.get",
        json!({"session_id":retry["id"]}),
        |v| v["state"] == "stopped",
    );
}
#[test]
fn malicious_git_metadata_fifo_cannot_block_live_admission_status_or_owned_stop() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let f = Scenario::new(10_000);
    let session = f.create();
    let ready = f.ready(&session["id"]);
    let root = Path::new(f.workspace["root"].as_str().unwrap());
    let gitfile = std::fs::read_to_string(root.join(".git")).unwrap();
    let gitdir = PathBuf::from(gitfile.trim().strip_prefix("gitdir: ").unwrap());
    let common = gitdir.join("commondir");
    std::fs::remove_file(&common).unwrap();
    let fifo = CString::new(common.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let before = Instant::now();
    let refused=f.server.request(json!({"method":"sessions.send","params":{"session_id":session["id"],"text":"must not hang on FIFO"},"expected_revision":ready["revision"],"idempotency_key":"fifo-send"}));
    assert_eq!(refused["error"]["code"], "INPUT_CHANGED");
    let current = result(
        f.server
            .request(json!({"method":"sessions.get","params":{"session_id":session["id"]}})),
    );
    assert_eq!(current["state"], "ready");
    f.cancel(&current);
    assert!(
        before.elapsed() < Duration::from_secs(2),
        "Filesystem admission must never block actor cancellation"
    );
    wait(
        &f.server,
        "sessions.get",
        json!({"session_id":session["id"]}),
        |s| s["state"] == "stopped",
    );
}

#[test]
fn native_codex_fixture_is_bound_to_verified_runtime_and_real_turn_terminal() {
    use agentbaton_config::runtime::{RuntimeRequirements, RuntimeRoot, SnapshotDescriptor};
    let f = Scenario::with_provider(10_000, "codex");
    assert_eq!(f.connection["protocol_route"], "codex_app_server");
    assert_eq!(f.connection["identity_status"], "unknown");
    let starting = f.create();
    let ready = f.ready(&starting["id"]);
    let binding = &ready["runtime_snapshot"];
    assert_eq!(binding["session_id"], ready["id"]);
    assert_eq!(binding["attempt_id"], ready["attempt_id"]);
    assert_eq!(
        binding["connection_config_revision"],
        f.connection["config_revision"]
    );
    let descriptor: SnapshotDescriptor =
        serde_json::from_value(binding["snapshot"].clone()).unwrap();
    let root = RuntimeRoot::open(&f.root.join("runtime/runtime-config")).unwrap();
    let snapshot = root
        .verify_before_launch(&descriptor, RuntimeRequirements::default())
        .unwrap();
    assert_eq!(
        snapshot.plan_digest(),
        binding["plan_digest"].as_str().unwrap()
    );
    assert!(snapshot
        .manifest()
        .files
        .iter()
        .any(|file| file.relative_path == Path::new("codex/config.toml")));
    assert!(snapshot.codex_settings().unwrap().model_name().is_none());
    assert_eq!(
        serde_json::to_value(snapshot.manifest().claims.native_runtime_loading).unwrap(),
        "unknown"
    );
    f.send(&ready, "你好 actual owned native fixture", "native-turn");
    let ready = f.ready(&starting["id"]);
    assert_eq!(ready["turn_state"], "completed");
    let events = result(
        f.server
            .request(json!({"method":"events.subscribe","params":{"cursor":0,"limit":500}})),
    );
    assert!(events
        .to_string()
        .contains("native-fixture:你好 actual owned native fixture"));
    let task = result(
        f.server
            .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
    );
    assert_eq!(
        task["state"], "running",
        "Native turn completion is not task acceptance"
    );
    f.send(&ready, "ack_only", "native-ack");
    std::thread::sleep(Duration::from_millis(30));
    let active = result(
        f.server
            .request(json!({"method":"sessions.get","params":{"session_id":ready["id"]}})),
    );
    assert_eq!(
        active["state"], "running",
        "Submission ACK is not a terminal notification"
    );
    f.cancel(&active);
    wait(
        &f.server,
        "sessions.get",
        json!({"session_id":ready["id"]}),
        |value| value["state"] == "stopped",
    );
}

#[test]
fn unsafe_configuration_namespace_has_truthful_local_commit_and_no_provider_start() {
    let f = Scenario::new(10_000);
    let namespace = f.root.join("runtime/runtime-config");
    std::fs::create_dir(&namespace).unwrap();
    std::fs::set_permissions(&namespace, std::fs::Permissions::from_mode(0o755)).unwrap();
    let response=f.server.request(json!({"method":"sessions.create","params":{"task_id":f.task["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"grant_id":f.grant["id"]},"expected_revision":1,"idempotency_key":"unsafe-config"}));
    assert_eq!(response["error"]["code"], "PERMISSION_DENIED");
    assert_eq!(response["error"]["side_effect_state"], "confirmed");
    assert!(response["error"]["details_ref"]
        .as_str()
        .unwrap()
        .starts_with("local-session-recorded-no-provider-start:"));
    assert_eq!(
        std::fs::metadata(&namespace).unwrap().permissions().mode() & 0o777,
        0o755,
        "Existing user permissions are not changed"
    );
    assert!(!Path::new(f.workspace["root"].as_str().unwrap())
        .join("fixture-started")
        .exists());
    let sessions = result(
        f.server
            .request(json!({"method":"sessions.list","params":{}})),
    );
    assert_eq!(sessions.as_array().unwrap().len(), 1);
    assert_eq!(sessions[0]["state"], "stopped");
    let task = result(
        f.server
            .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
    );
    assert_eq!(task["state"], "draft");
    let replay = result(f.server.request(json!({"method":"sessions.create","params":{"task_id":f.task["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"grant_id":f.grant["id"]},"expected_revision":1,"idempotency_key":"unsafe-config"})));
    assert_eq!(replay["id"], sessions[0]["id"]);
    assert_eq!(replay["state"], "stopped");
    let db = rusqlite::Connection::open_with_flags(
        f.root.join("runtime/baton.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let (consumed, active): (i64, i64) = db
        .query_row(
            "SELECT consumed,active FROM budgets WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((consumed, active), (1, 0));
}

#[test]
fn legacy_connection_without_approved_plan_never_silently_inherits_a_new_profile() {
    let f = Scenario::new(10_000);
    // Controlled legacy-record fault injection, not a product SQL endpoint.
    let db = rusqlite::Connection::open(f.root.join("runtime/baton.sqlite3")).unwrap();
    db.execute(
        "UPDATE connections SET document=json_remove(document,'$.runtime_plan') WHERE id=?1",
        [f.connection["id"].as_str().unwrap()],
    )
    .unwrap();
    let response=f.server.request(json!({"method":"sessions.create","params":{"task_id":f.task["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"grant_id":f.grant["id"]},"expected_revision":1,"idempotency_key":"legacy-config"}));
    assert_eq!(response["error"]["code"], "INPUT_CHANGED");
    assert_eq!(response["error"]["side_effect_state"], "confirmed");
    assert!(!Path::new(f.workspace["root"].as_str().unwrap())
        .join("fixture-started")
        .exists());
    let sessions = result(
        f.server
            .request(json!({"method":"sessions.list","params":{}})),
    );
    assert_eq!(sessions[0]["state"], "stopped");
    let snapshots: i64 = db
        .query_row(
            "SELECT count(*) FROM session_runtime_snapshots",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(snapshots, 0);
}

#[test]
fn documented_acp_routes_deliver_only_their_owned_configuration_to_generated_children() {
    for (provider, route) in [
        ("qoder", "qoder_native_acp"),
        ("claude_code", "claude_agent_acp"),
    ] {
        let f = Scenario::with_provider(10_000, provider);
        assert_eq!(f.connection["protocol_route"], route);
        let session = f.create();
        let ready = f.ready(&session["id"]);
        assert!(ready["runtime_snapshot"]["snapshot"]["digest"].is_string());
        f.send(&ready, "env", "approved-route-turn");
        let ready = f.ready(&session["id"]);
        assert_eq!(ready["turn_state"], "completed");
        let events = result(
            f.server
                .request(json!({"method":"events.subscribe","params":{"cursor":0,"limit":500}})),
        );
        assert!(!events.to_string().contains("OPENAI_API_KEY"));
        assert!(!events.to_string().contains("GH_TOKEN"));
        f.cancel(&ready);
        wait(
            &f.server,
            "sessions.get",
            json!({"session_id":session["id"]}),
            |s| s["state"] == "stopped",
        );
    }
}
