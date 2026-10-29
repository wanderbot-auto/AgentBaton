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
#[cfg(target_os = "linux")]
#[path = "fixtures/factory_large_data_gate.rs"]
mod large_data_gate;
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
        fs::create_dir_all(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/factory-runtime-core"),
        )
        .unwrap();
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
            child_planning_policy: None,
            child_goal_execution_policy: None,
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
def denied_tool(name,args,code):
 result=mcp_call('tools/call',{'name':name,'arguments':args})
 actual=json.loads(result['content'][0]['text'])
 assert actual['error']['code']==code,actual
 print('BOUNDED_SCOPED_DENIAL '+name+' '+code,file=sys.stderr,flush=True)
def bounded_child_request(key):
 return {'goal':'Generated bounded negative request','connection_id':status_connection,'workspace_id':status_workspace,'input_artifact_ids':['input'],'output_artifact_ids':['report'],'budget':{'attempts':1,'parallel':1,'runtime_ms':1000,'output_bytes':4096,'children':0,'reworks':0,'tokens':None,'cost_microunits':None},'wait_for_child':True,'idempotency_key':key}
mcp=None;mid=0
for line in sys.stdin:
 r=json.loads(line);method=r.get('method')
 if method=='initialize':
  assert not any(k in os.environ for k in ('GH_TOKEN','GITHUB_TOKEN','OPENAI_API_KEY','LD_PRELOAD'))
  response(r,{'protocolVersion':1,'agentCapabilities':{}})
 elif method=='session/new':
  servers=r['params']['mcpServers']
  if not servers:
   response(r,{'sessionId':'generated-native'});continue
  assert len(servers)==1
  server=servers[0];assert server['name']=='agentbaton-factory' and server['env']==[]
  mcp=subprocess.Popen([server['command']]+server['args'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
  mcp_call('initialize',{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'generated-local-fixture','version':'1'}})
  names=[v['name'] for v in mcp_call('tools/list',{})['tools']]
  assert names==['delegate_task','get_task_status','get_task_result','cancel_task']
  response(r,{'sessionId':'generated-native'})
 elif method=='session/prompt':
  if mcp is None:
   text=r['params']['prompt'][0]['text']
   emit({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'generated-native','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'generated managed non-Factory turn'}}}})
   if text=='generated-hold-ordinary':continue
   if text.startswith('{'):
    goal=json.loads(text);assert goal['kind']=='agentbaton_goal_task_request'
    assert goal['outputs'][0]['artifact']['relative_path']=='reports/goal-report.txt'
    with open('reports/goal-report.txt','w')as f:f.write('fixed generated Goal report\n')
   response(r,{'stopReason':'end_turn'});continue
  c=json.loads(r['params']['prompt'][0]['text']);assert c['kind']=='agentbaton_factory_task_request'
  status=tool('get_task_status',{'task_id':c['task_id']});assert status['task_id']==c['task_id']
  mid+=1;mcp.stdin.write(json.dumps({'jsonrpc':'2.0','id':mid,'method':'tools/call','params':{'name':'approvals.finalize','arguments':{'proposal_id':'forbidden'}}})+'\n');mcp.stdin.flush()
  denied=json.loads(mcp.stdout.readline());assert 'error' in denied
  if fixture_mode=='self_cancel':
   stopped=tool('cancel_task',{'task_id':c['task_id'],'expected_revision':status['revision'],'idempotency_key':'self-cancel'})
   assert stopped['cancel_requested'] is True
   continue
  if fixture_mode in ('pin_drift','scope_revoke','scope_ttl'):
   open('reports/pin-ready','w').write('generated safe gate')
   import time
   while not os.path.exists('reports/proceed'):time.sleep(0.005)
   continue
  if fixture_mode=='malformed':print('generated malformed protocol',flush=True);continue
  os.makedirs('reports',exist_ok=True)
  stage='child' if c['parent_task_id'] else ('parent-resumed' if c['handoff_result'] or c.get('handoff_results') else 'parent-initial')
  with open('reports/mcp-stages.txt','a')as f:f.write(stage+'\n')
  if stage=='parent-initial':
   if fixture_mode=='scope_bounds':
    forged=bounded_child_request('forged-parent-cycle')
    forged['parent_task_id']=c['task_id'];forged['root_task_id']=c['task_id']
    denied_tool('delegate_task',forged,'INVALID_REQUEST')
   child=tool('delegate_task',{'goal':'Write the fixed local child report','connection_id':status_connection,'workspace_id':status_workspace,'input_artifact_ids':['input'],'output_artifact_ids':['report_a' if fixture_mode=='two_children' else 'report'],'budget':{'attempts':2,'parallel':1,'runtime_ms':5000,'output_bytes':4096,'children':(1 if fixture_mode=='scope_bounds' else 0),'reworks':0,'tokens':None,'cost_microunits':None},'wait_for_child':True,'idempotency_key':'generated-delegate'})
   assert child['state']=='blocked_on_live_parent'
   if fixture_mode=='scope_bounds':denied_tool('delegate_task',bounded_child_request('denied-fanout'),'RESOURCE_LIMIT')
   if fixture_mode=='two_children':
    second=tool('delegate_task',{'goal':'Write the second fixed local child report','connection_id':status_connection,'workspace_id':status_workspace,'input_artifact_ids':['input'],'output_artifact_ids':['report_b'],'budget':{'attempts':2,'parallel':1,'runtime_ms':5000,'output_bytes':4096,'children':0,'reworks':0,'tokens':None,'cost_microunits':None},'wait_for_child':True,'idempotency_key':'generated-delegate-second'})
    assert second['task_id']!=child['task_id']
   if fixture_mode=='cancel_queued':
    cancelled=tool('cancel_task',{'task_id':child['task_id'],'expected_revision':child['revision'],'idempotency_key':'cancel-never-started-child'})
    assert cancelled['state']=='cancelled' and cancelled['confirmed_stop'] is True
   info=tool('get_task_result',{'task_id':child['task_id']});assert info['task']['result_available'] is False and info['acceptance_state']=='not_established'
  else:
   if fixture_mode=='scope_bounds' and stage=='child':
    denied_tool('delegate_task',bounded_child_request('denied-depth'),'RESOURCE_LIMIT')
    denied_tool('get_task_status',{'task_id':c['parent_task_id']},'AUTH_REQUIRED')
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

const PLANNING_CHILD_PAUSE: &str = r#"   import time
   open('reports/planning-child-paused','w').write('actual live child before its plan revision')
   until=time.monotonic()+2
   while not os.path.exists('reports/planning-child-continue'):
    assert time.monotonic()<until,'test coordinator did not finish exact-history negative controls'
    time.sleep(.005)
"#;
fn child_planning_fixture(mut script: String, pause: bool) -> String {
    script=script.replace("names==['delegate_task','get_task_status','get_task_result','cancel_task']","names==['delegate_task','get_task_status','get_task_result','cancel_task','submit_child_plan']");
    script = script.replace(
        "mcp=None;mid=0",
        &format!(
            "{}\nmcp=None;mid=0",
            include_str!("fixtures/factory_child_planning_acp.py")
        ),
    );
    let before = r#"  if stage=='parent-initial':
   assert c['child_planning_context']['goal']==status['child_planning_context']['goal']
   ctx=c['child_planning_context'];one,args1=submit_plan(ctx,1,'actual-root-plan-one');two,args2=submit_plan(ctx,2,'actual-root-plan-two')
   import copy
   bad=copy.deepcopy(args2);bad['plan_json']+=' ';denied_tool('submit_child_plan',bad,'CONFIG_CONFLICT')
   bad=copy.deepcopy(args2);bad['idempotency_key']='stale-plan-key';denied_tool('submit_child_plan',bad,'STALE_REVISION')
   for label,mutate in [('input',lambda p:(p['goal'].__setitem__('input_revision','coherent-foreign'),p['nodes'][0]['executor']['binding'].__setitem__('input_revision','coherent-foreign'))),('spec',lambda p:p['goal'].__setitem__('spec_revision',p['goal']['spec_revision']+1)),('inputs',lambda p:p['nodes'][0].__setitem__('required_inputs',[]))]:
    p=child_plan(ctx,3);mutate(p);denied_tool('submit_child_plan',{'task_id':c['task_id'],'plan_json':json.dumps(p),'expected_plan_revision':2,'idempotency_key':'coherent-'+label},'INPUT_CHANGED')
   p=child_plan(ctx,3);p['nodes'][0]['budget']['attempts']+=1;denied_tool('submit_child_plan',{'task_id':c['task_id'],'plan_json':json.dumps(p),'expected_plan_revision':2,'idempotency_key':'larger-budget'},'AUTH_REQUIRED')
   p=child_plan(ctx,3);p['acceptance_rules']=[];denied_tool('submit_child_plan',{'task_id':c['task_id'],'plan_json':json.dumps(p),'expected_plan_revision':2,'idempotency_key':'drop-acceptance'},'AUTH_REQUIRED')
   open('reports/actual-root-plans.json','w').write(json.dumps({'initial':one,'revised':two,'context':ctx},indent=2))
  if stage=='child':
   ctx=c['child_planning_context'];assert ctx['current_plan_revision']==1
__CHILD_PLAN_PAUSE__
   revised,_=submit_plan(ctx,2,'actual-child-self-revision-two')
   open('reports/actual-child-plan.json','w').write(json.dumps({'view':revised,'context':ctx},indent=2))
  if stage=='parent-resumed':
   assert c['handoff_result']['child_plan']['revision']==2
  if stage=='parent-initial':"#;
    let before = before.replace(
        "__CHILD_PLAN_PAUSE__\n",
        if pause { PLANNING_CHILD_PAUSE } else { "" },
    );
    script = script.replace("  if stage=='parent-initial':", &before);
    let after = r#"   assert child['state']=='blocked_on_live_parent'
   childctx=tool('get_task_status',{'task_id':child['task_id']})['child_planning_context']
   initial,_=submit_plan(childctx,1,'actual-parent-plans-child-one')
   assert initial['caller_task_id']==c['task_id'] and initial['reference']['task_id']==child['task_id']"#;
    script = script.replace("   assert child['state']=='blocked_on_live_parent'", after);
    script
}

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
#[test]
fn mixed_ordinary_goal_and_actual_factory_share_cumulative_project_and_root_grant_meters() {
    run_mode("mixed_sequence");
}
#[test]
fn mixed_live_ordinary_owned_process_blocks_factory_dispatch_without_refunding_shared_budget() {
    run_mode("mixed_busy");
}
#[test]
fn real_scoped_handle_revocation_denies_all_four_tools_while_owner_cancel_still_joins() {
    run_mode("scope_revoke");
}
#[test]
fn real_scoped_handle_expiry_denies_all_four_tools_without_replaying_or_faking_stop() {
    run_mode("scope_ttl");
}
#[test]
fn mixed_narrow_factory_root_uses_original_ledger_without_refunding_prior_work() {
    run_mode("mixed_narrow");
}
#[test]
fn actual_scoped_mcp_depth_fanout_and_forged_parent_are_bounded_without_grant_derivation() {
    run_mode("scope_bounds");
}
#[test]
fn actual_child_planning_scoped_mcp_revisions_parent_descendant_and_handoff_are_metadata_only() {
    run_mode("planning");
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
    let companion_source =
        if mode == "planning" {
            PathBuf::from(std::env::var_os("AGENTBATON_CHILD_PLAN_COMPANION").expect(
                "Use the explicitly frozen matching schema14 CLI; never borrow old shared13",
            ))
        } else {
            target.join("debug/baton")
        };
    fs::copy(&companion_source, &f.companion).unwrap();
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
    let mut script = ACP_FACTORY.replace(
        "mcp=None;mid=0",
        &format!(
            "status_connection={}\nstatus_workspace={}\nfixture_mode={}\nmcp=None;mid=0",
            serde_json::to_string(&f.selection.connection_ids[0]).unwrap(),
            serde_json::to_string(&f.selection.workspace_ids[0]).unwrap(),
            serde_json::to_string(mode).unwrap()
        ),
    );
    if mode == "planning" {
        script = child_planning_fixture(script, true);
    }
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
    if mode == "planning" {
        f.selection.capabilities.child_planning = true;
        f.selection.child_planning_policy = Some(FactoryChildPlanningPolicy {
            max_nodes: 8,
            max_revisions: 16,
            max_plan_bytes: 262144,
            outputs: f
                .selection
                .outputs
                .iter()
                .cloned()
                .map(
                    |artifact| agentbaton_core::goal_store::GoalOutputSelection {
                        artifact,
                        validators: vec![],
                    },
                )
                .collect(),
            writable_prefixes: vec!["reports/report.txt".into()],
            owner_acceptance_criteria:
                "Review the entire exact approved report; storing a plan is not acceptance".into(),
        });
    }
    f.selection.root_budget.output_bytes = 12288;
    f.selection.project_budget.output_bytes = 12288;
    if mode == "scope_ttl" {
        f.selection.ttl_ms = 5000;
    }
    if mode == "scope_bounds" {
        f.selection.max_depth = 1;
        f.selection.max_children = 1;
    }

    let mixed = if mode.starts_with("mixed_") {
        Some(prepare_mixed_goal(&mut f))
    } else {
        None
    };
    // The existing Goal ledger was explicitly approved at30000 above. Only
    // this new Factory root ceiling narrows; project and historical ledger stay.
    if mode == "mixed_narrow" {
        f.selection.root_budget.runtime_ms = 10000;
    }
    let scope = f.scope();
    let workspace = scope.bindings.workspaces[0].root.clone();
    let runtime = f._temp.path().canonicalize().unwrap();
    drop(db);
    drop(f.store);
    let daemon = Daemon::bind(&runtime).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let daemon_thread = std::thread::spawn(move || daemon.run(worker_stop));
    if let Some((goal, ordinary)) = &mixed {
        mixed_ordinary_then_goal(
            &f.path,
            &runtime,
            &scope,
            goal,
            ordinary,
            mode == "mixed_busy",
        );
    }
    let start=call(&runtime,json!({"method":"factory_runs.start","params":{"scope_id":scope.id},"expected_revision":scope.revision,"idempotency_key":"actual-run"})).unwrap();
    assert!(start.get("error").is_none(), "{start}");
    if mode == "scope_revoke" || mode == "scope_ttl" {
        let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scope_liveness_tools_and_owner_stop(&f.path, &runtime, &scope, &workspace, mode)
        }));
        stop.store(true, Ordering::Relaxed);
        assert!(daemon_thread.join().unwrap().is_ok());
        if let Err(panic) = checked {
            std::panic::resume_unwind(panic);
        }
        return;
    }
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
    let mut planning_history_checked = false;
    let final_run = loop {
        if mode == "planning"
            && !planning_history_checked
            && workspace.join("reports/planning-child-paused").exists()
        {
            let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                save_actual_planning_handoff_snapshot(&f.path, &scope)
            }));
            fs::write(
                workspace.join("reports/planning-child-continue"),
                b"checked",
            )
            .unwrap();
            planning_history_checked = true;
            if let Err(panic) = checked {
                stop.store(true, Ordering::Relaxed);
                let done = daemon_thread.join().unwrap();
                save_runtime_diagnostics(&f.path, &workspace);
                assert!(done.is_ok(), "snapshot-failure cleanup: {done:?}");
                std::panic::resume_unwind(panic);
            }
        }
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
        if (mode == "normal" || mode == "planning")
            || mode == "two_children"
            || mode == "scope_bounds"
            || mode.starts_with("mixed_")
        {
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
    if !((mode == "normal" || mode == "planning")
        || mode == "two_children"
        || mode == "scope_bounds"
        || mode.starts_with("mixed_"))
    {
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
    assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.payload.process_reaped')=1 AND json_extract(document,'$.payload.no_process_started')=0",[],|r|r.get::<_,u32>(0)).unwrap(),if mode.starts_with("mixed_"){5}else if mode=="two_children"{4}else{3});
    assert_eq!(
        db.query_row(
            "SELECT sum(consumed) FROM budgets WHERE scope LIKE 'root:%'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        if mode.starts_with("mixed_") {
            5
        } else if mode == "two_children" {
            4
        } else {
            3
        }
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM budgets WHERE scope LIKE 'root:%'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        if mode.starts_with("mixed_") { 3 } else { 1 }
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
        if mode.starts_with("mixed_") { 3 } else { 1 }
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
        if mode.starts_with("mixed_") { 2 } else { 1 }
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
    if mode == "scope_bounds" {
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_task_lineage", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row("SELECT max(depth) FROM factory_task_lineage", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_delegations", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM execution_grants WHERE parent_id IS NOT NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        let errors = fs::read_to_string(workspace.join("reports/fixture-stderr.txt")).unwrap();
        for expected in [
            "BOUNDED_SCOPED_DENIAL delegate_task INVALID_REQUEST",
            "BOUNDED_SCOPED_DENIAL delegate_task RESOURCE_LIMIT",
            "BOUNDED_SCOPED_DENIAL get_task_status AUTH_REQUIRED",
        ] {
            assert!(
                errors.contains(expected),
                "missing actual MCP denial: {errors}"
            );
        }
        assert_eq!(
            errors
                .matches("BOUNDED_SCOPED_DENIAL delegate_task RESOURCE_LIMIT")
                .count(),
            2
        );
        assert_eq!(db.query_row("SELECT count(*) FROM idempotency WHERE key IN ('forged-parent-cycle','denied-fanout','denied-depth')",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    if mode == "planning" {
        assert!(planning_history_checked);
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_child_plan_heads", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
            2
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM factory_child_plan_revisions",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            4
        );
        for table in ["goal_runs", "goal_graph_jobs", "goal_graph_acceptances"] {
            assert_eq!(
                db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        let grant: serde_json::Value = serde_json::from_str(
            &db.query_row(
                "SELECT document FROM execution_grants WHERE id=?1",
                [&scope.root_grant_id],
                |r| r.get::<_, String>(0),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(grant["scope"]["max_children"], 0);
        let mut q = db
            .prepare("SELECT document FROM factory_child_plan_revisions ORDER BY receipt_sequence")
            .unwrap();
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| serde_json::from_str::<serde_json::Value>(&r.unwrap()).unwrap())
            .collect::<Vec<_>>();
        assert!(rows.iter().all(|v| v["caller_session_id"].is_string()
            && v["caller_attempt_id"].is_string()
            && v["planning_only"] == true
            && v["dispatched"] == false
            && v["task_accepted"] == false));
        if let Some(output) = std::env::var_os("AGENTBATON_CHILD_PLAN_IPC_OUT") {
            let data = json!({"dimension":"actual local generated ACP->real Baton companion->scoped UnixIPC->schema14 SQLite; no real Provider/native dialog","source_hashes":{"factory_runtime_ipc.rs":sha256(include_bytes!("factory_runtime_ipc.rs")),"factory_child_planning_acp.py":sha256(include_bytes!("fixtures/factory_child_planning_acp.py"))},"scope":scope,"final_run":final_run,"grant":grant,"rows":rows,"root_provider_observation":serde_json::from_str::<serde_json::Value>(&fs::read_to_string(workspace.join("reports/actual-root-plans.json")).unwrap()).unwrap(),"child_provider_observation":serde_json::from_str::<serde_json::Value>(&fs::read_to_string(workspace.join("reports/actual-child-plan.json")).unwrap()).unwrap(),"journal":db_events(&f.path)});
            fs::write(output, serde_json::to_vec_pretty(&data).unwrap()).unwrap();
        }
    }
    if mode.starts_with("mixed_") {
        assert_mixed_cumulative_receipts(&db, &scope);
    }
}

// Explicit trusted-host Goal approval fixture. It changes only the Grant
// ceilings that the native Goal snapshot actually reviews, then Factory gets a
// separate approval of that resulting exact Grant. This is not an old Goal
// scope being upgraded to Factory authority.
fn prepare_mixed_goal(
    f: &mut Fixture,
) -> (
    agentbaton_core::goal_store::GoalScopeView,
    agentbaton_core::storage::Task,
) {
    use agentbaton_core::{goal_plan::*, goal_store::*};
    let project = f.store.get(&f.selection.root_task_id).unwrap().project_id;
    // Goal requires an actual native-confirmed Grant origin. The unused base
    // synthetic Grant stays unchanged; no confirmation journal is forged here.
    let grant_proposal = f
        .store
        .propose_grant(
            &project,
            &f.selection.workspace_ids[0],
            &f.selection.connection_ids[0],
            8,
            1,
            30_000,
            "mixed-native-grant",
        )
        .unwrap();
    let grant = f
        .store
        .finalize_proposal(&grant_proposal.id, grant_proposal.revision)
        .unwrap();
    f.selection.grant_id = grant["id"].as_str().unwrap().into();
    f.selection.expected_grant_revision = grant["revision"].as_u64().unwrap();

    let task = f
        .store
        .create_registered(
            &project,
            "Produce the fixed mixed-ledger Goal artifact",
            "mixed-goal-task",
        )
        .unwrap();
    let ordinary = f
        .store
        .create_registered(
            &project,
            "Plain managed conversation has no declared artifact contract",
            "mixed-ordinary-task",
        )
        .unwrap();
    let selection = GoalScopeSelection {
        task_id: task.id.clone(),
        expected_task_revision: task.revision,
        grant_id: f.selection.grant_id.clone(),
        expected_grant_revision: f.selection.expected_grant_revision,
        max_nodes: 4,
        new_max_children: 3,
        new_max_depth: 1,
        root_budget: f.selection.root_budget.clone(),
        project_budget: f.selection.project_budget.clone(),
        inputs: f.selection.inputs.clone(),
        outputs: vec![GoalOutputSelection {
            artifact: ArtifactRequirement {
                artifact_id: "goal-report".into(),
                relative_path: "reports/goal-report.txt".into(),
                max_bytes: 1024,
                expected_sha256: None,
            },
            validators: vec![],
        }],
        writable_prefixes: vec!["reports".into()],
        owner_acceptance_criteria: "An observed Goal candidate is not accepted by this budget test"
            .into(),
    };
    let proposal = f
        .store
        .propose_goal_scope(&selection, "mixed-goal-scope")
        .unwrap();
    let scope: GoalScopeView = serde_json::from_value(
        f.store
            .finalize_proposal(&proposal.id, proposal.revision)
            .unwrap(),
    )
    .unwrap();
    f.selection.expected_grant_revision =
        f.store.get_grant(&f.selection.grant_id).unwrap().revision;
    assert_eq!(scope.root_grant_id, f.selection.grant_id);
    let output = scope.outputs["goal-report"].clone();
    let node = PlanNode {
        id: "mixed-goal-report".into(),
        parent_id: None,
        kind: NodeKind::Work,
        goal: "Write the generated Goal report".into(),
        dependencies: vec![],
        workspace_id: scope.workspace_id.clone(),
        executor: Executor::Connection {
            binding: Box::new(scope.binding.clone()),
            program_sha256: scope.program_sha256.clone(),
        },
        recipients: vec![],
        required_inputs: vec![RequiredInput {
            goal_task_id: task.id,
            artifact_id: "input".into(),
            contract_id: scope.inputs["input"].contract_id.clone(),
            source: InputSource::ApprovedExternal {
                sha256: scope.inputs["input"].sha256.clone(),
            },
        }],
        outputs: vec![PlannedOutput {
            artifact_id: "goal-report".into(),
            contract_id: output.clone(),
        }],
        required_validators: scope.contracts[&output].clone(),
        side_effect: SideEffect::WorkspaceWrite,
        write_set: vec![WriteTarget::Workspace {
            path: "reports/goal-report.txt".into(),
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
            runtime_ms: 4000,
            output_bytes: 2048,
            tokens: None,
            cost_microunits: None,
        },
        retry: RetryPolicy {
            max_attempts: 1,
            max_reworks: 0,
            retry_classes: vec![],
        },
        idempotency_key: "mixed-goal-report-once".into(),
        external_action_id: None,
    };
    let plan = GoalPlanProposal {
        schema_version: 1,
        plan_id: "mixed-goal-plan".into(),
        goal: scope.goal.clone(),
        scope_revision: scope.authority_revision,
        revision: 1,
        expected_revision: 0,
        acceptance_rules: scope.required_acceptance_rules.clone(),
        nodes: vec![node],
    };
    f.store
        .submit_goal_plan(
            &GoalPlanSubmission {
                scope_id: scope.id.clone(),
                proposal: plan,
            },
            "mixed-goal-plan",
            0,
        )
        .unwrap();
    (scope, ordinary)
}
fn mixed_call(runtime: &Path, request: serde_json::Value) -> serde_json::Value {
    let reply = agentbaton_core::ipc::call(runtime, request).unwrap();
    assert!(reply.get("error").is_none(), "{reply}");
    reply["result"].clone()
}
fn mixed_wait(
    runtime: &Path,
    method: &str,
    params: serde_json::Value,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let result = mixed_call(runtime, json!({"method":method,"params":params}));
        if predicate(&result) {
            return result;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{method} observation timed out: {result}"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
fn mixed_ordinary_then_goal(
    path: &Path,
    runtime: &Path,
    scope: &FactoryScope,
    goal: &agentbaton_core::goal_store::GoalScopeView,
    task: &agentbaton_core::storage::Task,
    busy: bool,
) {
    let created = mixed_call(
        runtime,
        json!({"method":"sessions.create","params":{"task_id":task.id,"workspace_id":scope.bindings.workspaces[0].id,"connection_id":scope.bindings.connections[0].id,"grant_id":scope.root_grant_id},"expected_revision":task.revision,"idempotency_key":"mixed-ordinary-session"}),
    );
    let ready = mixed_wait(
        runtime,
        "sessions.get",
        json!({"session_id":created["id"]}),
        |s| s["state"] == "ready",
    );
    let prompt = if busy {
        "generated-hold-ordinary"
    } else {
        "generated ordinary turn"
    };
    mixed_call(
        runtime,
        json!({"method":"sessions.send","params":{"session_id":ready["id"],"text":prompt},"expected_revision":ready["revision"],"idempotency_key":"mixed-ordinary-prompt"}),
    );
    let current = mixed_wait(
        runtime,
        "sessions.get",
        json!({"session_id":ready["id"]}),
        |s| {
            if busy {
                s["state"] == "running"
            } else {
                s["state"] == "ready" && s["turn_state"] == "completed"
            }
        },
    );
    let db = Connection::open(path).unwrap();
    let before_attempts: i64 = db
        .query_row("SELECT count(*) FROM attempts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before_attempts, 1);
    assert_eq!(
        db.query_row(
            "SELECT active FROM budgets WHERE scope=?1",
            [format!("project:{}", scope.project_id)],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    if busy {
        let reply=agentbaton_core::ipc::call(runtime,json!({"method":"factory_runs.start","params":{"scope_id":scope.id},"expected_revision":scope.revision,"idempotency_key":"actual-run"})).unwrap();
        assert_eq!(reply["error"]["code"], "WORKSPACE_BUSY", "{reply}");
        assert_eq!(reply["error"]["side_effect_state"], "not_started");
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_runs", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM factory_delegations", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM factory_budget_reservations WHERE scope_id IS NOT NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            before_attempts
        );
    }
    mixed_call(
        runtime,
        json!({"method":"sessions.cancel","params":{"session_id":current["id"]},"expected_revision":current["revision"],"idempotency_key":"mixed-ordinary-owned-stop"}),
    );
    let stopped = mixed_wait(
        runtime,
        "sessions.get",
        json!({"session_id":current["id"]}),
        |s| s["state"] == "stopped",
    );
    assert_eq!(stopped["id"], created["id"]);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let settled:bool=db.query_row("SELECT json_extract(document,'$.settled') FROM factory_budget_reservations WHERE scope_id IS NULL",[],|r|r.get(0)).unwrap();
        if settled {
            break;
        }
        assert!(
            std::time::Instant::now() < until,
            "ordinary receipt not joined/settled"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let ordinary: String = db
        .query_row(
            "SELECT document FROM factory_budget_reservations WHERE scope_id IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let ordinary: serde_json::Value = serde_json::from_str(&ordinary).unwrap();
    assert_eq!(ordinary["authority_kind"], "legacy_shared_session");
    assert_eq!(
        ordinary["measurement_scope"],
        "empty_selected_artifacts_no_validator_streams"
    );
    assert_eq!(ordinary["actual_output_bytes"], 0);
    assert!(ordinary["filesystem_total_bytes"].is_null());
    assert_eq!(ordinary["provider_message_capture_complete"], false);
    assert!(
        ordinary["captured_provider_message_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(ordinary["actual_runtime_ms"].as_u64().unwrap() > 0);
    assert_eq!(
        db.query_row(
            "SELECT consumed FROM budgets WHERE scope=?1",
            [format!("project:{}", scope.project_id)],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT active FROM budgets WHERE scope=?1",
            [format!("project:{}", scope.project_id)],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.payload.process_reaped')=1 AND json_extract(document,'$.payload.no_process_started')=0",[created["id"].as_str().unwrap()],|r|r.get::<_,i64>(0)).unwrap(),1);
    let snapshot = mixed_call(runtime, json!({"method":"system.snapshot","params":{}}));
    let proposal = mixed_call(
        runtime,
        json!({"method":"goal_runs.propose","params":{"scope_id":goal.id,"expected_plan_revision":1,"expected_ledger_revision":snapshot["sequence"]},"idempotency_key":"mixed-actual-goal-execution"}),
    );
    // Explicit trusted-host Owner fixture, not exposed through an Agent tool.
    let run = mixed_call(
        runtime,
        json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":"mixed-actual-goal-owner-confirm"}),
    );
    let goal_state = mixed_wait(runtime, "goal_runs.get", json!({"run_id":run["id"]}), |r| {
        r["run"]["state"] == "awaiting_acceptance"
            || r["run"]["state"] == "blocked"
            || r["run"]["state"] == "outcome_unknown"
    });
    assert_eq!(
        goal_state["run"]["state"], "awaiting_acceptance",
        "{goal_state}"
    );
    assert_eq!(goal_state["run"]["task_accepted"], false);
    assert_eq!(db.query_row("SELECT count(*) FROM goal_run_reservations WHERE json_extract(document,'$.settled')=1 AND json_extract(document,'$.actual_runtime_ms')>0 AND json_extract(document,'$.actual_output_bytes')>0",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    assert_eq!(
        db.query_row(
            "SELECT consumed FROM budgets WHERE scope=?1",
            [format!("project:{}", scope.project_id)],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        db.query_row(
            "SELECT active FROM budgets WHERE scope=?1",
            [format!("project:{}", scope.project_id)],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
fn assert_mixed_cumulative_receipts(db: &Connection, scope: &FactoryScope) {
    let count: i64 = db
        .query_row("SELECT count(*) FROM attempts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 5);
    let prior_runtime:i64=db.query_row("SELECT sum(runtime) FROM (SELECT json_extract(document,'$.actual_runtime_ms') AS runtime FROM factory_budget_reservations WHERE scope_id IS NULL UNION ALL SELECT json_extract(document,'$.actual_runtime_ms') FROM goal_run_reservations)",[],|r|r.get(0)).unwrap();
    let first_reservation:i64=db.query_row("SELECT json_extract(document,'$.reservation.runtime_ms') FROM factory_budget_reservations WHERE scope_id IS NOT NULL ORDER BY rowid LIMIT 1",[],|r|r.get(0)).unwrap();
    assert!(prior_runtime > 0);
    assert_eq!(
        u64::try_from(first_reservation).unwrap(),
        scope
            .selection
            .root_budget
            .runtime_ms
            .checked_sub(u64::try_from(prior_runtime).unwrap())
            .unwrap()
    );
    assert!(
        u64::try_from(first_reservation).unwrap() < scope.selection.root_budget.runtime_ms,
        "earlier owned work must not be refunded by a fresh Factory root"
    );
    eprintln!("MIXED_REAL_OWNERSHIP attempts={count} prior_runtime_ms={prior_runtime} first_factory_reservation_ms={first_reservation} original_runtime_ceiling_ms={}",scope.selection.root_budget.runtime_ms);
    for key in [
        format!("project:{}", scope.project_id),
        format!("root-grant:{}", scope.root_grant_id),
    ] {
        let raw: String = db
            .query_row(
                "SELECT document FROM goal_ledgers WHERE scope=?1",
                [key],
                |r| r.get(0),
            )
            .unwrap();
        let ledger: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let (runtime,outputs):(i64,i64)=db.query_row("SELECT sum(runtime),sum(bytes) FROM (SELECT json_extract(document,'$.actual_runtime_ms') AS runtime,json_extract(document,'$.actual_output_bytes') AS bytes FROM factory_budget_reservations UNION ALL SELECT json_extract(document,'$.actual_runtime_ms'),json_extract(document,'$.actual_output_bytes') FROM goal_run_reservations)",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert!(runtime > 0 && outputs > 0);
        assert_eq!(ledger["runtime_ms"].as_i64(), Some(runtime));
        assert_eq!(ledger["output_bytes"].as_i64(), Some(outputs));
        eprintln!("MIXED_SHARED_METER runtime_ms={runtime} selected_artifact_bytes={outputs} observed_attempts={count} active=0 filesystem_total_bytes=unknown");
        assert_eq!(ledger["limits"], json!(scope.selection.project_budget));
    }
    assert_eq!(
        db.query_row(
            "SELECT consumed FROM budgets WHERE scope=?1",
            [format!("project:{}", scope.project_id)],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        5
    );
    assert_eq!(
        db.query_row(
            "SELECT active FROM budgets WHERE scope=?1",
            [format!("project:{}", scope.project_id)],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(db.query_row("SELECT count(*) FROM factory_budget_reservations WHERE scope_id IS NULL AND json_extract(document,'$.settled')=1",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    assert_eq!(db.query_row("SELECT count(*) FROM factory_budget_reservations WHERE scope_id IS NOT NULL AND json_extract(document,'$.settled')=1",[],|r|r.get::<_,i64>(0)).unwrap(),3);
    assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.payload.process_reaped')=1 AND json_extract(document,'$.payload.no_process_started')=0",[],|r|r.get::<_,i64>(0)).unwrap(),5);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM tasks WHERE json_extract(document,'$.state')='accepted'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

fn scope_liveness_tools_and_owner_stop(
    path: &Path,
    runtime: &Path,
    scope: &FactoryScope,
    workspace: &Path,
    mode: &str,
) {
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    let until = Instant::now() + Duration::from_secs(5);
    while !workspace.join("reports/pin-ready").exists() {
        assert!(
            Instant::now() < until,
            "generated scoped MCP did not reach its actual ready gate"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let files = fs::read_dir(runtime.join("factory-scoped-handles"))
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 1);
    let credential: serde_json::Value =
        serde_json::from_slice(&fs::read(&files[0]).unwrap()).unwrap();
    let params = json!({"task_id":scope.root_task_id});
    let valid = scoped_request(
        runtime,
        json!({"api_version":1,"request_id":"valid-before-scope-restriction","handle_id":credential["handle_id"],"scoped_token":credential["scoped_token"],"method":"get_task_status","params":params}),
    );
    assert!(
        valid.get("error").is_none(),
        "the control must first prove real scoped authorization: {valid}"
    );
    assert_eq!(valid["result"]["task_id"], scope.root_task_id);
    let db = Connection::open(path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM factory_attempts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_leases WHERE holder!=''",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    if mode == "scope_revoke" {
        let revoked = mixed_call(
            runtime,
            json!({"method":"factory_scopes.revoke","params":{"scope_id":scope.id},"expected_revision":scope.revision,"idempotency_key":"actual-revoke-after-valid-handle"}),
        );
        assert_eq!(revoked["revoked"], true);
        assert_eq!(revoked["revision"], scope.revision + 1);
    } else {
        let timeout = Instant::now() + Duration::from_secs(12);
        while i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap()
            < scope.expires_at_ms
        {
            assert!(
                Instant::now() < timeout,
                "real approved scope clock did not expire"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let delegate = json!({"goal":"This real but now invalid handle must never create a child","connection_id":scope.selection.connection_ids[0],"workspace_id":scope.selection.workspace_ids[0],"input_artifact_ids":["input"],"output_artifact_ids":["report"],"budget":{"attempts":1,"parallel":1,"runtime_ms":1000,"output_bytes":4096,"children":0,"reworks":0,"tokens":null,"cost_microunits":null},"wait_for_child":true});
    for (method, params, revision, key) in [
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
        (
            "delegate_task",
            delegate,
            None,
            Some("expired-or-revoked-no-child"),
        ),
        (
            "cancel_task",
            json!({"task_id":scope.root_task_id}),
            Some(1u64),
            Some("expired-or-revoked-no-agent-cancel"),
        ),
    ] {
        let reply = scoped_request(
            runtime,
            json!({"api_version":1,"request_id":"actual-now-invalid-scope-handle","handle_id":credential["handle_id"],"scoped_token":credential["scoped_token"],"method":method,"params":params,"expected_revision":revision,"idempotency_key":key}),
        );
        assert_eq!(
            reply["error"]["code"], "AUTH_REQUIRED",
            "{mode} {method}: {reply}"
        );
        assert_eq!(reply["revision"], 0);
        assert!(reply.get("result").is_none());
        eprintln!("REAL_SCOPED_LIVENESS mode={mode} method={method} authorized_before=true denied_after=true");
    }
    let current = mixed_call(
        runtime,
        json!({"method":"factory_runs.get","params":{"scope_id":scope.id}}),
    );
    // Owner stop is a separate actual authority; it must never require the now
    // invalid Agent token or a successful scope/input witness.
    let cancelled = mixed_call(
        runtime,
        json!({"method":"factory_runs.cancel","params":{"scope_id":scope.id},"expected_revision":current["run"]["revision"],"idempotency_key":"actual-owner-stop-invalid-agent-scope"}),
    );
    assert_eq!(cancelled["id"], current["run"]["id"]);
    if current["run"]["state"] == "cancelled" {
        // Scope revocation/expiry may have already completed the actual stop.
        // The existing API keeps a terminal run terminal instead of inventing a
        // new in-flight cancellation flag. The owned-stop checks below remain
        // mandatory even for this legitimate idempotent race.
        assert_eq!(cancelled["state"], "cancelled");
        assert!(current["run"]["active_session_id"].is_null());
        assert!(cancelled["active_session_id"].is_null());
    } else {
        assert_eq!(cancelled["cancel_requested"], true);
    }
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let (pending,active):(i64,i64)=db.query_row("SELECT (SELECT count(*) FROM dispatch_intents WHERE state!='stopped'),(SELECT COALESCE(sum(active),0) FROM budgets WHERE scope LIKE 'project:%')",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        let stopped:i64=db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.payload.process_reaped')=1 AND json_extract(document,'$.payload.no_process_started')=0",[],|r|r.get(0)).unwrap();
        if pending == 0 && active == 0 && stopped == 1 {
            break;
        }
        assert!(Instant::now()<until,"Owner/internal stop must retain authority until its actual join; pending={pending} active={active} stopped={stopped}");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        db.query_row("SELECT count(*) FROM factory_delegations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM factory_attempts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM factory_budget_reservations",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT consumed FROM budgets WHERE scope=?1",
            [format!("project:{}", scope.project_id)],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_leases WHERE holder!=''",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM tasks WHERE json_extract(document,'$.state')='accepted'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    // process_reaped refers to the original owned leader only. These checks do
    // not prove that an escaped descendant tree is gone or that effects are known.
}

// Exact read-only snapshot of a real in-flight joined parent/active child.
// Opening it with Store::open correctly performs crash recovery; separate
// private tests must not confuse that recovery with an unchanged live run.
fn save_actual_planning_handoff_snapshot(path: &Path, scope: &FactoryScope) {
    let db = Connection::open(path).unwrap();
    let root: serde_json::Value = serde_json::from_str(
        &db.query_row(
            "SELECT document FROM tasks WHERE id=?1",
            [&scope.root_task_id],
            |r| r.get::<_, String>(0),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(root["state"], "outcome_unknown");
    let r: serde_json::Value = serde_json::from_str(
        &db.query_row(
            "SELECT document FROM factory_runs WHERE scope_id=?1",
            [&scope.id],
            |r| r.get::<_, String>(0),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(r["state"], "running");
    let b: serde_json::Value = serde_json::from_str(
        &db.query_row(
            "SELECT document FROM factory_task_bindings WHERE task_id=?1",
            [&scope.root_task_id],
            |r| r.get::<_, String>(0),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(b["handle"]["state"], "waiting_child");
    assert_eq!(b["handle"]["confirmed_stop"], true);
    if let Ok(output) = std::env::var("AGENTBATON_CHILD_PLAN_HANDOFF_DB") {
        assert!(!Path::new(&output).exists());
        db.execute("VACUUM INTO ?1", [&output]).unwrap();
        fs::set_permissions(&output, fs::Permissions::from_mode(0o600)).unwrap();
        eprintln!("ACTUAL_PLANNING_HANDOFF_SNAPSHOT path={output} root_generic_unknown=true factory_running=true parent_joined=true");
    }
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
fn actual_data_child_planning_and_postcompletion_context_keep_exact_history_without_readmitting_input_drift(
) {
    run_data_mode("planning");
}

#[test]
fn actual_data_scoped_four_methods_fail_closed_after_input_drift_but_owner_stop_still_joins() {
    run_data_mode("input_drift");
}
#[test]
fn actual_data_factory_wrong_start_revision_is_stale_before_read_worker_or_intent() {
    run_data_mode("wrong_start_revision");
}
#[cfg(target_os = "linux")]
#[test]
fn actual_large_data_read_owner_cancel_is_responsive_until_release_and_true_join() {
    run_data_mode("read_gate_cancel");
}
#[cfg(target_os = "linux")]
#[test]
fn actual_large_data_read_owner_revoke_is_responsive_until_release_and_true_join() {
    run_data_mode("read_gate_revoke");
}
#[cfg(target_os = "linux")]
#[test]
fn actual_large_data_read_scope_expiry_cancels_without_inventing_kernel_interruption() {
    run_data_mode("read_gate_expiry");
}
fn run_data_mode(mode: &str) {
    use agentbaton_core::ipc::call;
    use serde_json::Value;
    use std::time::{Duration, Instant};
    struct OwnedDaemon(std::process::Child, bool);
    impl Drop for OwnedDaemon {
        fn drop(&mut self) {
            // Only this original unreaped Child is controlled; no PID scanning.
            if self.0.try_wait().ok().flatten().is_some() {
                return;
            }
            if self.1 {
                // Large-read failure cleanup first releases its private gate,
                // then gives the real daemon its original owned-worker shutdown.
                unsafe {
                    libc::kill(self.0.id() as i32, libc::SIGTERM);
                }
                let until = Instant::now() + Duration::from_secs(5);
                while Instant::now() < until {
                    if self.0.try_wait().ok().flatten().is_some() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
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
    let input_limit = if mode.starts_with("read_gate_") {
        16 * 1024 * 1024
    } else {
        1024
    };
    let before = if mode.starts_with("read_gate_") {
        vec![b'Q'; input_limit]
    } else {
        b"fixed input\n".to_vec()
    };
    fs::write(source.join("input.txt"), &before).unwrap();
    let target = PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap());
    let tools = if mode == "planning" {
        PathBuf::from(
            std::env::var_os("AGENTBATON_FACTORY_TEST_TOOLS")
                .expect("Planning requires explicit matching frozen schema14 daemon and CLI"),
        )
    } else {
        std::env::var_os("AGENTBATON_FACTORY_TEST_TOOLS")
            .map(PathBuf::from)
            .unwrap_or_else(|| target.join("debug"))
    };
    for name in ["batond", "baton"] {
        let output = installed.join(name);
        fs::copy(tools.join(name), &output).unwrap();
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(Command::new("strip")
            .args(["-S"])
            .arg(&output)
            .status()
            .unwrap()
            .success());
    }
    #[cfg(target_os = "linux")]
    let mut read_gate = if mode.starts_with("read_gate_") {
        Some(large_data_gate::ReadGate::compile(
            &base,
            mode,
            &installed.join("batond"),
            &tools.join("batond"),
        ))
    } else {
        None
    };
    #[cfg(target_os = "linux")]
    if let Some(gate) = &read_gate {
        gate.phase("source",&runtime,None,json!({"dataset_bytes":input_limit,"input_source":source.join("input.txt"),"input_sha256":sha256(&before)}));
    }
    let mut command = Command::new(installed.join("batond"));
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("BATON_RUNTIME_DIR", &runtime)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "linux")]
    if let Some(gate) = &read_gate {
        gate.configure_own_daemon(&mut command);
    }
    let mut daemon = OwnedDaemon(command.spawn().unwrap(), mode.starts_with("read_gate_"));
    let ready = Instant::now() + Duration::from_secs(5);
    while !runtime.join("batond.sock").exists() {
        assert!(
            Instant::now() < ready,
            "generated installed daemon did not bind"
        );
        assert!(daemon.0.try_wait().unwrap().is_none());
        std::thread::sleep(Duration::from_millis(5));
    }
    #[cfg(target_os = "linux")]
    if let Some(gate) = &read_gate {
        gate.phase(
            "daemon_ready",
            &runtime,
            None,
            json!({"daemon_id":daemon.0.id()}),
        );
    }
    assert_eq!(
        Connection::open(runtime.join("baton.sqlite3"))
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        14,
        "Do not use an old schema13 daemon as evidence for current Core14 tests"
    );
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
        json!({"project_id":project["id"],"files":[{"artifact_id":"input","relative_path":"input.txt","max_bytes":input_limit,"expected_sha256":sha256(&before)}],"limits":{"max_file_bytes":input_limit,"max_total_bytes":input_limit,"max_files":1}}),
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
    let mut body = ACP_FACTORY
        .replace("status_connection", "data_connection")
        .replace("status_workspace", "data_workspace");
    if mode.starts_with("read_gate_") {
        // Keep the original real MCP reply verbatim. This is fixture evidence,
        // never a product authorization or a reusable private Data witness.
        body=body.replace("result=json.loads(mcp.stdout.readline());assert 'error' not in result,result", "result=json.loads(mcp.stdout.readline());assert 'error' not in result,result\n global last_mcp_response_frame\n last_mcp_response_frame=result");
        body=body.replace("actual=json.loads(result['content'][0]['text']);assert 'error' not in actual,actual", "actual=json.loads(result['content'][0]['text']);assert 'error' not in actual,actual\n if name=='get_task_status':\n  global initial_scoped_response_text,initial_scoped_mcp_frame\n  initial_scoped_response_text=result['content'][0]['text']\n  initial_scoped_mcp_frame=last_mcp_response_frame");
        body=body.replace("status=tool('get_task_status',{'task_id':c['task_id']});assert status['task_id']==c['task_id']", "import time\n  initial_scoped_begin_ms=int(time.time()*1000)\n  status=tool('get_task_status',{'task_id':c['task_id']});assert status['task_id']==c['task_id']\n  initial_scoped_response_ms=int(time.time()*1000)");
        body=body.replace("open('reports/pin-ready','w').write('generated safe gate')", "open('reports/pin-ready','w').write(json.dumps({'provider_pid':os.getpid(),'initial_get_begin_epoch_ms':initial_scoped_begin_ms,'initial_get_response_epoch_ms':initial_scoped_response_ms,'task_id':status['task_id'],'task_revision':status['revision'],'task_state':status['state'],'method':'get_task_status','original_scoped_response_text':initial_scoped_response_text,'original_mcp_response_frame':initial_scoped_mcp_frame}))");
    }
    if mode == "planning" {
        body = child_planning_fixture(body, false);
    }
    // Fixture-only, nonsecret host-generated identifier; it grants no rights.
    let connection_label = base.join("generated-connection-label.txt");
    fs::write(&provider, body.replace("mcp=None;mid=0", &format!("data_connection=open({0}).read() if os.path.exists({0}) else ''\ndata_workspace={1}\nfixture_mode={2}\nmcp=None;mid=0",serde_json::to_string(&connection_label).unwrap(),copied["workspace_id"],serde_json::to_string(if mode=="input_drift"||mode.starts_with("read_gate_"){"pin_drift"}else{"normal"}).unwrap()))).unwrap();
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
    let mut selection = json!({"root_task_id":task["id"],"expected_task_revision":task["revision"],"grant_id":grant["id"],"expected_grant_revision":grant["revision"],"connection_ids":[connection["id"]],"workspace_ids":[copied["workspace_id"]],"inputs":[{"artifact_id":"input","relative_path":"inputs/input.txt","max_bytes":input_limit,"expected_sha256":sha256(&before)}],"outputs":[{"artifact_id":"report","relative_path":"reports/report.txt","max_bytes":4096,"expected_sha256":null}],"recipients":[],"max_depth":2,"max_children":2,"max_total_nodes":4,"root_budget":budget,"project_budget":budget,"ttl_ms":if mode=="read_gate_expiry"{4000}else{30000},"capabilities":{"delegate_task":true,"resume_parent_as_new_attempt":true,"child_planning":false,"child_goal_execution":false,"create_workspace":false}});
    if mode == "planning" {
        selection["capabilities"]["child_planning"] = json!(true);
        selection["child_planning_policy"] = json!({"max_nodes":8,"max_revisions":16,"max_plan_bytes":262144,"outputs":[{"artifact":selection["outputs"][0],"validators":[]}],"writable_prefixes":["reports/report.txt"],"owner_acceptance_criteria":"Review the full exact report; storing a plan never accepts it"});
    }
    #[cfg(target_os = "linux")]
    if let Some(gate) = &read_gate {
        gate.phase(
            "scope_propose_begin",
            &runtime,
            None,
            json!({"scope_ttl_ms":selection["ttl_ms"]}),
        );
    }
    let proposal = rpc(
        "factory_scopes.propose",
        selection.clone(),
        Some("data-factory-proposal"),
        None,
    );
    assert_eq!(proposal["kind"], "factory");
    #[cfg(target_os = "linux")]
    if let Some(gate) = &read_gate {
        gate.phase(
            "scope_finalize_begin",
            &runtime,
            None,
            json!({"proposal_id":proposal["id"],"proposal_revision":proposal["revision"]}),
        );
    }
    let scope = approve(&proposal, "trusted-factory-owner");
    #[cfg(target_os = "linux")]
    if let Some(gate) = &read_gate {
        gate.phase(
            "scope_finalize_response",
            &runtime,
            Some(&scope),
            json!({"contract_digest":scope["contract_digest"]}),
        );
    }
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
    let start_key = if mode == "wrong_start_revision" {
        "wrong-cas-must-not-consume-key"
    } else {
        "data-factory-run"
    };
    if mode == "wrong_start_revision" {
        let seq: i64 = check_db
            .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| {
                r.get(0)
            })
            .unwrap();
        let wrong=call(&runtime,json!({"method":"factory_runs.start","params":{"scope_id":scope["id"]},"expected_revision":scope["revision"].as_u64().unwrap()+1,"idempotency_key":start_key})).unwrap();
        eprintln!(
            "ACTUAL_FACTORY_WRONG_CAS code={} side_effect_state={} revision={}",
            wrong["error"]["code"], wrong["error"]["side_effect_state"], wrong["revision"]
        );
        let bytes = serde_json::to_vec_pretty(&wrong).unwrap();
        let name = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../artifacts/factory-runtime-core/wrong-start-cas-response-{}.json",
            sha256(&bytes)
        ));
        fs::write(name, bytes).unwrap();
        assert_eq!(wrong["error"]["code"], "STALE_REVISION", "{wrong}");
        assert_eq!(wrong["error"]["side_effect_state"], "not_started");
        assert_eq!(
            check_db
                .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            seq
        );
        for table in [
            "factory_runs",
            "factory_attempts",
            "factory_task_lineage",
            "factory_task_bindings",
            "factory_budget_reservations",
            "dispatch_intents",
        ] {
            assert_eq!(
                check_db
                    .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0,
                "wrong CAS mutated {table}"
            );
        }
        assert_eq!(
            check_db
                .query_row(
                    "SELECT count(*) FROM idempotency WHERE key=?1",
                    [start_key],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
    #[cfg(target_os = "linux")]
    if let Some(gate) = &read_gate {
        gate.phase(
            "start_begin",
            &runtime,
            Some(&scope),
            json!({"scope_revision":scope["revision"]}),
        );
    }
    let start = rpc(
        "factory_runs.start",
        json!({"scope_id":scope["id"]}),
        Some(start_key),
        scope["revision"].as_u64(),
    );
    assert_eq!(start["state"], "queued");
    #[cfg(target_os = "linux")]
    if let Some(gate) = &mut read_gate {
        gate.phase("start_response",&runtime,Some(&scope),json!({"run_id":start["id"],"run_state":start["state"],"run_revision":start["revision"]}));
        assert_eq!(scope["project_id"], project["id"]);
        large_data_gate::run_inflight_case(
            gate,
            &runtime,
            &workspace,
            &scope,
            &selection,
            daemon.0.id(),
            mode,
        );
        assert_eq!(fs::read(source.join("input.txt")).unwrap(), before);
        gate.stop_original_daemon(&mut daemon.0);
        return;
    }
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
    if mode == "wrong_start_revision" {
        fs::remove_file(installed.join("baton")).unwrap();
        let seq: i64 = check_db
            .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| {
                r.get(0)
            })
            .unwrap();
        let replay = rpc(
            "factory_runs.start",
            json!({"scope_id":scope["id"]}),
            Some(start_key),
            scope["revision"].as_u64(),
        );
        assert_eq!(
            replay, start,
            "same-key history must be exact, not an execution retry or latest-state substitution"
        );
        assert_eq!(
            check_db
                .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            seq
        );
        assert_eq!(
            check_db
                .query_row("SELECT count(*) FROM factory_attempts", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
    }

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
    if mode == "planning" {
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM factory_child_plan_revisions",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            4
        );
        let q = json!({"scope_id":scope["id"],"task_id":task["id"]});
        let context = rpc("factory_child_plans.context", q.clone(), None, None);
        let reference = rpc("factory_child_plans.get", q.clone(), None, None);
        let seq: i64 = db
            .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| {
                r.get(0)
            })
            .unwrap();
        fs::write(
            workspace.join("inputs/input.txt"),
            b"later changed input, historical context is not a read permission",
        )
        .unwrap();
        assert_eq!(
            rpc("factory_child_plans.context", q.clone(), None, None),
            context
        );
        assert_eq!(
            rpc("factory_child_plans.get", q.clone(), None, None),
            reference
        );
        let raw = json!({"scope_id":scope["id"],"task_id":task["id"],"plan_json":reference["plan_json"],"expected_plan_revision":2});
        let refused = call(&runtime,json!({"method":"factory_child_plans.submit","params":raw,"expected_revision":scope["revision"],"idempotency_key":"never-write-after-completion-drift"})).unwrap();
        assert!(refused.get("error").is_some(), "{refused}");
        assert_eq!(
            db.query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            seq
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM factory_child_plan_revisions",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            4
        );
        assert_eq!(fs::read(source.join("input.txt")).unwrap(), before);
        eprintln!(
            "ACTUAL_DATA_PLANNING history_after_input_drift=true no_new_execution_or_receipt=true"
        );
    }
}

#[path = "fixtures/factory_child_goal_runtime.rs"]
mod child_goal_runtime;
