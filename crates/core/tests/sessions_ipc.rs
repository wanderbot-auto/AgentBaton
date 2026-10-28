//! Real generated fixture child only. No vendor executable, identity, model or network is invoked.
use agentbaton_core::ipc::{call, Daemon};
use serde_json::{json, Value};
use std::{
    os::unix::fs::{DirBuilderExt, PermissionsExt},
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
        let method = v["method"].clone();
        let key = v["idempotency_key"].clone();
        let started = Instant::now();
        let response = call(&self.runtime, v);
        if let Err(error) = &response {
            eprintln!(
                "FIXTURE_RPC_TRANSPORT_FAILURE method={method} idempotency_key={key} elapsed_ms={} error={error:?}",
                started.elapsed().as_millis()
            );
        }
        let response = response.unwrap();
        if response.get("error").is_some() {
            eprintln!(
                "FIXTURE_RPC_FAILURE method={method} idempotency_key={key} elapsed_ms={} response={response}",
                started.elapsed().as_millis()
            );
        }
        response
    }
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown();
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
  if text.startswith('{'):
   contract=json.loads(text)
   assert contract['kind']=='agentbaton_goal_task_request' and contract['schema_version']==1
   assert set(contract)==({'schema_version','kind','goal','node','run_id','plan_revision','plan_digest','workspace','inputs','outputs','policy','lineage','budget','idempotency_key','return_location'}|({'task_binding','validation_contracts'} if 'task_binding' in contract else set()))
   assert ('validation_contracts' in contract)==('task_binding' in contract)
   assert contract['workspace']['canonical_root']==os.getcwd()
   assert contract['lineage']['attempt_id'] and contract['plan_digest'] and contract['return_location']['node_id']==contract['node']['id']
   if contract['inputs'][0].get('candidate'):
    import hashlib
    candidate=contract['inputs'][0]['candidate']
    assert contract['inputs'][0]['artifact']['relative_path']==candidate['relative_path']
    assert contract['inputs'][0]['sha256']==candidate['sha256'] and candidate['producer_attempt_id']
    assert hashlib.sha256(open(candidate['relative_path'],'rb').read()).hexdigest()==candidate['sha256']
   else:assert contract['inputs'][0]['artifact']['relative_path'] in ('file','inputs/file')
   assert contract['outputs'][0]['artifact']['relative_path']=='reports/report.txt'
   assert contract['policy']['enforced']=={'filesystem_sandbox':False,'network_sandbox':False,'output_quota':False}
   text=contract['goal']
   if text in ('goal-data-report','goal-data-report-bad'):
    import hashlib
    root=contract['workspace']['input_revision']
    outputs={o['artifact']['artifact_id']:o['artifact']['relative_path'] for o in contract['outputs']}
    assert {o['artifact_id'] for o in contract['node']['outputs']}=={'report','report-2','report-3'}
    os.makedirs('reports',exist_ok=True)
    script=b'# locally generated reproducibility fixture, not executed\n'
    environment=b'{"fixture":"local generated ACP","seed":7}\n'
    open(outputs['report-2'],'wb').write(script);open(outputs['report-3'],'wb').write(environment)
    fixed=next(i for i in contract['inputs'] if i['artifact']['artifact_id']=='fixed-input')
    value=len(open(fixed['artifact']['relative_path'],'rb').read())
    metadata={'input_revision':root,'dataset_revision':root,'input_digests':{'fixed-input':fixed['sha256']},'table_artifact_id':'report','row_count':1,'dataset_rows':1,'units':{'x':'bytes'},'timezone':'UTC','seed':7,'scope':{'kind':'full'},'script_sha256':hashlib.sha256(script).hexdigest(),'environment_sha256':hashlib.sha256(environment).hexdigest()}
    open(outputs['report'],'w').write(json.dumps({'metadata':metadata,'rows':[{'x':20 if text.endswith('-bad') else value}]}))
   if text in ('goal-report','goal-report-v2','goal-report-drift','goal-report-hold'):
    if text=='goal-report-hold':
     open('fixture-goal-held','w').write(str(os.getpid()))
     while not os.path.exists('fixture-goal-release'):time.sleep(.005)
    os.makedirs('reports',exist_ok=True);open('reports/report.txt','w').write('fixed report v2' if text=='goal-report-v2' else 'fixed report')
    if text=='goal-report-drift':open('file','w').write('changed original')
  if text.startswith('goal-report-extra-'):
   index=text.removeprefix('goal-report-extra-')
   artifact='report-'+index;path='reports/'+artifact+'.txt'
   assert contract['node']['outputs']==[{'artifact_id':artifact,'contract_id':'output:'+artifact}]
   assert contract['node']['write_set']==[{'kind':'workspace','path':path}]
   selected=next(o for o in contract['outputs'] if o['artifact']['artifact_id']==artifact)
   assert selected['artifact']['relative_path']==path and contract['inputs'][0]['sha256']
   os.makedirs('reports',exist_ok=True);open(path,'w').write('extra report '+index)
  if text in ('hang','goal-partial-hang'):
   if text=='goal-partial-hang':
    os.makedirs('reports',exist_ok=True);open('reports/report.txt','w').write('partial report')
   open('fixture-hang','w').write(str(os.getpid()));continue
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
    // Fields drop in declaration order: shutdown and join the daemon before
    // deleting the workspace it still authenticates during worker cleanup.
    server: Server,
    _dir: tempfile::TempDir,
    root: PathBuf,
    project: Value,
    workspace: Value,
    connection: Value,
    grant: Value,
    task: Value,
}
fn graph_artifact_path(name: impl AsRef<Path>) -> PathBuf {
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/goal-validation-graph");
    std::fs::create_dir_all(&directory).unwrap();
    directory.join(name)
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

fn goal_scope_and_plan(f: &Scenario, goal: &str) -> Value {
    goal_scope_and_plan_with_reworks(f, goal, 0)
}
fn goal_scope_and_plan_with_reworks(f: &Scenario, goal: &str, reworks: u32) -> Value {
    goal_scope_and_plan_with_independent_nodes(f, goal, reworks, 1)
}
fn goal_scope_and_plan_with_independent_nodes(
    f: &Scenario,
    goal: &str,
    reworks: u32,
    count: u32,
) -> Value {
    goal_scope_and_plan_with_capacity(f, goal, reworks, count, count.max(2))
}
fn goal_scope_and_plan_with_capacity(
    f: &Scenario,
    goal: &str,
    reworks: u32,
    count: u32,
    max_nodes: u32,
) -> Value {
    goal_scope_and_plan_with_options(f, goal, reworks, count, max_nodes, 2, vec![])
}
fn goal_scope_and_plan_with_options(
    f: &Scenario,
    goal: &str,
    reworks: u32,
    count: u32,
    max_nodes: u32,
    attempts: u32,
    validators: Vec<agentbaton_core::goal_store::GoalValidatorRef>,
) -> Value {
    use agentbaton_core::{goal_plan::*, goal_store::*};
    let limits = BudgetLimits {
        attempts,
        parallel: 1,
        runtime_ms: 10_000,
        output_bytes: 8192,
        children: if goal.starts_with("goal-data-report") {
            3
        } else {
            count.max(2) - 1
        },
        reworks,
        tokens: None,
        cost_microunits: None,
    };
    let mut selection = GoalScopeSelection {
        task_id: f.task["id"].as_str().unwrap().into(),
        expected_task_revision: f.task["revision"].as_u64().unwrap(),
        grant_id: f.grant["id"].as_str().unwrap().into(),
        expected_grant_revision: f.grant["revision"].as_u64().unwrap(),
        max_nodes,
        new_max_children: max_nodes - 1,
        new_max_depth: 1,
        root_budget: limits.clone(),
        project_budget: limits,
        inputs: vec![agentbaton_workspaces::validation::ArtifactRequirement {
            artifact_id: "fixed-input".into(),
            relative_path: if f.workspace["identity"]["kind"]["kind"] == "directory" {
                "inputs/file".into()
            } else {
                "file".into()
            },
            max_bytes: 1024,
            expected_sha256: Some(agentbaton_workspaces::validation::sha256(b"baseline")),
        }],
        outputs: vec![GoalOutputSelection {
            artifact: agentbaton_workspaces::validation::ArtifactRequirement {
                artifact_id: "report".into(),
                relative_path: "reports/report.txt".into(),
                max_bytes: 1024,
                expected_sha256: None,
            },
            validators,
        }],
        writable_prefixes: vec!["reports".into()],
        owner_acceptance_criteria: "Owner reviews the exact report separately from process success"
            .into(),
    };
    for index in 2..=count {
        selection.outputs.push(GoalOutputSelection {
            artifact: agentbaton_workspaces::validation::ArtifactRequirement {
                artifact_id: format!("report-{index}"),
                relative_path: format!("reports/report-{index}.txt").into(),
                max_bytes: 1024,
                expected_sha256: None,
            },
            validators: if index <= 3
                && selection.outputs[0]
                    .validators
                    .iter()
                    .any(|v| v.kind == GoalValidatorKind::Data)
            {
                selection.outputs[0].validators.clone()
            } else {
                vec![]
            },
        });
    }
    let proposal = result(f.server.request(
        json!({"method":"goal_scopes.propose","params":selection,"idempotency_key":format!("goal-scope:{}",f.task["id"].as_str().unwrap())}),
    ));
    let scope = approve(&f.server, proposal);
    let view: GoalScopeView = serde_json::from_value(scope.clone()).unwrap();
    let contract = view.outputs["report"].clone();
    let node = PlanNode {
        id: "produce-report".into(),
        parent_id: None,
        kind: NodeKind::Work,
        goal: goal.into(),
        dependencies: vec![],
        workspace_id: view.workspace_id.clone(),
        executor: Executor::Connection {
            binding: Box::new(view.binding.clone()),
            program_sha256: view.program_sha256.clone(),
        },
        recipients: vec![],
        required_inputs: vec![RequiredInput {
            goal_task_id: view.task_id.clone(),
            artifact_id: "fixed-input".into(),
            contract_id: view.inputs["fixed-input"].contract_id.clone(),
            source: InputSource::ApprovedExternal {
                sha256: view.inputs["fixed-input"].sha256.clone(),
            },
        }],
        outputs: vec![PlannedOutput {
            artifact_id: "report".into(),
            contract_id: contract.clone(),
        }],
        required_validators: view.contracts[&contract].clone(),
        side_effect: SideEffect::WorkspaceWrite,
        write_set: vec![WriteTarget::Workspace {
            path: "reports/report.txt".into(),
        }],
        budget: BudgetLimits {
            attempts: 1,
            parallel: 1,
            runtime_ms: 5000,
            output_bytes: 2048,
            children: 0,
            reworks: 0,
            tokens: None,
            cost_microunits: None,
        },
        reservation: AttemptReservation {
            runtime_ms: if goal == "hang" { 1200 } else { 4000 },
            output_bytes: 2048,
            tokens: None,
            cost_microunits: None,
        },
        retry: RetryPolicy {
            max_attempts: 1,
            max_reworks: 0,
            retry_classes: vec![],
        },
        idempotency_key: "produce-report-once".into(),
        external_action_id: None,
    };
    let mut nodes = vec![node.clone()];
    for index in 2..=count {
        let artifact_id = format!("report-{index}");
        let contract = view.outputs[&artifact_id].clone();
        let mut independent = node.clone();
        independent.id = format!("independent-{index}");
        independent.idempotency_key = format!("independent-once-{index}");
        independent.kind = NodeKind::Summary;
        independent.goal = format!("goal-report-extra-{index}");
        independent.outputs = vec![PlannedOutput {
            artifact_id,
            contract_id: contract.clone(),
        }];
        independent.required_validators = view.contracts[&contract].clone();
        independent.write_set = vec![WriteTarget::Workspace {
            path: format!("reports/report-{index}.txt"),
        }];
        nodes.push(independent);
    }
    let plan = GoalPlanProposal {
        schema_version: 1,
        plan_id: format!("runtime-plan:{}", f.task["id"].as_str().unwrap()),
        goal: view.goal.clone(),
        scope_revision: view.authority_revision,
        revision: 1,
        expected_revision: 0,
        acceptance_rules: view.required_acceptance_rules.clone(),
        nodes,
    };
    result(f.server.request(json!({"method":"goal_plans.propose","params":{"scope_id":view.id,"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":0,"idempotency_key":format!("runtime-plan:{}",f.task["id"].as_str().unwrap())})));
    scope
}
fn prepare_goal_execution(f: &Scenario, scope: &Value) -> Value {
    prepare_goal_execution_at(f, scope, 1)
}
fn prepare_goal_execution_at(f: &Scenario, scope: &Value, plan_revision: u64) -> Value {
    let snapshot = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    );
    let proposal=result(f.server.request(json!({"method":"goal_runs.propose","params":{"scope_id":scope["id"],"expected_plan_revision":plan_revision,"expected_ledger_revision":snapshot["sequence"]},"idempotency_key":format!("execute-plan:{}",scope["id"].as_str().unwrap())})));
    assert_eq!(proposal["kind"], "goal_execution");
    assert_eq!(proposal["snapshot"]["new_workspace_authority"], false);
    let raw = serde_json::to_vec_pretty(&proposal).unwrap();
    let digest = agentbaton_workspaces::validation::sha256(&raw);
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/goal-runtime-core");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("generated-goal-execution-{digest}.json"));
    match std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(&raw).unwrap();
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            assert_eq!(std::fs::read(&path).unwrap(), raw)
        }
        Err(error) => panic!("cannot preserve generated proposal: {error}"),
    }
    eprintln!("GENERATED_GOAL_EXECUTION_PROPOSAL sha256={digest} compact_snapshot_bytes={} pretty_snapshot_bytes={} path={} runtime_source_sha256={}",serde_json::to_vec(&proposal["snapshot"]).unwrap().len(),serde_json::to_vec_pretty(&proposal["snapshot"]).unwrap().len(),path.display(),agentbaton_workspaces::validation::sha256(include_bytes!("../src/goal_runtime.rs")));
    proposal
}
fn approve_goal_execution(f: &Scenario, scope: &Value) -> Value {
    approve(&f.server, prepare_goal_execution(f, scope))
}

#[test]
fn actual_goal_native_approval_reserves_dispatches_complete_contract_and_reaps_candidate_without_accepting_task(
) {
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-report");
    let before = result(
        f.server
            .request(json!({"method":"sessions.list","params":{}})),
    );
    assert!(before.as_array().unwrap().is_empty());
    let run = approve_goal_execution(&f, &scope);
    let current = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":run["id"]}),
        |r| {
            matches!(
                r["run"]["state"].as_str(),
                Some("awaiting_acceptance" | "failed" | "blocked" | "outcome_unknown")
            )
        },
    );
    assert_eq!(current["run"]["state"], "awaiting_acceptance", "{current}");
    assert_eq!(current["nodes"][0]["state"], "candidate");
    assert_eq!(current["nodes"][0]["stop_confirmed"], true);
    assert!(current["nodes"][0]["actual_runtime_ms"].as_u64().unwrap() > 0);
    assert_eq!(current["nodes"][0]["actual_output_bytes"], 12);
    assert!(current["nodes"][0]["tokens"].is_null());
    assert!(current["nodes"][0]["cost_microunits"].is_null());
    let sid = &current["nodes"][0]["session_id"];
    let session = result(
        f.server
            .request(json!({"method":"sessions.get","params":{"session_id":sid}})),
    );
    assert_eq!(session["state"], "stopped");
    assert_eq!(session["turn_state"], "completed");
    let extra=f.server.request(json!({"method":"sessions.send","params":{"session_id":sid,"text":"unreserved"},"expected_revision":session["revision"],"idempotency_key":"unreserved-turn"}));
    assert_eq!(extra["error"]["code"], "AUTH_REQUIRED");
    assert_eq!(
        std::fs::read(
            PathBuf::from(f.workspace["root"].as_str().unwrap()).join("reports/report.txt")
        )
        .unwrap(),
        b"fixed report"
    );
    let task = result(
        f.server
            .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
    );
    assert_ne!(task["state"], "accepted");
    assert_eq!(current["task_accepted"], false);
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM goal_run_reservations", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    let counts: (u32, u32) = db
        .query_row(
            "SELECT consumed,active FROM budgets WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(counts, (1, 0));
}

#[test]
fn actual_goal_same_scope_replan_after_real_writes_keeps_history_and_budget_without_new_owner() {
    use agentbaton_core::goal_plan::*;
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan_with_reworks(&f, "goal-report", 1);
    let initial = approve_goal_execution(&f, &scope);
    let first = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":initial["id"]}),
        |r| r["run"]["state"] == "awaiting_acceptance",
    );
    let old_attempt = first["nodes"][0]["attempt_id"].clone();
    let old_session = first["nodes"][0]["session_id"].clone();
    let old_plan = result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
    );
    let mut plan: GoalPlanProposal = serde_json::from_value(old_plan["proposal"].clone()).unwrap();
    plan.revision = 2;
    plan.expected_revision = 1;
    plan.nodes[0].goal = "goal-report-v2".into();
    plan.nodes[0].budget.attempts = 2;
    plan.nodes[0].budget.output_bytes = 4096;
    plan.nodes[0].budget.reworks = 1;
    plan.nodes[0].retry.max_attempts = 2;
    plan.nodes[0].retry.max_reworks = 1;
    plan.nodes[0].retry.retry_classes = vec![FailureClass::ValidationFailed];
    let saved=result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"safe-plan-revision-2"})));
    assert_eq!(saved["dispatched"], false);
    let pending = result(
        f.server
            .request(json!({"method":"goal_runs.get","params":{"run_id":initial["id"]}})),
    );
    assert_eq!(pending["run"]["plan_revision"], 1);
    let snapshot = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    );
    let advanced=result(f.server.request(json!({"method":"goal_runs.advance_plan","params":{"run_id":initial["id"],"expected_plan_revision":2,"expected_ledger_revision":snapshot["sequence"]},"expected_revision":pending["run"]["revision"],"idempotency_key":"apply-safe-plan-revision-2"})));
    assert_eq!(advanced["plan_revision"], 2);
    let second = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":initial["id"]}),
        |r| r["run"]["state"] == "awaiting_acceptance" && r["run"]["plan_revision"] == 2,
    );
    let nodes = second["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0]["plan_revision"], 1);
    assert_eq!(nodes[0]["attempt_id"], old_attempt);
    assert_eq!(nodes[0]["session_id"], old_session);
    assert_eq!(nodes[0]["actual_output_bytes"], 12);
    assert_eq!(nodes[1]["plan_revision"], 2);
    assert_ne!(nodes[1]["attempt_id"], old_attempt);
    assert_eq!(nodes[1]["state"], "candidate");
    assert_eq!(nodes[1]["actual_output_bytes"], 15);
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(db.query_row("SELECT count(*) FROM approval_proposals WHERE json_extract(document,'$.kind')='goal_execution'",[],|row|row.get::<_,u32>(0)).unwrap(),1);
    assert_eq!(
        db.query_row(
            "SELECT consumed FROM budgets WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        db.query_row(
            "SELECT json_extract(document,'$.output_bytes') FROM goal_ledgers WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        27
    );
    assert_eq!(second["task_accepted"], false);
}

#[test]
fn actual_goal_cancel_retains_partial_report_and_only_confirms_after_owned_reap() {
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-partial-hang");
    let run = approve_goal_execution(&f, &scope);
    let marker = PathBuf::from(f.workspace["root"].as_str().unwrap()).join("fixture-hang");
    let live = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":run["id"]}),
        |r| r["nodes"][0]["state"] == "running" && marker.is_file(),
    );
    let cancel_request = json!({"method":"goal_runs.cancel","params":{"run_id":run["id"]},"expected_revision":live["run"]["revision"],"idempotency_key":"one-goal-cancel"});
    let receipt = result(f.server.request(cancel_request.clone()));
    assert_eq!(receipt["state"], "cancel_requested");
    let current = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":run["id"]}),
        |r| r["run"]["state"] == "cancelled",
    );
    assert_eq!(current["nodes"][0]["state"], "cancelled");
    assert_eq!(current["nodes"][0]["stop_confirmed"], true);
    assert_eq!(current["nodes"][0]["actual_output_bytes"], 14);
    assert_eq!(current["task_accepted"], false);
    assert_eq!(
        std::fs::read(
            PathBuf::from(f.workspace["root"].as_str().unwrap()).join("reports/report.txt")
        )
        .unwrap(),
        b"partial report"
    );
    assert_eq!(result(f.server.request(cancel_request)), receipt);
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_leases WHERE holder!=''",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn actual_goal_timeout_and_malformed_terminal_never_replay_or_become_task_acceptance() {
    for goal in ["hang", "malformed"] {
        let f = Scenario::new(10_000);
        let scope = goal_scope_and_plan(&f, goal);
        let run = approve_goal_execution(&f, &scope);
        let current = wait(
            &f.server,
            "goal_runs.get",
            json!({"run_id":run["id"]}),
            |r| r["run"]["state"] == "outcome_unknown",
        );
        assert_eq!(current["nodes"][0]["state"], "outcome_unknown");
        assert_eq!(current["nodes"][0]["stop_confirmed"], true);
        assert_eq!(current["task_accepted"], false);
        assert!(current["nodes"][0]["actual_output_bytes"].is_null());
        if goal == "hang" {
            assert!(PathBuf::from(f.workspace["root"].as_str().unwrap())
                .join("fixture-hang")
                .is_file());
            assert!(current["nodes"][0]["actual_runtime_ms"].as_u64().unwrap() >= 1200);
        }
        let cancelled=result(f.server.request(json!({"method":"goal_runs.cancel","params":{"run_id":run["id"]},"expected_revision":current["run"]["revision"],"idempotency_key":"cancel-unknown-effect"})));
        assert_eq!(cancelled["state"], "outcome_unknown");
        assert_eq!(cancelled["cancel_requested"], true);
        let after = result(
            f.server
                .request(json!({"method":"goal_runs.get","params":{"run_id":run["id"]}})),
        );
        assert_eq!(after["run"]["state"], "outcome_unknown");
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |row| row
                .get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM goal_run_reservations", [], |row| row
                .get::<_, u32>(
                0
            ))
            .unwrap(),
            1
        );
    }
}

