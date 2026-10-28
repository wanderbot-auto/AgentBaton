//! Generated local files and explicit trusted-core connection/owner fixtures.
//! No MCP/Provider process or native dialog is claimed by these scope tests.
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
#[test]
fn separate_factory_confirmation_keeps_the_original_grant_unchanged_and_never_starts_a_run() {
    let mut f = Fixture::new();
    let original = f.store.get_grant(&f.selection.grant_id).unwrap();
    let p = f.proposal();
    assert_eq!(p.kind, "factory");
    assert_eq!(
        p.snapshot["bindings"]["start_contract"]["implicit_start_on_approval"],
        false
    );
    assert!(f
        .store
        .list_factory_scopes(&f.selection.root_task_id)
        .unwrap()
        .is_empty());
    let scope: FactoryScope =
        serde_json::from_value(f.store.finalize_proposal(&p.id, p.revision).unwrap()).unwrap();
    assert_eq!(scope.approval_id, p.id);
    assert_eq!(scope.bindings.approved_grant, original);
    assert_eq!(f.store.get_grant(&original.id).unwrap(), original);
    assert_eq!(scope.root_grant_revision, original.revision);
    assert_eq!(scope.selection.max_depth, 2);
    assert_eq!(
        f.store
            .delegate_grant(
                &original.id,
                original.revision,
                original.scope.clone(),
                "still-no-grant-delegation"
            )
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    let db = Connection::open(&f.path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM factory_runs", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM factory_scoped_handles", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM budgets", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.store
            .list_factory_scopes(&f.selection.root_task_id)
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn unsupported_factory_capabilities_and_cross_project_or_unapproved_targets_never_create_proposals()
{
    let mut f = Fixture::new();
    let baseline = f.selection.clone();
    for field in ["child_planning", "child_goal_execution", "create_workspace"] {
        let mut v = json!(baseline);
        v["capabilities"][field] = json!(true);
        let forged: FactoryScopeSelection = serde_json::from_value(v).unwrap();
        assert_eq!(
            f.store
                .propose_factory_scope(&forged, field, &f.companion)
                .unwrap_err()
                .code,
            "CAPABILITY_UNSUPPORTED"
        );
    }
    let mut bad = baseline.clone();
    bad.connection_ids = vec!["unapproved".into()];
    assert_eq!(
        f.store
            .propose_factory_scope(&bad, "bad-connection", &f.companion)
            .unwrap_err()
            .code,
        "AUTH_REQUIRED"
    );
    let mut v = json!(baseline);
    v["caller_id"] = json!("administrator");
    assert!(serde_json::from_value::<FactoryScopeSelection>(v).is_err());
    let db = Connection::open(&f.path).unwrap();
    assert_eq!(db.query_row("SELECT count(*) FROM approval_proposals WHERE json_extract(document,'$.kind')='factory'",[],|r|r.get::<_,u32>(0)).unwrap(),0);
}
#[test]
fn native_factory_snapshot_drift_and_confirmation_journal_fault_never_commit_authority() {
    let mut f = Fixture::new();
    let p = f.proposal();
    let db = Connection::open(&f.path).unwrap();
    db.execute_batch("CREATE TRIGGER reject_factory_owner BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='approval.confirmed' AND json_extract(NEW.document,'$.payload.kind')='factory' BEGIN SELECT RAISE(ABORT,'generated owner journal fault'); END;").unwrap();
    assert!(f.store.finalize_proposal(&p.id, p.revision).is_err());
    assert!(f
        .store
        .list_factory_scopes(&f.selection.root_task_id)
        .unwrap()
        .is_empty());
    assert!(db
        .query_row(
            "SELECT result IS NULL FROM approval_proposals WHERE id=?1",
            [&p.id],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
    db.execute_batch("DROP TRIGGER reject_factory_owner")
        .unwrap();
    let workspace = f
        .store
        .list_workspaces(&f.store.get(&f.selection.root_task_id).unwrap().project_id)
        .unwrap()
        .remove(0);
    fs::write(workspace.root.join("input.txt"), b"changed input\n").unwrap();
    assert_eq!(
        f.store
            .finalize_proposal(&p.id, p.revision)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    assert!(f
        .store
        .list_factory_scopes(&f.selection.root_task_id)
        .unwrap()
        .is_empty());
}
#[test]
fn factory_revoke_is_idempotent_and_prior_confirmation_replay_is_only_historical_data() {
    let mut f = Fixture::new();
    let s = f.scope();
    let r = f
        .store
        .revoke_factory_scope(&s.id, s.revision, "revoke")
        .unwrap();
    assert!(r.revoked);
    assert_eq!(r.revision, 2);
    assert_eq!(
        f.store
            .revoke_factory_scope(&s.id, s.revision, "revoke")
            .unwrap()
            .revision,
        2
    );
    assert_eq!(
        f.store
            .revoke_factory_scope(&s.id, 2, "revoke")
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    let old: FactoryScope =
        serde_json::from_value(f.store.finalize_proposal(&s.id, 1).unwrap()).unwrap();
    assert!(!old.revoked);
    assert!(f.store.get_factory_scope(&s.id).unwrap().revoked);
}

#[test]
fn one_durable_run_replays_its_original_start_and_restart_never_upgrades_it_to_execution() {
    let mut f = Fixture::new();
    let s = f.scope();
    let before = f.store.sequence().unwrap();
    assert_eq!(
        f.store.get_factory_run(&s.id).unwrap_err().code,
        "NOT_FOUND"
    );
    assert_eq!(f.store.sequence().unwrap(), before);
    let run = f
        .store
        .start_factory_run(&s.id, s.revision, "start")
        .unwrap();
    assert_eq!(run.state, "queued");
    assert_eq!(
        f.store
            .start_factory_run(&s.id, s.revision, "start")
            .unwrap()
            .id,
        run.id
    );
    assert_eq!(
        f.store
            .start_factory_run(&s.id, s.revision, "new-start-key")
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    let details = f.store.get_factory_run(&s.id).unwrap();
    assert_eq!(details.tasks.len(), 1);
    assert!(details.tasks[0].parent_task_id.is_none());
    assert!(!details.tasks[0].confirmed_stop);
    let counts = f.store.factory_record_counts(&s.root_task_id).unwrap();
    assert_eq!(counts.runs, 1);
    assert_eq!(counts.task_lineage, 1);
    assert_eq!(counts.task_bindings, 1);
    assert_eq!(counts.attempts, 0);
    assert_eq!(counts.scoped_handles, 0);
    // A real Store close/reopen interrupts the prepared run. No child/Provider
    // started here; conservative unknown must not be promoted from a queued row.
    let path = f.path.clone();
    drop(f.store);
    f.store = Store::open(&path).unwrap();
    assert_eq!(
        f.store.get_factory_run(&s.id).unwrap().run.state,
        "outcome_unknown"
    );
    assert_eq!(
        f.store
            .start_factory_run(&s.id, s.revision, "start")
            .unwrap()
            .state,
        "queued"
    );
    f.store
        .revoke_factory_scope(&s.id, s.revision, "revoke")
        .unwrap();
    let mut retry = f.selection.clone();
    retry.expected_task_revision = f.store.get(&s.root_task_id).unwrap().revision;
    assert_eq!(
        f.store
            .propose_factory_scope(&retry, "not-a-reconciliation", &f.companion)
            .unwrap_err()
            .code,
        "OUTCOME_UNKNOWN"
    );
    assert_eq!(
        f.store.factory_record_counts(&s.root_task_id).unwrap(),
        counts
    );
}
#[test]
fn factory_start_journal_failure_rolls_back_run_lineage_binding_and_idempotency_without_resources()
{
    let mut f = Fixture::new();
    let s = f.scope();
    let db = Connection::open(&f.path).unwrap();
    db.execute_batch("CREATE TRIGGER reject_factory_start BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='factory.run_started' BEGIN SELECT RAISE(ABORT,'generated start journal fault'); END;").unwrap();
    assert!(f
        .store
        .start_factory_run(&s.id, s.revision, "start")
        .is_err());
    let counts = f.store.factory_record_counts(&s.root_task_id).unwrap();
    assert_eq!(counts.runs, 0);
    assert_eq!(counts.task_lineage, 0);
    assert_eq!(counts.task_bindings, 0);
    assert_eq!(counts.attempts, 0);
    assert_eq!(counts.scoped_handles, 0);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM idempotency WHERE scope=?1",
            [format!("factory:{}:start", s.id)],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM budgets", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
    db.execute_batch("DROP TRIGGER reject_factory_start")
        .unwrap();
    assert_eq!(
        f.store
            .start_factory_run(&s.id, s.revision, "start")
            .unwrap()
            .state,
        "queued"
    );
}
#[test]
fn factory_cannot_reset_existing_project_caps_or_an_unknown_shared_meter() {
    let mut f = Fixture::new();
    let project = f.store.get(&f.selection.root_task_id).unwrap().project_id;
    f.store.configure_budget(true, &project, 2, 1).unwrap();
    assert_eq!(
        f.store
            .propose_factory_scope(&f.selection, "over-project-cap", &f.companion)
            .unwrap_err()
            .code,
        "AUTH_REQUIRED"
    );
    let db = Connection::open(&f.path).unwrap();
    db.execute(
        "DELETE FROM budgets WHERE scope=?1",
        [format!("project:{project}")],
    )
    .unwrap();
    let meter = json!({"limits":f.selection.project_budget,"observed_attempts":0,"runtime_ms":null,"output_bytes":null,"tokens":null,"cost_microunits":null});
    db.execute(
        "INSERT INTO goal_ledgers(scope,document) VALUES(?1,?2)",
        params![format!("project:{project}"), meter.to_string()],
    )
    .unwrap();
    assert_eq!(
        f.store
            .propose_factory_scope(&f.selection, "unknown-meter", &f.companion)
            .unwrap_err()
            .code,
        "BUDGET_UNKNOWN"
    );
    assert_eq!(
        f.store
            .factory_record_counts(&f.selection.root_task_id)
            .unwrap()
            .scopes,
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT document FROM goal_ledgers WHERE scope=?1",
            [format!("project:{project}")],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        meter.to_string()
    );
}

#[test]
fn approved_factory_start_rejects_input_and_companion_drift_without_run_intent() {
    for drift in ["input", "companion"] {
        let mut f = Fixture::new();
        let s = f.scope();
        if drift == "input" {
            fs::write(
                s.bindings.workspaces[0].root.join("input.txt"),
                b"unapproved changed bytes\n",
            )
            .unwrap();
        } else {
            fs::write(&f.companion, b"unapproved companion bytes\n").unwrap();
        }
        let before = f.store.sequence().unwrap();
        let result = f
            .store
            .start_factory_run(&s.id, s.revision, "stale-binding");
        assert!(result.is_err(), "{drift} drift was admitted: {result:?}");
        assert_eq!(
            f.store.factory_record_counts(&s.root_task_id).unwrap().runs,
            0
        );
        assert_eq!(f.store.sequence().unwrap(), before);
    }
}

#[test]
fn stored_factory_expiry_cannot_outlive_the_native_confirmed_envelope() {
    let mut f = Fixture::new();
    let s = f.scope();
    let db = Connection::open(&f.path).unwrap();
    db.execute(
        "UPDATE factory_scopes SET document=json_set(document,'$.expires_at_ms',?1) WHERE id=?2",
        params![s.expires_at_ms + 60_000, s.id],
    )
    .unwrap();
    assert!(
        f.store.get_factory_scope(&s.id).is_err(),
        "corrupt expiry remained authoritative"
    );
    assert!(f
        .store
        .start_factory_run(&s.id, s.revision, "extended-expiry")
        .is_err());
    assert_eq!(
        f.store.factory_record_counts(&s.root_task_id).unwrap().runs,
        0
    );
}

#[test]
fn factory_commit_rechecks_selected_db_bindings_and_draft_even_if_a_journal_trigger_changes_them() {
    for boundary in ["finalize", "start"] {
        for target in ["task", "connection", "workspace", "project"] {
            let mut f = Fixture::new();
            let p = f.proposal();
            let s = if boundary == "start" {
                Some(
                    serde_json::from_value::<FactoryScope>(
                        f.store.finalize_proposal(&p.id, p.revision).unwrap(),
                    )
                    .unwrap(),
                )
            } else {
                None
            };
            let db = Connection::open(&f.path).unwrap();
            let mutation=match target {
                "task"=>format!("UPDATE tasks SET document=json_set(document,'$.state','running','$.revision',999) WHERE id='{}';",f.selection.root_task_id),
                "connection"=>"UPDATE connections SET document=json_set(document,'$.config_revision','changed-in-commit');".into(),
                "workspace"=>"UPDATE managed_workspaces SET document=json_set(document,'$.revision',999);".into(),
                _=>"UPDATE projects SET document=json_set(document,'$.revision',999);".into(),
            };
            let kind = if boundary == "start" {
                "factory.run_started"
            } else {
                "approval.confirmed"
            };
            db.execute_batch(&format!("CREATE TRIGGER concurrent_binding AFTER INSERT ON events WHEN json_extract(NEW.document,'$.kind')='{kind}' BEGIN {mutation} END;")).unwrap();
            let result = if let Some(s) = &s {
                f.store
                    .start_factory_run(&s.id, s.revision, "raced-start")
                    .map(|_| ())
            } else {
                f.store.finalize_proposal(&p.id, p.revision).map(|_| ())
            };
            assert!(result.is_err(), "{boundary}/{target} mutation was admitted");
            let counts = f
                .store
                .factory_record_counts(&f.selection.root_task_id)
                .unwrap();
            assert_eq!(counts.runs, 0, "{boundary}/{target}");
            assert_eq!(counts.scopes, u64::from(boundary == "start"));
            assert_eq!(
                f.store.get(&f.selection.root_task_id).unwrap().revision,
                f.selection.expected_task_revision
            );
        }
    }
}

#[test]
fn generated_factory_wire_bundle_comes_from_real_owner_commit_and_durable_start() {
    let mut f = Fixture::new();
    let proposal = f.proposal();
    let scope: FactoryScope = serde_json::from_value(
        f.store
            .finalize_proposal(&proposal.id, proposal.revision)
            .unwrap(),
    )
    .unwrap();
    f.store
        .start_factory_run(&scope.id, scope.revision, "export-run")
        .unwrap();
    let details = f.store.get_factory_run(&scope.id).unwrap();
    let bundle = json!({"provenance":"generated files; real Store native-owner finalizer fixture; no actual OS dialog or Provider execution","selection":f.selection,"proposal":proposal,"scope":scope,"run_details":details});
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/factory-runtime-core");
    fs::create_dir_all(&dir).unwrap();
    let bytes = serde_json::to_vec_pretty(&bundle).unwrap();
    let hash = sha256(&bytes);
    fs::write(dir.join(format!("wire-bundle-{hash}.json")), bytes).unwrap();
    eprintln!("FACTORY_WIRE_BUNDLE sha256={hash}");
}
