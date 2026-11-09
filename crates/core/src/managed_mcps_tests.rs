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