fn another_goal_root(f: &mut Scenario, index: u32) {
    let source = f.root.join("source");
    let head = git(&source, &["rev-parse", "HEAD"]);
    f.workspace=result(f.server.request(json!({"method":"workspaces.create","params":{"project_id":f.project["id"],"expected_head":head},"expected_revision":f.project["revision"],"idempotency_key":format!("new-root-workspace:{index}")})));
    let p=result(f.server.request(json!({"method":"grants.propose","params":{"project_id":f.project["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"max_attempts":2,"max_concurrency":1,"max_duration_ms":10000},"idempotency_key":format!("new-root-grant:{index}")})));
    f.grant = approve(&f.server, p);
    f.task=result(f.server.request(json!({"method":"tasks.create","params":{"project_id":f.project["id"],"goal":format!("Another bounded root {index}")},"idempotency_key":format!("new-root-task:{index}")})));
}
#[test]
fn actual_goal_reservation_failure_rolls_back_attempt_session_lease_and_all_shared_counters() {
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-report");
    let proposal = prepare_goal_execution(&f, &scope);
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER generated_goal_reservation_fault BEFORE INSERT ON goal_run_reservations BEGIN SELECT RAISE(ABORT,'generated exact transaction fault'); END;").unwrap();
    let run = approve(&f.server, proposal);
    let current = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":run["id"]}),
        |r| r["run"]["state"] == "blocked",
    );
    assert_eq!(current["nodes"][0]["state"], "not_started");
    assert!(current["nodes"][0]["attempt_id"].is_null());
    for table in [
        "attempts",
        "sessions",
        "dispatch_intents",
        "goal_run_reservations",
    ] {
        assert_eq!(
            db.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                .get::<_, u32>(0))
                .unwrap(),
            0,
            "{table}"
        );
    }
    let totals: (u32, u32) = db
        .query_row("SELECT sum(consumed),sum(active) FROM budgets", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(totals, (0, 0));
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_leases WHERE holder!=''",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert!(!PathBuf::from(f.workspace["root"].as_str().unwrap())
        .join("fixture-started")
        .exists());
    assert_eq!(current["task_accepted"], false);
}
#[test]
fn actual_goal_two_roots_share_project_consumption_and_third_root_cannot_reset_it() {
    let mut f = Scenario::new(10_000);
    let first_scope = goal_scope_and_plan(&f, "goal-report");
    let first = approve_goal_execution(&f, &first_scope);
    wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":first["id"]}),
        |r| r["run"]["state"] == "awaiting_acceptance",
    );
    let root1 = f.task["id"].as_str().unwrap().to_string();
    another_goal_root(&mut f, 2);
    let second_scope = goal_scope_and_plan(&f, "goal-report");
    let second = approve_goal_execution(&f, &second_scope);
    wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":second["id"]}),
        |r| r["run"]["state"] == "awaiting_acceptance",
    );
    let root2 = f.task["id"].as_str().unwrap().to_string();
    another_goal_root(&mut f, 3);
    let third_scope = goal_scope_and_plan(&f, "goal-report");
    let third = approve_goal_execution(&f, &third_scope);
    let blocked = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":third["id"]}),
        |r| r["run"]["state"] == "blocked",
    );
    assert!(blocked["nodes"][0]["attempt_id"].is_null());
    assert!(!PathBuf::from(f.workspace["root"].as_str().unwrap())
        .join("fixture-started")
        .exists());
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    let counts: (u32, u32) = db
        .query_row(
            "SELECT consumed,active FROM budgets WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(counts, (2, 0));
    for root in [root1, root2] {
        assert_eq!(
            db.query_row(
                "SELECT consumed FROM budgets WHERE scope=?1",
                [format!("root:{root}")],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
            1
        );
    }
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM goal_run_reservations", [], |row| row
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        2
    );
}

#[test]
fn actual_goal_complete_two_node_native_review_crosses_pretty_boundary_without_losing_fields() {
    use agentbaton_core::goal_plan::*;
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-report");
    let view = result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
    );
    let mut plan: GoalPlanProposal = serde_json::from_value(view["proposal"].clone()).unwrap();
    let mut summary = plan.nodes[0].clone();
    summary.id = "aaa-owner-review".into();
    summary.kind = NodeKind::OwnerApproval;
    summary.executor = Executor::Owner {
        rule_id: "goal-owner-review".into(),
    };
    summary.required_validators = vec!["goal-owner-review".into()];
    summary.dependencies = vec!["produce-report".into()];
    summary.required_inputs = vec![RequiredInput {
        goal_task_id: plan.goal.task_id.clone(),
        artifact_id: "report".into(),
        contract_id: scope["outputs"]["report"].as_str().unwrap().into(),
        source: InputSource::Node {
            node_id: "produce-report".into(),
            candidate_for_validation: true,
        },
    }];
    summary.goal = format!(
        "Summarize the fixed input without changing any permission. {}",
        "bounded-context ".repeat(300)
    );
    summary.side_effect = SideEffect::AdvisoryReadOnly;
    summary.outputs.clear();
    summary.write_set.clear();
    summary.idempotency_key = "summary-only-once".into();
    plan.nodes.insert(0, summary);
    plan.revision = 2;
    plan.expected_revision = 1;
    result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"bounded-large-review-plan"})));
    let proposal = prepare_goal_execution_at(&f, &scope, 2);
    let compact = serde_json::to_vec(&proposal["snapshot"]).unwrap();
    let pretty = serde_json::to_vec_pretty(&proposal["snapshot"]).unwrap();
    assert!(compact.len() <= 16384);
    assert!(pretty.len()>16384,"This real Core proposal should exercise lossless native compact display, got compact={} pretty={}",compact.len(),pretty.len());
    assert_eq!(
        proposal["snapshot"]["plan"]["proposal"]["nodes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        result(
            f.server
                .request(json!({"method":"sessions.list","params":{}}))
        )
        .as_array()
        .unwrap()
        .len(),
        0
    );
}

#[test]
fn actual_goal_replan_cannot_introduce_unmetered_finite_token_or_cost_cap() {
    use agentbaton_core::goal_plan::*;
    for dimension in ["tokens", "cost_microunits"] {
        let f = Scenario::new(10_000);
        let scope = goal_scope_and_plan_with_reworks(&f, "goal-report", 1);
        let initial = approve_goal_execution(&f, &scope);
        let before = wait(
            &f.server,
            "goal_runs.get",
            json!({"run_id":initial["id"]}),
            |r| r["run"]["state"] == "awaiting_acceptance",
        );
        let current = result(
            f.server
                .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
        );
        let mut plan: GoalPlanProposal =
            serde_json::from_value(current["proposal"].clone()).unwrap();
        plan.revision = 2;
        plan.expected_revision = 1;
        plan.nodes[0].budget.attempts = 2;
        plan.nodes[0].budget.reworks = 1;
        plan.nodes[0].budget.output_bytes = 4096;
        plan.nodes[0].retry.max_attempts = 2;
        plan.nodes[0].retry.max_reworks = 1;
        if dimension == "tokens" {
            plan.nodes[0].budget.tokens = Some(100);
            plan.nodes[0].reservation.tokens = Some(10);
        } else {
            plan.nodes[0].budget.cost_microunits = Some(100);
            plan.nodes[0].reservation.cost_microunits = Some(10);
        }
        result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"unsupported-finite-revision"})));
        let snap = result(
            f.server
                .request(json!({"method":"system.snapshot","params":{}})),
        );
        let response=f.server.request(json!({"method":"goal_runs.advance_plan","params":{"run_id":initial["id"],"expected_plan_revision":2,"expected_ledger_revision":snap["sequence"]},"expected_revision":before["run"]["revision"],"idempotency_key":"cannot-adopt-unmeasured-cap"}));
        assert_eq!(
            response["error"]["code"], "CAPABILITY_UNSUPPORTED",
            "{dimension}: {response}"
        );
        let after = result(
            f.server
                .request(json!({"method":"goal_runs.get","params":{"run_id":initial["id"]}})),
        );
        assert_eq!(after["run"]["plan_revision"], 1);
        assert_eq!(after["nodes"].as_array().unwrap().len(), 1);
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |row| row
                .get::<_, u32>(0))
                .unwrap(),
            1
        );
    }
}
#[test]
fn actual_goal_output_reservation_overrun_blocks_renamed_replan_without_erasing_observed_bytes() {
    use agentbaton_core::goal_plan::*;
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan_with_reworks(&f, "goal-report", 1);
    let current = result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
    );
    let mut plan: GoalPlanProposal = serde_json::from_value(current["proposal"].clone()).unwrap();
    plan.revision = 2;
    plan.expected_revision = 1;
    plan.nodes[0].reservation.output_bytes = 1;
    result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"tiny-output-reservation"})));
    let initial = approve(&f.server, prepare_goal_execution_at(&f, &scope, 2));
    let blocked = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":initial["id"]}),
        |r| r["run"]["state"] == "blocked",
    );
    assert_eq!(blocked["nodes"][0]["actual_output_bytes"], 12);
    assert_eq!(
        blocked["nodes"][0]["node"]["reservation"]["output_bytes"],
        1
    );
    assert_eq!(blocked["nodes"][0]["stop_confirmed"], true);
    plan.revision = 3;
    plan.expected_revision = 2;
    plan.nodes[0].id = "renamed-node-must-not-reset-budget".into();
    plan.nodes[0].reservation.output_bytes = 2048;
    result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":2,"idempotency_key":"rename-after-overrun"})));
    let snap = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    );
    let response=f.server.request(json!({"method":"goal_runs.advance_plan","params":{"run_id":initial["id"],"expected_plan_revision":3,"expected_ledger_revision":snap["sequence"]},"expected_revision":blocked["run"]["revision"],"idempotency_key":"cannot-escape-overrun"}));
    assert_eq!(response["error"]["code"], "RESOURCE_LIMIT");
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("per-attempt reservation"));
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT json_extract(document,'$.output_bytes') FROM goal_ledgers WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        12
    );
}
#[test]
fn actual_goal_strict_unknown_requirements_and_unapproved_dispatch_are_rejected() {
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-report");
    let snapshot = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    );
    for field in [
        "require_output_quota",
        "filesystem_sandbox",
        "network_sandbox",
        "approved",
        "tokens_enforced",
    ] {
        let mut parameters = json!({"scope_id":scope["id"],"expected_plan_revision":1,"expected_ledger_revision":snapshot["sequence"]});
        parameters[field] = json!(true);
        let response=f.server.request(json!({"method":"goal_runs.propose","params":parameters,"idempotency_key":format!("unsupported-strict:{field}")}));
        assert_eq!(
            response["error"]["code"], "INVALID_REQUEST",
            "{field}: {response}"
        );
    }
    let refused=f.server.request(json!({"method":"goal_plans.dispatch","params":{"scope_id":scope["id"]},"expected_revision":1,"idempotency_key":"old-preview-never-executes"}));
    assert_eq!(refused["error"]["code"], "AUTH_REQUIRED");
    assert_eq!(
        result(
            f.server
                .request(json!({"method":"sessions.list","params":{}}))
        )
        .as_array()
        .unwrap()
        .len(),
        0
    );
}

fn use_actual_dataset_workspace(f: &mut Scenario) {
    let source = f.root.join("goal-data-source");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("file"), b"baseline").unwrap();
    f.project=result(f.server.request(json!({"method":"projects.import","params":{"path":source,"name":"Generated Goal dataset"},"idempotency_key":"goal-data-project"})));
    let preview=result(f.server.request(json!({"method":"data_workspace_previews.start","params":{"project_id":f.project["id"],"files":[{"artifact_id":"source","relative_path":"file","max_bytes":1024,"expected_sha256":null}],"limits":{"max_file_bytes":1024,"max_total_bytes":1024,"max_files":1}},"expected_revision":f.project["revision"],"idempotency_key":"goal-data-preview"})));
    let preview = wait(
        &f.server,
        "data_workspace_previews.get",
        json!({"preview_id":preview["id"]}),
        |p| p["state"] == "verified",
    );
    let proposal=result(f.server.request(json!({"method":"data_workspaces.propose","params":{"preview_id":preview["id"]},"expected_revision":preview["revision"],"idempotency_key":"goal-data-copy"})));
    let started = approve(&f.server, proposal);
    let ready = wait(
        &f.server,
        "data_workspaces.get",
        json!({"creation_id":started["id"]}),
        |c| c["state"] == "ready",
    );
    let list = result(
        f.server
            .request(json!({"method":"workspaces.list","params":{"project_id":f.project["id"]}})),
    );
    f.workspace = list
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["id"] == ready["workspace_id"])
        .unwrap()
        .clone();
    assert_eq!(f.workspace["identity"]["kind"]["kind"], "directory");
    let proposal=result(f.server.request(json!({"method":"grants.propose","params":{"project_id":f.project["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"max_attempts":2,"max_concurrency":1,"max_duration_ms":10000},"idempotency_key":"goal-data-grant"})));
    f.grant = approve(&f.server, proposal);
    f.task=result(f.server.request(json!({"method":"tasks.create","params":{"project_id":f.project["id"],"goal":"Execute bounded data nodes using actual copied inputs"},"idempotency_key":"goal-data-task"})));
    std::fs::remove_dir_all(source).unwrap();
}
#[test]
fn actual_data_goal_proposal_recovery_is_historical_after_input_drift_but_new_authority_is_refused()
{
    let mut f = Scenario::new(10_000);
    use_actual_dataset_workspace(&mut f);
    let scope = goal_scope_and_plan(&f, "goal-report");
    let proposal = prepare_goal_execution(&f, &scope);
    let key = format!("execute-plan:{}", scope["id"].as_str().unwrap());
    std::fs::write(
        PathBuf::from(f.workspace["root"].as_str().unwrap()).join("inputs/file"),
        b"changed",
    )
    .unwrap();
    let recovered=result(f.server.request(json!({"method":"goal_runs.propose","params":proposal["snapshot"]["intent"],"idempotency_key":key})));
    assert_eq!(recovered, proposal);
    let fresh=f.server.request(json!({"method":"goal_runs.propose","params":proposal["snapshot"]["intent"],"idempotency_key":"do-not-reapprove-changed-data"}));
    assert_eq!(fresh["error"]["code"], "INPUT_CHANGED");
    let finalize=f.server.request(json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":"changed-data-cannot-finalize"}));
    assert_eq!(finalize["error"]["code"], "INPUT_CHANGED");
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM goal_runs", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
}
#[test]
fn actual_data_goal_executes_and_plan_advance_receipt_recovers_after_drift_without_reexecution() {
    use agentbaton_core::goal_plan::*;
    let mut f = Scenario::new(10_000);
    use_actual_dataset_workspace(&mut f);
    let scope = goal_scope_and_plan_with_reworks(&f, "goal-report", 1);
    let initial = approve_goal_execution(&f, &scope);
    let first = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":initial["id"]}),
        |r| r["run"]["state"] == "awaiting_acceptance",
    );
    let current = result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
    );
    let mut plan: GoalPlanProposal = serde_json::from_value(current["proposal"].clone()).unwrap();
    plan.revision = 2;
    plan.expected_revision = 1;
    plan.nodes[0].goal = "goal-report-v2".into();
    plan.nodes[0].budget.attempts = 2;
    plan.nodes[0].budget.reworks = 1;
    plan.nodes[0].budget.output_bytes = 4096;
    plan.nodes[0].retry.max_attempts = 2;
    plan.nodes[0].retry.max_reworks = 1;
    result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"data-goal-plan-v2"})));
    let snapshot = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    );
    let request = json!({"method":"goal_runs.advance_plan","params":{"run_id":initial["id"],"expected_plan_revision":2,"expected_ledger_revision":snapshot["sequence"]},"expected_revision":first["run"]["revision"],"idempotency_key":"data-goal-adopt-v2"});
    let receipt = result(f.server.request(request.clone()));
    let second = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":initial["id"]}),
        |r| r["run"]["state"] == "awaiting_acceptance" && r["run"]["plan_revision"] == 2,
    );
    assert_eq!(second["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(second["nodes"][1]["actual_output_bytes"], 15);
    std::fs::write(
        PathBuf::from(f.workspace["root"].as_str().unwrap()).join("inputs/file"),
        b"changed",
    )
    .unwrap();
    assert_eq!(result(f.server.request(request)), receipt);
    let snapshot = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    );
    let fresh=f.server.request(json!({"method":"goal_runs.advance_plan","params":{"run_id":initial["id"],"expected_plan_revision":3,"expected_ledger_revision":snapshot["sequence"]},"expected_revision":second["run"]["revision"],"idempotency_key":"changed-data-cannot-advance-new-key"}));
    assert_eq!(fresh["error"]["code"], "INPUT_CHANGED");
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        2
    );
}

