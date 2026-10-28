//! Generated local files and explicit trusted-core connection/owner fixtures.
//! Runtime uses generated ACP + actual fixed Baton MCP child; no vendor/model/native dialog.
use agentbaton_config::runtime::{RuntimePlan, RuntimeProvider};
use agentbaton_core::{factory::*, goal_plan::BudgetLimits, storage::Store};
use agentbaton_workspaces::validation::{sha256, ArtifactRequirement};
use rusqlite::{params, Connection};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};
struct Fixture {
    _temp: tempfile::TempDir,
    path: PathBuf,
    store: Store,
    selection: FactoryScopeSelection,
    companion: PathBuf,
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Generated fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@invalid")
        .env("GIT_COMMITTER_NAME", "Generated fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@invalid")
        .args(["-c", "core.hooksPath=/dev/null", "-C"])
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let base = temp.path().canonicalize().unwrap();
        let path = base.join("baton.sqlite3");
        drop(
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&path)
                .unwrap(),
        );
        let mut store = Store::open(&path).unwrap();
        let source = base.join("source");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        fs::write(source.join("input.txt"), b"fixed input\n").unwrap();
        fs::write(source.join(".gitignore"), b"reports/\n").unwrap();
        git(&source, &["add", "input.txt", ".gitignore"]);
        git(&source, &["commit", "-m", "generated input"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let project = store
            .import_project(&source, "Generated Factory scope", "project")
            .unwrap();
        let managed = base.join("managed");
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        let workspace = store
            .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
            .unwrap();
        let companion = base.join("generated-companion-binding-only");
        fs::write(
            &companion,
            b"Generated companion hash fixture. Never executed by these tests.\n",
        )
        .unwrap();
        fs::set_permissions(&companion, fs::Permissions::from_mode(0o700)).unwrap();
        let connection = json!({"id":"generated-connection","name":"Explicit synthetic connection; no Provider call","provider":"custom_acp","protocol_route":"custom_acp","executable":companion,"arguments":[],"executable_sha256":sha256(&fs::read(&companion).unwrap()),"environment_id":"generated-environment","identity_ref":"unknown","host_id":"local","config_revision":"generated-config","runtime_plan":RuntimePlan::empty(RuntimeProvider::CustomAcp),"probe_state":"ready","revision":1,"approval_id":"synthetic-no-provider","capability_snapshot_id":"generated-capability","identity_status":"unknown","config_status":"baton_managed","created_at":"2026-10-28T00:00:00Z","version_evidence":null});
        Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO connections(id,document) VALUES(?1,?2)",
                params!["generated-connection", connection.to_string()],
            )
            .unwrap();
        let task = store
            .create_registered(
                &project.id,
                "Delegate only the fixed generated input and return a report",
                "root-task",
            )
            .unwrap();
        let grant = store
            .create_grant(
                &project.id,
                &workspace.id,
                "generated-connection",
                8,
                1,
                30000,
                true,
                "trusted-generated-grant",
            )
            .unwrap();
        assert_eq!(grant.scope.max_depth, 0);
        assert_eq!(grant.scope.max_children, 0);
        let budget = BudgetLimits {
            attempts: 8,
            parallel: 1,
            runtime_ms: 30000,
            output_bytes: 4096,
            children: 3,
            reworks: 0,
            tokens: None,
            cost_microunits: None,
        };
        let selection = FactoryScopeSelection {
            root_task_id: task.id,
            expected_task_revision: task.revision,
            grant_id: grant.id,
            expected_grant_revision: grant.revision,
            connection_ids: vec!["generated-connection".into()],
            workspace_ids: vec![workspace.id],
            inputs: vec![ArtifactRequirement {
                artifact_id: "input".into(),
                relative_path: "input.txt".into(),
                max_bytes: 1024,
                expected_sha256: Some(sha256(b"fixed input\n")),
            }],
            outputs: vec![ArtifactRequirement {
                artifact_id: "report".into(),
                relative_path: "reports/report.txt".into(),
                max_bytes: 4096,
                expected_sha256: None,
            }],
            recipients: vec![],
            max_depth: 2,
            max_children: 2,
            max_total_nodes: 4,
            root_budget: budget.clone(),
            project_budget: budget,
            ttl_ms: 30000,
            capabilities: FactoryCapabilities {
                delegate_task: true,
                resume_parent_as_new_attempt: true,
                child_planning: false,
                child_goal_execution: false,
                create_workspace: false,
            },
        };
        Self {
            _temp: temp,
            path,
            store,
            selection,
            companion,
        }
    }
    fn proposal(&mut self) -> agentbaton_core::sessions::ApprovalProposal {
        self.store
            .propose_factory_scope(&self.selection, "factory-proposal", &self.companion)
            .unwrap()
    }
    fn scope(&mut self) -> FactoryScope {
        let p = self.proposal();
        serde_json::from_value(self.store.finalize_proposal(&p.id, p.revision).unwrap()).unwrap()
    }
}

