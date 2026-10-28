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