#[test]
fn actual_goal_shared_budget_cannot_be_bypassed_by_normal_session_or_trusted_lowlevel_attempt() {
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-report");
    let run = approve_goal_execution(&f, &scope);
    wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":run["id"]}),
        |r| r["run"]["state"] == "awaiting_acceptance",
    );
    let task=result(f.server.request(json!({"method":"tasks.create","params":{"project_id":f.project["id"],"goal":"Ordinary bounded-policy bypass control"},"idempotency_key":"ordinary-bypass-task"})));
    let refused=f.server.request(json!({"method":"sessions.create","params":{"task_id":task["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"grant_id":f.grant["id"]},"expected_revision":task["revision"],"idempotency_key":"ordinary-session-bypass"}));
    assert_eq!(refused["error"]["code"], "AUTH_REQUIRED");
    let db_path = f.server.runtime.join("baton.sqlite3");
    let mut store = agentbaton_core::storage::Store::open(&db_path).unwrap();
    let task_id = task["id"].as_str().unwrap();
    store.configure_budget(false, task_id, 2, 1).unwrap();
    let view: agentbaton_core::goal_store::GoalScopeView = serde_json::from_value(scope).unwrap();
    let error = store
        .prepare_attempt(
            task_id,
            task["revision"].as_u64().unwrap(),
            view.binding,
            "lowlevel-time-output-bypass",
        )
        .unwrap_err();
    assert_eq!(error.code, "AUTH_REQUIRED");
    drop(store);
    let db = rusqlite::Connection::open(db_path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT consumed FROM budgets WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
}

struct OwnedGoalDaemonProcess {
    child: std::process::Child,
}
impl OwnedGoalDaemonProcess {
    fn start(runtime: &Path) -> Self {
        let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","actual_goal_live_daemon_sigkill_does_not_replay_or_infer_stop_from_an_empty_supervisor","--nocapture","--test-threads=1"]).env("AB_GOAL_LIVE_CRASH_RUNTIME",runtime).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("owned generated daemon exited before ready: {status}");
            }
            if call(runtime, json!({"method":"system.snapshot","params":{}})).is_ok() {
                return Self { child };
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("owned daemon did not become ready");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn kill_now(&mut self) {
        self.child.kill().unwrap();
        let status = self.child.wait().unwrap();
        assert!(!status.success());
    }
}
impl Drop for OwnedGoalDaemonProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
#[test]
fn actual_goal_live_daemon_sigkill_does_not_replay_or_infer_stop_from_an_empty_supervisor() {
    if let Some(runtime) = std::env::var_os("AB_GOAL_LIVE_CRASH_RUNTIME") {
        let daemon = Daemon::bind(Path::new(&runtime)).unwrap();
        daemon.run(Arc::new(AtomicBool::new(false))).unwrap();
        return;
    }
    let mut f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-partial-hang");
    f.server.shutdown();
    let mut daemon = OwnedGoalDaemonProcess::start(&f.server.runtime);
    let run = approve_goal_execution(&f, &scope);
    let marker = PathBuf::from(f.workspace["root"].as_str().unwrap()).join("fixture-hang");
    let running = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":run["id"]}),
        |r| r["nodes"][0]["state"] == "running" && marker.is_file(),
    );
    let original_session = running["nodes"][0]["session_id"].clone();
    let original_attempt = running["nodes"][0]["attempt_id"].clone();
    let pid = std::fs::read(&marker).unwrap();
    daemon.kill_now();
    let _restarted = OwnedGoalDaemonProcess::start(&f.server.runtime);
    let current = result(
        f.server
            .request(json!({"method":"goal_runs.get","params":{"run_id":run["id"]}})),
    );
    assert_eq!(current["run"]["state"], "outcome_unknown");
    assert_eq!(current["nodes"][0]["session_id"], original_session);
    assert_eq!(current["nodes"][0]["attempt_id"], original_attempt);
    assert_eq!(current["nodes"][0]["state"], "outcome_unknown");
    assert_eq!(current["nodes"][0]["stop_confirmed"], false);
    assert!(current["nodes"][0]["actual_runtime_ms"].is_null());
    assert!(current["nodes"][0]["actual_output_bytes"].is_null());
    let receipt=result(f.server.request(json!({"method":"goal_runs.cancel","params":{"run_id":run["id"]},"expected_revision":current["run"]["revision"],"idempotency_key":"no-owned-controller-is-not-a-stop-proof"})));
    assert_eq!(receipt["state"], "outcome_unknown");
    assert_eq!(receipt["cancel_requested"], true);
    assert_eq!(std::fs::read(marker).unwrap(), pid);
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM sessions", [], |row| row
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    let held: (u32, u32) = db
        .query_row(
            "SELECT consumed,active FROM budgets WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(held, (1, 1));
    assert_eq!(current["task_accepted"], false);
}

#[test]
fn actual_goal_independent_summary_and_long_node_ids_use_the_same_authorized_executor() {
    use agentbaton_core::goal_plan::*;
    for mode in ["summary", "long-id"] {
        let f = Scenario::new(10_000);
        let scope = goal_scope_and_plan(&f, "goal-report");
        let view = result(
            f.server
                .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
        );
        let mut plan: GoalPlanProposal = serde_json::from_value(view["proposal"].clone()).unwrap();
        plan.revision = 2;
        plan.expected_revision = 1;
        if mode == "summary" {
            plan.nodes[0].kind = NodeKind::Summary;
        } else {
            plan.nodes[0].id = "n".repeat(256);
        }
        result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"summary-or-long-id-plan"})));
        let run = approve(&f.server, prepare_goal_execution_at(&f, &scope, 2));
        let current = wait(
            &f.server,
            "goal_runs.get",
            json!({"run_id":run["id"]}),
            |r| {
                matches!(
                    r["run"]["state"].as_str(),
                    Some("awaiting_acceptance" | "blocked" | "outcome_unknown")
                )
            },
        );
        assert_eq!(
            current["run"]["state"], "awaiting_acceptance",
            "{mode}: {current}"
        );
        assert_eq!(current["nodes"][0]["actual_output_bytes"], 12);
        assert_eq!(current["nodes"][0]["stop_confirmed"], true);
        assert_eq!(current["task_accepted"], false);
    }
}

#[test]
fn actual_goal_completed_candidate_requires_unchanged_lower_owned_stop_receipts() {
    for mode in [
        "attempt_source",
        "supervisor_source",
        "supervisor_order",
        "goal_state",
    ] {
        let mut f = Scenario::new(10_000);
        let scope = goal_scope_and_plan(&f, "goal-report");
        let run = approve_goal_execution(&f, &scope);
        let current = wait(
            &f.server,
            "goal_runs.get",
            json!({"run_id":run["id"]}),
            |r| r["run"]["state"] == "awaiting_acceptance",
        );
        let sid = current["nodes"][0]["session_id"]
            .as_str()
            .unwrap()
            .to_string();
        f.server.shutdown();
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        match mode {
            "attempt_source" => {
                db.execute("UPDATE events SET document=json_set(document,'$.source','generated_untrusted_output') WHERE json_extract(document,'$.kind')='attempt.stopped'",[]).unwrap();
            }
            "supervisor_source" => {
                db.execute("UPDATE events SET document=json_set(document,'$.source','generated_untrusted_output') WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.aggregate_id')=?1",[&sid]).unwrap();
            }
            "supervisor_order" => {
                let doc:String=db.query_row("SELECT document FROM events WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.aggregate_id')=?1",[&sid],|row|row.get(0)).unwrap();
                db.execute("DELETE FROM events WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.aggregate_id')=?1",[&sid]).unwrap();
                db.execute("UPDATE events SET document=?1 WHERE json_extract(document,'$.kind')='session.launch_reserved' AND json_extract(document,'$.aggregate_id')=?2",rusqlite::params![doc,sid]).unwrap();
            }
            _ => {
                db.execute("UPDATE goal_run_nodes SET document=json_set(document,'$.actual_output_bytes',0)",[]).unwrap();
            }
        }
        drop(db);
        let store =
            agentbaton_core::storage::Store::open(&f.server.runtime.join("baton.sqlite3")).unwrap();
        let error = store
            .goal_run_details(run["id"].as_str().unwrap())
            .unwrap_err();
        assert!(
            matches!(error.code.as_str(), "PERMISSION_DENIED" | "INPUT_CHANGED"),
            "{mode}: {error:?}"
        );
    }
}

#[test]
fn actual_goal_changed_original_git_input_refuses_new_plan_or_advance_after_real_output() {
    use agentbaton_core::goal_plan::*;
    for action in ["revise", "advance"] {
        let f = Scenario::new(10_000);
        let scope = goal_scope_and_plan_with_reworks(&f, "goal-report", 1);
        let initial = approve_goal_execution(&f, &scope);
        let first = wait(
            &f.server,
            "goal_runs.get",
            json!({"run_id":initial["id"]}),
            |r| r["run"]["state"] == "awaiting_acceptance",
        );
        let old = result(
            f.server
                .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
        );
        let mut plan: GoalPlanProposal = serde_json::from_value(old["proposal"].clone()).unwrap();
        plan.revision = 2;
        plan.expected_revision = 1;
        plan.nodes[0].goal = "goal-report-v2".into();
        if action == "advance" {
            result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"valid-before-drift"})));
        }
        std::fs::write(
            PathBuf::from(f.workspace["root"].as_str().unwrap()).join("file"),
            b"changed original input",
        )
        .unwrap();
        let denied = if action == "revise" {
            f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"no-changed-git-plan"}))
        } else {
            let snap = result(
                f.server
                    .request(json!({"method":"system.snapshot","params":{}})),
            );
            f.server.request(json!({"method":"goal_runs.advance_plan","params":{"run_id":initial["id"],"expected_plan_revision":2,"expected_ledger_revision":snap["sequence"]},"expected_revision":first["run"]["revision"],"idempotency_key":"no-changed-git-advance"}))
        };
        assert_eq!(
            denied["error"]["code"], "INPUT_CHANGED",
            "{action}: {denied}"
        );
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row(
                "SELECT json_extract(document,'$.plan_revision') FROM goal_runs WHERE id=?1",
                [initial["id"].as_str().unwrap()],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            1
        );
    }
}

#[test]
fn actual_goal_same_plan_two_independent_nodes_both_execute_before_awaiting_acceptance() {
    for data in [false, true] {
        let mut f = Scenario::new(10_000);
        if data {
            use_actual_dataset_workspace(&mut f);
        }
        let scope = goal_scope_and_plan_with_independent_nodes(&f, "goal-report", 0, 2);
        let initial = approve_goal_execution(&f, &scope);
        let observed_first = std::cell::Cell::new(false);
        let final_state = wait(
            &f.server,
            "goal_runs.get",
            json!({"run_id":initial["id"]}),
            |r| {
                if !observed_first.get()
                    && r["nodes"][0]["state"] == "candidate"
                    && r["nodes"][1]["state"] == "not_started"
                {
                    let db =
                        rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
                    let attempts = db
                        .query_row("SELECT count(*) FROM attempts", [], |row| {
                            row.get::<_, u32>(0)
                        })
                        .unwrap();
                    eprintln!("GOAL_FRONTIER_OBSERVED first=candidate second=not_started attempts={attempts} run_state={}",r["run"]["state"]);
                    observed_first.set(true);
                }
                r["nodes"].as_array().unwrap().len() == 2
                    && r["nodes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|n| n["state"] == "candidate")
            },
        );
        assert_eq!(final_state["run"]["state"], "awaiting_acceptance");
        assert_eq!(final_state["run"]["id"], initial["id"]);
        assert_eq!(final_state["run"]["plan_revision"], 1);
        assert_eq!(final_state["task_accepted"], false);
        let nodes = final_state["nodes"].as_array().unwrap();
        assert_ne!(nodes[0]["attempt_id"], nodes[1]["attempt_id"]);
        assert_ne!(nodes[0]["session_id"], nodes[1]["session_id"]);
        assert_eq!(nodes[0]["actual_output_bytes"], 12);
        assert_eq!(nodes[1]["actual_output_bytes"], 14);
        for node in nodes {
            assert_eq!(node["stop_confirmed"], true);
            assert_eq!(node["plan_revision"], 1);
        }
        let root = PathBuf::from(f.workspace["root"].as_str().unwrap());
        assert_eq!(
            std::fs::read(root.join("reports/report.txt")).unwrap(),
            b"fixed report"
        );
        assert_eq!(
            std::fs::read(root.join("reports/report-2.txt")).unwrap(),
            b"extra report 2"
        );
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM goal_run_reservations", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM goal_plan_revisions", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM workspace_leases WHERE holder!=''",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
        for key in [
            format!("project:{}", f.project["id"].as_str().unwrap()),
            format!(
                "root-grant:{}",
                final_state["run"]["root_grant_id"].as_str().unwrap()
            ),
        ] {
            assert_eq!(
            db.query_row(
                "SELECT json_extract(document,'$.output_bytes') FROM goal_ledgers WHERE scope=?1",
                [&key],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            26
        );
            assert_eq!(db.query_row("SELECT json_extract(document,'$.observed_attempts') FROM goal_ledgers WHERE scope=?1",[&key],|r|r.get::<_,u32>(0)).unwrap(),2);
        }
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='task.accepted'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
        for key in [
            format!("project:{}", f.project["id"].as_str().unwrap()),
            format!("root:{}", f.task["id"].as_str().unwrap()),
        ] {
            assert_eq!(
                db.query_row("SELECT consumed FROM budgets WHERE scope=?1", [&key], |r| r
                    .get::<_, u32>(0))
                    .unwrap(),
                2
            );
            assert_eq!(
                db.query_row("SELECT active FROM budgets WHERE scope=?1", [&key], |r| r
                    .get::<_, u32>(
                    0
                ))
                .unwrap(),
                0
            );
        }
        assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.source')='baton_supervisor' AND json_extract(document,'$.payload.process_reaped')=1",[],|r|r.get::<_,u32>(0)).unwrap(),2);
    }
}

#[test]
fn actual_goal_same_plan_frontier_preserves_budget_cancel_unknown_revocation_and_input_guards() {
    use agentbaton_core::goal_plan::*;
    for mode in [
        "third_budget",
        "cancel",
        "unknown",
        "overrun",
        "revoked",
        "input_drift",
    ] {
        let f = Scenario::new(10_000);
        let goal = match mode {
            "cancel" | "revoked" => "goal-partial-hang",
            "unknown" => "malformed",
            "input_drift" => "goal-report-drift",
            _ => "goal-report",
        };
        let scope = goal_scope_and_plan_with_independent_nodes(
            &f,
            goal,
            0,
            if mode == "third_budget" { 3 } else { 2 },
        );
        let mut approved_revision = 1;
        if mode == "overrun" {
            let current = result(
                f.server
                    .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
            );
            let mut plan: GoalPlanProposal =
                serde_json::from_value(current["proposal"].clone()).unwrap();
            plan.revision = 2;
            plan.expected_revision = 1;
            plan.nodes[0].reservation.output_bytes = 1;
            result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"frontier-overrun-plan"})));
            approved_revision = 2;
        }
        let run = approve(
            &f.server,
            prepare_goal_execution_at(&f, &scope, approved_revision),
        );
        if matches!(mode, "cancel" | "revoked") {
            let marker = PathBuf::from(f.workspace["root"].as_str().unwrap()).join("fixture-hang");
            let live = wait(
                &f.server,
                "goal_runs.get",
                json!({"run_id":run["id"]}),
                |r| r["nodes"][0]["state"] == "running" && marker.exists(),
            );
            if mode == "cancel" {
                result(f.server.request(json!({"method":"goal_runs.cancel","params":{"run_id":run["id"]},"expected_revision":live["run"]["revision"],"idempotency_key":"frontier-owned-cancel"})));
            } else {
                result(f.server.request(json!({"method":"grants.revoke","params":{"grant_id":scope["root_grant_id"]},"expected_revision":scope["root_grant_revision"],"idempotency_key":"frontier-revoke"})));
            }
        }
        let current = wait(
            &f.server,
            "goal_runs.get",
            json!({"run_id":run["id"]}),
            |r| {
                matches!(
                    r["run"]["state"].as_str(),
                    Some("blocked" | "cancelled" | "outcome_unknown")
                ) && r["nodes"][0]["stop_confirmed"] == true
            },
        );
        let nodes = current["nodes"].as_array().unwrap();
        let expected = if mode == "third_budget" { 2 } else { 1 };
        for node in &nodes[expected..] {
            assert_eq!(node["state"], "not_started", "{mode}: {current}");
            assert!(node["attempt_id"].is_null());
        }
        assert_eq!(current["run"]["plan_revision"], approved_revision);
        assert_eq!(current["task_accepted"], false);
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            expected as u32,
            "{mode}"
        );
        assert_eq!(
            db.query_row(
                "SELECT consumed FROM budgets WHERE scope=?1",
                [format!("project:{}", f.project["id"].as_str().unwrap())],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            expected as u32
        );
        let seq = result(
            f.server
                .request(json!({"method":"system.snapshot","params":{}})),
        )["sequence"]
            .clone();
        for _ in 0..5 {
            let _ = result(
                f.server
                    .request(json!({"method":"goal_runs.get","params":{"run_id":run["id"]}})),
            );
        }
        assert_eq!(
            result(
                f.server
                    .request(json!({"method":"system.snapshot","params":{}}))
            )["sequence"],
            seq,
            "{mode}"
        );
        if mode == "overrun" {
            assert_eq!(nodes[0]["actual_output_bytes"], 12);
            assert_eq!(current["run"]["state"], "blocked");
        }
        if mode == "third_budget" {
            assert_eq!(nodes[1]["state"], "candidate");
            assert_eq!(current["run"]["state"], "blocked");
        }
        assert!(!PathBuf::from(f.workspace["root"].as_str().unwrap())
            .join(if mode == "third_budget" {
                "reports/report-3.txt"
            } else {
                "reports/report-2.txt"
            })
            .exists());
    }
}

