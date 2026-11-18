//! Real native-confirmation persistence and generated Codex-protocol process.
//! Reading a materialized Skill proves availability, not real Codex adoption.
use super::*;
use agentbaton_config::runtime::{RuntimeRequirements, RuntimeRoot};
use std::{fs, os::unix::fs::PermissionsExt};
const TEXT:&str="---\nname: write-report\ndescription: Produce a concise report\n---\nUse only the approved input and write a concise report.\n";
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    store: Store,
    base: ProviderConnection,
    folder: PathBuf,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let runtime = root.join("runtime");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let python = std::process::Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let exe = root.join("generated-codex");
    let script = r#"
import sys,json,os,tomllib,subprocess
if sys.argv[1:]==['--version']:print('Generated Codex MCP peer 1.0');sys.exit(0)
assert sys.argv[1:]==['app-server','--listen','stdio://']
open('provider-started','w').write('generated')
config=tomllib.load(open(os.path.join(os.environ['CODEX_HOME'],'config.toml'),'rb'));assert config['sandbox_mode']=='read-only' and config['approval_policy']=='untrusted' and 'model' not in config
for line in sys.stdin:
 r=json.loads(line);m=r['method']
 if m=='initialized':continue
 if m=='initialize':v={'codexHome':os.environ['CODEX_HOME'],'platformFamily':'unix','platformOs':'generated','userAgent':'generated MCP fixture'}
 elif m=='thread/start':v={'thread':{'id':'skill-thread'},'cwd':os.getcwd(),'approvalPolicy':'untrusted','sandbox':{'type':'readOnly','networkAccess':False}}
 elif m=='turn/start':v={'turn':{'id':'skill-turn','items':[],'status':'inProgress'}}
 else:raise AssertionError(m)
 print(json.dumps({'id':r['id'],'result':v}),flush=True)
 if m=='turn/start':
  observed={'server':None,'callback_denied':False}
  if config['mcp_servers']:
   assert list(config['mcp_servers'])==['local_notes']
   server=config['mcp_servers']['local_notes'];assert server['args']==[] and not server.get('env') and not server.get('env_vars') and server['enabled_tools']==['read_note'] and server['default_tools_approval_mode']=='prompt'
   child=subprocess.Popen([server['command']],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,env=os.environ.copy())
   try:
    def rpc(i,m,p):
     child.stdin.write(json.dumps({'jsonrpc':'2.0','id':i,'method':m,'params':p})+'\n');child.stdin.flush();return json.loads(child.stdout.readline())['result']
    init=rpc(1,'initialize',{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'generated','version':'1'}})
    child.stdin.write(json.dumps({'jsonrpc':'2.0','method':'notifications/initialized'})+'\n');child.stdin.flush()
    listing=rpc(2,'resources/list',{});resource=rpc(3,'resources/read',{'uri':listing['resources'][0]['uri']})
    assert resource['contents'][0]['text']=='generated local note'
    print(json.dumps({'id':'approval-callback','method':'item/mcpToolCall/requestApproval','params':{'threadId':'skill-thread','turnId':'skill-turn','itemId':'call','server':'local_notes','tool':'read_note'}}),flush=True)
    response=json.loads(sys.stdin.readline());assert response['id']=='approval-callback' and response['error']['code']==-32601
    observed={'server':server,'resource':resource,'callback_denied':True}
   finally:
    child.terminate();child.wait(timeout=2)
  open('generated-mcp-observation.json','w').write(json.dumps(observed))
  print(json.dumps({'method':'turn/completed','params':{'threadId':'skill-thread','turn':{'id':'skill-turn','items':[],'status':'completed'}}}),flush=True)