const ACP_FACTORY: &str = r#"#!/usr/bin/python3
import json,os,sys,subprocess,hashlib
os.makedirs('reports',exist_ok=True)
sys.stderr=open('reports/fixture-stderr.txt','a')
if '--version' in sys.argv:
 print('Generated Factory ACP 1');sys.exit(0)
def emit(v):print(json.dumps(v),flush=True)
def response(r,v):emit({'jsonrpc':'2.0','id':r['id'],'result':v})
def mcp_call(method,params):
 global mid
 mid+=1;mcp.stdin.write(json.dumps({'jsonrpc':'2.0','id':mid,'method':method,'params':params})+'\n');mcp.stdin.flush()
 result=json.loads(mcp.stdout.readline());assert 'error' not in result,result
 return result['result']
def tool(name,args):
 result=mcp_call('tools/call',{'name':name,'arguments':args})
 actual=json.loads(result['content'][0]['text']);assert 'error' not in actual,actual
 return actual['result']
mcp=None;mid=0
for line in sys.stdin:
 r=json.loads(line);method=r.get('method')
 if method=='initialize':
  assert not any(k in os.environ for k in ('GH_TOKEN','GITHUB_TOKEN','OPENAI_API_KEY'))
  response(r,{'protocolVersion':1,'agentCapabilities':{}})
 elif method=='session/new':
  servers=r['params']['mcpServers'];assert len(servers)==1
  server=servers[0];assert server['name']=='agentbaton-factory' and server['env']==[]
  mcp=subprocess.Popen([server['command']]+server['args'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
  mcp_call('initialize',{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'generated-local-fixture','version':'1'}})
  names=[v['name'] for v in mcp_call('tools/list',{})['tools']]
  assert names==['delegate_task','get_task_status','get_task_result','cancel_task']
  response(r,{'sessionId':'generated-native'})
 elif method=='session/prompt':
  c=json.loads(r['params']['prompt'][0]['text']);assert c['kind']=='agentbaton_factory_task_request'
  status=tool('get_task_status',{'task_id':c['task_id']});assert status['task_id']==c['task_id']
  mid+=1;mcp.stdin.write(json.dumps({'jsonrpc':'2.0','id':mid,'method':'tools/call','params':{'name':'approvals.finalize','arguments':{'proposal_id':'forbidden'}}})+'\n');mcp.stdin.flush()
  denied=json.loads(mcp.stdout.readline());assert 'error' in denied
  if fixture_mode=='self_cancel':
   stopped=tool('cancel_task',{'task_id':c['task_id'],'expected_revision':status['revision'],'idempotency_key':'self-cancel'})
   assert stopped['cancel_requested'] is True
   continue
  if fixture_mode=='pin_drift':
   open('reports/pin-ready','w').write('generated safe gate')
   import time
   while not os.path.exists('reports/proceed'):time.sleep(0.005)
   continue
  if fixture_mode=='malformed':print('generated malformed protocol',flush=True);continue
  os.makedirs('reports',exist_ok=True)
  stage='child' if c['parent_task_id'] else ('parent-resumed' if c['handoff_result'] or c.get('handoff_results') else 'parent-initial')
  with open('reports/mcp-stages.txt','a')as f:f.write(stage+'\n')
  if stage=='parent-initial':
   child=tool('delegate_task',{'goal':'Write the fixed local child report','connection_id':status_connection,'workspace_id':status_workspace,'input_artifact_ids':['input'],'output_artifact_ids':['report_a' if fixture_mode=='two_children' else 'report'],'budget':{'attempts':2,'parallel':1,'runtime_ms':5000,'output_bytes':4096,'children':0,'reworks':0,'tokens':None,'cost_microunits':None},'wait_for_child':True,'idempotency_key':'generated-delegate'})
   assert child['state']=='blocked_on_live_parent'
   if fixture_mode=='two_children':
    second=tool('delegate_task',{'goal':'Write the second fixed local child report','connection_id':status_connection,'workspace_id':status_workspace,'input_artifact_ids':['input'],'output_artifact_ids':['report_b'],'budget':{'attempts':2,'parallel':1,'runtime_ms':5000,'output_bytes':4096,'children':0,'reworks':0,'tokens':None,'cost_microunits':None},'wait_for_child':True,'idempotency_key':'generated-delegate-second'})
    assert second['task_id']!=child['task_id']
   if fixture_mode=='cancel_queued':
    cancelled=tool('cancel_task',{'task_id':child['task_id'],'expected_revision':child['revision'],'idempotency_key':'cancel-never-started-child'})
    assert cancelled['state']=='cancelled' and cancelled['confirmed_stop'] is True
   info=tool('get_task_result',{'task_id':child['task_id']});assert info['task']['result_available'] is False and info['acceptance_state']=='not_established'
  else:
   if stage=='parent-resumed':
    if fixture_mode=='two_children':
     results=c['handoff_results'];assert len(results)==2 and c['handoff_digest']
     assert len(set(x['task']['task_id']for x in results))==2
     assert len(set(x['artifacts'][0]['attempt_id']for x in results))==2
     for result in results:
      assert result['task']['task_id']!=c['task_id']
      assert tool('get_task_result',{'task_id':result['task']['task_id']})['task']['task_id']==result['task']['task_id']
      for artifact in result['artifacts']:assert hashlib.sha256(open(artifact['relative_path'],'rb').read()).hexdigest()==artifact['sha256']
     assert tool('get_task_result',{'task_id':c['task_id']})['task']['task_id']==c['task_id']
    else:assert c['handoff_result']['artifacts'] and c['handoff_result']['handoff_digest']
   if not(stage=='parent-resumed'and fixture_mode=='two_children'):
    output=c['output_artifacts'][0]['requirement']['relative_path']
    with open(output,'w')as f:f.write(stage+' '+c['output_artifacts'][0]['requirement']['artifact_id']+' report\n')
  response(r,{'stopReason':'end_turn'})
 elif method=='session/cancel':
  if mcp is not None:
   mcp.stdin.close();mcp.wait(timeout=1)
"#;

#[test]
fn actual_scoped_mcp_parent_child_new_parent_attempt_preserves_grant_root_and_owned_stop() {
    run_mode("normal");
}
#[test]
fn actual_scoped_mcp_self_cancel_only_confirms_after_owned_reap() {
    run_mode("self_cancel");
}
#[test]
fn actual_malformed_parent_never_automatically_dispatches_child_or_continuation() {
    run_mode("malformed");
}
#[test]
fn scoped_four_methods_reject_drift_of_an_unselected_original_grant_binding() {
    run_mode("pin_drift");
}
#[test]
fn two_actual_children_keep_both_hash_bound_results_in_the_new_parent_attempt() {
    run_mode("two_children");
}
#[test]
fn cancelled_queued_child_reaches_explicit_blocked_join_without_child_dispatch() {
    run_mode("cancel_queued");
}
fn run_mode(mode: &str) {
    use agentbaton_core::ipc::{call, Daemon};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::{Duration, Instant};
    let mut f = Fixture::new();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap();
    fs::remove_file(&f.companion).unwrap();
    fs::copy(target.join("debug/baton"), &f.companion).unwrap();
    fs::set_permissions(&f.companion, fs::Permissions::from_mode(0o700)).unwrap();
    // Remove debug sections from this private test-installed copy only. The
    // actual fixed program stays executable and its resulting bytes are approved.
    assert!(Command::new("strip")
        .arg("-S")
        .arg(&f.companion)
        .status()
        .unwrap()
        .success());
    let provider = f
        ._temp
        .path()
        .canonicalize()
        .unwrap()
        .join("generated-factory-acp");
    let script = ACP_FACTORY.replace(
        "mcp=None;mid=0",
        &format!(
            "status_connection={}\nstatus_workspace={}\nfixture_mode={}\nmcp=None;mid=0",
            serde_json::to_string(&f.selection.connection_ids[0]).unwrap(),
            serde_json::to_string(&f.selection.workspace_ids[0]).unwrap(),
            serde_json::to_string(mode).unwrap()
        ),
    );
    fs::write(&provider, script).unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o700)).unwrap();
    let db = Connection::open(&f.path).unwrap();
    let raw: String = db
        .query_row(
            "SELECT document FROM connections WHERE id=?1",
            [&f.selection.connection_ids[0]],
            |r| r.get(0),
        )
        .unwrap();
    let mut connection: serde_json::Value = serde_json::from_str(&raw).unwrap();
    connection["executable"] = json!(provider);
    connection["executable_sha256"] = json!(sha256(&fs::read(&provider).unwrap()));
    db.execute(
        "UPDATE connections SET document=?1 WHERE id=?2",
        params![connection.to_string(), f.selection.connection_ids[0]],
    )
    .unwrap();
    let plan = RuntimePlan::empty(RuntimeProvider::CustomAcp);
    let config=sha256(&serde_json::to_vec(&json!({"arguments":[],"protocol_route":"custom_acp","runtime_plan_digest":plan.plan_digest().unwrap(),"host_capabilities":{"fs":false,"terminal":false}})).unwrap());
    connection["config_revision"] = json!(config);
    db.execute(
        "UPDATE connections SET document=?1 WHERE id=?2",
        params![connection.to_string(), f.selection.connection_ids[0]],
    )
    .unwrap();
    db.execute(
        "UPDATE execution_grants SET document=json_set(document,?1,?2) WHERE id=?3",
        params![
            format!(
                "$.connection_bindings.{}.config_revision",
                f.selection.connection_ids[0]
            ),
            config,
            f.selection.grant_id
        ],
    )
    .unwrap();
    let hash = connection["executable_sha256"].as_str().unwrap();
    db.execute(
        "UPDATE execution_grants SET document=json_set(document,?1,?2) WHERE id=?3",
        params![
            format!(
                "$.connection_bindings.{}.executable_sha256",
                f.selection.connection_ids[0]
            ),
            hash,
            f.selection.grant_id
        ],
    )
    .unwrap();
    if mode == "pin_drift" {
        let mut other = connection.clone();
        other["id"] = json!("generated-unselected");
        other["name"] = json!("Unselected original Grant recipient");
        db.execute(
            "INSERT INTO connections(id,document) VALUES('generated-unselected',?1)",
            [other.to_string()],
        )
        .unwrap();
        let mut grant: serde_json::Value = serde_json::from_str(
            &db.query_row(
                "SELECT document FROM execution_grants WHERE id=?1",
                [&f.selection.grant_id],
                |r| r.get::<_, String>(0),
            )
            .unwrap(),
        )
        .unwrap();
        grant["scope"]["connection_ids"]
            .as_array_mut()
            .unwrap()
            .push(json!("generated-unselected"));
        grant["connection_bindings"]["generated-unselected"] =
            grant["connection_bindings"][&f.selection.connection_ids[0]].clone();
        db.execute(
            "UPDATE execution_grants SET document=?1 WHERE id=?2",
            params![grant.to_string(), f.selection.grant_id],
        )
        .unwrap();
    }
    // Fixture only: trusted connection+grant snapshots are explicit above, not a live account probe.
    if mode == "two_children" {
        f.selection.outputs = vec![
            ArtifactRequirement {
                artifact_id: "report_a".into(),
                relative_path: "reports/report-a.txt".into(),
                max_bytes: 4096,
                expected_sha256: None,
            },
            ArtifactRequirement {
                artifact_id: "report_b".into(),
                relative_path: "reports/report-b.txt".into(),
                max_bytes: 4096,
                expected_sha256: None,
            },
        ];
    }
    f.selection.root_budget.output_bytes = 12288;
    f.selection.project_budget.output_bytes = 12288;
    let scope = f.scope();
    let workspace = scope.bindings.workspaces[0].root.clone();
    let runtime = f._temp.path().canonicalize().unwrap();
    drop(db);
    drop(f.store);
    let daemon = Daemon::bind(&runtime).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let daemon_thread = std::thread::spawn(move || daemon.run(worker_stop));
    let start=call(&runtime,json!({"method":"factory_runs.start","params":{"scope_id":scope.id},"expected_revision":scope.revision,"idempotency_key":"actual-run"})).unwrap();
    assert!(start.get("error").is_none(), "{start}");
    if mode == "pin_drift" {
        let until = Instant::now() + Duration::from_secs(5);
        while !workspace.join("reports/pin-ready").exists() {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(5));
        }
        let db = Connection::open(&f.path).unwrap();
        db.execute("UPDATE connections SET document=json_set(document,'$.config_revision','unapproved-drift') WHERE id='generated-unselected'",[]).unwrap();
        let path = fs::read_dir(runtime.join("factory-scoped-handles"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .next()
            .unwrap();
        let credential: serde_json::Value =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let delegate = json!({"goal":"Must not admit after whole original Grant drift","connection_id":f.selection.connection_ids[0],"workspace_id":f.selection.workspace_ids[0],"input_artifact_ids":["input"],"output_artifact_ids":["report"],"budget":{"attempts":2,"parallel":1,"runtime_ms":5000,"output_bytes":4096,"children":0,"reworks":0,"tokens":null,"cost_microunits":null},"wait_for_child":true});
        let mut responses = Vec::new();
        for (method, params, rev, key) in [
            (
                "get_task_status",
                json!({"task_id":scope.root_task_id}),
                None,
                None,
            ),
            (
                "get_task_result",
                json!({"task_id":scope.root_task_id}),
                None,
                None,
            ),
            ("delegate_task", delegate, None, Some("denied-delegate")),
            (
                "cancel_task",
                json!({"task_id":scope.root_task_id}),
                Some(1),
                Some("denied-cancel"),
            ),
        ] {
            let response = scoped_request(
                &runtime,
                json!({"api_version":1,"request_id":"controlled-original-grant-drift","handle_id":credential["handle_id"],"scoped_token":credential["scoped_token"],"method":method,"params":params,"expected_revision":rev,"idempotency_key":key}),
            );
            eprintln!(
                "SCOPED_UNSELECTED_PIN method={method} error={:?} result_present={}",
                response["error"]["code"].as_str(),
                response.get("result").is_some()
            );
            responses.push(response);
        }
        stop.store(true, Ordering::Relaxed);
        let done = daemon_thread.join().unwrap();
        assert!(done.is_ok(), "{done:?}");
        for response in responses {
            assert_eq!(
                response["error"]["code"], "AUTH_REQUIRED",
                "stale Grant method was admitted"
            );
            assert_eq!(response["revision"], 0);
        }
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_delegations", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
        return;
    }
    let until = Instant::now() + Duration::from_secs(12);
    let final_run = loop {
        let current = call(
            &runtime,
            json!({"method":"factory_runs.get","params":{"scope_id":scope.id}}),
        );
        let value = match current {
            Ok(v) => v,
            Err(e) => {
                stop.store(true, Ordering::Relaxed);
                let done = daemon_thread.join().unwrap();
                save_runtime_diagnostics(&f.path, &workspace);
                panic!("read error={e:?}; daemon={done:?}")
            }
        };
        let state = value["result"]["run"]["state"].as_str().unwrap_or("");
        if matches!(
            state,
            "completed" | "outcome_unknown" | "blocked" | "cancelled"
        ) {
            break value;
        }
        if Instant::now() > until {
            stop.store(true, Ordering::Relaxed);
            let done = daemon_thread.join().unwrap();
            save_runtime_diagnostics(&f.path, &workspace);
            panic!("Factory deadline current={value}; daemon={done:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    stop.store(true, Ordering::Relaxed);
    let done = daemon_thread.join().unwrap();
    assert!(done.is_ok(), "daemon={done:?}");
    let mut docs = db_events(&f.path);
    docs["fixture_stderr"] =
        json!(fs::read_to_string(workspace.join("reports/fixture-stderr.txt")).ok());
    fs::write(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/factory-runtime-core/real-runtime-last-events.json"),
        serde_json::to_vec_pretty(&docs).unwrap(),
    )
    .unwrap();
    assert_eq!(
        final_run["result"]["run"]["state"],
        if mode == "normal" || mode == "two_children" {
            "completed"
        } else if mode == "cancel_queued" {
            "blocked"
        } else if mode == "self_cancel" {
            "cancelled"
        } else {
            "outcome_unknown"
        },
        "{final_run}"
    );
    if mode != "normal" && mode != "two_children" {
        assert_eq!(
            final_run["result"]["tasks"].as_array().unwrap().len(),
            if mode == "cancel_queued" { 2 } else { 1 }
        );
        if mode == "cancel_queued" {
            assert_eq!(
                final_run["result"]["run"]["last_error"]["code"],
                "CHILD_CANCELLED"
            );
        }
        let db = Connection::open(&f.path).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_attempts", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_delegations", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            u32::from(mode == "cancel_queued")
        );
        return;
    }
    assert_eq!(
        fs::read_to_string(workspace.join("reports/mcp-stages.txt")).unwrap(),
        if mode == "two_children" {
            "parent-initial\nchild\nchild\nparent-resumed\n"
        } else {
            "parent-initial\nchild\nparent-resumed\n"
        }
    );
    let db = Connection::open(&f.path).unwrap();
    assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='factory.internal_stop_prepared'",[],|r|r.get::<_,u32>(0)).unwrap(),if mode=="two_children"{4}else{3});
    assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.payload.process_reaped')=1 AND json_extract(document,'$.payload.no_process_started')=0",[],|r|r.get::<_,u32>(0)).unwrap(),if mode=="two_children"{4}else{3});
    assert_eq!(
        db.query_row(
            "SELECT sum(consumed) FROM budgets WHERE scope LIKE 'root:%'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        if mode == "two_children" { 4 } else { 3 }
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM budgets WHERE scope LIKE 'root:%'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM factory_attempts", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        if mode == "two_children" { 4 } else { 3 }
    );
    assert_eq!(
        db.query_row(
            "SELECT count(DISTINCT root_task_id) FROM attempts",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM dispatch_intents WHERE state!='stopped'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
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
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM factory_scoped_handles WHERE revoked=0",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM execution_grants", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM tasks WHERE json_extract(document,'$.state')='accepted'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
}

fn db_events(path: &Path) -> serde_json::Value {
    let db = Connection::open(path).unwrap();
    let mut q = db
        .prepare("SELECT document FROM events ORDER BY sequence")
        .unwrap();
    let rows = q
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    json!({"events":rows.iter().map(|s|serde_json::from_str::<serde_json::Value>(s).unwrap()).collect::<Vec<_>>()})
}

fn save_runtime_diagnostics(path: &Path, workspace: &Path) {
    let mut v = db_events(path);
    v["fixture_stderr"] =
        json!(fs::read_to_string(workspace.join("reports/fixture-stderr.txt")).ok());
    let bytes = serde_json::to_vec_pretty(&v).unwrap();
    let sha = sha256(&bytes);
    fs::write(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../artifacts/factory-runtime-core/runtime-diagnostics-{sha}.json"
        )),
        bytes,
    )
    .unwrap();
}

fn scoped_request(runtime: &Path, request: serde_json::Value) -> serde_json::Value {
    use std::io::{BufRead, BufReader, Write};
    let mut socket = std::os::unix::net::UnixStream::connect(runtime.join("batond.sock")).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let bytes = serde_json::to_vec(&request).unwrap();
    socket.write_all(&bytes).unwrap();
    socket.write_all(b"\n").unwrap();
    let mut line = String::new();
    BufReader::new(socket).read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}

/// Full public Owner IPC for a generated Directory dataset. Owner finalization
/// below is an explicit trusted-host fixture, never a Renderer/Agent authority.
#[test]
fn actual_data_copy_owner_scope_and_scoped_mcp_parent_child_parent_use_fresh_private_witnesses() {
    run_data_mode("normal");
}
#[test]
fn actual_data_scoped_four_methods_fail_closed_after_input_drift_but_owner_stop_still_joins() {
    run_data_mode("input_drift");
}
fn run_data_mode(mode: &str) {
    use agentbaton_core::ipc::call;
    use serde_json::Value;
    use std::time::{Duration, Instant};
    struct OwnedDaemon(std::process::Child);
    impl Drop for OwnedDaemon {
        fn drop(&mut self) {
            // Only this original Child is controlled; no PID scanning.
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let temp = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let base = temp.path().canonicalize().unwrap();
    let runtime = base.join("runtime");
    let source = base.join("source");
    let installed = base.join("installed");
    for path in [&runtime, &source, &installed] {
        fs::create_dir(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::write(source.join("input.txt"), b"fixed input\n").unwrap();
    let before = fs::read(source.join("input.txt")).unwrap();
    let target = PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap());
    for name in ["batond", "baton"] {
        let output = installed.join(name);
        fs::copy(target.join("debug").join(name), &output).unwrap();
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(Command::new("strip")
            .args(["-S"])
            .arg(&output)
            .status()
            .unwrap()
            .success());
    }
    let mut daemon = OwnedDaemon(
        Command::new(installed.join("batond"))
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("BATON_RUNTIME_DIR", &runtime)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let ready = Instant::now() + Duration::from_secs(5);
    while !runtime.join("batond.sock").exists() {
        assert!(
            Instant::now() < ready,
            "generated installed daemon did not bind"
        );
        assert!(daemon.0.try_wait().unwrap().is_none());
        std::thread::sleep(Duration::from_millis(5));
    }
    let rpc = |method: &str, params: Value, key: Option<&str>, rev: Option<u64>| -> Value {
        let mut request = json!({"method":method,"params":params});
        if let Some(key) = key {
            request["idempotency_key"] = json!(key);
        }
        if let Some(rev) = rev {
            request["expected_revision"] = json!(rev);
        }
        let response = call(&runtime, request).unwrap();
        assert!(response.get("error").is_none(), "{method}: {response}");
        response["result"].clone()
    };
    let approve = |p: &Value, key: &str| {
        rpc(
            "approvals.finalize",
            json!({"proposal_id":p["id"]}),
            Some(key),
            p["revision"].as_u64(),
        )
    };
    let wait = |method: &str, field: &str, id: &str, active: &[&str]| -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let v = rpc(method, json!({field:id}), None, None);
            if !active.contains(&v["state"].as_str().unwrap()) {
                return v;
            }
            assert!(Instant::now() < deadline, "{method}: {v}");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    let project = rpc(
        "projects.import",
        json!({"path":source,"name":"Generated Directory Factory"}),
        Some("data-project"),
        None,
    );
    let preview = rpc(
        "data_workspace_previews.start",
        json!({"project_id":project["id"],"files":[{"artifact_id":"input","relative_path":"input.txt","max_bytes":1024,"expected_sha256":sha256(&before)}],"limits":{"max_file_bytes":1024,"max_total_bytes":1024,"max_files":1}}),
        Some("data-preview"),
        project["revision"].as_u64(),
    );
    let preview = wait(
        "data_workspace_previews.get",
        "preview_id",
        preview["id"].as_str().unwrap(),
        &["checking", "cancel_requested"],
    );
    assert_eq!(preview["state"], "verified");
    let copy_proposal = rpc(
        "data_workspaces.propose",
        json!({"preview_id":preview["id"]}),
        Some("data-copy-proposal"),
        preview["revision"].as_u64(),
    );
    assert_eq!(copy_proposal["kind"], "data_workspace");
    let copying = approve(&copy_proposal, "trusted-data-copy-owner");
    let copied = wait(
        "data_workspaces.get",
        "creation_id",
        copying["id"].as_str().unwrap(),
        &["creating", "cancel_requested"],
    );
    assert_eq!(copied["state"], "ready");
    let workspace = PathBuf::from(copied["destination"].as_str().unwrap());
    let provider = base.join("generated-data-factory-acp");
    // Connection/workspace IDs are filled only once returned by Core; provider
    // code reads the approved task prompt for workspace and managed MCP config.
    let body = ACP_FACTORY
        .replace("status_connection", "data_connection")
        .replace("status_workspace", "data_workspace");
    // Fixture-only, nonsecret host-generated identifier; it grants no rights.
    let connection_label = base.join("generated-connection-label.txt");
    fs::write(&provider, body.replace("mcp=None;mid=0", &format!("data_connection=open({0}).read() if os.path.exists({0}) else ''\ndata_workspace={1}\nfixture_mode={2}\nmcp=None;mid=0",serde_json::to_string(&connection_label).unwrap(),copied["workspace_id"],serde_json::to_string(if mode=="input_drift"{"pin_drift"}else{"normal"}).unwrap()))).unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o700)).unwrap();
    let p = rpc(
        "connections.propose",
        json!({"name":"Generated data Factory ACP","provider":"custom_acp","executable":provider,"arguments":[]}),
        Some("data-connection-proposal"),
        None,
    );
    let connection = approve(&p, "trusted-data-connection-owner");
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let all = rpc("connections.list", json!({}), None, None);
        let current = all
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == connection["id"])
            .unwrap();
        if current["probe_state"] == "ready" {
            break;
        }
        assert!(Instant::now() < until, "{current}");
        std::thread::sleep(Duration::from_millis(5));
    }
    fs::write(&connection_label, connection["id"].as_str().unwrap()).unwrap();
    let task = rpc(
        "tasks.create",
        json!({"project_id":project["id"],"goal":"Use the explicitly copied data for bounded generated delegation"}),
        Some("data-root"),
        None,
    );
    let p = rpc(
        "grants.propose",
        json!({"project_id":project["id"],"workspace_id":copied["workspace_id"],"connection_id":connection["id"],"max_attempts":8,"max_concurrency":1,"max_duration_ms":30000}),
        Some("data-grant-proposal"),
        None,
    );
    let grant = approve(&p, "trusted-data-grant-owner");
    let budget = json!({"attempts":8,"parallel":1,"runtime_ms":30000,"output_bytes":12288,"children":3,"reworks":0,"tokens":null,"cost_microunits":null});
    let selection = json!({"root_task_id":task["id"],"expected_task_revision":task["revision"],"grant_id":grant["id"],"expected_grant_revision":grant["revision"],"connection_ids":[connection["id"]],"workspace_ids":[copied["workspace_id"]],"inputs":[{"artifact_id":"input","relative_path":"inputs/input.txt","max_bytes":1024,"expected_sha256":sha256(&before)}],"outputs":[{"artifact_id":"report","relative_path":"reports/report.txt","max_bytes":4096,"expected_sha256":null}],"recipients":[],"max_depth":2,"max_children":2,"max_total_nodes":4,"root_budget":budget,"project_budget":budget,"ttl_ms":30000,"capabilities":{"delegate_task":true,"resume_parent_as_new_attempt":true,"child_planning":false,"child_goal_execution":false,"create_workspace":false}});
    let proposal = rpc(
        "factory_scopes.propose",
        selection,
        Some("data-factory-proposal"),
        None,
    );
    assert_eq!(proposal["kind"], "factory");
    let scope = approve(&proposal, "trusted-factory-owner");
    let check_db = Connection::open(runtime.join("baton.sqlite3")).unwrap();
    let sequence: i64 = check_db
        .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| {
            r.get(0)
        })
        .unwrap();
    for method in ["factory_runs.dispatch", "private_factory_dispatch"] {
        let denied=call(&runtime,json!({"method":method,"params":{"scope_id":scope["id"],"run_id":"renderer-cannot-mint-ticket","nonce":"self-reported"},"idempotency_key":"forged-private-ticket"})).unwrap();
        assert!(denied.get("error").is_some(), "{denied}");
        assert_eq!(denied["error"]["side_effect_state"], "not_started");
    }
    assert_eq!(
        check_db
            .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        sequence
    );
    assert_eq!(
        check_db
            .query_row("SELECT count(*) FROM factory_attempts", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        check_db
            .query_row(
                "SELECT count(*) FROM factory_budget_reservations",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
    let start = rpc(
        "factory_runs.start",
        json!({"scope_id":scope["id"]}),
        Some("data-factory-run"),
        scope["revision"].as_u64(),
    );
    assert_eq!(start["state"], "queued");
    if mode == "input_drift" {
        let until = Instant::now() + Duration::from_secs(5);
        while !workspace.join("reports/pin-ready").exists() {
            assert!(
                Instant::now() < until,
                "Data scoped fixture did not reach its actual prompt"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let db = Connection::open(runtime.join("baton.sqlite3")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_attempts", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            1
        );
        let path = fs::read_dir(runtime.join("factory-scoped-handles"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let credential: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        fs::write(
            workspace.join("inputs/input.txt"),
            b"changed data after actual session start\n",
        )
        .unwrap();
        let delegate = json!({"goal":"Must not consume stale dataset","connection_id":connection["id"],"workspace_id":copied["workspace_id"],"input_artifact_ids":["input"],"output_artifact_ids":["report"],"budget":{"attempts":2,"parallel":1,"runtime_ms":5000,"output_bytes":4096,"children":0,"reworks":0,"tokens":null,"cost_microunits":null},"wait_for_child":true});
        for (method, params, rev, key) in [
            ("get_task_status", json!({"task_id":task["id"]}), None, None),
            ("get_task_result", json!({"task_id":task["id"]}), None, None),
            ("delegate_task", delegate, None, Some("data-drift-delegate")),
            (
                "cancel_task",
                json!({"task_id":task["id"]}),
                Some(1),
                Some("data-drift-agent-cancel"),
            ),
        ] {
            let response = scoped_request(
                &runtime,
                json!({"api_version":1,"request_id":"generated-data-drift","handle_id":credential["handle_id"],"scoped_token":credential["scoped_token"],"method":method,"params":params,"expected_revision":rev,"idempotency_key":key}),
            );
            assert!(
                matches!(
                    response["error"]["code"].as_str(),
                    Some("INPUT_CHANGED" | "AUTH_REQUIRED")
                ),
                "{method}: {response}"
            );
            assert_eq!(response["revision"], 0);
            assert!(response.get("result").is_none());
        }
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_delegations", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM factory_budget_reservations",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            1
        );
        let observed = rpc(
            "factory_runs.get",
            json!({"scope_id":scope["id"]}),
            None,
            None,
        );
        let requested_at = Instant::now();
        let cancelled = rpc(
            "factory_runs.cancel",
            json!({"scope_id":scope["id"]}),
            Some("owner-stop-after-data-drift"),
            observed["run"]["revision"].as_u64(),
        );
        assert!(requested_at.elapsed() < Duration::from_secs(3));
        assert_eq!(cancelled["cancel_requested"], true);
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let current = rpc(
                "factory_runs.get",
                json!({"scope_id":scope["id"]}),
                None,
                None,
            );
            if current["run"]["state"] == "cancelled" {
                assert!(current["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|t| t["confirmed_stop"] == true));
                break;
            }
            assert!(Instant::now() < until, "{current}");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_attempts", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM workspace_leases WHERE holder<>''",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(fs::read(source.join("input.txt")).unwrap(), before);
        return;
    }
    let deadline = Instant::now() + Duration::from_secs(12);
    let final_run = loop {
        let v = rpc(
            "factory_runs.get",
            json!({"scope_id":scope["id"]}),
            None,
            None,
        );
        if matches!(
            v["run"]["state"].as_str(),
            Some("completed" | "blocked" | "outcome_unknown" | "cancelled")
        ) {
            break v;
        }
        assert!(Instant::now() < deadline, "{v}");
        std::thread::sleep(Duration::from_millis(10));
    };
    save_runtime_diagnostics(&runtime.join("baton.sqlite3"), &workspace);
    assert_eq!(final_run["run"]["state"], "completed", "{final_run}");
    assert_eq!(final_run["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(
        fs::read_to_string(workspace.join("reports/mcp-stages.txt")).unwrap(),
        "parent-initial\nchild\nparent-resumed\n"
    );
    assert_eq!(fs::read(source.join("input.txt")).unwrap(), before);
    assert!(!source.join(".git").exists());
    let db = Connection::open(runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM factory_attempts", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM factory_scoped_handles WHERE revoked=0",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        rpc("tasks.get", json!({"task_id":task["id"]}), None, None)["state"],
        "outcome_unknown"
    );
}