#[test]
fn actual_goal_temporary_global_slot_waits_without_intents_then_executes_after_owned_completion() {
    for pending_version in [false, true] {
        let mut f = Scenario::new(10_000);
        let first_scope = goal_scope_and_plan(&f, "goal-report-hold");
        let first_root = PathBuf::from(f.workspace["root"].as_str().unwrap());
        another_goal_root(&mut f, 42);
        let second_scope = goal_scope_and_plan(&f, "goal-report");
        let first = approve_goal_execution(&f, &first_scope);
        wait(
            &f.server,
            "goal_runs.get",
            json!({"run_id":first["id"]}),
            |r| {
                r["nodes"][0]["state"] == "running" && first_root.join("fixture-goal-held").exists()
            },
        );
        let second = approve_goal_execution(&f, &second_scope);
        let second_root = PathBuf::from(f.workspace["root"].as_str().unwrap());
        assert_ne!(first_root, second_root);
        let mut advance_request = None;
        if pending_version {
            use agentbaton_core::goal_plan::GoalPlanProposal;
            let current = result(f.server.request(
                json!({"method":"goal_plans.get","params":{"scope_id":second_scope["id"]}}),
            ));
            let mut plan: GoalPlanProposal =
                serde_json::from_value(current["proposal"].clone()).unwrap();
            plan.revision = 2;
            plan.expected_revision = 1;
            plan.nodes[0].goal = "goal-report-v2".into();
            result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":second_scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"queued-save-version2-only"})));
            let paused = wait(
                &f.server,
                "goal_runs.get",
                json!({"run_id":second["id"]}),
                |r| r["run"]["state"] == "awaiting_acceptance",
            );
            assert_eq!(paused["run"]["plan_revision"], 1);
            assert_eq!(paused["nodes"][0]["state"], "not_started");
        }
        let quiet = result(
            f.server
                .request(json!({"method":"system.snapshot","params":{}})),
        )["sequence"]
            .clone();
        let until = Instant::now() + Duration::from_millis(200);
        while Instant::now() < until {
            let queued = result(
                f.server
                    .request(json!({"method":"goal_runs.get","params":{"run_id":second["id"]}})),
            );
            let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
            let total = db
                .query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap();
            if queued["run"]["state"]
                != if pending_version {
                    "awaiting_acceptance"
                } else {
                    "queued"
                }
            {
                let reason:String=db.query_row("SELECT json_extract(document,'$.payload.reason_code') FROM events WHERE json_extract(document,'$.kind')='goal.run_blocked' AND json_extract(document,'$.aggregate_id')=?1",[second["id"].as_str().unwrap()],|r|r.get(0)).unwrap();
                eprintln!(
                "TEMPORARY_GOAL_SLOT second_state={} total_attempts={total} second_node={} reason_code={reason}",
                queued["run"]["state"], queued["nodes"][0]["state"]
            );
            }
            assert_eq!(
                queued["run"]["state"],
                if pending_version {
                    "awaiting_acceptance"
                } else {
                    "queued"
                },
                "{queued}"
            );
            assert_eq!(queued["nodes"][0]["state"], "not_started");
            assert!(queued["nodes"][0]["session_id"].is_null());
            assert!(queued["nodes"][0]["attempt_id"].is_null());
            assert_eq!(total, 1);
            assert_eq!(
                result(
                    f.server
                        .request(json!({"method":"system.snapshot","params":{}}))
                )["sequence"],
                quiet
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        std::fs::write(
            first_root.join("fixture-goal-release"),
            b"release generated fixture",
        )
        .unwrap();
        let first_done = wait(
            &f.server,
            "goal_runs.get",
            json!({"run_id":first["id"]}),
            |r| r["run"]["state"] == "awaiting_acceptance",
        );
        if pending_version {
            let paused = result(
                f.server
                    .request(json!({"method":"goal_runs.get","params":{"run_id":second["id"]}})),
            );
            assert_eq!(paused["run"]["state"], "awaiting_acceptance");
            assert_eq!(paused["run"]["plan_revision"], 1);
            let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
            assert_eq!(
                db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                    .unwrap(),
                1
            );
            assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.run_plan_pending' AND json_extract(document,'$.aggregate_id')=?1",[second["id"].as_str().unwrap()],|r|r.get::<_,u32>(0)).unwrap(),1);
            let sequence = result(
                f.server
                    .request(json!({"method":"system.snapshot","params":{}})),
            )["sequence"]
                .clone();
            for _ in 0..5 {
                let _ =
                    result(f.server.request(
                        json!({"method":"goal_runs.get","params":{"run_id":second["id"]}}),
                    ));
            }
            assert_eq!(
                result(
                    f.server
                        .request(json!({"method":"system.snapshot","params":{}}))
                )["sequence"],
                sequence
            );
            let request = json!({"method":"goal_runs.advance_plan","params":{"run_id":second["id"],"expected_plan_revision":2,"expected_ledger_revision":sequence},"expected_revision":paused["run"]["revision"],"idempotency_key":"explicit-adopt-queued-version2"});
            let receipt = result(f.server.request(request.clone()));
            advance_request = Some((request, receipt));
        }
        let second_done = wait(
            &f.server,
            "goal_runs.get",
            json!({"run_id":second["id"]}),
            |r| r["run"]["state"] == "awaiting_acceptance",
        );
        for (index, done) in [&first_done, &second_done].iter().enumerate() {
            let node = done["nodes"].as_array().unwrap().last().unwrap();
            assert_eq!(node["state"], "candidate");
            assert_eq!(node["stop_confirmed"], true);
            assert_eq!(
                node["actual_output_bytes"],
                if pending_version && index == 1 {
                    15
                } else {
                    12
                }
            );
            assert_eq!(done["task_accepted"], false);
        }
        if let Some((request, receipt)) = advance_request {
            let sequence = result(
                f.server
                    .request(json!({"method":"system.snapshot","params":{}})),
            )["sequence"]
                .clone();
            assert_eq!(result(f.server.request(request)), receipt);
            assert_eq!(
                result(
                    f.server
                        .request(json!({"method":"system.snapshot","params":{}}))
                )["sequence"],
                sequence
            );
            assert_eq!(second_done["run"]["plan_revision"], 2);
            assert_eq!(second_done["nodes"].as_array().unwrap().len(), 2);
            assert_eq!(second_done["nodes"][0]["state"], "not_started");
            assert!(second_done["nodes"][0]["attempt_id"].is_null());
        }
        assert_ne!(
            first_done["nodes"][0]["attempt_id"],
            second_done["nodes"].as_array().unwrap().last().unwrap()["attempt_id"]
        );
        assert_eq!(
            std::fs::read(first_root.join("reports/report.txt")).unwrap(),
            b"fixed report"
        );
        assert_eq!(
            std::fs::read(second_root.join("reports/report.txt")).unwrap(),
            if pending_version {
                b"fixed report v2".as_slice()
            } else {
                b"fixed report".as_slice()
            }
        );
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row(
                "SELECT consumed FROM budgets WHERE scope=?1",
                [format!("project:{}", f.project["id"].as_str().unwrap())],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            2
        );
        assert_eq!(
            db.query_row(
                "SELECT json_extract(document,'$.output_bytes') FROM goal_ledgers WHERE scope=?1",
                [format!("project:{}", f.project["id"].as_str().unwrap())],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            if pending_version { 27 } else { 24 }
        );
        assert_eq!(db.query_row("SELECT count(*) FROM approval_proposals WHERE json_extract(document,'$.kind')='goal_execution'",[],|r|r.get::<_,u32>(0)).unwrap(),2);
    }
}

#[test]
fn actual_goal_fixed_concurrent_review_keeps_other_kinds_setup_and_corrupt_authority_refused() {
    let mut f = Scenario::new(10_000);
    let first_scope = goal_scope_and_plan(&f, "goal-report-hold");
    let first_root = PathBuf::from(f.workspace["root"].as_str().unwrap());
    another_goal_root(&mut f, 73);
    let second_scope = goal_scope_and_plan(&f, "goal-report");
    let legacy=result(f.server.request(json!({"method":"grants.propose","params":{"project_id":f.project["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"max_attempts":2,"max_concurrency":1,"max_duration_ms":10000},"idempotency_key":"legacy-kind-before-worker"})));
    let first = approve_goal_execution(&f, &first_scope);
    wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":first["id"]}),
        |r| r["nodes"][0]["state"] == "running" && first_root.join("fixture-goal-held").exists(),
    );
    let old=f.server.request(json!({"method":"approvals.finalize","params":{"proposal_id":legacy["id"]},"expected_revision":legacy["revision"],"idempotency_key":"legacy-not-idle"}));
    assert_eq!(old["error"]["code"], "WORKSPACE_BUSY");
    let setup=f.server.request(json!({"method":"workspaces.create","params":{"project_id":f.project["id"],"expected_head":"untrusted-not-used"},"expected_revision":f.project["revision"],"idempotency_key":"setup-must-remain-idle"}));
    assert_eq!(setup["error"]["code"], "WORKSPACE_BUSY");
    let seq = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    )["sequence"]
        .clone();
    let same=f.server.request(json!({"method":"goal_runs.propose","params":{"scope_id":first_scope["id"],"expected_plan_revision":1,"expected_ledger_revision":seq},"idempotency_key":"same-workspace-new-authority-refused"}));
    assert_eq!(same["error"]["code"], "WORKSPACE_BUSY");
    let proposal = prepare_goal_execution(&f, &second_scope);
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    let id = proposal["id"].as_str().unwrap();
    let original: String = db
        .query_row(
            "SELECT document FROM approval_proposals WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    let legacy_id = legacy["id"].as_str().unwrap();
    let legacy_original: String = db
        .query_row(
            "SELECT document FROM approval_proposals WHERE id=?1",
            [legacy_id],
            |r| r.get(0),
        )
        .unwrap();
    for mode in [
        "unknown_id",
        "renderer_kind",
        "oversize",
        "wrong_doc_id",
        "wrong_stored_kind",
        "tampered_snapshot",
        "forged_legacy_kind",
    ] {
        db.execute(
            "UPDATE approval_proposals SET document=?1 WHERE id=?2",
            rusqlite::params![original, id],
        )
        .unwrap();
        db.execute(
            "UPDATE approval_proposals SET document=?1 WHERE id=?2",
            rusqlite::params![legacy_original, legacy_id],
        )
        .unwrap();
        let mut request_id = id;
        match mode {
            "unknown_id" | "renderer_kind" => request_id = "unknown-owner-proposal",
            "oversize" => {
                db.execute(
                    "UPDATE approval_proposals SET document=?1 WHERE id=?2",
                    rusqlite::params![original.clone() + &" ".repeat(1048576), id],
                )
                .unwrap();
            }
            "wrong_doc_id" => {
                db.execute("UPDATE approval_proposals SET document=json_set(document,'$.id','foreign-stored-proposal') WHERE id=?1",[id]).unwrap();
            }
            "wrong_stored_kind" => {
                db.execute("UPDATE approval_proposals SET document=json_set(document,'$.kind','goal_execution_other') WHERE id=?1",[id]).unwrap();
            }
            "tampered_snapshot" => {
                db.execute("UPDATE approval_proposals SET document=json_set(document,'$.snapshot.new_workspace_authority',json('true')) WHERE id=?1",[id]).unwrap();
            }
            _ => {
                db.execute("UPDATE approval_proposals SET document=json_set(document,'$.kind','goal_execution') WHERE id=?1",[legacy_id]).unwrap();
                request_id = legacy_id;
            }
        }
        let mut params = json!({"proposal_id":request_id});
        if mode == "renderer_kind" {
            params["kind"] = json!("goal_execution");
        }
        let response=f.server.request(json!({"method":"approvals.finalize","params":params,"expected_revision":1,"idempotency_key":format!("no-false-goal-authority-{mode}")}));
        let code = response["error"]["code"]
            .as_str()
            .expect("all corrupt/foreign authority must be refused");
        assert!(
            matches!(
                code,
                "WORKSPACE_BUSY"
                    | "RESOURCE_LIMIT"
                    | "INPUT_CHANGED"
                    | "PERMISSION_DENIED"
                    | "INVALID_REQUEST"
            ),
            "{mode}: {response}"
        );
        if mode == "renderer_kind" {
            assert_eq!(code, "INVALID_REQUEST");
        }
        if mode == "oversize" {
            assert_eq!(code, "RESOURCE_LIMIT");
        }
        if matches!(mode, "unknown_id" | "wrong_doc_id" | "wrong_stored_kind") {
            assert_eq!(code, "WORKSPACE_BUSY");
        }
        assert_eq!(
            db.query_row("SELECT count(*) FROM goal_runs", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            1,
            "{mode}"
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            1,
            "{mode}"
        );
        assert!(db
            .query_row(
                "SELECT result IS NULL FROM approval_proposals WHERE id=?1",
                [id],
                |r| r.get::<_, bool>(0)
            )
            .unwrap());
        assert_eq!(
            db.query_row("SELECT count(*) FROM managed_workspaces", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            2
        );
        eprintln!(
            "FIXED_GOAL_REVIEW_REFUSED mode={mode} code={code} goal_runs=1 attempts=1 workspaces=2"
        );
    }
    db.execute(
        "UPDATE approval_proposals SET document=?1 WHERE id=?2",
        rusqlite::params![original, id],
    )
    .unwrap();
    db.execute(
        "UPDATE approval_proposals SET document=?1 WHERE id=?2",
        rusqlite::params![legacy_original, legacy_id],
    )
    .unwrap();
    std::fs::write(
        first_root.join("fixture-goal-release"),
        b"release only the real first process",
    )
    .unwrap();
    let first = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":first["id"]}),
        |r| r["run"]["state"] == "awaiting_acceptance",
    );
    assert_eq!(first["nodes"][0]["stop_confirmed"], true);
    assert_eq!(first["task_accepted"], false);
}

#[test]
fn actual_graph_proposal_is_complete_new_authority_and_never_dispatches_before_executor_is_connected(
) {
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-report");
    let ledger = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    )["sequence"]
        .clone();
    let request = json!({"request_id":"\0".repeat(128),"method":"goal_graph_runs.propose","params":{"scope_id":scope["id"],"expected_plan_revision":1,"expected_ledger_revision":ledger},"idempotency_key":"graph-proposal-real-first"});
    let proposal = result(f.server.request(request.clone()));
    assert_eq!(proposal["kind"], "goal_graph_execution");
    assert_eq!(proposal["snapshot"]["graph_contract_version"], 1);
    assert_eq!(proposal["snapshot"]["new_workspace_authority"], false);
    assert_eq!(
        proposal["snapshot"]["execution"],
        "goal_graph_v1_existing_workspace"
    );
    assert_eq!(
        proposal["snapshot"]["subject_rebinding"]["allows_old_policy_execution_reuse"],
        false
    );
    assert_eq!(result(f.server.request(request)), proposal);
    let raw = serde_json::to_vec_pretty(&proposal).unwrap();
    let digest = agentbaton_workspaces::validation::sha256(&raw);
    let dir = graph_artifact_path("");
    let path = dir.join(format!("actual-graph-proposal-{digest}.json"));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .unwrap();
    file.write_all(&raw).unwrap();
    eprintln!(
        "ACTUAL_GRAPH_PROPOSAL sha256={digest} compact={} pretty={} path={}",
        serde_json::to_vec(&proposal).unwrap().len(),
        raw.len(),
        path.display()
    );
    let list = result(
        f.server
            .request(json!({"method":"goal_graph_runs.list","params":{"task_id":f.task["id"]}})),
    );
    assert_eq!(list, json!([]));
    {
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        for table in ["goal_run_reservations", "goal_graph_jobs", "attempts"] {
            assert_eq!(
                db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, u32>(0))
                    .unwrap(),
                0
            );
        }
    }
    let finalize = json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":"native-graph-finalize"});
    let authorized = result(f.server.request(finalize.clone()));
    assert_eq!(authorized["run"]["id"], proposal["id"]);
    assert_eq!(authorized["graph_contract_version"], 1);
    assert_eq!(result(f.server.request(finalize)), authorized);
    assert_eq!(
        result(
            f.server
                .request(json!({"method":"goal_runs.list","params":{"task_id":f.task["id"]}}))
        ),
        json!([])
    );
    let legacy = f
        .server
        .request(json!({"method":"goal_runs.get","params":{"run_id":proposal["id"]}}));
    assert_eq!(legacy["error"]["code"], "CAPABILITY_UNSUPPORTED");
    let details = result(
        f.server
            .request(json!({"method":"goal_graph_runs.get","params":{"run_id":proposal["id"]}})),
    );
    assert_eq!(details["run"]["id"], proposal["id"]);
    assert_eq!(details["task_accepted"], false);
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        12
    );
    assert!(db
        .query_row(
            "SELECT result IS NOT NULL FROM approval_proposals WHERE id=?1",
            [proposal["id"].as_str().unwrap()],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
}

#[test]
fn actual_legacy_candidate_migration_preserves_bytes_without_backfilling_graph_hashes_or_authority()
{
    let mut f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-report");
    let run = approve_goal_execution(&f, &scope);
    let stopped = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":run["id"]}),
        |r| r["run"]["state"] == "awaiting_acceptance",
    );
    assert_eq!(stopped["nodes"][0]["actual_output_bytes"], 12);
    f.server.shutdown();
    let path = f.server.runtime.join("baton.sqlite3");
    let db = rusqlite::Connection::open(&path).unwrap();
    let raw_run: String = db
        .query_row("SELECT document FROM goal_runs", [], |r| r.get(0))
        .unwrap();
    let raw_node: String = db
        .query_row("SELECT document FROM goal_run_nodes", [], |r| r.get(0))
        .unwrap();
    db.execute_batch("DROP TRIGGER graph_fence_lease_insert; DROP TRIGGER graph_fence_lease_update; DROP TABLE goal_graph_acceptances;DROP TABLE goal_graph_owner_decisions;DROP TABLE goal_graph_validations;DROP TABLE goal_graph_candidates;DROP TABLE goal_graph_jobs;DROP TABLE goal_graph_capabilities;PRAGMA user_version=11;").unwrap();
    drop(db);
    let store = agentbaton_core::storage::Store::open(&path).unwrap();
    let current = store.goal_run_details(run["id"].as_str().unwrap()).unwrap();
    assert_eq!(current["nodes"], stopped["nodes"]);
    assert_eq!(current["task_accepted"], false);
    assert_eq!(
        store
            .list_goal_graph_runs(f.task["id"].as_str().unwrap())
            .unwrap()
            .len(),
        0
    );
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("SELECT document FROM goal_runs", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        raw_run
    );
    assert_eq!(
        db.query_row("SELECT document FROM goal_run_nodes", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        raw_node
    );
    for table in [
        "goal_graph_capabilities",
        "goal_graph_candidates",
        "goal_graph_acceptances",
    ] {
        assert_eq!(
            db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
}

#[test]
fn actual_graph_work_captures_owned_versioned_sha_without_task_acceptance() {
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-report");
    let ledger = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    )["sequence"]
        .clone();
    let proposal=result(f.server.request(json!({"method":"goal_graph_runs.propose","params":{"scope_id":scope["id"],"expected_plan_revision":1,"expected_ledger_revision":ledger},"idempotency_key":"graph-captured-work"})));
    let approved = approve(&f.server, proposal);
    let details = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":approved["run"]["id"]}),
        |v| v["candidates"].as_array().is_some_and(|c| c.len() == 1),
    );
    let c = &details["candidates"][0];
    assert_eq!(details["task_accepted"], false);
    assert_eq!(c["source"], "baton_goal_evidence");
    assert_eq!(c["integrity"], "verified");
    assert_eq!(c["task_acceptance"], "not_established");
    assert!(
        c["capture_receipt_sequence"].as_u64().unwrap()
            > c["stop_receipt_sequence"].as_u64().unwrap()
    );
    assert_eq!(
        c["artifacts"][0]["sha256"],
        agentbaton_workspaces::validation::sha256(b"fixed report")
    );
    assert_eq!(c["artifacts"][0]["bytes"], 12);
    assert_eq!(details["nodes"][0]["candidate_ids"][0], c["id"]);
    assert_eq!(details["nodes"][0]["stop_confirmed"], true);
    assert_eq!(
        c["consumed_inputs"][0]["sha256"],
        agentbaton_workspaces::validation::sha256(b"baseline")
    );
}

fn graph_owner_summary_plan(f: &Scenario) -> Value {
    use agentbaton_core::{goal_plan::*, goal_store::*};
    let scope = goal_scope_and_plan_with_capacity(f, "goal-report", 0, 2, 4);
    let view: GoalScopeView = serde_json::from_value(scope.clone()).unwrap();
    let current: GoalPlanView = serde_json::from_value(result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":view.id}})),
    ))
    .unwrap();
    let mut plan = current.proposal;
    let producer = plan.nodes[0].clone();
    let mut summary = plan.nodes[1].clone();
    let input = RequiredInput {
        goal_task_id: view.task_id.clone(),
        artifact_id: "report".into(),
        contract_id: "output:report".into(),
        source: InputSource::Node {
            node_id: producer.id.clone(),
            candidate_for_validation: true,
        },
    };
    let mut owner = producer.clone();
    owner.id = "owner-report".into();
    owner.kind = NodeKind::OwnerApproval;
    owner.executor = Executor::Owner {
        rule_id: "goal-owner-review".into(),
    };
    owner.goal = "Review exact original report".into();
    owner.dependencies = vec![producer.id.clone()];
    owner.required_inputs = vec![input.clone()];
    owner.outputs = vec![];
    owner.required_validators = vec!["goal-owner-review".into()];
    owner.side_effect = SideEffect::AdvisoryReadOnly;
    owner.write_set = vec![];
    owner.idempotency_key = "owner-report-once".into();
    summary.dependencies = vec![owner.id.clone()];
    let mut accepted_input = input;
    accepted_input.source = InputSource::Node {
        node_id: producer.id.clone(),
        candidate_for_validation: false,
    };
    summary.required_inputs = vec![accepted_input];
    let mut final_owner = owner.clone();
    final_owner.id = "owner-summary".into();
    final_owner.goal = "Review exact dependent summary".into();
    final_owner.dependencies = vec![summary.id.clone()];
    final_owner.required_inputs = vec![RequiredInput {
        goal_task_id: view.task_id.clone(),
        artifact_id: "report-2".into(),
        contract_id: "output:report-2".into(),
        source: InputSource::Node {
            node_id: summary.id.clone(),
            candidate_for_validation: true,
        },
    }];
    final_owner.idempotency_key = "owner-summary-once".into();
    plan.nodes = vec![producer, owner, summary, final_owner];
    plan.revision = 2;
    plan.expected_revision = 1;
    result(f.server.request(json!({"method":"goal_plans.propose","params":{"scope_id":view.id,"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"graph-summary-plan-v2"})));
    scope
}
fn approve_graph_at(f: &Scenario, scope: &Value, plan: u64) -> Value {
    let ledger = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    )["sequence"]
        .clone();
    let p=result(f.server.request(json!({"method":"goal_graph_runs.propose","params":{"scope_id":scope["id"],"expected_plan_revision":plan,"expected_ledger_revision":ledger},"idempotency_key":format!("actual-graph-plan:{}",scope["id"].as_str().unwrap())})));
    approve(&f.server, p)
}
fn graph_owner_proposal(f: &Scenario, run_id: &Value, node: &str, key: &str) -> Value {
    let d = result(
        f.server
            .request(json!({"method":"goal_graph_runs.get","params":{"run_id":run_id}})),
    );
    result(f.server.request(json!({"method":"goal_node_approvals.propose","params":{"run_id":run_id,"node_id":node,"expected_plan_revision":d["run"]["plan_revision"],"expected_ledger_revision":d["ledger_revision"]},"expected_revision":d["run"]["revision"],"idempotency_key":key})))
}
#[test]
fn actual_graph_owner_decision_unlocks_dependent_summary_with_exact_candidate_input() {
    let build_source_hashes = graph_build_source_hashes();
    eprintln!("GRAPH_BUILD_SOURCES {}", build_source_hashes);
    let f = Scenario::new(10_000);
    let scope = graph_owner_summary_plan(&f);
    let approved = approve_graph_at(&f, &scope, 2);
    let run_id = &approved["run"]["id"];
    let initial = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":run_id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-report" && n["state"] == "awaiting_owner")
            })
        },
    );
    assert_eq!(initial["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(initial["task_accepted"], false);
    eprintln!("GRAPH_PIPELINE first owned capture and owner-ready observed");
    let p = graph_owner_proposal(&f, run_id, "owner-report", "accept-exact-report");
    assert_eq!(p["kind"], "goal_node_acceptance");
    assert_eq!(p["snapshot"]["candidate_receipts"], initial["candidates"]);
    assert_eq!(
        p["snapshot"]["candidate_refs"][0]["sha256"],
        agentbaton_workspaces::validation::sha256(b"fixed report")
    );
    let path = graph_artifact_path("actual-node-proposal.json");
    std::fs::write(path, serde_json::to_vec_pretty(&p).unwrap()).unwrap();
    eprintln!("GRAPH_PIPELINE exact Owner proposal persisted");
    let decision = approve(&f.server, p.clone());
    assert_eq!(decision["decision"], "accepted");
    assert_eq!(decision["approval_id"], p["id"]);
    eprintln!("GRAPH_PIPELINE private Owner decision committed");
    let after = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":run_id}),
        |d| d["candidates"].as_array().is_some_and(|c| c.len() == 2),
    );
    let summary = after["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["node_id"] == "independent-2")
        .unwrap();
    assert_eq!(
        summary["consumed_inputs"][0]["candidate"],
        initial["candidates"][0]["artifacts"][0]
    );
    assert_eq!(
        summary["artifacts"][0]["sha256"],
        agentbaton_workspaces::validation::sha256(b"extra report 2")
    );
    assert_eq!(
        after["owner_decisions"][0]["source"],
        "local_owner_confirmation"
    );
    assert_eq!(after["task_accepted"], false);
    let sessions = result(
        f.server
            .request(json!({"method":"sessions.list","params":{}})),
    );
    assert_eq!(sessions.as_array().unwrap().len(), 2);
    let ready = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":run_id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-summary" && n["state"] == "awaiting_owner")
            })
        },
    );
    assert_eq!(ready["task_accepted"], false);
    let final_proposal = graph_owner_proposal(&f, run_id, "owner-summary", "accept-exact-summary");
    let final_decision = approve(&f.server, final_proposal.clone());
    assert_eq!(final_decision["task_accepted"], true);
    let accepted = result(
        f.server
            .request(json!({"method":"goal_graph_runs.get","params":{"run_id":run_id}})),
    );
    assert_eq!(accepted["task_accepted"], true);
    assert_eq!(accepted["run"]["state"], "accepted");
    let task = result(
        f.server
            .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
    );
    assert_eq!(task["state"], "accepted");
    assert_eq!(task["acceptance_ref"], accepted["task_acceptance"]["id"]);
    assert_eq!(
        task["acceptance_sequence"],
        accepted["task_acceptance"]["sequence"]
    );
    assert_eq!(
        task["revision"],
        accepted["task_acceptance"]["task_revision"]
    );
    assert_eq!(
        accepted["task_acceptance"]["source"],
        "baton_goal_acceptance"
    );
    assert_eq!(
        accepted["task_acceptance"]["artifact_refs"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    use std::io::Write;
    let evidence = json!({"build_source_hashes":build_source_hashes,"graph_details":accepted,"task":task,"final_owner_proposal":final_proposal,"final_owner_decision":final_decision});
    let bytes = serde_json::to_vec_pretty(&evidence).unwrap();
    let hash = agentbaton_workspaces::validation::sha256(&bytes);
    let path = graph_artifact_path(format!("actual-task-accepted-{hash}.json"));
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .unwrap()
        .write_all(&bytes)
        .unwrap();
    eprintln!(
        "ACTUAL_TASK_ACCEPTED_JSON sha256={hash} bytes={} path={}",
        bytes.len(),
        path.display()
    );
    let cancelled=result(f.server.request(json!({"method":"tasks.cancel","params":{"task_id":task["id"]},"expected_revision":task["revision"],"idempotency_key":"accepted-task-cancel-no-reopen"})));
    assert_eq!(cancelled, task);
    let ordinary=f.server.request(json!({"method":"sessions.create","params":{"task_id":task["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"grant_id":f.grant["id"]},"expected_revision":task["revision"],"idempotency_key":"accepted-task-cannot-reopen"}));
    assert!(ordinary.get("error").is_some());
    let before = accepted["ledger_revision"].clone();
    for (method, params, revision) in [
        (
            "goal_runs.dispatch",
            json!({"run_id":run_id,"expected_ledger_revision":before}),
            accepted["run"]["revision"].clone(),
        ),
        (
            "goal_runs.advance_plan",
            json!({"run_id":run_id,"expected_plan_revision":3,"expected_ledger_revision":before}),
            accepted["run"]["revision"].clone(),
        ),
        (
            "goal_graph_runs.advance_plan",
            json!({"run_id":run_id,"expected_plan_revision":3,"expected_ledger_revision":before}),
            accepted["run"]["revision"].clone(),
        ),
        (
            "goal_graph_runs.cancel",
            json!({"run_id":run_id}),
            accepted["run"]["revision"].clone(),
        ),
        (
            "goal_runs.propose",
            json!({"scope_id":scope["id"],"expected_plan_revision":2,"expected_ledger_revision":before}),
            Value::Null,
        ),
        (
            "goal_graph_runs.propose",
            json!({"scope_id":scope["id"],"expected_plan_revision":2,"expected_ledger_revision":before}),
            Value::Null,
        ),
        (
            "tasks.create",
            json!({"project_id":task["project_id"],"goal":"Agent text cannot accept a task","state":"accepted","acceptance_ref":task["acceptance_ref"]}),
            Value::Null,
        ),
    ] {
        let response=f.server.request(json!({"method":method,"params":params,"expected_revision":revision,"idempotency_key":format!("accepted-terminal-{method}")}));
        assert!(
            response.get("error").is_some(),
            "terminal bypass {method}: {response}"
        );
        assert_eq!(
            response["error"]["side_effect_state"], "not_started",
            "{method}"
        );
    }
    let original_plan = result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
    );
    let mut new_plan = original_plan["proposal"].clone();
    new_plan["revision"] = json!(3);
    new_plan["expected_revision"] = json!(2);
    let plan_refused=f.server.request(json!({"method":"goal_plans.propose","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&new_plan).unwrap()},"expected_revision":2,"idempotency_key":"accepted-task-no-new-plan"}));
    assert!(plan_refused.get("error").is_some());
    let raw_db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    let original_scope: String = raw_db
        .query_row(
            "SELECT document FROM approval_proposals WHERE id=?1",
            [scope["id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    let mut scope_intent =
        serde_json::from_str::<Value>(&original_scope).unwrap()["snapshot"]["intent"].clone();
    scope_intent["expected_task_revision"] = task["revision"].clone();
    scope_intent["expected_grant_revision"] = scope["root_grant_revision"].clone();
    drop(raw_db);
    let scope_refused=f.server.request(json!({"method":"goal_scopes.propose","params":scope_intent,"idempotency_key":"accepted-task-no-new-scope"}));
    assert!(scope_refused.get("error").is_some());
    for _ in 0..3 {
        let stable = result(
            f.server
                .request(json!({"method":"goal_graph_runs.get","params":{"run_id":run_id}})),
        );
        assert_eq!(stable["run"], accepted["run"]);
        assert_eq!(stable["ledger_revision"], before);
    }
    assert_eq!(
        result(
            f.server
                .request(json!({"method":"sessions.list","params":{}}))
        )
        .as_array()
        .unwrap()
        .len(),
        2
    );
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    for key in [
        format!("project:{}", f.project["id"].as_str().unwrap()),
        format!("root:{}", f.task["id"].as_str().unwrap()),
    ] {
        let count: u32 = db
            .query_row(
                "SELECT consumed FROM budgets WHERE scope=?1",
                [key],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }
    // Acceptance is a historical versioned fact. A later file edit does not
    // create new authority and cannot erase the completed receipt.
    std::fs::write(
        PathBuf::from(f.workspace["root"].as_str().unwrap()).join("reports/report.txt"),
        b"later user edit",
    )
    .unwrap();
    assert_eq!(
        result(
            f.server
                .request(json!({"method":"tasks.get","params":{"task_id":task["id"]}}))
        ),
        task
    );
    assert_eq!(approve(&f.server, final_proposal), final_decision);
}

fn graph_build_source_hashes() -> Value {
    let sources: [(&str, &[u8]); 15] = [
        ("goal_graph.rs", include_bytes!("../src/goal_graph.rs")),
        (
            "goal_graph_validators.rs",
            include_bytes!("../src/goal_graph_validators.rs"),
        ),
        (
            "goal_validator_io.rs",
            include_bytes!("../src/goal_validator_io.rs"),
        ),
        (
            "core_command_validation.rs",
            include_bytes!("../src/command_validation.rs"),
        ),
        (
            "workspaces_command_validation.rs",
            include_bytes!("../../workspaces/src/command_validation.rs"),
        ),
        (
            "goal_evidence.rs",
            include_bytes!("../src/goal_evidence.rs"),
        ),
        ("goal_runtime.rs", include_bytes!("../src/goal_runtime.rs")),
        ("goal_store.rs", include_bytes!("../src/goal_store.rs")),
        ("storage.rs", include_bytes!("../src/storage.rs")),
        ("sessions.rs", include_bytes!("../src/sessions.rs")),
        ("ipc.rs", include_bytes!("../src/ipc.rs")),
        ("execution.rs", include_bytes!("../src/execution.rs")),
        (
            "data_workspace_workers.rs",
            include_bytes!("../src/data_workspace_workers.rs"),
        ),
        (
            "data_workspaces.rs",
            include_bytes!("../src/data_workspaces.rs"),
        ),
        ("tests/sessions_ipc.rs", include_bytes!("sessions_ipc.rs")),
    ];
    json!(sources
        .into_iter()
        .map(|(name, bytes)| (name, agentbaton_workspaces::validation::sha256(bytes)))
        .collect::<std::collections::BTreeMap<_, _>>())
}

fn accepted_owner_graph_fixture() -> (Scenario, Value, Value, Value, Value) {
    let f = Scenario::new(10_000);
    let scope = graph_owner_summary_plan(&f);
    let summary = approve_graph_at(&f, &scope, 2);
    let run_id = summary["run"]["id"].clone();
    wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":run_id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-report" && n["state"] == "awaiting_owner")
            })
        },
    );
    approve(
        &f.server,
        graph_owner_proposal(&f, &run_id, "owner-report", "fixture-owner-first"),
    );
    wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":run_id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-summary" && n["state"] == "awaiting_owner")
            })
        },
    );
    let proposal = graph_owner_proposal(&f, &run_id, "owner-summary", "fixture-owner-last");
    let decision = approve(&f.server, proposal.clone());
    assert_eq!(decision["task_accepted"], true);
    let accepted = result(
        f.server
            .request(json!({"method":"goal_graph_runs.get","params":{"run_id":run_id}})),
    );
    (f, scope, proposal, decision, accepted)
}
#[test]
fn actual_accepted_history_and_owner_replay_reject_durable_receipt_tampering_without_file_reads() {
    let (mut f, _scope, proposal, decision, accepted) = accepted_owner_graph_fixture();
    let run_id = accepted["run"]["id"].as_str().unwrap();
    let task_id = f.task["id"].as_str().unwrap();
    std::fs::write(
        PathBuf::from(f.workspace["root"].as_str().unwrap()).join("reports/report.txt"),
        b"legitimate later edit",
    )
    .unwrap();
    let replay_request = json!({"method":"goal_node_approvals.propose","params":proposal["snapshot"]["intent"],"expected_revision":proposal["snapshot"]["expected_run_revision"],"idempotency_key":"fixture-owner-last"});
    let recovered = result(f.server.request(replay_request));
    assert_eq!(recovered["id"], proposal["id"]);
    assert_eq!(recovered["snapshot"], proposal["snapshot"]);
    assert_eq!(recovered["consumed"], true);
    assert_eq!(approve(&f.server, recovered), decision);
    let cross=f.server.request(json!({"method":"approvals.decline","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":"fixed-opposite-action"}));
    assert_eq!(cross["error"]["code"], "CONFIG_CONFLICT");
    assert_eq!(
        result(
            f.server
                .request(json!({"method":"tasks.get","params":{"task_id":task_id}}))
        )["state"],
        "accepted"
    );
    let foreign_path = f.root.join("foreign-project-source");
    std::fs::create_dir(&foreign_path).unwrap();
    git(&foreign_path, &["init", "-q"]);
    std::fs::write(foreign_path.join("file"), b"another project").unwrap();
    git(&foreign_path, &["add", "file"]);
    git(&foreign_path, &["commit", "-qm", "foreign fixture"]);
    let foreign_project=result(f.server.request(json!({"method":"projects.import","params":{"path":foreign_path,"name":"Separate registered fixture"},"idempotency_key":"foreign-project-evidence-control"})));
    f.server.shutdown();
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    let frozen = f.root.join("accepted-original.sqlite3");
    let mut snapshot = rusqlite::Connection::open(&frozen).unwrap();
    rusqlite::backup::Backup::new(&db, &mut snapshot)
        .unwrap()
        .run_to_completion(128, Duration::from_millis(1), None)
        .unwrap();
    drop(snapshot);
    drop(db);
    {
        let control = agentbaton_core::storage::Store::open(&frozen).unwrap();
        assert_eq!(
            control.get(task_id).unwrap().state,
            agentbaton_core::storage::TaskState::Accepted
        );
        assert!(control.goal_graph_details(run_id).unwrap().task_accepted);
    }
    let cases=[
        ("owner-deleted","DELETE FROM goal_graph_owner_decisions WHERE rowid=(SELECT min(rowid) FROM goal_graph_owner_decisions)"),
        ("owner-source","UPDATE events SET document=json_set(document,'$.source','agent_output') WHERE sequence=(SELECT min(sequence) FROM events WHERE json_extract(document,'$.kind')='goal.owner_decided')"),
        ("owner-reordered","UPDATE events SET sequence=(SELECT max(sequence)+100 FROM events) WHERE sequence=(SELECT min(sequence) FROM events WHERE json_extract(document,'$.kind')='goal.owner_decided')"),
        ("capture-prepared-deleted","DELETE FROM events WHERE sequence=(SELECT min(sequence) FROM events WHERE json_extract(document,'$.kind')='goal.capture_prepared')"),
        ("capture-prepared-source","UPDATE events SET document=json_set(document,'$.source','agent_output') WHERE sequence=(SELECT min(sequence) FROM events WHERE json_extract(document,'$.kind')='goal.capture_prepared')"),
        ("capture-reordered","UPDATE events SET sequence=(SELECT max(sequence)+100 FROM events) WHERE sequence=(SELECT min(sequence) FROM events WHERE json_extract(document,'$.kind')='goal.capture_prepared')"),
        ("candidate-sha","UPDATE goal_graph_candidates SET document=json_set(document,'$.artifacts[0].sha256',printf('%064d',0)) WHERE rowid=(SELECT min(rowid) FROM goal_graph_candidates)"),
        ("task-project","UPDATE tasks SET document=json_set(document,'$.project_id','foreign-registered-project')"),
        ("task-goal","UPDATE tasks SET document=json_set(document,'$.goal','different acceptance subject')"),
        ("task-id","UPDATE tasks SET document=json_set(document,'$.id','foreign-task')"),
        ("result-run","UPDATE approval_proposals SET result=json_set(result,'$.run_id','foreign-run') WHERE id=?1"),
        ("result-node","UPDATE approval_proposals SET result=json_set(result,'$.node_id','foreign-node') WHERE id=?1"),
        ("result-plan","UPDATE approval_proposals SET result=json_set(result,'$.plan_revision',999) WHERE id=?1"),
        ("result-owner","UPDATE approval_proposals SET result=json_set(result,'$.owner_decision_id','foreign-decision') WHERE id=?1"),
        ("result-revision","UPDATE approval_proposals SET result=json_set(result,'$.run_revision',999) WHERE id=?1"),
        ("result-taskaccept","UPDATE approval_proposals SET result=json_set(result,'$.task_accepted',json('false')) WHERE id=?1"),
        ("result-approval","UPDATE approval_proposals SET result=json_set(result,'$.approval_id','foreign-proposal') WHERE id=?1"),
    ];
    for (name, sql) in cases {
        let path = f.root.join(format!("tampered-{name}.sqlite3"));
        std::fs::copy(&frozen, &path).unwrap();
        let db = rusqlite::Connection::open(&path).unwrap();
        if name == "task-project" {
            db.execute(
                "UPDATE tasks SET document=json_set(document,'$.project_id',?1)",
                [foreign_project["id"].as_str().unwrap()],
            )
            .unwrap();
        } else if name.starts_with("result-") {
            db.execute(sql, [proposal["id"].as_str().unwrap()]).unwrap();
        } else {
            db.execute_batch(sql).unwrap();
        }
        let seq: u64 = db
            .query_row("SELECT max(sequence) FROM events", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
            .try_into()
            .unwrap();
        drop(db);
        let mut store = agentbaton_core::storage::Store::open(&path).unwrap();
        assert!(
            store.get(task_id).is_err(),
            "{name}: generic accepted read borrowed damaged evidence"
        );
        assert!(
            store.list().is_err(),
            "{name}: list borrowed damaged accepted evidence"
        );
        assert!(
            store.goal_graph_details(run_id).is_err(),
            "{name}: Graph accepted read borrowed damaged evidence"
        );
        assert!(
            store
                .finalize_proposal(
                    proposal["id"].as_str().unwrap(),
                    proposal["revision"].as_u64().unwrap()
                )
                .is_err(),
            "{name}: historical finalize borrowed damaged result/evidence"
        );
        assert_eq!(
            store.sequence().unwrap(),
            seq,
            "{name}: rejection created events"
        );
        eprintln!("ACCEPTED_HISTORY_CONTROL {name} refused without mutation");
    }
}

#[test]
fn actual_last_owner_rechecks_all_original_products_and_never_accepts_changed_cancelled_or_revoked_graph(
) {
    eprintln!("GRAPH_BUILD_SOURCES {}", graph_build_source_hashes());
    for mode in [
        "changed-report",
        "same-bytes-replacement",
        "revoked",
        "cancelled",
    ] {
        let f = Scenario::new(10_000);
        let scope = graph_owner_summary_plan(&f);
        let approved = approve_graph_at(&f, &scope, 2);
        let run_id = &approved["run"]["id"];
        wait(
            &f.server,
            "goal_graph_runs.get",
            json!({"run_id":run_id}),
            |d| {
                d["nodes"].as_array().is_some_and(|ns| {
                    ns.iter().any(|n| {
                        n["node"]["id"] == "owner-report" && n["state"] == "awaiting_owner"
                    })
                })
            },
        );
        let first = approve(
            &f.server,
            graph_owner_proposal(&f, run_id, "owner-report", "fresh-all-first-owner"),
        );
        let before = wait(
            &f.server,
            "goal_graph_runs.get",
            json!({"run_id":run_id}),
            |d| {
                d["nodes"].as_array().is_some_and(|ns| {
                    ns.iter().any(|n| {
                        n["node"]["id"] == "owner-summary" && n["state"] == "awaiting_owner"
                    })
                })
            },
        );
        let proposal = graph_owner_proposal(&f, run_id, "owner-summary", "fresh-all-last-owner");
        let report =
            PathBuf::from(f.workspace["root"].as_str().unwrap()).join("reports/report.txt");
        let original = std::fs::read(&report).unwrap();
        match mode {
            "changed-report" => {
                std::fs::write(&report, b"report changed after first Owner accepted").unwrap()
            }
            "same-bytes-replacement" => {
                let replacement = report.with_file_name("replacement.txt");
                std::fs::write(&replacement, &original).unwrap();
                std::fs::set_permissions(
                    &replacement,
                    std::fs::metadata(&report).unwrap().permissions(),
                )
                .unwrap();
                std::fs::rename(replacement, &report).unwrap();
            }
            "revoked" => {
                result(f.server.request(json!({"method":"goal_scopes.revoke","params":{"scope_id":scope["id"]},"expected_revision":scope["revision"],"idempotency_key":"revoke-before-final-owner"})));
            }
            "cancelled" => {
                let request = json!({"method":"goal_graph_runs.cancel","params":{"run_id":run_id},"expected_revision":before["run"]["revision"],"idempotency_key":"cancel-before-final-owner"});
                let receipt = result(f.server.request(request.clone()));
                assert_eq!(receipt["run"]["cancel_requested"], true);
                assert_eq!(result(f.server.request(request)), receipt);
            }
            _ => unreachable!(),
        }
        let ledger_before = result(
            f.server
                .request(json!({"method":"system.snapshot","params":{}})),
        )["sequence"]
            .clone();
        let refused=f.server.request(json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":"must-not-accept-stale-whole-task"}));
        assert!(refused.get("error").is_some(), "{mode}: {refused}");
        assert_eq!(
            refused["error"]["side_effect_state"], "not_started",
            "{mode}"
        );
        let after = result(
            f.server
                .request(json!({"method":"goal_graph_runs.get","params":{"run_id":run_id}})),
        );
        assert_eq!(after["task_accepted"], false, "{mode}");
        assert_eq!(
            after["owner_decisions"].as_array().unwrap().len(),
            1,
            "{mode}"
        );
        assert_eq!(
            after["owner_decisions"][0]["id"],
            first["owner_decision_id"]
        );
        assert_eq!(
            after["candidates"], before["candidates"],
            "{mode}: old history was erased"
        );
        let task = result(
            f.server
                .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
        );
        assert_ne!(task["state"], "accepted");
        assert!(task.get("acceptance_ref").is_none());
        let recovered=result(f.server.request(json!({"method":"goal_node_approvals.propose","params":proposal["snapshot"]["intent"],"expected_revision":proposal["snapshot"]["expected_run_revision"],"idempotency_key":"fresh-all-last-owner"})));
        assert_eq!(
            recovered, proposal,
            "{mode}: exact unconsumed proposal history cannot mint fresh authority"
        );
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        assert!(db
            .query_row(
                "SELECT result IS NULL FROM approval_proposals WHERE id=?1",
                [proposal["id"].as_str().unwrap()],
                |r| r.get::<_, bool>(0)
            )
            .unwrap());
        assert_eq!(
            db.query_row("SELECT count(*) FROM goal_graph_acceptances", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
        for key in [
            format!("project:{}", f.project["id"].as_str().unwrap()),
            format!("root:{}", f.task["id"].as_str().unwrap()),
        ] {
            assert_eq!(
                db.query_row("SELECT consumed FROM budgets WHERE scope=?1", [key], |r| {
                    r.get::<_, u32>(0)
                })
                .unwrap(),
                2,
                "{mode}: no budget refund"
            );
        }
        if mode != "cancelled" {
            assert_eq!(
                after["ledger_revision"], ledger_before,
                "{mode}: denied finalize wrote events"
            );
        }
        assert!(report.exists());
        eprintln!("FINAL_OWNER_FRESH_CONTROL {mode} refused, original Owner/version and charged attempts retained");
    }
}

static CAPTURE_DELAY_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CAPTURE_DELAY_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
unsafe extern "C" fn capture_delay_sql(
    context: *mut rusqlite::ffi::sqlite3_context,
    _argc: std::ffi::c_int,
    _values: *mut *mut rusqlite::ffi::sqlite3_value,
) {
    let began = Instant::now();
    std::thread::sleep(Duration::from_millis(1500));
    CAPTURE_DELAY_MS.store(
        began.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        Ordering::SeqCst,
    );
    CAPTURE_DELAY_CALLS.fetch_add(1, Ordering::SeqCst);
    unsafe {
        rusqlite::ffi::sqlite3_result_int(context, 1);
    }
}
unsafe extern "C" fn capture_delay_extension(
    db: *mut rusqlite::ffi::sqlite3,
    _error: *mut *mut std::ffi::c_char,
    _api: *const rusqlite::ffi::sqlite3_api_routines,
) -> std::ffi::c_int {
    unsafe {
        rusqlite::ffi::sqlite3_create_function_v2(
            db,
            c"goal_fixture_capture_delay".as_ptr(),
            0,
            rusqlite::ffi::SQLITE_UTF8 | rusqlite::ffi::SQLITE_INNOCUOUS,
            std::ptr::null_mut(),
            Some(capture_delay_sql),
            None,
            None,
            None,
        )
    }
}
struct CaptureDelayRegistration;
impl CaptureDelayRegistration {
    fn new() -> Self {
        CAPTURE_DELAY_CALLS.store(0, Ordering::SeqCst);
        CAPTURE_DELAY_MS.store(0, Ordering::SeqCst);
        assert_eq!(
            unsafe { rusqlite::ffi::sqlite3_auto_extension(Some(capture_delay_extension)) },
            rusqlite::ffi::SQLITE_OK
        );
        Self
    }
}
impl Drop for CaptureDelayRegistration {
    fn drop(&mut self) {
        unsafe {
            rusqlite::ffi::sqlite3_cancel_auto_extension(Some(capture_delay_extension));
        }
    }
}

#[test]
fn actual_capture_prestart_queue_exhaustion_never_spawns_or_claims_an_io_join() {
    eprintln!("GRAPH_BUILD_SOURCES {}", graph_build_source_hashes());
    let _registration = CaptureDelayRegistration::new();
    let f = Scenario::new(10_000);
    let scope = goal_scope_and_plan(&f, "goal-report");
    let current = result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
    );
    let mut plan = current["proposal"].clone();
    plan["revision"] = json!(2);
    plan["expected_revision"] = json!(1);
    plan["nodes"][0]["reservation"]["runtime_ms"] = json!(1200);
    result(f.server.request(json!({"method":"goal_plans.propose","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"queue-delay-approved-1200ms"})));
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    // Hold the actual daemon transaction after its capture queue clock starts.
    // This real monotonic delay is independent of host CPU speed and remains
    // inside the unchanged 5s RPC bound. No capture worker or fake stop exists.
    db.execute_batch("CREATE TRIGGER controlled_capture_prepare_delay AFTER INSERT ON goal_graph_jobs BEGIN SELECT goal_fixture_capture_delay(); END;").unwrap();
    drop(db);
    let approved = approve_graph_at(&f, &scope, 2);
    let run_id = &approved["run"]["id"];
    let blocked = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":run_id}),
        |d| d["run"]["state"] == "blocked",
    );
    assert_eq!(CAPTURE_DELAY_CALLS.load(Ordering::SeqCst), 1);
    let actual_delay = CAPTURE_DELAY_MS.load(Ordering::SeqCst);
    assert!(actual_delay >= 1500);
    eprintln!(
        "CONTROLLED_CAPTURE_DELAY elapsed_ms={actual_delay} calls=1 approved_runtime_ms=1200"
    );
    assert_eq!(blocked["task_accepted"], false);
    assert!(blocked["candidates"].as_array().unwrap().is_empty());
    assert_eq!(blocked["nodes"][0]["stop_confirmed"], true);
    assert!(blocked["nodes"][0]["actual_runtime_ms"].as_u64().unwrap() >= 1200);
    assert!(blocked["nodes"][0]["actual_output_bytes"].is_null());
    let sessions = result(
        f.server
            .request(json!({"method":"sessions.list","params":{}})),
    );
    assert_eq!(sessions.as_array().unwrap().len(), 1);
    assert_eq!(sessions[0]["state"], "stopped");
    assert_eq!(sessions[0]["turn_state"], "completed");
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.capture_worker_started'",[],|r|r.get::<_,u32>(0)).unwrap(),0);
    let raw:String=db.query_row("SELECT document FROM events WHERE json_extract(document,'$.kind')='goal.capture_settled'",[],|r|r.get(0)).unwrap();
    let receipt: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(receipt["payload"]["io_completion"], "not_started");
    assert_eq!(receipt["payload"]["capture_worker_joined"], false);
    assert_eq!(receipt["payload"]["producer_stop_confirmed"], true);
    assert_eq!(
        db.query_row("SELECT count(*) FROM goal_graph_candidates", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT consumed FROM budgets WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    eprintln!("CAPTURE_PRESTART actual queued delay exhausted 1200ms reservation, no thread started/no join/candidate/refund; actual_runtime_ms={}",receipt["payload"]["actual_runtime_ms"]);
}

fn actual_integrity_definition_graph_fixture() -> (Scenario, Value, Value) {
    actual_integrity_definition_graph_fixture_with_budget(8)
}
fn trace_integrity_grant(f: &Scenario, phase: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    eprintln!(
        "INTEGRITY_GRANT_PHASE {}",
        json!({"phase":phase,"now_epoch_ms":now,"grant_id":f.grant["id"],
            "approved_at":f.grant["approved_at"],"expires_at_ms":f.grant["expires_at_ms"]})
    );
}
fn actual_integrity_definition_graph_fixture_with_budget(
    attempts: u32,
) -> (Scenario, Value, Value) {
    use agentbaton_core::{goal_plan::*, goal_store::*};
    let mut f = Scenario::new(10_000);
    use_actual_dataset_workspace(&mut f);
    // Enclosing fixture lifetime includes bootstrap and two Owner reviews.
    // Match Data/Command's 30s Grant; Graph runtime/node/negative limits stay fixed.
    let grant_proposal=result(f.server.request(json!({"method":"grants.propose","params":{"project_id":f.project["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"max_attempts":8,"max_concurrency":1,"max_duration_ms":30000},"idempotency_key":"initial-eight-shared-attempt-policy"})));
    f.grant = approve(&f.server, grant_proposal);
    trace_integrity_grant(&f, "enclosing_grant_approved_before_bootstrap");
    let bootstrap = goal_scope_and_plan_with_options(&f, "goal-report", 0, 1, 2, 8, vec![]);
    let source = approve_goal_execution(&f, &bootstrap);
    let source_done = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":source["id"]}),
        |d| d["run"]["state"] == "awaiting_acceptance",
    );
    let session = &source_done["nodes"][0]["session_id"];
    trace_integrity_grant(&f, "bootstrap_candidate_before_definition");
    let policy_proposal=result(f.server.request(json!({"method":"validation_policies.propose","params":{"session_id":session,"inputs":[{"artifact_id":"fixed-input","relative_path":"inputs/file","max_bytes":1024,"expected_sha256":agentbaton_workspaces::validation::sha256(b"baseline")}],"outputs":[{"artifact_id":"report","relative_path":"reports/report.txt","max_bytes":1024,"expected_sha256":null}],"analysis":null},"idempotency_key":"source-additional-integrity-definition"})));
    let definition = approve(&f.server, policy_proposal);
    result(f.server.request(json!({"method":"goal_runs.cancel","params":{"run_id":source["id"]},"expected_revision":source_done["run"]["revision"],"idempotency_key":"cancel-source-candidate-before-new-graph"})));
    wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":source["id"]}),
        |d| d["run"]["state"] == "cancelled",
    );
    trace_integrity_grant(&f, "bootstrap_cancelled_before_new_task");
    let grants = result(
        f.server
            .request(json!({"method":"grants.list","params":{"project_id":f.project["id"]}})),
    );
    f.grant = grants
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == f.grant["id"])
        .unwrap()
        .clone();
    f.task=result(f.server.request(json!({"method":"tasks.create","params":{"project_id":f.project["id"],"goal":"Use approved same-workspace graph definition with a new owned producer subject"},"idempotency_key":"new-graph-task-keeps-project-history"})));
    let validator = GoalValidatorRef {
        kind: GoalValidatorKind::Integrity,
        policy_id: definition["id"].as_str().unwrap().into(),
        revision: definition["revision"].as_u64().unwrap(),
    };
    let scope =
        goal_scope_and_plan_with_options(&f, "goal-report", 0, 2, 5, attempts, vec![validator]);
    let view: GoalScopeView = serde_json::from_value(scope.clone()).unwrap();
    let current: GoalPlanView = serde_json::from_value(result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
    ))
    .unwrap();
    let mut plan = current.proposal;
    let producer = plan.nodes[0].clone();
    let mut summary = plan.nodes[1].clone();
    let rule_id = view.contracts["output:report"]
        .iter()
        .find(|id| id.starts_with("integrity:"))
        .unwrap()
        .clone();
    let candidate = RequiredInput {
        goal_task_id: view.task_id.clone(),
        artifact_id: "report".into(),
        contract_id: "output:report".into(),
        source: InputSource::Node {
            node_id: producer.id.clone(),
            candidate_for_validation: true,
        },
    };
    let mut validate = producer.clone();
    validate.id = "validate-report".into();
    validate.kind = NodeKind::Validate;
    validate.executor = Executor::Validator {
        rule_id: rule_id.clone(),
    };
    validate.goal = "Apply exact approved integrity definition to the new captured producer".into();
    validate.dependencies = vec![producer.id.clone()];
    validate.required_inputs = vec![candidate.clone()];
    validate.outputs.clear();
    validate.required_validators = vec![rule_id.clone()];
    validate.side_effect = SideEffect::AdvisoryReadOnly;
    validate.write_set.clear();
    validate.idempotency_key = "validate-owned-report-once".into();
    let mut owner = validate.clone();
    owner.id = "owner-report".into();
    owner.kind = NodeKind::OwnerApproval;
    owner.executor = Executor::Owner {
        rule_id: "goal-owner-review".into(),
    };
    owner.goal = "Owner reviews exact report after deterministic integrity only".into();
    owner.dependencies = vec![producer.id.clone(), validate.id.clone()];
    owner.required_validators = vec!["goal-owner-review".into()];
    owner.idempotency_key = "owner-report-once".into();
    summary.dependencies = vec![validate.id.clone(), owner.id.clone()];
    let mut accepted = candidate;
    accepted.source = InputSource::Node {
        node_id: producer.id.clone(),
        candidate_for_validation: false,
    };
    summary.required_inputs = vec![accepted];
    let mut final_owner = owner.clone();
    final_owner.id = "owner-summary".into();
    final_owner.dependencies = vec![summary.id.clone()];
    final_owner.required_inputs = vec![RequiredInput {
        goal_task_id: view.task_id.clone(),
        artifact_id: "report-2".into(),
        contract_id: "output:report-2".into(),
        source: InputSource::Node {
            node_id: summary.id.clone(),
            candidate_for_validation: true,
        },
    }];
    final_owner.idempotency_key = "owner-summary-once".into();
    plan.nodes = vec![producer, validate, owner, summary, final_owner];
    plan.revision = 2;
    plan.expected_revision = 1;
    result(f.server.request(json!({"method":"goal_plans.propose","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"actual-integrity-five-node-plan"})));
    trace_integrity_grant(&f, "five_node_plan_prepared");
    (f, scope, definition)
}
#[test]
fn actual_graph_integrity_validator_is_a_charged_owned_attempt_before_owner_and_summary() {
    eprintln!("GRAPH_BUILD_SOURCES {}", graph_build_source_hashes());
    let (f, scope, definition) = actual_integrity_definition_graph_fixture();
    trace_integrity_grant(&f, "before_graph_approval");
    let approved = approve_graph_at(&f, &scope, 2);
    let run_id = &approved["run"]["id"];
    let checked = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":run_id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-report" && n["state"] == "awaiting_owner")
            })
        },
    );
    assert_eq!(checked["validations"].as_array().unwrap().len(), 1);
    let receipt = &checked["validations"][0];
    authentic_validation_from_projection(receipt);
    assert_eq!(receipt["passed"], true);
    assert_eq!(receipt["scope"], "artifact_integrity_only");
    assert_eq!(receipt["correctness"], "not_established");
    assert_eq!(receipt["task_acceptance"], "not_established");
    assert_eq!(receipt["source"], "baton_goal_validator");
    assert_eq!(receipt["io_completion"], "worker_joined");
    assert_eq!(receipt["stop_confirmed"], true);
    assert_eq!(checked["task_accepted"], false);
    let validator = checked["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["node"]["id"] == "validate-report")
        .unwrap();
    assert!(validator["session_id"].is_null());
    assert!(validator["job_id"].is_string());
    assert!(validator["attempt_id"].is_string());
    assert_eq!(validator["execution_kind"], "validator");
    assert_eq!(validator["state"], "validated");
    trace_integrity_grant(&f, "before_first_owner_proposal");
    let owner_proposal = graph_owner_proposal(&f, run_id, "owner-report", "integrity-owner-report");
    assert_eq!(
        owner_proposal["snapshot"]["validation_refs"][0],
        authentic_validation_from_projection(receipt)
    );
    trace_integrity_grant(&f, "before_first_owner_finalize");
    let first_decision = approve(&f.server, owner_proposal.clone());
    wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":run_id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-summary" && n["state"] == "awaiting_owner")
            })
        },
    );
    trace_integrity_grant(&f, "before_final_owner_proposal");
    let final_proposal =
        graph_owner_proposal(&f, run_id, "owner-summary", "integrity-owner-summary");
    trace_integrity_grant(&f, "before_final_owner_finalize");
    let decision = approve(&f.server, final_proposal.clone());
    assert_eq!(decision["task_accepted"], true);
    let accepted = result(
        f.server
            .request(json!({"method":"goal_graph_runs.get","params":{"run_id":run_id}})),
    );
    assert_eq!(accepted["task_accepted"], true);
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        db.query_row(
            "SELECT consumed FROM budgets WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        4
    );
    let task = result(
        f.server
            .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
    );
    for key in [
        format!("project:{}", f.project["id"].as_str().unwrap()),
        format!("root-grant:{}", f.grant["id"].as_str().unwrap()),
    ] {
        let raw: String = db
            .query_row(
                "SELECT document FROM goal_ledgers WHERE scope=?1",
                [key],
                |r| r.get(0),
            )
            .unwrap();
        let ledger: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(ledger["output_bytes"], 38);
    }
    let output:u64=db.query_row("SELECT sum(json_extract(document,'$.actual_output_bytes')) FROM goal_run_reservations WHERE run_id=?1",[run_id.as_str().unwrap()],|r|r.get::<_,i64>(0)).unwrap().try_into().unwrap();
    assert_eq!(output, 26);
    let evidence = json!({"build_source_hashes":graph_build_source_hashes(),"task":task,"scope":scope,"definition":definition,"graph_details":accepted,"owner_proposal":owner_proposal,"first_decision":first_decision,"final_owner_proposal":final_proposal,"final_decision":decision});
    let bytes = serde_json::to_vec_pretty(&evidence).unwrap();
    let sha = agentbaton_workspaces::validation::sha256(&bytes);
    let path = graph_artifact_path(format!("actual-integrity-graph-{sha}.json"));
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .unwrap()
        .write_all(&bytes)
        .unwrap();
    eprintln!(
        "ACTUAL_INTEGRITY_GRAPH sha256={sha} bytes={} path={}",
        bytes.len(),
        path.display()
    );
}

fn actual_data_definition_graph_fixture(bad: bool) -> (Scenario, Value, Value) {
    use agentbaton_core::{goal_plan::*, goal_store::*};
    let mut f = Scenario::new(10_000);
    use_actual_dataset_workspace(&mut f);
    let p=result(f.server.request(json!({"method":"grants.propose","params":{"project_id":f.project["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"max_attempts":8,"max_concurrency":1,"max_duration_ms":30000},"idempotency_key":"data-eight-shared-attempt-policy"})));
    f.grant = approve(&f.server, p);
    let bootstrap = goal_scope_and_plan_with_options(&f, "goal-data-report", 0, 3, 5, 8, vec![]);
    let current: GoalPlanView = serde_json::from_value(result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":bootstrap["id"]}})),
    ))
    .unwrap();
    let mut plan = current.proposal;
    let mut producer = plan.nodes[0].clone();
    producer.outputs = plan.nodes.iter().flat_map(|n| n.outputs.clone()).collect();
    producer.write_set = plan
        .nodes
        .iter()
        .flat_map(|n| n.write_set.clone())
        .collect();
    producer.required_validators = plan
        .nodes
        .iter()
        .flat_map(|n| n.required_validators.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    plan.nodes = vec![producer];
    plan.revision = 2;
    plan.expected_revision = 1;
    result(f.server.request(json!({"method":"goal_plans.propose","params":{"scope_id":bootstrap["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"data-source-three-artifacts-plan"})));
    let source = approve(&f.server, prepare_goal_execution_at(&f, &bootstrap, 2));
    let done = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":source["id"]}),
        |d| d["run"]["state"] == "awaiting_acceptance",
    );
    let artifacts=(1..=3).map(|i|json!({"artifact_id":if i==1 {"report".into()}else{format!("report-{i}")},"relative_path":if i==1 {"reports/report.txt".into()}else{format!("reports/report-{i}.txt")},"max_bytes":1024,"expected_sha256":null})).collect::<Vec<_>>();
    let table = json!({"table_artifact_id":"report","dataset_rows":1,"columns":[{"name":"x","required":true,"nullable":false,"scalar":{"type":"integer","min":0,"max":10}}],"analysis":{"script_artifact_id":"report-2","environment_artifact_id":"report-3","units":{"x":"bytes"},"timezone":"UTC","seed":7,"scope":{"kind":"full"}},"limits":agentbaton_workspaces::deterministic::JsonLimits::default()});
    let p=result(f.server.request(json!({"method":"data_validation_policies.propose","params":{"session_id":done["nodes"][0]["session_id"],"inputs":[{"artifact_id":"fixed-input","relative_path":"inputs/file","max_bytes":1024,"expected_sha256":agentbaton_workspaces::validation::sha256(b"baseline")}],"outputs":artifacts,"table":table},"idempotency_key":"source-additional-json-definition"})));
    let definition = approve(&f.server, p);
    result(f.server.request(json!({"method":"goal_runs.cancel","params":{"run_id":source["id"]},"expected_revision":done["run"]["revision"],"idempotency_key":"cancel-data-source-candidate"})));
    wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":source["id"]}),
        |d| d["run"]["state"] == "cancelled",
    );
    let grants = result(
        f.server
            .request(json!({"method":"grants.list","params":{"project_id":f.project["id"]}})),
    );
    f.grant = grants
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == f.grant["id"])
        .unwrap()
        .clone();
    f.task=result(f.server.request(json!({"method":"tasks.create","params":{"project_id":f.project["id"],"goal":"Validate generated JSON with the explicitly approved rebinding capability"},"idempotency_key":"new-data-graph-task-keeps-history"})));
    let validator = GoalValidatorRef {
        kind: GoalValidatorKind::Data,
        policy_id: definition["id"].as_str().unwrap().into(),
        revision: definition["revision"].as_u64().unwrap(),
    };
    let scope = goal_scope_and_plan_with_options(
        &f,
        if bad {
            "goal-data-report-bad"
        } else {
            "goal-data-report"
        },
        0,
        4,
        5,
        8,
        vec![validator],
    );
    let view: GoalScopeView = serde_json::from_value(scope.clone()).unwrap();
    let current: GoalPlanView = serde_json::from_value(result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
    ))
    .unwrap();
    let mut plan = current.proposal;
    let mut producer = plan.nodes[0].clone();
    producer.outputs = plan.nodes[..3]
        .iter()
        .flat_map(|n| n.outputs.clone())
        .collect();
    producer.write_set = plan.nodes[..3]
        .iter()
        .flat_map(|n| n.write_set.clone())
        .collect();
    producer.required_validators = plan.nodes[..3]
        .iter()
        .flat_map(|n| n.required_validators.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let candidate_inputs = producer
        .outputs
        .iter()
        .map(|o| RequiredInput {
            goal_task_id: view.task_id.clone(),
            artifact_id: o.artifact_id.clone(),
            contract_id: o.contract_id.clone(),
            source: InputSource::Node {
                node_id: producer.id.clone(),
                candidate_for_validation: true,
            },
        })
        .collect::<Vec<_>>();
    let rule = view.contracts["output:report"]
        .iter()
        .find(|id| id.starts_with("data:"))
        .unwrap()
        .clone();
    let mut validate = producer.clone();
    validate.id = "validate-report".into();
    validate.kind = NodeKind::Validate;
    validate.executor = Executor::Validator {
        rule_id: rule.clone(),
    };
    validate.goal =
        "Validate exact new JSON schema/ranges/metadata without scientific acceptance".into();
    validate.dependencies = vec![producer.id.clone()];
    validate.required_inputs = candidate_inputs.clone();
    validate.outputs.clear();
    validate.write_set.clear();
    validate.side_effect = SideEffect::AdvisoryReadOnly;
    validate.required_validators = vec![rule];
    validate.idempotency_key = "data-validate-once".into();
    let mut owner = validate.clone();
    owner.id = "owner-report".into();
    owner.kind = NodeKind::OwnerApproval;
    owner.executor = Executor::Owner {
        rule_id: "goal-owner-review".into(),
    };
    owner.required_validators = vec!["goal-owner-review".into()];
    owner.dependencies.push(validate.id.clone());
    owner.idempotency_key = "data-owner-report-once".into();
    let mut summary = plan.nodes[3].clone();
    summary.dependencies = vec![owner.id.clone(), validate.id.clone()];
    let mut accepted = candidate_inputs[0].clone();
    accepted.source = InputSource::Node {
        node_id: producer.id.clone(),
        candidate_for_validation: false,
    };
    summary.required_inputs = vec![accepted];
    let mut last = owner.clone();
    last.id = "owner-summary".into();
    last.dependencies = vec![summary.id.clone()];
    last.required_inputs = vec![RequiredInput {
        goal_task_id: view.task_id.clone(),
        artifact_id: "report-4".into(),
        contract_id: "output:report-4".into(),
        source: InputSource::Node {
            node_id: summary.id.clone(),
            candidate_for_validation: true,
        },
    }];
    last.idempotency_key = "data-owner-summary-once".into();
    plan.nodes = vec![producer, validate, owner, summary, last];
    plan.revision = 2;
    plan.expected_revision = 1;
    result(f.server.request(json!({"method":"goal_plans.propose","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"actual-json-five-node-plan"})));
    (f, scope, definition)
}
#[test]
fn actual_graph_data_contract_passes_before_owner_and_summary_but_is_not_scientific_acceptance() {
    eprintln!("GRAPH_BUILD_SOURCES {}", graph_build_source_hashes());
    let (f, scope, definition) = actual_data_definition_graph_fixture(false);
    let approved = approve_graph_at(&f, &scope, 2);
    let id = &approved["run"]["id"];
    let checked = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-report" && n["state"] == "awaiting_owner")
            })
        },
    );
    let v = &checked["validations"][0];
    authentic_validation_from_projection(v);
    assert_eq!(v["passed"], true);
    assert_eq!(v["scope"], "data_contract_only");
    assert_eq!(v["correctness"], "approved_schema_ranges_metadata_only");
    assert_eq!(v["task_acceptance"], "not_established");
    assert_eq!(v["io_completion"], "worker_joined");
    assert_eq!(checked["task_accepted"], false);
    let owner_proposal = graph_owner_proposal(&f, id, "owner-report", "json-owner-first");
    let first_decision = approve(&f.server, owner_proposal.clone());
    wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-summary" && n["state"] == "awaiting_owner")
            })
        },
    );
    let final_owner_proposal = graph_owner_proposal(&f, id, "owner-summary", "json-owner-final");
    let final_decision = approve(&f.server, final_owner_proposal.clone());
    assert_eq!(final_decision["task_accepted"], true);
    let graph_details = result(
        f.server
            .request(json!({"method":"goal_graph_runs.get","params":{"run_id":id}})),
    );
    let task = result(
        f.server
            .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
    );
    assert_eq!(graph_details["task_accepted"], true);
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    let evidence = json!({"build_source_hashes":graph_build_source_hashes(),"task":task,"scope":scope,"definition":definition,"graph_details":graph_details,"owner_proposal":owner_proposal,"first_decision":first_decision,"final_owner_proposal":final_owner_proposal,"final_decision":final_decision});
    export_graph_fixture("data", &evidence);
}
#[test]
fn actual_graph_data_range_failure_never_unlocks_owner_or_dependent_summary() {
    eprintln!("GRAPH_BUILD_SOURCES {}", graph_build_source_hashes());
    let (f, scope, _) = actual_data_definition_graph_fixture(true);
    let approved = approve_graph_at(&f, &scope, 2);
    let id = &approved["run"]["id"];
    let failed = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":id}),
        |d| d["validations"].as_array().is_some_and(|v| !v.is_empty()),
    );
    assert_eq!(failed["validations"][0]["passed"], false);
    assert_eq!(failed["task_accepted"], false);
    assert!(failed["owner_decisions"].as_array().unwrap().is_empty());
    assert!(failed["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["node"]["id"] == "owner-report" || n["node"]["id"] == "independent-4")
        .all(|n| n["state"] == "not_started"));
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
}
fn export_graph_fixture(kind: &str, evidence: &Value) {
    use std::io::Write;
    let bytes = serde_json::to_vec_pretty(evidence).unwrap();
    let sha = agentbaton_workspaces::validation::sha256(&bytes);
    let path = graph_artifact_path(format!("actual-{kind}-graph-{sha}.json"));
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .unwrap()
        .write_all(&bytes)
        .unwrap();
    eprintln!(
        "ACTUAL_GRAPH kind={kind} sha256={sha} bytes={} path={}",
        bytes.len(),
        path.display()
    );
}

fn actual_command_definition_graph_fixture(command_case: &str) -> (Scenario, Value, Value) {
    use agentbaton_core::{goal_plan::*, goal_store::*};
    let mut f = Scenario::new(10_000);
    use_actual_dataset_workspace(&mut f);
    let grant_proposal=result(f.server.request(json!({"method":"grants.propose","params":{"project_id":f.project["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"max_attempts":8,"max_concurrency":1,"max_duration_ms":30000},"idempotency_key":"initial-eight-shared-attempt-policy"})));
    f.grant = approve(&f.server, grant_proposal);
    let bootstrap = goal_scope_and_plan_with_options(&f, "goal-report", 0, 1, 2, 8, vec![]);
    let source = approve_goal_execution(&f, &bootstrap);
    let source_done = wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":source["id"]}),
        |d| d["run"]["state"] == "awaiting_acceptance",
    );
    let session = &source_done["nodes"][0]["session_id"];
    let executable = if command_case == "normal" {
        "/usr/bin/printf"
    } else {
        if Path::new("/usr/bin/sleep").exists() {
            "/usr/bin/sleep"
        } else {
            "/bin/sleep"
        }
    };
    let arguments = if command_case == "normal" {
        vec!["fixed command observed"]
    } else {
        vec!["3"]
    };
    let program = std::fs::canonicalize(executable).unwrap();
    let policy_proposal=result(f.server.request(json!({"method":"command_validation_policies.propose","params":{"session_id":session,"executable":program,"mode":"native_executable","script":null,"inputs":[{"artifact_id":"fixed-input","relative_path":"inputs/file","max_bytes":1024,"expected_sha256":agentbaton_workspaces::validation::sha256(b"baseline")}],"outputs":[{"artifact_id":"report","relative_path":"reports/report.txt","max_bytes":1024,"expected_sha256":null}],"arguments":arguments,"environment":{},"expected_exit_codes":[0],"wall_time_ms":if command_case=="normal" {500}else{1500},"cleanup_time_ms":500,"stdout_limit":512,"stderr_limit":512,"require":agentbaton_core::command_validation::CommandEnforcement::default()},"idempotency_key":"source-additional-command-definition"})));
    let definition = approve(&f.server, policy_proposal);
    result(f.server.request(json!({"method":"goal_runs.cancel","params":{"run_id":source["id"]},"expected_revision":source_done["run"]["revision"],"idempotency_key":"cancel-source-candidate-before-new-graph"})));
    wait(
        &f.server,
        "goal_runs.get",
        json!({"run_id":source["id"]}),
        |d| d["run"]["state"] == "cancelled",
    );
    let grants = result(
        f.server
            .request(json!({"method":"grants.list","params":{"project_id":f.project["id"]}})),
    );
    f.grant = grants
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == f.grant["id"])
        .unwrap()
        .clone();
    f.task=result(f.server.request(json!({"method":"tasks.create","params":{"project_id":f.project["id"],"goal":"Use approved same-workspace graph definition with a new owned producer subject"},"idempotency_key":"new-graph-task-keeps-project-history"})));
    let validator = GoalValidatorRef {
        kind: GoalValidatorKind::Command,
        policy_id: definition["id"].as_str().unwrap().into(),
        revision: definition["revision"].as_u64().unwrap(),
    };
    let scope = goal_scope_and_plan_with_options(&f, "goal-report", 0, 2, 5, 8, vec![validator]);
    let view: GoalScopeView = serde_json::from_value(scope.clone()).unwrap();
    let current: GoalPlanView = serde_json::from_value(result(
        f.server
            .request(json!({"method":"goal_plans.get","params":{"scope_id":scope["id"]}})),
    ))
    .unwrap();
    let mut plan = current.proposal;
    let producer = plan.nodes[0].clone();
    let mut summary = plan.nodes[1].clone();
    let rule_id = view.contracts["output:report"]
        .iter()
        .find(|id| id.starts_with("command:"))
        .unwrap()
        .clone();
    let candidate = RequiredInput {
        goal_task_id: view.task_id.clone(),
        artifact_id: "report".into(),
        contract_id: "output:report".into(),
        source: InputSource::Node {
            node_id: producer.id.clone(),
            candidate_for_validation: true,
        },
    };
    let mut validate = producer.clone();
    validate.id = "validate-report".into();
    validate.kind = NodeKind::Validate;
    validate.executor = Executor::Validator {
        rule_id: rule_id.clone(),
    };
    validate.goal = "Apply exact approved command definition to the new captured producer".into();
    validate.dependencies = vec![producer.id.clone()];
    validate.required_inputs = vec![candidate.clone()];
    validate.outputs.clear();
    validate.required_validators = vec![rule_id.clone()];
    validate.side_effect = SideEffect::AdvisoryReadOnly;
    validate.write_set.clear();
    validate.idempotency_key = "validate-owned-report-once".into();
    let mut owner = validate.clone();
    owner.id = "owner-report".into();
    owner.kind = NodeKind::OwnerApproval;
    owner.executor = Executor::Owner {
        rule_id: "goal-owner-review".into(),
    };
    owner.goal = "Owner reviews exact report after deterministic command only".into();
    owner.dependencies = vec![producer.id.clone(), validate.id.clone()];
    owner.required_validators = vec!["goal-owner-review".into()];
    owner.idempotency_key = "owner-report-once".into();
    summary.dependencies = vec![validate.id.clone(), owner.id.clone()];
    let mut accepted = candidate;
    accepted.source = InputSource::Node {
        node_id: producer.id.clone(),
        candidate_for_validation: false,
    };
    summary.required_inputs = vec![accepted];
    let mut final_owner = owner.clone();
    final_owner.id = "owner-summary".into();
    final_owner.dependencies = vec![summary.id.clone()];
    final_owner.required_inputs = vec![RequiredInput {
        goal_task_id: view.task_id.clone(),
        artifact_id: "report-2".into(),
        contract_id: "output:report-2".into(),
        source: InputSource::Node {
            node_id: summary.id.clone(),
            candidate_for_validation: true,
        },
    }];
    final_owner.idempotency_key = "owner-summary-once".into();
    plan.nodes = vec![producer, validate, owner, summary, final_owner];
    plan.revision = 2;
    plan.expected_revision = 1;
    result(f.server.request(json!({"method":"goal_plans.propose","params":{"scope_id":scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"actual-command-five-node-plan"})));
    (f, scope, definition)
}

#[test]
fn actual_graph_fixed_command_observation_is_charged_without_recounting_checked_producer_bytes() {
    eprintln!("GRAPH_BUILD_SOURCES {}", graph_build_source_hashes());
    let (f, scope, definition) = actual_command_definition_graph_fixture("normal");
    let approved = approve_graph_at(&f, &scope, 2);
    let id = &approved["run"]["id"];
    let checked = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-report" && n["state"] == "awaiting_owner")
            })
        },
    );
    let receipt = &checked["validations"][0];
    authentic_validation_from_projection(receipt);
    assert_eq!(receipt["passed"], true);
    assert_eq!(receipt["scope"], "approved_deterministic_command_only");
    assert_eq!(
        receipt["correctness"],
        "approved_fixed_command_observations_only"
    );
    assert_eq!(receipt["task_acceptance"], "not_established");
    assert_eq!(receipt["io_completion"], "worker_joined");
    assert_eq!(checked["task_accepted"], false);
    let owner_proposal = graph_owner_proposal(&f, id, "owner-report", "command-owner-first");
    let first_decision = approve(&f.server, owner_proposal.clone());
    wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-summary" && n["state"] == "awaiting_owner")
            })
        },
    );
    let final_owner_proposal = graph_owner_proposal(&f, id, "owner-summary", "command-owner-final");
    let final_decision = approve(&f.server, final_owner_proposal.clone());
    assert_eq!(final_decision["task_accepted"], true);
    let graph_details = result(
        f.server
            .request(json!({"method":"goal_graph_runs.get","params":{"run_id":id}})),
    );
    let task = result(
        f.server
            .request(json!({"method":"tasks.get","params":{"task_id":f.task["id"]}})),
    );
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    for key in [
        format!("project:{}", f.project["id"].as_str().unwrap()),
        format!("root-grant:{}", f.grant["id"].as_str().unwrap()),
    ] {
        let raw: String = db
            .query_row(
                "SELECT document FROM goal_ledgers WHERE scope=?1",
                [&key],
                |r| r.get(0),
            )
            .unwrap();
        let meter: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(meter["output_bytes"], 60, "{meter}");
    }
    let evidence = json!({"build_source_hashes":graph_build_source_hashes(),"task":task,"scope":scope,"definition":definition,"graph_details":graph_details,"owner_proposal":owner_proposal,"first_decision":first_decision,"final_owner_proposal":final_owner_proposal,"final_decision":final_decision});
    export_graph_fixture("command", &evidence);
}

fn command_pid_exists(pid: u32) -> bool {
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "pid="])
        .output()
        .unwrap()
        .stdout
        .iter()
        .any(u8::is_ascii_digit)
}
fn wait_actual_graph_command_child(_f: &Scenario) -> u32 {
    let started = Instant::now();
    loop {
        let output = Command::new("ps")
            .args(["-axo", "pid=,ppid=,args="])
            .output()
            .unwrap();
        assert!(output.status.success());
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let mut fields = line.split_whitespace();
            let Some(pid) = fields.next().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            let Some(parent) = fields.next().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            let Some(command) = fields.next() else {
                continue;
            };
            if parent == std::process::id()
                && Path::new(command).file_name().is_some_and(|n| n == "sleep")
            {
                return pid;
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "actual approved command did not spawn"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn actual_graph_command_cancel_and_definition_revocation_join_the_owned_child_and_block_downstream()
{
    eprintln!("GRAPH_BUILD_SOURCES {}", graph_build_source_hashes());
    for action in ["cancel", "revoke"] {
        let (f, scope, definition) = actual_command_definition_graph_fixture("hold");
        let approved = approve_graph_at(&f, &scope, 2);
        let id = &approved["run"]["id"];
        let pid = wait_actual_graph_command_child(&f);
        let current = result(
            f.server
                .request(json!({"method":"goal_graph_runs.get","params":{"run_id":id}})),
        );
        if action == "cancel" {
            let receipt=result(f.server.request(json!({"method":"goal_graph_runs.cancel","params":{"run_id":id},"expected_revision":current["run"]["revision"],"idempotency_key":"cancel-real-validator-child"})));
            assert_eq!(receipt["run"]["cancel_requested"], true);
        } else {
            result(f.server.request(json!({"method":"command_validation_policies.revoke","params":{"policy_id":definition["id"]},"expected_revision":definition["revision"],"idempotency_key":"revoke-live-graph-definition"})));
        }
        let stopped = wait(
            &f.server,
            "goal_graph_runs.get",
            json!({"run_id":id}),
            |d| d["validations"].as_array().is_some_and(|vs| !vs.is_empty()),
        );
        let v = &stopped["validations"][0];
        assert_eq!(v["passed"], false);
        assert_eq!(v["stop_confirmed"], true);
        assert_eq!(v["io_completion"], "worker_joined");
        assert_eq!(stopped["task_accepted"], false);
        assert!(stopped["owner_decisions"].as_array().unwrap().is_empty());
        assert!(stopped["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["execution_kind"] == "owner" || n["node"]["kind"] == "summary")
            .all(|n| n["state"] == "not_started"));
        assert!(
            !command_pid_exists(pid),
            "owned command leader must actually be reaped"
        );
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            db.query_row(
                "SELECT consumed FROM budgets WHERE scope=?1",
                [format!("project:{}", f.project["id"].as_str().unwrap())],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            3
        );
        assert_eq!(
            db.query_row(
                "SELECT active FROM budgets WHERE scope=?1",
                [format!("project:{}", f.project["id"].as_str().unwrap())],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
        eprintln!("ACTUAL_COMMAND_STOP action={action} pid={pid} joined=true attempts=3 no downstream/acceptance/refund");
    }
}
#[test]
fn actual_graph_command_wall_timeout_reaps_without_owner_or_acceptance() {
    eprintln!("GRAPH_BUILD_SOURCES {}", graph_build_source_hashes());
    let (f, scope, _) = actual_command_definition_graph_fixture("timeout");
    let approved = approve_graph_at(&f, &scope, 2);
    let id = &approved["run"]["id"];
    let pid = wait_actual_graph_command_child(&f);
    let failed = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":id}),
        |d| d["validations"].as_array().is_some_and(|v| !v.is_empty()),
    );
    let v = &failed["validations"][0];
    assert_eq!(v["passed"], false);
    assert_eq!(v["stop_confirmed"], true);
    assert_eq!(v["io_completion"], "worker_joined");
    assert_eq!(v["observation"]["reason"], "deadline_exceeded");
    assert_eq!(failed["task_accepted"], false);
    assert!(!command_pid_exists(pid));
    assert!(failed["owner_decisions"].as_array().unwrap().is_empty());
    eprintln!(
        "ACTUAL_COMMAND_TIMEOUT pid={pid} observed={}ms",
        v["actual_runtime_ms"]
    );
}
#[test]
fn actual_graph_interrupted_validator_snapshot_is_unknown_fenced_and_never_automatically_replayed()
{
    eprintln!("GRAPH_BUILD_SOURCES {}", graph_build_source_hashes());
    let (mut f, scope, _) = actual_command_definition_graph_fixture("recovery");
    let approved = approve_graph_at(&f, &scope, 2);
    let id = &approved["run"]["id"];
    let _pid = wait_actual_graph_command_child(&f);
    let recovery = f.root.join("restore");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&recovery)
        .unwrap();
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    db.execute(
        "VACUUM INTO ?1",
        [recovery.join("baton.sqlite3").to_str().unwrap()],
    )
    .unwrap();
    std::fs::set_permissions(
        recovery.join("baton.sqlite3"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    drop(db);
    // Consistent in-flight durable crash image; original owned process is actually
    // stopped/joined before a second daemon is allowed to read its snapshot.
    f.server.shutdown();
    // Probe the original socket-length shape only in an exclusively created
    // private sibling. Never remove or bind an existing/unknown path.
    use std::os::unix::{fs::FileTypeExt, net::UnixListener};
    let old_name = "validator-interrupted-runtime";
    let old_socket = f.root.join(old_name).join("batond.sock");
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let probe = f.root.join(&nonce[..old_name.len()]);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&probe)
        .unwrap();
    let probe_socket = probe.join("batond.sock");
    assert_eq!(probe_socket.as_os_str().len(), old_socket.as_os_str().len());
    let outcome = match UnixListener::bind(&probe_socket) {
        Ok(listener) => {
            std::fs::set_permissions(&probe_socket, std::fs::Permissions::from_mode(0o600))
                .unwrap();
            drop(listener);
            assert!(std::fs::symlink_metadata(&probe_socket)
                .unwrap()
                .file_type()
                .is_socket());
            std::fs::remove_file(&probe_socket).unwrap();
            json!({"result":"bound_and_closed","error_kind":null,"raw_os_error":null})
        }
        Err(error) => json!({"result":"bind_failed","error_kind":format!("{:?}",error.kind()),
            "raw_os_error":error.raw_os_error(),"message":error.to_string()}),
    };
    eprintln!(
        "RECOVERY_SOCKET_PROBE {}",
        json!({"original_socket":old_socket,
        "original_socket_bytes":old_socket.as_os_str().len(),"probe_socket":probe_socket,
        "probe_socket_bytes":probe_socket.as_os_str().len(),"outcome":outcome,
        "scope":"test-only newly created private path; not a historical errno reconstruction"})
    );
    std::fs::remove_dir(&probe).unwrap();
    let recovered = Server::start(&recovery);
    let state =
        result(recovered.request(json!({"method":"goal_graph_runs.get","params":{"run_id":id}})));
    assert_eq!(state["run"]["state"], "outcome_unknown");
    assert_eq!(state["task_accepted"], false);
    let db = rusqlite::Connection::open(recovery.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM goal_graph_jobs WHERE state='outcome_unknown'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    let request = json!({"method":"goal_graph_runs.cancel","params":{"run_id":id},"expected_revision":state["run"]["revision"],"idempotency_key":"cancel-interrupted-unknown-validator"});
    result(recovered.request(request));
    let current =
        result(recovered.request(json!({"method":"goal_graph_runs.get","params":{"run_id":id}})));
    assert_ne!(current["run"]["state"], "cancelled");
    assert_eq!(current["task_accepted"], false);
    let sequence = result(recovered.request(json!({"method":"system.snapshot","params":{}})))
        ["sequence"]
        .clone();
    for _ in 0..5 {
        result(recovered.request(json!({"method":"goal_graph_runs.get","params":{"run_id":id}})));
    }
    assert_eq!(
        result(recovered.request(json!({"method":"system.snapshot","params":{}})))["sequence"],
        sequence
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    for key in [
        format!("project:{}", f.project["id"].as_str().unwrap()),
        format!("root-grant:{}", f.grant["id"].as_str().unwrap()),
    ] {
        let raw: String = db
            .query_row(
                "SELECT document FROM goal_ledgers WHERE scope=?1",
                [key],
                |r| r.get(0),
            )
            .unwrap();
        let ledger: Value = serde_json::from_str(&raw).unwrap();
        assert!(ledger["runtime_ms"].is_null());
        assert!(ledger["output_bytes"].is_null());
    }
    eprintln!("ACTUAL_RECOVERY consistent in-flight validator snapshot: no replay/refund/falsecancelled, unknown meters/fence preserved");
}

fn authentic_validation_from_projection(view: &Value) -> Value {
    let raw = view["observation_json"]
        .as_str()
        .expect("lossless observation string");
    assert_eq!(
        agentbaton_workspaces::validation::sha256(raw.as_bytes()),
        view["result_digest"]
    );
    let restored: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(restored, view["observation"]);
    let mut original = view.clone();
    original.as_object_mut().unwrap().remove("observation_json");
    original
}

#[test]
fn actual_graph_temporary_global_slot_and_pending_head_require_explicit_adoption_without_replay() {
    for pending_version in [false, true] {
        let mut f = Scenario::new(10_000);
        let first_scope = goal_scope_and_plan(&f, "goal-report-hold");
        let first_root = PathBuf::from(f.workspace["root"].as_str().unwrap());
        another_goal_root(&mut f, 42);
        let second_scope = goal_scope_and_plan(&f, "goal-report");
        let first = approve_graph_at(&f, &first_scope, 1)["run"].clone();
        wait(
            &f.server,
            "goal_graph_runs.get",
            json!({"run_id":first["id"]}),
            |r| {
                r["nodes"][0]["state"] == "running" && first_root.join("fixture-goal-held").exists()
            },
        );
        let second = approve_graph_at(&f, &second_scope, 1)["run"].clone();
        let second_root = PathBuf::from(f.workspace["root"].as_str().unwrap());
        assert_ne!(first_root, second_root);
        let mut advance_request = None;
        if pending_version {
            use agentbaton_core::goal_plan::GoalPlanProposal;
            let current = result(f.server.request(
                json!({"method":"goal_plans.get","params":{"scope_id":second_scope["id"]}}),
            ));
            let mut plan: GoalPlanProposal =
                serde_json::from_value(current["proposal"].clone()).unwrap();
            plan.revision = 2;
            plan.expected_revision = 1;
            plan.nodes[0].goal = "goal-report-v2".into();
            result(f.server.request(json!({"method":"goal_plans.revise","params":{"scope_id":second_scope["id"],"plan_json":serde_json::to_string(&plan).unwrap()},"expected_revision":1,"idempotency_key":"queued-save-version2-only"})));
            let paused = wait(
                &f.server,
                "goal_graph_runs.get",
                json!({"run_id":second["id"]}),
                |r| r["run"]["state"] == "awaiting_acceptance",
            );
            assert_eq!(paused["run"]["plan_revision"], 1);
            assert_eq!(paused["nodes"][0]["state"], "not_started");
        }
        let quiet = result(
            f.server
                .request(json!({"method":"system.snapshot","params":{}})),
        )["sequence"]
            .clone();
        let until = Instant::now() + Duration::from_millis(200);
        while Instant::now() < until {
            let queued =
                result(f.server.request(
                    json!({"method":"goal_graph_runs.get","params":{"run_id":second["id"]}}),
                ));
            let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
            let total = db
                .query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap();
            if queued["run"]["state"]
                != if pending_version {
                    "awaiting_acceptance"
                } else {
                    "queued"
                }
            {
                let reason:String=db.query_row("SELECT json_extract(document,'$.payload.reason_code') FROM events WHERE json_extract(document,'$.kind')='goal.run_blocked' AND json_extract(document,'$.aggregate_id')=?1",[second["id"].as_str().unwrap()],|r|r.get(0)).unwrap();
                eprintln!(
                "TEMPORARY_GOAL_SLOT second_state={} total_attempts={total} second_node={} reason_code={reason}",
                queued["run"]["state"], queued["nodes"][0]["state"]
            );
            }
            assert_eq!(
                queued["run"]["state"],
                if pending_version {
                    "awaiting_acceptance"
                } else {
                    "queued"
                },
                "{queued}"
            );
            assert_eq!(queued["nodes"][0]["state"], "not_started");
            assert!(queued["nodes"][0]["session_id"].is_null());
            assert!(queued["nodes"][0]["attempt_id"].is_null());
            assert_eq!(total, 1);
            assert_eq!(
                result(
                    f.server
                        .request(json!({"method":"system.snapshot","params":{}}))
                )["sequence"],
                quiet
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        std::fs::write(
            first_root.join("fixture-goal-release"),
            b"release generated fixture",
        )
        .unwrap();
        let first_done = wait(
            &f.server,
            "goal_graph_runs.get",
            json!({"run_id":first["id"]}),
            |r| r["run"]["state"] == "awaiting_acceptance",
        );
        if pending_version {
            let paused =
                result(f.server.request(
                    json!({"method":"goal_graph_runs.get","params":{"run_id":second["id"]}}),
                ));
            assert_eq!(paused["run"]["state"], "awaiting_acceptance");
            assert_eq!(paused["run"]["plan_revision"], 1);
            let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
            assert_eq!(
                db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                    .unwrap(),
                1
            );
            assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.run_plan_pending' AND json_extract(document,'$.aggregate_id')=?1",[second["id"].as_str().unwrap()],|r|r.get::<_,u32>(0)).unwrap(),1);
            let sequence = result(
                f.server
                    .request(json!({"method":"system.snapshot","params":{}})),
            )["sequence"]
                .clone();
            for _ in 0..5 {
                let _ = result(f.server.request(
                    json!({"method":"goal_graph_runs.get","params":{"run_id":second["id"]}}),
                ));
            }
            assert_eq!(
                result(
                    f.server
                        .request(json!({"method":"system.snapshot","params":{}}))
                )["sequence"],
                sequence
            );
            let request = json!({"method":"goal_graph_runs.advance_plan","params":{"run_id":second["id"],"expected_plan_revision":2,"expected_ledger_revision":sequence},"expected_revision":paused["run"]["revision"],"idempotency_key":"explicit-adopt-queued-version2"});
            let receipt = result(f.server.request(request.clone()));
            advance_request = Some((request, receipt));
        }
        let second_done = wait(
            &f.server,
            "goal_graph_runs.get",
            json!({"run_id":second["id"]}),
            |r| r["run"]["state"] == "awaiting_acceptance",
        );
        for (index, done) in [&first_done, &second_done].iter().enumerate() {
            let node = done["nodes"].as_array().unwrap().last().unwrap();
            assert_eq!(node["state"], "candidate");
            assert_eq!(node["stop_confirmed"], true);
            assert_eq!(
                node["actual_output_bytes"],
                if pending_version && index == 1 {
                    15
                } else {
                    12
                }
            );
            assert_eq!(done["task_accepted"], false);
        }
        if let Some((request, receipt)) = advance_request {
            let sequence = result(
                f.server
                    .request(json!({"method":"system.snapshot","params":{}})),
            )["sequence"]
                .clone();
            assert_eq!(result(f.server.request(request)), receipt);
            assert_eq!(
                result(
                    f.server
                        .request(json!({"method":"system.snapshot","params":{}}))
                )["sequence"],
                sequence
            );
            assert_eq!(second_done["run"]["plan_revision"], 2);
            assert_eq!(second_done["nodes"].as_array().unwrap().len(), 2);
            assert_eq!(second_done["nodes"][0]["state"], "not_started");
            assert!(second_done["nodes"][0]["attempt_id"].is_null());
        }
        assert_ne!(
            first_done["nodes"][0]["attempt_id"],
            second_done["nodes"].as_array().unwrap().last().unwrap()["attempt_id"]
        );
        assert_eq!(
            std::fs::read(first_root.join("reports/report.txt")).unwrap(),
            b"fixed report"
        );
        assert_eq!(
            std::fs::read(second_root.join("reports/report.txt")).unwrap(),
            if pending_version {
                b"fixed report v2".as_slice()
            } else {
                b"fixed report".as_slice()
            }
        );
        let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row(
                "SELECT consumed FROM budgets WHERE scope=?1",
                [format!("project:{}", f.project["id"].as_str().unwrap())],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            2
        );
        assert_eq!(
            db.query_row(
                "SELECT json_extract(document,'$.output_bytes') FROM goal_ledgers WHERE scope=?1",
                [format!("project:{}", f.project["id"].as_str().unwrap())],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            if pending_version { 27 } else { 24 }
        );
        assert_eq!(db.query_row("SELECT count(*) FROM approval_proposals WHERE json_extract(document,'$.kind')='goal_graph_execution'",[],|r|r.get::<_,u32>(0)).unwrap(),2);
    }
}

#[test]
fn actual_graph_fixed_concurrent_review_keeps_other_kinds_setup_and_corrupt_authority_refused() {
    let mut f = Scenario::new(10_000);
    let first_scope = goal_scope_and_plan(&f, "goal-report-hold");
    let first_root = PathBuf::from(f.workspace["root"].as_str().unwrap());
    another_goal_root(&mut f, 73);
    let second_scope = goal_scope_and_plan(&f, "goal-report");
    let legacy=result(f.server.request(json!({"method":"grants.propose","params":{"project_id":f.project["id"],"workspace_id":f.workspace["id"],"connection_id":f.connection["id"],"max_attempts":2,"max_concurrency":1,"max_duration_ms":10000},"idempotency_key":"legacy-kind-before-worker"})));
    let first = approve_graph_at(&f, &first_scope, 1)["run"].clone();
    wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":first["id"]}),
        |r| r["nodes"][0]["state"] == "running" && first_root.join("fixture-goal-held").exists(),
    );
    let old=f.server.request(json!({"method":"approvals.finalize","params":{"proposal_id":legacy["id"]},"expected_revision":legacy["revision"],"idempotency_key":"legacy-not-idle"}));
    assert_eq!(old["error"]["code"], "WORKSPACE_BUSY");
    let setup=f.server.request(json!({"method":"workspaces.create","params":{"project_id":f.project["id"],"expected_head":"untrusted-not-used"},"expected_revision":f.project["revision"],"idempotency_key":"setup-must-remain-idle"}));
    assert_eq!(setup["error"]["code"], "WORKSPACE_BUSY");
    let seq = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    )["sequence"]
        .clone();
    let same=f.server.request(json!({"method":"goal_graph_runs.propose","params":{"scope_id":first_scope["id"],"expected_plan_revision":1,"expected_ledger_revision":seq},"idempotency_key":"same-workspace-new-authority-refused"}));
    assert_eq!(same["error"]["code"], "WORKSPACE_BUSY");
    let sequence = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    )["sequence"]
        .clone();
    let proposal=result(f.server.request(json!({"method":"goal_graph_runs.propose","params":{"scope_id":second_scope["id"],"expected_plan_revision":1,"expected_ledger_revision":sequence},"idempotency_key":"queued-graph-fixed-proposal"})));
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    let id = proposal["id"].as_str().unwrap();
    let original: String = db
        .query_row(
            "SELECT document FROM approval_proposals WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    let legacy_id = legacy["id"].as_str().unwrap();
    let legacy_original: String = db
        .query_row(
            "SELECT document FROM approval_proposals WHERE id=?1",
            [legacy_id],
            |r| r.get(0),
        )
        .unwrap();
    for mode in [
        "unknown_id",
        "renderer_kind",
        "oversize",
        "wrong_doc_id",
        "wrong_stored_kind",
        "tampered_snapshot",
        "forged_legacy_kind",
    ] {
        db.execute(
            "UPDATE approval_proposals SET document=?1 WHERE id=?2",
            rusqlite::params![original, id],
        )
        .unwrap();
        db.execute(
            "UPDATE approval_proposals SET document=?1 WHERE id=?2",
            rusqlite::params![legacy_original, legacy_id],
        )
        .unwrap();
        let mut request_id = id;
        match mode {
            "unknown_id" | "renderer_kind" => request_id = "unknown-owner-proposal",
            "oversize" => {
                db.execute(
                    "UPDATE approval_proposals SET document=?1 WHERE id=?2",
                    rusqlite::params![original.clone() + &" ".repeat(1048576), id],
                )
                .unwrap();
            }
            "wrong_doc_id" => {
                db.execute("UPDATE approval_proposals SET document=json_set(document,'$.id','foreign-stored-proposal') WHERE id=?1",[id]).unwrap();
            }
            "wrong_stored_kind" => {
                db.execute("UPDATE approval_proposals SET document=json_set(document,'$.kind','goal_graph_execution_other') WHERE id=?1",[id]).unwrap();
            }
            "tampered_snapshot" => {
                db.execute("UPDATE approval_proposals SET document=json_set(document,'$.snapshot.new_workspace_authority',json('true')) WHERE id=?1",[id]).unwrap();
            }
            _ => {
                db.execute("UPDATE approval_proposals SET document=json_set(document,'$.kind','goal_graph_execution') WHERE id=?1",[legacy_id]).unwrap();
                request_id = legacy_id;
            }
        }
        let mut params = json!({"proposal_id":request_id});
        if mode == "renderer_kind" {
            params["kind"] = json!("goal_graph_execution");
        }
        let response=f.server.request(json!({"method":"approvals.finalize","params":params,"expected_revision":1,"idempotency_key":format!("no-false-goal-authority-{mode}")}));
        let code = response["error"]["code"]
            .as_str()
            .expect("all corrupt/foreign authority must be refused");
        assert!(
            matches!(
                code,
                "WORKSPACE_BUSY"
                    | "RESOURCE_LIMIT"
                    | "INPUT_CHANGED"
                    | "PERMISSION_DENIED"
                    | "INVALID_REQUEST"
            ),
            "{mode}: {response}"
        );
        if mode == "renderer_kind" {
            assert_eq!(code, "INVALID_REQUEST");
        }
        if mode == "oversize" {
            assert_eq!(code, "RESOURCE_LIMIT");
        }
        if matches!(mode, "unknown_id" | "wrong_doc_id" | "wrong_stored_kind") {
            assert_eq!(code, "WORKSPACE_BUSY");
        }
        assert_eq!(
            db.query_row("SELECT count(*) FROM goal_runs", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            1,
            "{mode}"
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            1,
            "{mode}"
        );
        assert!(db
            .query_row(
                "SELECT result IS NULL FROM approval_proposals WHERE id=?1",
                [id],
                |r| r.get::<_, bool>(0)
            )
            .unwrap());
        assert_eq!(
            db.query_row("SELECT count(*) FROM managed_workspaces", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            2
        );
        eprintln!(
            "FIXED_GOAL_REVIEW_REFUSED mode={mode} code={code} goal_runs=1 attempts=1 workspaces=2"
        );
    }
    db.execute(
        "UPDATE approval_proposals SET document=?1 WHERE id=?2",
        rusqlite::params![original, id],
    )
    .unwrap();
    db.execute(
        "UPDATE approval_proposals SET document=?1 WHERE id=?2",
        rusqlite::params![legacy_original, legacy_id],
    )
    .unwrap();
    std::fs::write(
        first_root.join("fixture-goal-release"),
        b"release only the real first process",
    )
    .unwrap();
    let first = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":first["id"]}),
        |r| r["run"]["state"] == "awaiting_acceptance",
    );
    assert_eq!(first["nodes"][0]["stop_confirmed"], true);
    assert_eq!(first["task_accepted"], false);
    let sequence = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    )["sequence"]
        .clone();
    let idle=f.server.request(json!({"method":"approvals.finalize","params":{"proposal_id":"unknown-owner-proposal"},"expected_revision":1,"idempotency_key":"idle-unknown-never-authorizes"}));
    assert_eq!(idle["error"]["code"], "NOT_FOUND");
    assert_eq!(idle["error"]["side_effect_state"], "not_started");
    assert_eq!(
        result(
            f.server
                .request(json!({"method":"system.snapshot","params":{}}))
        )["sequence"],
        sequence
    );
}

#[test]
fn actual_graph_hard_shared_attempt_limit_keeps_bootstrap_usage_and_blocks_summary_after_owner() {
    eprintln!("GRAPH_BUILD_SOURCES {}", graph_build_source_hashes());
    let (f, scope, _) = actual_integrity_definition_graph_fixture_with_budget(3);
    let approved = approve_graph_at(&f, &scope, 2);
    let id = &approved["run"]["id"];
    wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":id}),
        |d| {
            d["nodes"].as_array().is_some_and(|ns| {
                ns.iter()
                    .any(|n| n["node"]["id"] == "owner-report" && n["state"] == "awaiting_owner")
            })
        },
    );
    let proposal = graph_owner_proposal(&f, id, "owner-report", "budget-accept-report-only");
    let decision = approve(&f.server, proposal);
    assert_eq!(decision["task_accepted"], false);
    let blocked = wait(
        &f.server,
        "goal_graph_runs.get",
        json!({"run_id":id}),
        |d| d["run"]["state"] == "blocked",
    );
    assert_eq!(blocked["task_accepted"], false);
    let summary = blocked["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["node"]["kind"] == "summary")
        .unwrap();
    assert_eq!(summary["state"], "not_started");
    assert!(summary["attempt_id"].is_null());
    let db = rusqlite::Connection::open(f.server.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        db.query_row(
            "SELECT consumed FROM budgets WHERE scope=?1",
            [format!("project:{}", f.project["id"].as_str().unwrap())],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        3
    );
    assert!(!PathBuf::from(f.workspace["root"].as_str().unwrap())
        .join("reports/report-2.txt")
        .exists());
    let sequence = result(
        f.server
            .request(json!({"method":"system.snapshot","params":{}})),
    )["sequence"]
        .clone();
    for _ in 0..5 {
        result(
            f.server
                .request(json!({"method":"goal_graph_runs.get","params":{"run_id":id}})),
        );
    }
    assert_eq!(
        result(
            f.server
                .request(json!({"method":"system.snapshot","params":{}}))
        )["sequence"],
        sequence
    );
    eprintln!("ACTUAL_GRAPH_BUDGET bootstrap1+Work1+Validator1 reaches approved3; Owner decision neither refunds nor mints Summary capacity");
}
