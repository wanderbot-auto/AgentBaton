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
import sys,json,os,tomllib
if sys.argv[1:]==['--version']:print('Generated Codex protocol peer 1.0');sys.exit(1 if os.path.exists(__file__+'.fail') else 0)
assert sys.argv[1:]==['app-server','--listen','stdio://']
config=tomllib.load(open(os.path.join(os.environ['CODEX_HOME'],'config.toml'),'rb'));assert config['sandbox_mode']=='read-only' and config['approval_policy']=='untrusted';assert not config['mcp_servers'];assert 'model' not in config
skills=os.path.join(os.environ['HOME'],'.agents','skills');entries=[]
if os.path.isdir(skills):
 for name in os.listdir(skills):
  assert os.listdir(os.path.join(skills,name))==['SKILL.md']
  entries.append({'name':name,'text':open(os.path.join(skills,name,'SKILL.md')).read()})
assert not os.path.exists(os.path.join(os.environ['CODEX_HOME'],'skills'))
for line in sys.stdin:
 r=json.loads(line);m=r['method']
 if m=='initialized':continue
 if m=='initialize':v={'codexHome':os.environ['CODEX_HOME'],'platformFamily':'unix','platformOs':'generated','userAgent':'generated skills fixture'}
 elif m=='thread/start':
  assert r['params']=={'cwd':os.getcwd(),'approvalPolicy':'untrusted','sandbox':'read-only','ephemeral':True}
  v={'thread':{'id':'skill-thread'},'cwd':os.getcwd(),'approvalPolicy':'untrusted','sandbox':{'type':'readOnly','networkAccess':False}}
 elif m=='turn/start':
  assert r['params']['sandboxPolicy']=={'type':'readOnly','networkAccess':False}
  open('generated-skill-observation.json','w').write(json.dumps(entries))
  v={'turn':{'id':'skill-turn','items':[],'status':'inProgress'}}
 else:raise AssertionError(m)
 print(json.dumps({'id':r['id'],'result':v}),flush=True)
 if m=='turn/start':print(json.dumps({'method':'turn/completed','params':{'threadId':'skill-thread','turn':{'id':'skill-turn','items':[],'status':'completed'}}}),flush=True)
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
fn request(f: &Fixture) -> ManagedSkillRequest {
    ManagedSkillRequest {
        base_connection_id: f.base.id.clone(),
        expected_base_revision: f.base.revision,
        new_name: "With report skill".into(),
        operation: ManagedSkillOperation::Replace,
        skill_folder: Some(f.folder.to_str().unwrap().into()),
    }
}
fn approve(
    f: &mut Fixture,
    r: &ManagedSkillRequest,
    key: &str,
) -> (
    ManagedSkillPreview,
    ApprovalProposal,
    ManagedSkillConfirmation,
) {
    let preview = f.store.preview_managed_skill(r).unwrap();
    let s = ManagedSkillSelection {
        request: r.clone(),
        expected_preview_digest: preview.preview_digest.clone(),
    };
    let p = f.store.propose_managed_skill(&s, key).unwrap();
    let c = serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    (preview, p, c)
}
fn run(
    f: &Fixture,
    c: &ProviderConnection,
) -> (
    Value,
    agentbaton_config::runtime::RuntimeSnapshot,
    RuntimeRoot,
) {
    let root = f.root.join(format!("materialized-{}", Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let runtime = RuntimeRoot::open(&root).unwrap();
    let snapshot = runtime
        .materialize(
            crate::runtime_config::approved_plan(c).unwrap(),
            Ownership::BatonManaged,
        )
        .unwrap();
    let verified = runtime
        .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .unwrap();
    let mut child = ManagedProviderProcess::start(
        c.protocol_route,
        ApprovedInstallation {
            executable: c.executable.clone(),
            expected_sha256: executable_digest(&c.executable).unwrap(),
            arguments: vec!["app-server".into(), "--listen".into(), "stdio://".into()],
            workspace: f.root.clone(),
            approval_reference: c.approval_id.clone(),
            environment: verified.environment(),
        },
        Duration::from_secs(3),
    )
    .unwrap();
    child
        .configure_session_configuration(
            &crate::runtime_config::codex_snapshot_configuration(&verified).unwrap(),
        )
        .unwrap();
    let id = child.new_session("generated-skill").unwrap();
    child
        .begin_prompt(&id, "Use the selected report skill")
        .unwrap();
    assert_eq!(
        child.wait_prompt(&id).unwrap(),
        agentbaton_providers::TurnState::Completed
    );
    child.terminate();
    assert!(child.diagnostics().process_reaped);
    (
        serde_json::from_slice(&fs::read(f.root.join("generated-skill-observation.json")).unwrap())
            .unwrap(),
        snapshot,
        runtime,
    )
}
#[test]
fn managed_skill_replace_source_deleted_generated_private_home_then_remove_preserves_base() {
    let mut f = fixture();
    let base = encode(&f.base).unwrap();
    let r = request(&f);
    let before = f.store.sequence().unwrap();
    let preview = f.store.preview_managed_skill(&r).unwrap();
    assert_eq!(before, f.store.sequence().unwrap());
    assert_eq!(preview.instruction.as_deref(), Some(TEXT));
    let (preview, proposal, created) = approve(&mut f, &r, "replace");
    assert_eq!(created.profile.sandbox_mode, "read-only");
    assert_eq!(created.profile.approval_policy, "untrusted");
    assert!(created.profile.runtime_loaded.is_none() && created.profile.runtime_used.is_none());
    assert_eq!(created.connection.probe_state, "pending");
    assert_ne!(created.connection.environment_id, f.base.environment_id);
    fs::remove_file(f.folder.join("SKILL.md")).unwrap();
    fs::remove_dir(&f.folder).unwrap();
    let replay: ManagedSkillConfirmation =
        serde_json::from_value(f.store.finalize_proposal(&proposal.id, 1).unwrap()).unwrap();
    assert_eq!(replay.connection.id, created.connection.id);
    assert_eq!(
        f.store
            .find_managed_skill(&ManagedSkillFind {
                request_key: "replace".into()
            })
            .unwrap()
            .unwrap()
            .connection
            .id,
        created.connection.id
    );
    let (observed, snapshot, runtime) = run(&f, &created.connection);
    assert_eq!(observed, json!([{"name":"write-report","text":TEXT}]));
    let home = snapshot
        .environment()
        .into_iter()
        .find(|(k, _)| k == "HOME")
        .map(|(_, v)| PathBuf::from(v))
        .unwrap();
    assert_eq!(
        fs::read_to_string(home.join(".agents/skills/write-report/SKILL.md")).unwrap(),
        TEXT
    );
    let mut supervisor = Supervisor::new(&f.root.join("runtime"));
    supervisor.probe(&f.store, &created.connection.id).unwrap();
    let at = Instant::now();
    while supervisor.has_owned_io() {
        supervisor.tick(&mut f.store).unwrap();
        assert!(at.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    let ready = f.store.get_connection(&created.connection.id).unwrap();
    let remove = ManagedSkillRequest {
        base_connection_id: ready.id.clone(),
        expected_base_revision: ready.revision,
        new_name: "Without skill".into(),
        operation: ManagedSkillOperation::Remove,
        skill_folder: None,
    };
    let (remove_preview, remove_proposal, removed) = approve(&mut f, &remove, "remove");
    assert!(removed.profile.managed_skill.is_none());
    let events: String = f
        .store
        .conn
        .query_row("SELECT group_concat(document) FROM events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(!events.contains("Use only the approved input and write a concise report."));
    assert!(!format!("{preview:?}").contains("Use only the approved input"));
    assert_eq!(removed.profile.config_revision, f.base.config_revision);
    assert_eq!(run(&f, &removed.connection).0, json!([]));
    assert_eq!(
        base,
        encode(&f.store.get_connection(&f.base.id).unwrap()).unwrap()
    );
    assert_eq!(
        fs::read_to_string(home.join(".agents/skills/write-report/SKILL.md")).unwrap(),
        TEXT
    );
    runtime
        .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .unwrap();
    fs::write(
        home.join(".agents/skills/write-report/SKILL.md"),
        "tampered",
    )
    .unwrap();
    assert!(runtime
        .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .is_err());
    if let Some(path) = std::env::var_os("AGENTBATON_MANAGED_SKILL_FIXTURE") {
        fs::write(path,serde_json::to_vec_pretty(&json!({"request":r,"request_key":"replace","preview":preview,"proposal":proposal,"confirmed":created,"removed":removed,"remove_request":remove,"remove_preview":remove_preview,"remove_proposal":remove_proposal,"catalog":f.store.list_managed_skill_profiles().unwrap()})).unwrap()).unwrap();
    }
}
#[test]
fn managed_skill_native_rechecks_source_base_and_rejects_unsupported_content() {
    for changed_source in [true, false] {
        let mut f = fixture();
        let r = request(&f);
        let preview = f.store.preview_managed_skill(&r).unwrap();
        let p = f
            .store
            .propose_managed_skill(
                &ManagedSkillSelection {
                    request: r,
                    expected_preview_digest: preview.preview_digest,
                },
                "draft",
            )
            .unwrap();
        let count = f.store.list_connections().unwrap().len();
        if changed_source {
            fs::write(
                f.folder.join("SKILL.md"),
                TEXT.replace("concise report.", "different report."),
            )
            .unwrap()
        } else {
            let mut c = f.base.clone();
            c.revision += 1;
            f.store
                .conn
                .execute(
                    "UPDATE connections SET document=?1 WHERE id=?2",
                    params![encode(&c).unwrap(), c.id],
                )
                .unwrap();
        }
        assert!(f.store.finalize_proposal(&p.id, 1).is_err());
        assert_eq!(count, f.store.list_connections().unwrap().len());
    }
    let f = fixture();
    let r = request(&f);
    fs::write(f.folder.join("secret.txt"), "not imported").unwrap();
    assert!(f.store.preview_managed_skill(&r).is_err());
    fs::remove_file(f.folder.join("secret.txt")).unwrap();
    fs::write(
        f.folder.join("SKILL.md"),
        TEXT.replace("description:", "unknown_key: denied\ndescription:"),
    )
    .unwrap();
    assert!(f.store.preview_managed_skill(&r).is_err());
    let mut remove = r;
    remove.operation = ManagedSkillOperation::Remove;
    remove.skill_folder = None;
    assert!(f.store.preview_managed_skill(&remove).is_err());
}
#[test]
fn managed_skill_closed_runtime_plan_cannot_enable_mcp_assets_or_models_and_legacy_catalog_survives(
) {
    let mut f = fixture();
    let r = request(&f);
    let (_, _, c) = approve(&mut f, &r, "replace");
    for mode in 0..4 {
        let mut c = c.connection.clone();
        let p = c.runtime_plan.as_mut().unwrap();
        match mode {
            0 => p.codex.model = Some(NonSecretText::approved("unapproved-model".into()).unwrap()),
            1 => p.skills[0]
                .assets
                .push(agentbaton_config::runtime::SkillAsset {
                    relative_path: "run.sh".into(),
                    content: agentbaton_config::runtime::NonSecretBytes::approved(b"no".to_vec())
                        .unwrap(),
                    script: true,
                }),
            2 => p
                .mcp_servers
                .push(agentbaton_config::runtime::ManagedMcpServer {
                    name: "unapproved".into(),
                    codex_policy: None,
                    transport: agentbaton_config::runtime::ManagedMcpTransport::Http {
                        endpoint: NonSecretText::approved("https://example.invalid/mcp".into())
                            .unwrap(),
                    },
                    provenance: p.source_refs[0].clone(),
                }),
            _ => p.source_refs.push(SourceProvenance {
                source_ref: "unapproved".into(),
                revision: "1".into(),
                sha256: "0".repeat(64),
            }),
        }
        let pd = p.plan_digest();
        if let Ok(pd) = pd {
            c.config_revision =
                crate::runtime_config::configuration_revision(&c.arguments, c.protocol_route, &pd)
                    .unwrap();
            assert!(crate::runtime_config::approved_plan(&c).is_err());
        }
    }
    let mut legacy = f.base.clone();
    legacy.id = "legacy-codex".into();
    legacy.runtime_plan = None;
    f.store
        .conn
        .execute(
            "INSERT INTO connections VALUES(?1,?2)",
            params![legacy.id, encode(&legacy).unwrap()],
        )
        .unwrap();
    let catalog = f.store.list_managed_skill_profiles().unwrap();
    assert_eq!(catalog.profiles.len(), 2);
    assert_eq!(catalog.unavailable.len(), 1);
    assert_eq!(catalog.unavailable[0].connection_id, legacy.id);
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
fn managed_skill_new_task_approved_grant_supervisor_launch_reads_private_copy() {
    let mut f = fixture();
    let original = encode(&f.base).unwrap();
    let r = request(&f);
    let (_, _, derived) = approve(&mut f, &r, "task-skill");
    fs::remove_file(f.folder.join("SKILL.md")).unwrap();
    fs::remove_dir(&f.folder).unwrap();
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
        &fs::read(workspace.root.join("generated-skill-observation.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(observed, json!([{"name":"write-report","text":TEXT}]));
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
fn managed_skill_later_probe_approval_redacts_body_but_binds_exact_digest() {
    let mut f = fixture();
    let r = request(&f);
    let (_, _, derived) = approve(&mut f, &r, "skill");
    fs::write(
        format!("{}.fail", derived.connection.executable.display()),
        b"fail version",
    )
    .unwrap();
    let mut supervisor = Supervisor::new(&f.root.join("runtime"));
    supervisor.probe(&f.store, &derived.connection.id).unwrap();
    let at = Instant::now();
    while supervisor.has_owned_io() {
        supervisor.tick(&mut f.store).unwrap();
        assert!(at.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    let c = f.store.get_connection(&derived.connection.id).unwrap();
    assert_eq!(c.probe_state, "failed");
    assert!(c.version_evidence.as_ref().unwrap().process_reaped);
    let p = f
        .store
        .propose_connection_probe(
            &ConnectionInspectRequest {
                connection_id: c.id,
                expected_revision: c.revision,
            },
            "probe-review",
        )
        .unwrap();
    assert!(encode(&p.snapshot)
        .unwrap()
        .contains("Use only the approved input"));
    let events: String = f
        .store
        .conn
        .query_row("SELECT group_concat(document) FROM events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(!events.contains("Use only the approved input"));
    let expected = super::super::approval_event_payload(&p.kind, &p.snapshot).unwrap();
    assert!(expected.get("snapshot").is_none());
    let stored:String=f.store.conn.query_row("SELECT document FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='approval.proposed'",[&p.id],|r|r.get(0)).unwrap();
    let stored: crate::storage::Event = decode(&stored).unwrap();
    assert_eq!(stored.payload, expected);
    let mut corrupt = expected.clone();
    corrupt["snapshot_digest"] = json!("0".repeat(64));
    assert_ne!(stored.payload, corrupt);
    let plain = json!({"original":"unmarked"});
    assert_eq!(
        super::super::approval_event_payload("goal", &plain).unwrap(),
        json!({"kind":"goal","snapshot":plain})
    );
}