"#;
    fs::write(
        &exe,
        format!(
            "#!{}\n{}",
            String::from_utf8(python.stdout).unwrap().trim(),
            script
        ),
    )
    .unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    let config = root.join("policy.toml");
    fs::write(
        &config,
        "sandbox_mode='read-only'\napproval_policy='untrusted'\n",
    )
    .unwrap();
    let mut store = Store::open(&runtime.join("db.sqlite")).unwrap();
    let p = store
        .propose_connection_with_configuration(
            "Base",
            "codex",
            &exe,
            &[],
            Some(config.to_str().unwrap()),
            "base",
        )
        .unwrap();
    let base: ProviderConnection =
        serde_json::from_value(store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    let mut supervisor = Supervisor::new(&runtime);
    supervisor.probe(&store, &base.id).unwrap();
    let at = Instant::now();
    while supervisor.has_owned_io() {
        supervisor.tick(&mut store).unwrap();
        assert!(at.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    let base = store.get_connection(&base.id).unwrap();
    assert_eq!(base.probe_state, "ready");
    let folder = root.join("write-report");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("SKILL.md"), TEXT).unwrap();
    Fixture {
        _dir: dir,
        root,
        store,
        base,
        folder,
    }
}

fn mcp(f: &Fixture) -> PathBuf {
    let path = f.root.join("generated-mcp");
    let shebang = fs::read_to_string(&f.base.executable)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_owned();
    let body = r#"
import sys,json,os
assert len(sys.argv)==1
for line in sys.stdin:
 r=json.loads(line);m=r['method']
 if m=='notifications/initialized':continue
 if m=='initialize':v={'protocolVersion':'2024-11-05','capabilities':{'resources':{},'tools':{}},'serverInfo':{'name':'generated','version':'1'}}
 elif m=='resources/list':v={'resources':[{'uri':'generated://note','name':'note'}]}
 elif m=='resources/read':v={'contents':[{'uri':'generated://note','text':'generated local note'}]}
 elif m=='tools/list':v={'tools':[{'name':'read_note','description':'generated','inputSchema':{'type':'object'}}]}
 elif m=='tools/call':open('business-canary','w').write('must not happen');v={'content':[]}
 else:raise AssertionError(m)
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':v}),flush=True)
"#;
    fs::write(&path, format!("{shebang}\n{body}")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}
fn request(f: &Fixture) -> ManagedMcpRequest {
    ManagedMcpRequest {
        base_connection_id: f.base.id.clone(),
        expected_base_revision: f.base.revision,
        new_name: "With local notes".into(),
        operation: ManagedMcpOperation::Replace,
        server: Some(ManagedMcpServerSelection {
            name: "local_notes".into(),
            executable: mcp(f).to_str().unwrap().into(),
            enabled_tools: vec!["read_note".into()],
        }),
    }
}
fn approve(
    f: &mut Fixture,
    r: &ManagedMcpRequest,
    key: &str,
) -> (ManagedMcpPreview, ApprovalProposal, ManagedMcpConfirmation) {
    let preview = f.store.preview_managed_mcp(r).unwrap();
    let p = f
        .store
        .propose_managed_mcp(
            &ManagedMcpSelection {
                request: r.clone(),
                expected_preview_digest: preview.preview_digest.clone(),
            },
            key,
        )
        .unwrap();
    let c = serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    (preview, p, c)
}
fn ready(f: &mut Fixture, id: &str) {
    let mut supervisor = Supervisor::new(&f.root.join("runtime"));
    supervisor.probe(&f.store, id).unwrap();
    let at = Instant::now();
    while supervisor.has_owned_io() {
        supervisor.tick(&mut f.store).unwrap();
        assert!(at.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn runtime(
    f: &Fixture,
    c: &ProviderConnection,
) -> (
    RuntimeRoot,
    agentbaton_config::runtime::RuntimeSnapshot,
    ApprovedInstallation,
    ManagedMcpLaunch,
) {
    let dir = f.root.join(format!("snapshot-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    let root = RuntimeRoot::open(&dir).unwrap();
    let plan = crate::runtime_config::approved_plan(c).unwrap();
    let snapshot = root
        .materialize(plan.clone(), Ownership::BatonManaged)
        .unwrap();
    let snapshot = root
        .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .unwrap();
    let authority = ManagedMcpLaunch::new(&plan, &snapshot).unwrap();
    let installation = ApprovedInstallation {
        executable: c.executable.clone(),
        expected_sha256: executable_digest(&c.executable).unwrap(),
        arguments: vec!["app-server".into(), "--listen".into(), "stdio://".into()],
        workspace: f.root.clone(),
        approval_reference: c.approval_id.clone(),
        environment: snapshot.environment(),
    };
    (root, snapshot, installation, authority)
}
#[test]
fn managed_mcp_confirmation_preserves_skill_policy_replay_and_remove() {
    let mut f = fixture();
    let original = encode(&f.base).unwrap();
    let original_id = f.base.id.clone();
    let skill = super::super::ManagedSkillRequest {
        base_connection_id: f.base.id.clone(),
        expected_base_revision: f.base.revision,
        new_name: "Skill base".into(),
        operation: super::super::ManagedSkillOperation::Replace,
        skill_folder: Some(f.folder.to_str().unwrap().into()),
    };
    let sp = f.store.preview_managed_skill(&skill).unwrap();
    let sp = f
        .store
        .propose_managed_skill(
            &super::super::ManagedSkillSelection {
                request: skill,
                expected_preview_digest: sp.preview_digest,
            },
            "skill",
        )
        .unwrap();
    let sc: super::super::ManagedSkillConfirmation =
        serde_json::from_value(f.store.finalize_proposal(&sp.id, 1).unwrap()).unwrap();
    ready(&mut f, &sc.connection.id);
    f.base = f.store.get_connection(&sc.connection.id).unwrap();
    let skill_before = encode(&f.base.runtime_plan.as_ref().unwrap().skills).unwrap();
    let r = request(&f);
    let sequence = f.store.sequence().unwrap();
    let preview = f.store.preview_managed_mcp(&r).unwrap();
    assert_eq!(sequence, f.store.sequence().unwrap());
    assert!(preview.base.has_managed_skill);
    let (preview, p, c) = approve(&mut f, &r, "mcp");
    assert_eq!(
        encode(&c.connection.runtime_plan.as_ref().unwrap().skills).unwrap(),
        skill_before
    );
    assert_eq!(c.profile.sandbox_mode, "read-only");
    assert!(c.profile.runtime_connected.is_none() && c.profile.runtime_used.is_none());
    assert_eq!(c.connection.probe_state, "pending");
    let saved = fs::read(&r.server.as_ref().unwrap().executable).unwrap();
    fs::remove_file(&r.server.as_ref().unwrap().executable).unwrap();
    let replay: ManagedMcpConfirmation =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    assert_eq!(replay.connection.id, c.connection.id);
    assert_eq!(
        f.store
            .find_managed_mcp(&ManagedMcpFind {
                request_key: "mcp".into()
            })
            .unwrap()
            .unwrap()
            .connection
            .id,
        c.connection.id
    );
    fs::write(&r.server.as_ref().unwrap().executable, saved).unwrap();
    fs::set_permissions(
        &r.server.as_ref().unwrap().executable,
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    ready(&mut f, &c.connection.id);
    let base = f.store.get_connection(&c.connection.id).unwrap();
    let remove = ManagedMcpRequest {
        base_connection_id: base.id.clone(),
        expected_base_revision: base.revision,
        new_name: "Without MCP".into(),
        operation: ManagedMcpOperation::Remove,
        server: None,
    };
    let (rp, proposal, removed) = approve(&mut f, &remove, "remove");
    assert!(removed.profile.managed_mcp.is_none());
    assert_eq!(
        encode(&removed.connection.runtime_plan.as_ref().unwrap().skills).unwrap(),
        skill_before
    );
    assert_eq!(removed.connection.config_revision, f.base.config_revision);
    let events: String = f
        .store
        .conn
        .query_row("SELECT group_concat(document) FROM events", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(!events.contains("Use only the approved input"));
    assert!(!events.contains(&r.server.as_ref().unwrap().executable));
    assert_eq!(
        original,
        encode(&f.store.get_connection(&original_id).unwrap()).unwrap()
    );
    if let Some(path) = std::env::var_os("AGENTBATON_MANAGED_MCP_FIXTURE") {
        fs::write(path,serde_json::to_vec_pretty(&json!({"base_connection":f.base,"request":r,"preview":preview,"proposal":p,"confirmed":c,"remove_request":remove,"remove_preview":rp,"remove_proposal":proposal,"removed":removed,"catalog":f.store.list_managed_mcp_profiles().unwrap()})).unwrap()).unwrap();
    }
}
#[test]
fn managed_mcp_source_cas_closed_fields_and_worker_boundary_refuse_drift() {
    let mut f = fixture();
    let r = request(&f);
    let preview = f.store.preview_managed_mcp(&r).unwrap();
    let selection = ManagedMcpSelection {
        request: r.clone(),
        expected_preview_digest: preview.preview_digest,
    };
    let p = f.store.propose_managed_mcp(&selection, "pending").unwrap();
    fs::write(&r.server.as_ref().unwrap().executable, "changed").unwrap();
    let count = f.store.list_connections().unwrap().len();
    assert!(f.store.finalize_proposal(&p.id, 1).is_err());
    assert_eq!(count, f.store.list_connections().unwrap().len());
    let r = request(&f);
    let (_, _, c) = approve(&mut f, &r, "fresh");
    let (_root, snapshot, installation, authority) = runtime(&f, &c.connection);
    fs::write(&r.server.as_ref().unwrap().executable, "drift after proof").unwrap();
    let (sender, _receive) = mpsc::sync_channel(8);
    let (_commands, receiver) = mpsc::sync_channel(8);
    let receipt = super::super::run_worker(
        "drift".into(),
        WorkerLaunch::conversation(
            c.connection.protocol_route,
            installation,
            Some(crate::runtime_config::codex_snapshot_configuration(&snapshot).unwrap()),
            0,
            Duration::from_secs(3),
            authority,
        ),
        receiver,
        Arc::new(AtomicBool::new(false)),
        sender,
    );
    assert!(receipt.never_started);
    assert_eq!(receipt.reason, "managed_mcp_launch_binding_changed");
    assert!(!f.root.join("provider-started").exists());
    for tools in [
        vec![],
        vec!["read_note".into(), "read_note".into()],
        vec!["bad:tool".into()],
        vec!["x".repeat(65)],
    ] {
        let mut r = request(&f);
        r.server.as_mut().unwrap().enabled_tools = tools;
        assert!(f.store.preview_managed_mcp(&r).is_err());
    }
    let mut raw = json!(request(&f));
    raw["server"]["env"] = json!({"TOKEN":"not imported"});
    assert!(serde_json::from_value::<ManagedMcpRequest>(raw).is_err());
}
fn git(path: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Generated",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().into()
}
#[test]
fn managed_mcp_new_task_grant_worker_reads_resource_and_denies_tool_callback() {
    let mut f = fixture();
    let original = encode(&f.base).unwrap();
    let r = request(&f);
    let (_, _, derived) = approve(&mut f, &r, "task-skill");

    let mut supervisor = Supervisor::new(&f.root.join("runtime"));
    supervisor.probe(&f.store, &derived.connection.id).unwrap();
    let at = Instant::now();
    while supervisor.has_owned_io() {
        supervisor.tick(&mut f.store).unwrap();
        assert!(at.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    let source = f.root.join("git-source");
    fs::create_dir(&source).unwrap();
    git(&source, &["init", "--initial-branch=main"]);
    fs::write(source.join("input.txt"), "generated input").unwrap();
    git(&source, &["add", "input.txt"]);
    git(&source, &["commit", "-m", "generated fixture"]);
    let head = git(&source, &["rev-parse", "HEAD"]);
    let project = f
        .store
        .import_project(&source, "Skill task", "project")
        .unwrap();
    let managed = f.root.join("workspaces");
    fs::create_dir(&managed).unwrap();
    fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
    let workspace = f
        .store
        .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
        .unwrap();
    let task = f
        .store
        .create_registered(&project.id, "Use the selected report skill", "task")
        .unwrap();
    let p = f
        .store
        .propose_grant(
            &project.id,
            &workspace.id,
            &derived.connection.id,
            4,
            1,
            10000,
            "grant",
        )
        .unwrap();
    assert_eq!(
        p.snapshot["refs"]["native_mcp"]["server"]["approval_mode"],
        "prompt"
    );
    let grant: crate::policy::ExecutionGrant =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    use crate::goal_plan::BudgetLimits;
    use crate::goal_store::{GoalOutputSelection, GoalScopeSelection};
    use agentbaton_workspaces::validation::ArtifactRequirement;
    use sha2::{Digest, Sha256};
    let budget = BudgetLimits {
        attempts: 4,
        parallel: 1,
        runtime_ms: 10000,
        output_bytes: 4096,
        children: 1,
        reworks: 0,
        tokens: None,
        cost_microunits: None,
    };
    let scope = GoalScopeSelection {
        task_id: task.id.clone(),
        expected_task_revision: task.revision,
        grant_id: grant.id.clone(),
        expected_grant_revision: grant.revision,
        max_nodes: 2,
        new_max_children: 1,
        new_max_depth: 1,
        root_budget: budget.clone(),
        project_budget: budget,
        inputs: vec![ArtifactRequirement {
            artifact_id: "source".into(),
            relative_path: "input.txt".into(),
            max_bytes: 1024,
            expected_sha256: Some(format!("{:x}", Sha256::digest(b"generated input"))),
        }],
        outputs: vec![GoalOutputSelection {
            artifact: ArtifactRequirement {
                artifact_id: "report".into(),
                relative_path: "outputs/report.txt".into(),
                max_bytes: 1024,
                expected_sha256: None,
            },
            validators: vec![],
        }],
        writable_prefixes: vec!["outputs".into()],
        owner_acceptance_criteria: "Owner reviews the resulting report".into(),
    };
    let _goal = f.store.propose_goal_scope(&scope, "goal-review").unwrap();
    let events: String = f
        .store
        .conn
        .query_row("SELECT group_concat(document) FROM events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(!events.contains("Use only the approved input"));
    let session = f
        .store
        .create_session(
            &task.id,
            &workspace.id,
            &derived.connection.id,
            &grant.id,
            task.revision,
            "create",
        )
        .unwrap();
    supervisor.launch(&mut f.store, &session.id).unwrap();
    let at = Instant::now();
    loop {
        supervisor.tick(&mut f.store).unwrap();
        let s = f.store.get_session(&session.id).unwrap();
        if s.state == "ready" {
            let (_, new) = f
                .store
                .send_session(&s.id, "Use the selected report skill", s.revision, "send")
                .unwrap();
            assert!(new);
            supervisor
                .send(&s.id, "Use the selected report skill")
                .unwrap();
            break;
        }
        assert!(at.elapsed() < Duration::from_secs(5), "{:?}", s);
        std::thread::sleep(Duration::from_millis(5));
    }
    let at = Instant::now();
    loop {
        supervisor.tick(&mut f.store).unwrap();
        let s = f.store.get_session(&session.id).unwrap();
        if s.turn_state.as_deref() == Some("completed") {
            break;
        }
        assert!(at.elapsed() < Duration::from_secs(5), "{:?}", s);
        std::thread::sleep(Duration::from_millis(5));
    }
    let observed: Value = serde_json::from_slice(
        &fs::read(workspace.root.join("generated-mcp-observation.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(observed["callback_denied"], true);
    assert_eq!(
        observed["resource"]["contents"][0]["text"],
        "generated local note"
    );
    assert!(!workspace.root.join("business-canary").exists());
    assert_eq!(
        original,
        encode(&f.store.get_connection(&f.base.id).unwrap()).unwrap()
    );
    let binding = f.store.runtime_binding(&session.id).unwrap().unwrap();
    assert_eq!(
        binding.connection_config_revision,
        derived.connection.config_revision
    );
    assert_eq!(binding.plan_digest, derived.profile.runtime_plan_digest);
    supervisor.shutdown(&mut f.store).unwrap();
    assert!(!supervisor.has_owned_io());
}

#[test]
fn managed_mcp_cancel_after_verified_preflight_never_spawns_provider() {
    let mut f = fixture();
    let r = request(&f);
    let (_, _, c) = approve(&mut f, &r, "cancel-proof");
    let (_root, snapshot, installation, mut authority) = runtime(&f, &c.connection);
    let cancel = Arc::new(AtomicBool::new(false));
    authority.cancel_after_verify = Some(cancel.clone());
    let (sender, _receive) = mpsc::sync_channel(8);
    let (_commands, receiver) = mpsc::sync_channel(8);
    let receipt = super::super::run_worker(
        "cancel".into(),
        WorkerLaunch::conversation(
            c.connection.protocol_route,
            installation,
            Some(crate::runtime_config::codex_snapshot_configuration(&snapshot).unwrap()),
            0,
            Duration::from_secs(3),
            authority,
        ),
        receiver,
        cancel,
        sender,
    );
    assert!(receipt.never_started);
    assert_eq!(receipt.reason, "managed_preflight_cancelled_or_deadline");
    assert!(!f.root.join("provider-started").exists());
}

mod active_diagnostic {
    use super::diagnostics::*;
    use super::*;
    use McpDiagnosticOutcome::*;

    struct DiagnosticFixture {
        base: Fixture,
        request: ManagedMcpDiagnosticRequest,
        executable: PathBuf,
        started: PathBuf,
    }

    fn diagnostic_fixture(mode: &str) -> DiagnosticFixture {
        let mut f = fixture();
        let create = request(&f);
        let executable = PathBuf::from(&create.server.as_ref().unwrap().executable);
        let shebang = fs::read_to_string(&executable)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_owned();
        let started = f.root.join("diagnostic-started");
        let body = r#"
import json,os,stat,sys,time
assert len(sys.argv)==1
assert stat.S_IMODE(os.stat('.').st_mode)==0o700
assert os.path.basename(os.getcwd())=='work'
entered=os.path.dirname(os.getcwd())
for name in ['HOME','TMPDIR','XDG_CONFIG_HOME','XDG_CACHE_HOME','XDG_DATA_HOME']:
 assert os.environ[name].startswith(entered+'/')
 assert os.path.isdir(os.environ[name])
assert 'CODEX_HOME' not in os.environ and 'BATON_RUNTIME_DIR' not in os.environ
assert 'AB_MCP_DIAGNOSTIC_SYNTHETIC_EVIDENCE' not in os.environ
with open(os.path.join(os.path.dirname(__file__),'diagnostic-started'),'w') as f:
 json.dump({'pid':os.getpid(),'work':os.getcwd()}, f)
print('untrusted-stderr-content',file=sys.stderr,flush=True)
seen=[]
for line in sys.stdin:
 query=json.loads(line)
 method=query.get('method','forbidden-response')
 seen.append(method)
 with open('wire-methods.json','w') as f:
  json.dump(seen,f)
 if method=='initialize':
  assert query['jsonrpc']=='2.0' and query['id']==1
  assert query['params']['protocolVersion']=='2025-06-18' and query['params']['capabilities']=={}
  if MODE=='hang':
   time.sleep(120)
  if MODE=='oversize':
   print('x'*70000,flush=True)
   continue
  if MODE=='callback':
   print(json.dumps({'jsonrpc':'2.0','id':99,'method':'sampling/createMessage','params':{'untrusted':'untrusted-callback-content'}}),flush=True)
   continue
  result={'protocolVersion':'2024-11-05','capabilities':{'tools':{},'resources':{}},'serverInfo':{'name':'untrusted-server-name','version':'1'},'instructions':'untrusted-instructions'}
  if MODE=='unsupported-version':
   result['protocolVersion']='2035-01-01'
  if MODE=='no-tools':
   del result['capabilities']['tools']
 elif method=='notifications/initialized':
  assert not 'id' in query and seen==['initialize','notifications/initialized']
  continue
 elif method=='tools/list':
  assert seen[:2]==['initialize','notifications/initialized']
  assert not set(query['params'])-{'cursor'}
  if query['id']==2:
   assert query['params']=={}
   result={'tools':[{'name':'read_note','description':'untrusted-description','inputSchema':{'type':'object'}}],'nextCursor':'untrusted-cursor'}
  else:
   assert query['id']==3 and query['params']=={'cursor':'untrusted-cursor'}
   result={'tools':[{'name':'untrusted-extra-tool','inputSchema':{'type':'object'}}]}
   if MODE=='repeated-cursor':
    result={'tools':[],'nextCursor':'untrusted-cursor'}
   if MODE=='drift':
    os.chmod(__file__,0o500)
 else:
  with open('forbidden-method','w') as f:
   f.write(method)
  raise AssertionError(method)
 print(json.dumps({'jsonrpc':'2.0','id':query['id'],'result':result}),flush=True)
with open('closed-input','w') as f:
 f.write('observed')
if MODE=='exit-nonzero':
 sys.exit(19)
"#;
        fs::write(&executable, format!("{shebang}\nMODE={mode:?}\n{body}")).unwrap();
        let (_, _, derived) = approve(&mut f, &create, "diagnostic-derived");
        ready(&mut f, &derived.connection.id);
        let connection = f.store.get_connection(&derived.connection.id).unwrap();
        assert_eq!(connection.probe_state, "ready");
        let source = f.root.join("diagnostic-project");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        fs::write(source.join("project-canary"), "private project input").unwrap();
        git(&source, &["add", "project-canary"]);
        git(&source, &["commit", "-m", "generated diagnostic project"]);
        let project = f
            .store
            .import_project(&source, "Diagnostics", "project")
            .unwrap();
        DiagnosticFixture {
            base: f,
            executable,
            started,
            request: ManagedMcpDiagnosticRequest {
                project_id: project.id,
                project_revision: project.revision,
                connection_id: connection.id,
                connection_revision: connection.revision,
                server_name: "local_notes".into(),
            },
        }
    }

    impl DiagnosticFixture {
        fn preview(&self) -> PendingMcpDiagnostic {
            self.base
                .store
                .preview_managed_mcp_diagnostic(&self.request)
                .unwrap()
        }

        fn approval(&self) -> OwnerConfirmedMcpDiagnostic {
            confirm(self.preview())
        }

        fn runtime_root(&self) -> PathBuf {
            self.base.root.join("runtime")
        }

        fn run(
            &mut self,
            approval: OwnerConfirmedMcpDiagnostic,
            cancel: &AtomicBool,
        ) -> Result<ManagedMcpDiagnosticAttempt> {
            let runtime = self.runtime_root();
            self.base
                .store
                .run_managed_mcp_diagnostic(approval, &runtime, cancel)
        }

        fn trace(&self) -> (u32, PathBuf, Value) {
            let info: Value = serde_json::from_slice(&fs::read(&self.started).unwrap()).unwrap();
            let pid = info["pid"].as_u64().unwrap().try_into().unwrap();
            let work = PathBuf::from(info["work"].as_str().unwrap());
            let methods =
                serde_json::from_slice(&fs::read(work.join("wire-methods.json")).unwrap()).unwrap();
            assert!(work.starts_with(self.runtime_root()));
            assert!(!work.join("forbidden-method").exists());
            (pid, work, methods)
        }

        fn assert_reaped(&self, attempt: &ManagedMcpDiagnosticAttempt) {
            assert!(!attempt.has_unreaped_child());
            assert!(attempt.journal_recorded());
            let (pid, _, _) = self.trace();
            let pid = rustix::process::Pid::from_raw(pid.try_into().unwrap()).unwrap();
            assert_eq!(
                rustix::process::test_kill_process(pid),
                Err(rustix::io::Errno::SRCH),
                "the fixture direct child must already be reaped"
            );
            assert!(matches!(
                attempt.receipt().direct_child,
                McpDiagnosticExit::Reaped { .. }
            ));
            let record = self
                .base
                .store
                .get_managed_mcp_diagnostic(&attempt.receipt().diagnostic_id)
                .unwrap();
            assert_eq!(json!(record.receipt), json!(attempt.receipt()));
        }
    }

    fn confirm(review: PendingMcpDiagnostic) -> OwnerConfirmedMcpDiagnostic {
        let reviewed_digest = review.review_digest().to_owned();
        review
            .confirm_after_owner_dialog(Some(&reviewed_digest))
            .unwrap()
    }

    #[test]
    fn static_discovery_and_unconfirmed_or_stale_review_never_spawn() {
        let mut f = diagnostic_fixture("list");
        let first = f.base.store.sequence().unwrap();
        assert!(f.base.store.list_managed_mcp_profiles().is_ok());
        assert!(f
            .base
            .store
            .get_managed_mcp_profile(&f.request.connection_id)
            .is_ok());
        let review = f.preview();
        assert_eq!(review.review().request.project_id, f.request.project_id);
        assert_eq!(
            review.review().actions,
            ["initialize", "notifications/initialized", "tools/list"]
        );
        assert_eq!(
            review.confirm_after_owner_dialog(None).unwrap_err().code,
            "PERMISSION_DENIED"
        );
        assert_eq!(
            f.preview()
                .confirm_after_owner_dialog(Some("different digest"))
                .unwrap_err()
                .code,
            "PERMISSION_DENIED"
        );
        let mut expired = f.preview();
        expired.expire_for_test();
        let digest = expired.review_digest().to_string();
        assert_eq!(
            expired
                .confirm_after_owner_dialog(Some(&digest))
                .unwrap_err()
                .code,
            "PERMISSION_DENIED"
        );
        let bad_input = json!({"project_id":f.request.project_id,"project_revision":f.request.project_revision,"connection_id":f.request.connection_id,"connection_revision":f.request.connection_revision,"server_name":f.request.server_name,"owner_confirmed":true});
        assert!(serde_json::from_value::<ManagedMcpDiagnosticRequest>(bad_input).is_err());
        let approved = f.approval();
        let mut wrong_server = f.request.clone();
        wrong_server.server_name = "other".into();
        assert!(f
            .base
            .store
            .preview_managed_mcp_diagnostic(&wrong_server)
            .is_err());

        let copied = f.executable.with_extension("copy");
        fs::copy(&f.executable, &copied).unwrap();
        fs::rename(copied, &f.executable).unwrap();
        assert_eq!(
            f.run(approved, &AtomicBool::new(false)).unwrap_err().code,
            "INPUT_CHANGED"
        );
        assert!(!f.started.exists());
        assert_eq!(f.base.store.sequence().unwrap(), first);
        assert!(fs::read_dir(f.runtime_root()).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("mcp-diagnostic")));
    }

    #[test]
    fn registered_project_and_connection_config_compare_before_launch() {
        let mut f = diagnostic_fixture("list");
        let approval = f.approval();
        let conn = f
            .base
            .store
            .get_connection(&f.request.connection_id)
            .unwrap();
        let mut altered = conn.clone();
        altered.config_revision.push_str("-changed");
        f.base
            .store
            .conn
            .execute(
                "UPDATE connections SET document=?1 WHERE id=?2",
                params![encode(&altered).unwrap(), conn.id],
            )
            .unwrap();
        assert_eq!(
            f.run(approval, &AtomicBool::new(false)).unwrap_err().code,
            "INPUT_CHANGED"
        );
        f.base
            .store
            .conn
            .execute(
                "UPDATE connections SET document=?1 WHERE id=?2",
                params![encode(&conn).unwrap(), conn.id],
            )
            .unwrap();

        let mut remote = conn.clone();
        remote.host_id = "registered-elsewhere".into();
        f.base
            .store
            .conn
            .execute(
                "UPDATE connections SET document=?1 WHERE id=?2",
                params![encode(&remote).unwrap(), conn.id],
            )
            .unwrap();
        assert_eq!(
            f.base
                .store
                .preview_managed_mcp_diagnostic(&f.request)
                .unwrap_err()
                .code,
            "CAPABILITY_UNSUPPORTED",
        );
        f.base
            .store
            .conn
            .execute(
                "UPDATE connections SET document=?1 WHERE id=?2",
                params![encode(&conn).unwrap(), conn.id],
            )
            .unwrap();

        let approval = f.approval();
        f.base
            .store
            .archive_project(&f.request.project_id, f.request.project_revision, "archive")
            .unwrap();
        assert_eq!(
            f.run(approval, &AtomicBool::new(false)).unwrap_err().code,
            "INPUT_CHANGED"
        );
        assert!(!f.started.exists());
    }

    #[test]
    fn actual_initialize_then_only_paginated_tools_list_with_independent_receipts() {
        let mut f = diagnostic_fixture("list");
        let connection_before = f
            .base
            .store
            .get_managed_mcp_profile(&f.request.connection_id)
            .unwrap();
        let events_before = f.base.store.sequence().unwrap();
        let approved = f.approval();
        let attempt = f.run(approved, &AtomicBool::new(false)).unwrap();
        f.assert_reaped(&attempt);
        let receipt = attempt.receipt();
        assert!(receipt.verified());
        assert_eq!(receipt.protocol_version.as_deref(), Some("2024-11-05"));
        assert!(receipt
            .runtime_snapshot_digest
            .as_ref()
            .is_some_and(|v| v.len() == 64));
        assert_eq!(receipt.configured_tools_seen, ["read_note"]);
        assert_eq!(receipt.other_tool_count, Some(1));
        assert!(receipt.stderr_bytes_observed > 0);
        assert_eq!(
            receipt.direct_child,
            McpDiagnosticExit::Reaped {
                exit_code: Some(0),
                signal: None,
                kill_requested: false
            }
        );
        let (_, work, methods) = f.trace();
        assert!(work.join("closed-input").is_file());
        assert!(!work.join("project-canary").exists());
        assert_eq!(
            methods,
            json!([
                "initialize",
                "notifications/initialized",
                "tools/list",
                "tools/list"
            ])
        );
        let evidence = f.base.store.events(events_before, 10).unwrap();
        let json = encode(&evidence).unwrap();
        assert!(
            json.contains("mcp_diagnostic.launch_intent")
                && json.contains("mcp_diagnostic.observed")
        );
        assert!(!json.contains("untrusted-"));
        assert_eq!(connection_before.runtime_connected, None);
        assert_eq!(connection_before.runtime_used, None);
        assert_eq!(
            json!(connection_before),
            json!(f
                .base
                .store
                .get_managed_mcp_profile(&f.request.connection_id)
                .unwrap())
        );
        let sessions: i64 = f
            .base
            .store
            .conn
            .query_row("SELECT count(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sessions, 0);
        let next = f.approval();
        let second = f.run(next, &AtomicBool::new(false)).unwrap();
        f.assert_reaped(&second);
        assert!(second.receipt().verified());
        assert_ne!(second.receipt().diagnostic_id, receipt.diagnostic_id);
        if let Some(path) = std::env::var_os("AB_MCP_DIAGNOSTIC_SYNTHETIC_EVIDENCE") {
            fs::write(
                path,
                serde_json::to_vec_pretty(&json!({
                    "receipt":receipt,
                    "observed_methods":methods,
                    "stdin_close_observed":true,
                    "direct_child_exit_verified":true,
                    "session_count":sessions,
                    "connection_projection_equal":true
                }))
                .unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn actual_timeout_signal_and_reap_owned_child() {
        let mut f = diagnostic_fixture("hang");
        let approval = f.approval();
        let at = Instant::now();
        let attempt = f.run(approval, &AtomicBool::new(false)).unwrap();
        assert_eq!(attempt.receipt().outcome, TimedOut);
        assert!(at.elapsed() < Duration::from_secs(7));
        f.assert_reaped(&attempt);
        assert_eq!(f.trace().2, json!(["initialize"]));
        assert_eq!(
            attempt.receipt().direct_child,
            McpDiagnosticExit::Reaped {
                exit_code: None,
                signal: Some(rustix::process::Signal::KILL.as_raw()),
                kill_requested: true
            }
        );
    }

    #[test]
    fn cancelled_before_and_during_real_spawn_have_distinct_exit_proof() {
        let mut f = diagnostic_fixture("hang");
        let approval = f.approval();
        let absent = f.run(approval, &AtomicBool::new(true)).unwrap();
        assert_eq!(absent.receipt().outcome, Cancelled);
        assert_eq!(absent.receipt().direct_child, McpDiagnosticExit::NotStarted);
        assert!(absent.journal_recorded());
        assert!(!f.started.exists());

        let approval = f.approval();
        let cancel = AtomicBool::new(false);
        let start = f.started.clone();
        let attempt = std::thread::scope(|scope| {
            let cancel = &cancel;
            let cancel_on_init = scope.spawn(move || {
                let at = Instant::now();
                loop {
                    if let Ok(Some(methods)) = fs::read(&start).map(|data| {
                        let value: Value = serde_json::from_slice(&data).unwrap_or(Value::Null);
                        value["work"]
                            .as_str()
                            .map(|work| Path::new(work).join("wire-methods.json"))
                    }) {
                        if fs::read(methods)
                            .ok()
                            .is_some_and(|bytes| bytes == br#"["initialize"]"#)
                        {
                            cancel.store(true, Ordering::Release);
                            return true;
                        }
                    }
                    if at.elapsed() >= Duration::from_secs(3) {
                        return false;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            });
            let attempt = f.run(approval, cancel).unwrap();
            assert!(cancel_on_init.join().unwrap());
            attempt
        });
        assert_eq!(attempt.receipt().outcome, Cancelled);
        f.assert_reaped(&attempt);
    }

    #[test]
    fn rejected_protocol_budget_and_live_source_drift_do_not_verify() {
        for (mode, expected) in [
            ("callback", ProtocolFailed),
            ("oversize", ResourceLimit),
            ("unsupported-version", UnsupportedProtocol),
            ("repeated-cursor", ProtocolFailed),
            ("no-tools", ToolsUnsupported),
            ("exit-nonzero", ProcessFailed),
            ("drift", InputChanged),
        ] {
            let mut f = diagnostic_fixture(mode);
            let approval = f.approval();
            let attempt = f.run(approval, &AtomicBool::new(false)).unwrap();
            assert_eq!(attempt.receipt().outcome, expected, "{mode}");
            assert!(!attempt.receipt().verified());
            f.assert_reaped(&attempt);
            assert!(!encode(attempt.receipt()).unwrap().contains("untrusted-"));
        }
    }

    #[test]
    fn unresolved_durable_intent_blocks_a_second_spawn_and_retained_receipt_recovers() {
        let mut f = diagnostic_fixture("list");
        let approval = f.approval();
        f.base
            .store
            .conn
            .execute_batch(
                "CREATE TRIGGER reject_diagnostic_receipt BEFORE INSERT ON events
            WHEN json_extract(NEW.document,'$.kind')='mcp_diagnostic.observed'
            BEGIN SELECT RAISE(ABORT,'generated receipt fault'); END",
            )
            .unwrap();
        let mut attempt = f.run(approval, &AtomicBool::new(false)).unwrap();
        assert!(attempt.receipt().verified());
        assert!(!attempt.journal_recorded());
        assert!(!attempt.has_unreaped_child());
        let id = attempt.receipt().diagnostic_id.clone();
        assert!(f
            .base
            .store
            .get_managed_mcp_diagnostic(&id)
            .unwrap()
            .receipt
            .is_none());
        // Store checks the schema on reopen, so remove the injected fault
        // before reopening the still-pending durable diagnostic record.
        f.base
            .store
            .conn
            .execute_batch("DROP TRIGGER reject_diagnostic_receipt")
            .unwrap();
        let reopened = Store::open(&f.runtime_root().join("db.sqlite")).unwrap();
        assert!(reopened
            .get_managed_mcp_diagnostic(&id)
            .unwrap()
            .receipt
            .is_none());
        assert!(reopened.has_unresolved_managed_mcp_diagnostic().unwrap());
        drop(reopened);
        let first_start = fs::read(&f.started).unwrap();
        let second = f.approval();
        assert_eq!(
            f.run(second, &AtomicBool::new(false)).unwrap_err().code,
            "DIAGNOSTIC_RECONCILIATION_REQUIRED"
        );
        assert_eq!(fs::read(&f.started).unwrap(), first_start);
        attempt.reconcile(&mut f.base.store).unwrap();
        assert!(!f
            .base
            .store
            .has_unresolved_managed_mcp_diagnostic()
            .unwrap());
        f.assert_reaped(&attempt);
        let next = f.approval();
        assert!(f
            .run(next, &AtomicBool::new(false))
            .unwrap()
            .receipt()
            .verified());
    }
}
