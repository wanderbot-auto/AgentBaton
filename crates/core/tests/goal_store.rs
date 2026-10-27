//! Real SQLite/native-finalizer/Unix IPC contracts with synthetic registered
//! connection fixtures. No Provider process, model, real owner dialog or account.
use agentbaton_core::{
    execution::StopEvidence,
    goal_plan::*,
    goal_store::*,
    ipc::{self, Daemon},
    policy::ExecutionGrant,
    storage::{Store, Task},
};
use agentbaton_workspaces::validation::{sha256, ArtifactRequirement};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@invalid")
        .args(["-c", "core.hooksPath=/dev/null", "-C"])
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
struct Fixture {
    _dir: tempfile::TempDir,
    runtime: PathBuf,
    db: PathBuf,
    root: PathBuf,
    store: Store,
    task: Task,
    grant: ExecutionGrant,
    selection: GoalScopeSelection,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let runtime = dir.path().canonicalize().unwrap().join("runtime");
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let db = runtime.join("baton.sqlite3");
        drop(
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&db)
                .unwrap(),
        );
        let source = dir.path().join("source");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        fs::write(source.join("input.txt"), "fixed input\n").unwrap();
        fs::write(source.join(".gitignore"), "outputs/\n").unwrap();
        git(&source, &["add", "input.txt", ".gitignore"]);
        git(&source, &["commit", "-m", "generated source"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let managed = runtime.join("workspaces");
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        let mut store = Store::open(&db).unwrap();
        let project = store
            .import_project(&source, "Goal fixture", "project")
            .unwrap();
        let workspace = store
            .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
            .unwrap();
        let task = store
            .create_registered(&project.id, "Produce an owner-reviewed report", "task")
            .unwrap();
        let connection = json!({"id":"fixture-connection","name":"Explicit synthetic ready connection; no Provider invoked","provider":"custom_acp","probe_state":"ready","host_id":"local","revision":1,"environment_id":"fixture-environment","identity_ref":"unknown-fixture","config_revision":"fixture-config","executable_sha256":"a".repeat(64),"capability_snapshot_id":"fixture-capability"});
        Connection::open(&db)
            .unwrap()
            .execute(
                "INSERT INTO connections VALUES(?1,?2)",
                params!["fixture-connection", connection.to_string()],
            )
            .unwrap();
        // Real persisted native proposal/finalizer path; owner confirmation is
        // explicitly simulated by this trusted Core fixture, not claimed GUI proof.
        let gp = store
            .propose_grant(
                &project.id,
                &workspace.id,
                "fixture-connection",
                8,
                2,
                3_600_000,
                "grant",
            )
            .unwrap();
        let grant: ExecutionGrant =
            serde_json::from_value(store.finalize_proposal(&gp.id, 1).unwrap()).unwrap();
        let root_budget = BudgetLimits {
            attempts: 8,
            parallel: 2,
            runtime_ms: 60_000,
            output_bytes: 8192,
            children: 3,
            reworks: 2,
            tokens: None,
            cost_microunits: None,
        };
        let selection=GoalScopeSelection {task_id:task.id.clone(),expected_task_revision:task.revision,grant_id:grant.id.clone(),expected_grant_revision:grant.revision,max_nodes:4,new_max_children:3,new_max_depth:2,root_budget:root_budget.clone(),project_budget:root_budget,inputs:vec![ArtifactRequirement {artifact_id:"source".into(),relative_path:"input.txt".into(),max_bytes:1024,expected_sha256:Some(sha256(b"fixed input\n"))}],outputs:vec![GoalOutputSelection {artifact:ArtifactRequirement {artifact_id:"report".into(),relative_path:"outputs/report.txt".into(),max_bytes:1024,expected_sha256:None},validators:vec![]}],writable_prefixes:vec!["outputs".into()],owner_acceptance_criteria:"Owner reviews the report against the exact requested outcome; approval alone never passes it".into()};
        Self {
            _dir: dir,
            runtime,
            db,
            root: workspace.root,
            store,
            task,
            grant,
            selection,
        }
    }
    fn sql(&self) -> Connection {
        Connection::open(&self.db).unwrap()
    }
    fn approve(&mut self) -> GoalScopeView {
        let p = self
            .store
            .propose_goal_scope(&self.selection, "goal-scope")
            .unwrap();
        assert_eq!(p.kind, "goal");
        serde_json::from_value(self.store.finalize_proposal(&p.id, 1).unwrap()).unwrap()
    }
    fn plan(view: &GoalScopeView) -> GoalPlanSubmission {
        let contract = view.outputs["report"].clone();
        let budget = BudgetLimits {
            attempts: 2,
            parallel: 1,
            runtime_ms: 1000,
            output_bytes: 1024,
            children: 0,
            reworks: 0,
            tokens: None,
            cost_microunits: None,
        };
        let node = PlanNode {
            id: "write-report".into(),
            parent_id: None,
            kind: NodeKind::Work,
            goal: "Write report using fixed input".into(),
            dependencies: vec![],
            workspace_id: view.workspace_id.clone(),
            executor: Executor::Connection {
                binding: Box::new(view.binding.clone()),
                program_sha256: view.program_sha256.clone(),
            },
            recipients: vec![],
            required_inputs: vec![RequiredInput {
                goal_task_id: view.task_id.clone(),
                artifact_id: "source".into(),
                contract_id: view.inputs["source"].contract_id.clone(),
                source: InputSource::ApprovedExternal {
                    sha256: view.inputs["source"].sha256.clone(),
                },
            }],
            outputs: vec![PlannedOutput {
                artifact_id: "report".into(),
                contract_id: contract.clone(),
            }],
            required_validators: view.contracts[&contract].clone(),
            side_effect: SideEffect::WorkspaceWrite,
            write_set: vec![WriteTarget::Workspace {
                path: "outputs/report.txt".into(),
            }],
            budget,
            reservation: AttemptReservation {
                runtime_ms: 500,
                output_bytes: 1024,
                tokens: None,
                cost_microunits: None,
            },
            retry: RetryPolicy {
                max_attempts: 2,
                max_reworks: 0,
                retry_classes: vec![FailureClass::BeforeDispatchTransient],
            },
            idempotency_key: "report-action".into(),
            external_action_id: None,
        };
        GoalPlanSubmission {
            scope_id: view.id.clone(),
            proposal: GoalPlanProposal {
                schema_version: 1,
                plan_id: "fixture-plan".into(),
                goal: view.goal.clone(),
                scope_revision: view.authority_revision,
                revision: 1,
                expected_revision: 0,
                acceptance_rules: view.required_acceptance_rules.clone(),
                nodes: vec![node],
            },
        }
    }
}
#[test]
fn native_goal_confirmation_is_distinct_and_updates_only_ceilings_atomically() {
    let mut f = Fixture::new();
    let p = f.store.propose_goal_scope(&f.selection, "pending").unwrap();
    assert_eq!(
        p.snapshot["resolved"]["grant_before"]["scope"]["max_children"],
        0
    );
    assert_eq!(
        p.snapshot["resolved"]["grant_after"]["scope"]["max_children"],
        3
    );
    assert_eq!(f.store.get_grant(&f.grant.id).unwrap(), f.grant);
    assert_eq!(native_grant_view(&p.snapshot, "grant_before"), f.grant);
    assert_eq!(
        f.sql()
            .query_row("SELECT count(*) FROM goal_scopes", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    let view: GoalScopeView =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    assert!(view.planning_only);
    let updated = f.store.get_grant(&f.grant.id).unwrap();
    assert_eq!(updated.revision, 2);
    assert_eq!(native_grant_view(&p.snapshot, "grant_after"), updated);
    assert_eq!(updated.scope.max_children, 3);
    assert_eq!(updated.scope.max_depth, 2);
    let mut only = updated.clone();
    only.revision = f.grant.revision;
    only.scope.max_children = 0;
    only.scope.max_depth = 0;
    assert_eq!(only, f.grant);
    assert_eq!(
        f.store.execution_grant_revision(&f.grant.id, 1).unwrap(),
        f.grant
    );
    let repeated = f.store.finalize_proposal(&p.id, 1).unwrap();
    assert_eq!(repeated["id"], view.id);
    assert_eq!(
        f.store.get(&f.task.id).unwrap().state,
        agentbaton_core::storage::TaskState::Draft
    );
    assert_eq!(
        f.sql()
            .query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
}
#[test]
fn persisted_plan_rehydrates_after_restart_and_preview_is_read_only() {
    let mut f = Fixture::new();
    let view = f.approve();
    let submission = Fixture::plan(&view);
    let saved = f.store.submit_goal_plan(&submission, "initial", 0).unwrap();
    let before = f.store.sequence().unwrap();
    let preview = f.store.preview_goal_ready(&view.id, 1).unwrap();
    assert!(preview.read_only);
    assert!(!preview.reservation_made);
    assert!(!preview.dispatched);
    let d = preview.decision.unwrap();
    assert_eq!(d.admissions.len(), 1);
    assert_eq!(d.admissions[0].resolved_inputs[0].artifact_id, "source");
    assert!(!d.required_checks_satisfied);
    assert_eq!(f.store.sequence().unwrap(), before);
    assert_eq!(
        f.store.budget(false, &f.task.id).unwrap().unwrap().consumed,
        0
    );
    f.store = Store::open(&f.db).unwrap();
    assert_eq!(
        f.store.get_goal_plan(&view.id).unwrap().digest,
        saved.digest
    );
    assert_eq!(
        f.store
            .preview_goal_ready(&view.id, 1)
            .unwrap()
            .decision
            .unwrap()
            .admissions
            .len(),
        1
    );
    assert_eq!(f.store.sequence().unwrap(), before);
}
#[test]
fn revision_cas_idempotency_and_journal_are_consistent() {
    let mut f = Fixture::new();
    let scope = f.approve();
    let first = Fixture::plan(&scope);
    let saved = f.store.submit_goal_plan(&first, "same", 0).unwrap();
    let sequence = f.store.sequence().unwrap();
    assert_eq!(
        f.store.submit_goal_plan(&first, "same", 0).unwrap().digest,
        saved.digest
    );
    assert_eq!(f.store.sequence().unwrap(), sequence);
    let mut next = first.clone();
    next.proposal.revision = 2;
    next.proposal.expected_revision = 1;
    next.proposal.nodes[0].goal = "Refined step, unchanged approved acceptance".into();
    assert_eq!(
        f.store.submit_goal_plan(&next, "same", 1).unwrap_err().code,
        "CONFIG_CONFLICT"
    );
    f.store.submit_goal_plan(&next, "revision-two", 1).unwrap();
    assert_eq!(
        f.store
            .submit_goal_plan(&next, "stale", 1)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    assert_eq!(
        f.store.preview_goal_ready(&scope.id, 1).unwrap_err().code,
        "STALE_REVISION"
    );
    assert_eq!(
        f.store.list_goal_plan_revisions(&scope.id).unwrap().len(),
        2
    );
}
#[test]
fn grant_and_goal_approval_roll_back_together_on_confirmation_event_failure() {
    let mut f = Fixture::new();
    let p = f
        .store
        .propose_goal_scope(&f.selection, "rollback")
        .unwrap();
    f.sql().execute_batch("CREATE TRIGGER reject_goal_confirmation BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='approval.confirmed' AND json_extract(NEW.document,'$.payload.kind')='goal' BEGIN SELECT RAISE(ABORT,'synthetic Goal failure'); END;").unwrap();
    assert!(f.store.finalize_proposal(&p.id, 1).is_err());
    assert_eq!(f.store.get_grant(&f.grant.id).unwrap(), f.grant);
    for table in ["goal_grant_revisions", "goal_scopes", "goal_ledgers"] {
        assert_eq!(
            f.sql()
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    assert!(f
        .sql()
        .query_row(
            "SELECT result IS NULL FROM approval_proposals WHERE id=?1",
            [&p.id],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
    f.sql()
        .execute_batch("DROP TRIGGER reject_goal_confirmation")
        .unwrap();
    assert!(f.store.finalize_proposal(&p.id, 1).is_ok());
}
#[test]
fn plan_event_failure_rolls_back_head_proposal_revision_and_idempotency() {
    let mut f = Fixture::new();
    let scope = f.approve();
    let p = Fixture::plan(&scope);
    f.sql().execute_batch("CREATE TRIGGER reject_plan BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='goal.plan_validated' BEGIN SELECT RAISE(ABORT,'synthetic plan failure'); END;").unwrap();
    assert!(f.store.submit_goal_plan(&p, "initial", 0).is_err());
    for table in [
        "goal_plan_heads",
        "goal_plan_proposals",
        "goal_plan_revisions",
    ] {
        assert_eq!(
            f.sql()
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    f.sql().execute_batch("DROP TRIGGER reject_plan").unwrap();
    assert!(f.store.submit_goal_plan(&p, "initial", 0).is_ok());
}
#[test]
fn stale_input_connection_grant_and_acceptance_changes_are_rejected() {
    let mut f = Fixture::new();
    let p = f
        .store
        .propose_goal_scope(&f.selection, "input-proposal")
        .unwrap();
    fs::write(f.root.join("input.txt"), "changed input\n").unwrap();
    assert_eq!(
        f.store.finalize_proposal(&p.id, 1).unwrap_err().code,
        "INPUT_CHANGED"
    );
    fs::write(f.root.join("input.txt"), "fixed input\n").unwrap();
    let scope: GoalScopeView =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    let mut plan = Fixture::plan(&scope);
    plan.proposal.acceptance_rules.clear();
    assert_eq!(
        f.store.submit_goal_plan(&plan, "weak", 0).unwrap_err().code,
        "PERMISSION_DENIED"
    );
    let plan = Fixture::plan(&scope);
    f.store.submit_goal_plan(&plan, "ok", 0).unwrap();
    f.sql().execute("UPDATE connections SET document=json_set(document,'$.identity_ref','changed-account') WHERE id='fixture-connection'",[]).unwrap();
    assert_eq!(
        f.store.preview_goal_ready(&scope.id, 1).unwrap_err().code,
        "INPUT_CHANGED"
    );
}
#[test]
fn revoke_goal_or_underlying_grant_blocks_preview_without_resetting_history() {
    let mut f = Fixture::new();
    let scope = f.approve();
    let p = Fixture::plan(&scope);
    f.store.submit_goal_plan(&p, "plan", 0).unwrap();
    let revoked = f.store.revoke_goal_scope(&scope.id, 1, "revoke").unwrap();
    assert!(revoked.revoked);
    assert_eq!(
        f.store.preview_goal_ready(&scope.id, 1).unwrap_err().code,
        "PERMISSION_DENIED"
    );
    assert!(f.store.finalize_proposal(&scope.id, 1).unwrap()["revoked"]
        .as_bool()
        .unwrap());
    assert_eq!(
        f.store.execution_grant_revision(&f.grant.id, 1).unwrap(),
        f.grant
    );
    assert_eq!(
        f.sql()
            .query_row("SELECT count(*) FROM goal_plan_revisions", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        1
    );
    let mut f = Fixture::new();
    let scope = f.approve();
    f.store
        .submit_goal_plan(&Fixture::plan(&scope), "plan", 0)
        .unwrap();
    f.store
        .revoke_grant(&f.grant.id, 2, "revoke-grant")
        .unwrap();
    assert_eq!(
        f.store.preview_goal_ready(&scope.id, 1).unwrap_err().code,
        "PERMISSION_DENIED"
    );
}
#[test]
fn unknown_historical_meter_never_becomes_zero_or_resets_with_new_plan() {
    let mut f = Fixture::new();
    f.store
        .configure_budget(true, &f.task.project_id, 8, 2)
        .unwrap();
    f.store.configure_budget(false, &f.task.id, 8, 2).unwrap();
    let g = &f.grant.connection_bindings["fixture-connection"];
    let binding = agentbaton_core::execution::ExecutionBinding {
        connection_id: "fixture-connection".into(),
        environment_id: g.environment_id.clone(),
        identity_ref: g.identity_ref.clone(),
        host_id: g.host_id.clone(),
        workspace_id: f.grant.scope.workspace_ids[0].clone(),
        config_revision: g.config_revision.clone(),
        input_revision: match &f.grant.workspace_bindings.values().next().unwrap().kind {
            agentbaton_workspaces::WorkspaceKind::Git { head, .. } => head.clone(),
            _ => panic!("actual Git fixture"),
        },
        capability_snapshot_id: g.capability_snapshot_id.clone(),
        policy_revision: format!("{}:1", f.grant.id),
    };
    let intent = f
        .store
        .prepare_attempt(&f.task.id, 1, binding, "legacy")
        .unwrap();
    f.store
        .confirm_stop(&intent.id, StopEvidence::NeverDispatched)
        .unwrap();
    f.selection.expected_task_revision = f.store.get(&f.task.id).unwrap().revision;
    let scope = f.approve();
    let mut p = Fixture::plan(&scope);
    f.store.submit_goal_plan(&p, "one", 0).unwrap();
    let preview = f.store.preview_goal_ready(&scope.id, 1).unwrap();
    assert!(preview.decision.is_none());
    assert!(preview
        .unknown_dimensions
        .contains(&"root.runtime_ms".into()));
    assert!(preview
        .blocked_reasons
        .contains(&"historical_runtime_or_output_ledger_unknown".into()));
    p.proposal.revision = 2;
    p.proposal.expected_revision = 1;
    f.store.submit_goal_plan(&p, "two", 1).unwrap();
    assert!(f
        .store
        .preview_goal_ready(&scope.id, 2)
        .unwrap()
        .decision
        .is_none());
    assert_eq!(
        f.store.budget(false, &f.task.id).unwrap().unwrap().consumed,
        1
    );
}
#[test]
fn direct_grant_consent_is_not_a_native_goal_approval() {
    let mut f = Fixture::new();
    let rogue = f
        .store
        .create_grant(
            &f.task.project_id,
            &f.grant.scope.workspace_ids[0],
            "fixture-connection",
            8,
            2,
            3_600_000,
            true,
            "trusted-core-no-native-proposal",
        )
        .unwrap();
    f.selection.grant_id = rogue.id;
    f.selection.expected_grant_revision = rogue.revision;
    assert_eq!(
        f.store
            .propose_goal_scope(&f.selection, "not-native")
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
}
#[test]
fn scope_and_plan_flags_counts_paths_and_raw_bounds_cannot_supply_authority() {
    let mut f = Fixture::new();
    let mut v = json!(f.selection);
    v["verified"] = true.into();
    assert!(serde_json::from_value::<GoalScopeSelection>(v).is_err());
    let mut bad = f.selection.clone();
    bad.outputs[0].artifact.relative_path = "../escape".into();
    assert!(f.store.propose_goal_scope(&bad, "escape").is_err());
    let mut bad = f.selection.clone();
    bad.root_budget.attempts = 9;
    assert!(f.store.propose_goal_scope(&bad, "more").is_err());
    let mut bad = f.selection.clone();
    bad.root_budget.tokens = Some(100);
    assert_eq!(
        f.store.propose_goal_scope(&bad, "meter").unwrap_err().code,
        "CAPABILITY_UNSUPPORTED"
    );
    let scope = f.approve();
    let mut p = Fixture::plan(&scope);
    let mut v = json!(p);
    v["proposal"]["passed"] = true.into();
    assert!(serde_json::from_value::<GoalPlanSubmission>(v).is_err());
    p.proposal.nodes[0].goal = "x".repeat(MAX_GOAL_PLAN_BYTES);
    assert_eq!(
        f.store.submit_goal_plan(&p, "huge", 0).unwrap_err().code,
        "RESOURCE_LIMIT"
    );
}
#[test]
fn retained_unmapped_goal_evidence_is_blocked_not_adopted_from_passed_json() {
    let mut f = Fixture::new();
    let scope = f.approve();
    f.store
        .submit_goal_plan(&Fixture::plan(&scope), "plan", 0)
        .unwrap();
    f.sql()
        .execute(
            "INSERT INTO goal_node_evidence VALUES(?1,'write-report',?2)",
            params![
                scope.id,
                json!({"passed":true,"state":"execution_succeeded"}).to_string()
            ],
        )
        .unwrap();
    let preview = f.store.preview_goal_ready(&scope.id, 1).unwrap();
    assert!(preview.decision.is_none());
    assert_eq!(
        preview.blocked_reasons,
        ["goal_dispatch_evidence_requires_scheduler_reconciliation_not_json_adoption"]
    );
}

struct Server {
    runtime: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new(runtime: &Path) -> Self {
        let daemon = Daemon::bind(runtime).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let child = stop.clone();
        let thread = std::thread::spawn(move || daemon.run(child).unwrap());
        Self {
            runtime: runtime.into(),
            stop,
            thread: Some(thread),
        }
    }
    fn call(&self, command: Value) -> Value {
        ipc::call(&self.runtime, command).unwrap()
    }
    fn raw(&self, command: Value) -> Value {
        let mut socket = UnixStream::connect(self.runtime.join("batond.sock")).unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        socket
            .set_write_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut bytes = serde_json::to_vec(&command).unwrap();
        bytes.push(b'\n');
        socket.write_all(&bytes).unwrap();
        let mut result = String::new();
        BufReader::new(socket).read_line(&mut result).unwrap();
        serde_json::from_str(&result).unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
#[test]
fn real_unix_ipc_goal_flow_roundtrips_original_plan_text_and_has_no_execution_endpoint() {
    let mut f = Fixture::new();
    let scope = f.approve();
    let mut plan = Fixture::plan(&scope);
    let mut owner = plan.proposal.nodes[0].clone();
    owner.id = "owner-review".into();
    owner.kind = NodeKind::OwnerApproval;
    owner.goal = "Owner reviews fixed report".into();
    owner.executor = Executor::Owner {
        rule_id: scope.required_acceptance_rules[0].clone(),
    };
    owner.dependencies = vec!["write-report".into()];
    owner.required_inputs = vec![RequiredInput {
        goal_task_id: scope.task_id.clone(),
        artifact_id: "report".into(),
        contract_id: scope.outputs["report"].clone(),
        source: InputSource::Node {
            node_id: "write-report".into(),
            candidate_for_validation: true,
        },
    }];
    owner.outputs.clear();
    owner.side_effect = SideEffect::AdvisoryReadOnly;
    owner.write_set.clear();
    owner.idempotency_key = "owner-review-step".into();
    plan.proposal.nodes.push(owner);
    let raw = serde_json::to_string_pretty(&plan.proposal).unwrap();
    let server = Server::new(&f.runtime);
    assert_eq!(
        server.call(json!({"method":"goal_plans.list","params":{"scope_id":scope.id}}))["result"],
        json!([])
    );
    let saved=server.call(json!({"method":"goal_plans.propose","params":{"scope_id":scope.id,"plan_json":raw},"idempotency_key":"ipc-plan","expected_revision":0}));
    assert!(saved.get("error").is_none(), "{saved}");
    assert_eq!(
        saved["result"]["proposal"]["nodes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let before = server.call(json!({"method":"events.subscribe","params":{"cursor":0,"limit":1}}))
        ["result"]["sequence"]
        .clone();
    let preview=server.call(json!({"method":"goal_plans.ready_preview","params":{"scope_id":scope.id},"expected_revision":1}));
    assert_eq!(preview["result"]["read_only"], true);
    assert_eq!(preview["result"]["reservation_made"], false);
    assert_eq!(preview["result"]["budget_usage"]["root"]["attempts"], 0);
    assert_eq!(
        preview["result"]["decision"]["admissions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        preview["result"]["decision"]["required_checks_satisfied"],
        false
    );
    let after = server.call(json!({"method":"events.subscribe","params":{"cursor":0,"limit":1}}))
        ["result"]["sequence"]
        .clone();
    assert_eq!(before, after);
    assert_eq!(
        server.call(json!({"method":"goal_plans.dispatch","params":{"scope_id":scope.id}}))
            ["error"]["code"],
        "AUTH_REQUIRED"
    );
    assert_eq!(
        server.call(
            json!({"method":"goal_scopes.approve","params":{"scope_id":scope.id,"approved":true}})
        )["error"]["code"],
        "AUTH_REQUIRED"
    );
    assert_eq!(
        f.store.budget(false, &f.task.id).unwrap().unwrap().consumed,
        0
    );
    drop(server);
    let restarted = Server::new(&f.runtime);
    assert_eq!(
        restarted.call(json!({"method":"goal_plans.get","params":{"scope_id":scope.id}}))["result"]
            ["digest"],
        saved["result"]["digest"]
    );
    let revoked = restarted.call(json!({"method":"goal_scopes.revoke","params":{"scope_id":scope.id},"expected_revision":1,"idempotency_key":"ipc-revoke"}));
    assert_eq!(revoked["result"]["revoked"], true);
    let after_revoke = f.store.sequence().unwrap();
    assert_eq!(
        restarted.call(json!({"method":"goal_plans.list","params":{"scope_id":scope.id}}))
            ["result"][0]["digest"],
        saved["result"]["digest"]
    );
    assert_eq!(
        restarted.call(json!({"method":"goal_plans.get","params":{"scope_id":scope.id}}))["result"]
            ["digest"],
        saved["result"]["digest"]
    );
    assert_eq!(restarted.call(json!({"method":"goal_plans.ready_preview","params":{"scope_id":scope.id},"expected_revision":1}))["error"]["code"], "PERMISSION_DENIED");
    assert_eq!(f.store.sequence().unwrap(), after_revoke);
}
#[test]
fn actual_raw_unix_duplicate_revision_and_nested_node_keys_are_rejected_without_journal() {
    let mut f = Fixture::new();
    let scope = f.approve();
    let p = Fixture::plan(&scope);
    let good = serde_json::to_string(&p.proposal).unwrap();
    let server = Server::new(&f.runtime);
    let baseline = f.store.sequence().unwrap();
    let duplicate_revision = good.replacen("\"revision\":1", "\"revision\":1,\"revision\":1", 1);
    let duplicate_node = good.replacen(
        "\"id\":\"write-report\"",
        "\"id\":\"write-report\",\"id\":\"write-report\"",
        1,
    );
    let unknown = good.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"passed\":true",
        1,
    );
    for (i, text) in [duplicate_revision, duplicate_node, unknown]
        .into_iter()
        .enumerate()
    {
        assert_ne!(text, good);
        let token = fs::read_to_string(f.runtime.join("session.token")).unwrap();
        let response=server.raw(json!({"api_version":1,"request_id":format!("duplicate-{i}"),"token":token,"method":"goal_plans.propose","params":{"scope_id":scope.id,"plan_json":text},"expected_revision":0,"idempotency_key":format!("duplicate-{i}")}));
        assert_eq!(response["error"]["code"], "INVALID_REQUEST", "{response}");
    }
    assert_eq!(f.store.sequence().unwrap(), baseline);
    assert_eq!(
        f.sql()
            .query_row("SELECT count(*) FROM goal_plan_heads", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    let valid=server.call(json!({"method":"goal_plans.propose","params":{"scope_id":scope.id,"plan_json":good},"expected_revision":0,"idempotency_key":"actual-good"}));
    assert!(valid.get("error").is_none(), "{valid}");
}
#[test]
fn raw_plan_size_and_exact_text_idempotency_are_real() {
    let mut f = Fixture::new();
    let scope = f.approve();
    let p = Fixture::plan(&scope);
    let raw = serde_json::to_string(&p.proposal).unwrap();
    let first = GoalPlanRawSubmission {
        scope_id: scope.id.clone(),
        plan_json: raw.clone(),
    };
    let saved = f.store.submit_goal_plan_json(&first, "raw", 0).unwrap();
    assert_eq!(
        f.store
            .submit_goal_plan_json(&first, "raw", 0)
            .unwrap()
            .digest,
        saved.digest
    );
    let mut whitespace = first.clone();
    whitespace.plan_json = " ".to_string() + &raw;
    assert_eq!(
        f.store
            .submit_goal_plan_json(&whitespace, "raw", 0)
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    let too_big = GoalPlanRawSubmission {
        scope_id: scope.id,
        plan_json: " ".repeat(MAX_GOAL_PLAN_BYTES + 1) + &raw,
    };
    assert_eq!(
        f.store
            .submit_goal_plan_json(&too_big, "big", 0)
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
}

#[test]
fn lost_goal_finalize_response_replays_same_proposal_before_stale_grant_resolution() {
    let mut f = Fixture::new();
    let p = f.store.propose_goal_scope(&f.selection, "lost").unwrap();
    let result = f.store.finalize_proposal(&p.id, 1).unwrap();
    let sequence = f.store.sequence().unwrap();
    assert_eq!(f.store.get_grant(&f.grant.id).unwrap().revision, 2);
    let replay = f.store.propose_goal_scope(&f.selection, "lost").unwrap();
    assert_eq!(replay.id, p.id);
    assert_eq!(f.store.finalize_proposal(&replay.id, 1).unwrap(), result);
    assert_eq!(f.store.sequence().unwrap(), sequence);
    let mut different = f.selection.clone();
    different.owner_acceptance_criteria.push_str(" changed");
    assert_eq!(
        f.store
            .propose_goal_scope(&different, "lost")
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    assert_eq!(
        f.store
            .propose_goal_scope(&f.selection, "fresh-key-still-stale")
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
}
#[test]
fn later_unmapped_legacy_attempt_makes_previously_zero_meter_unknown() {
    let mut f = Fixture::new();
    let scope = f.approve();
    f.store
        .submit_goal_plan(&Fixture::plan(&scope), "plan", 0)
        .unwrap();
    assert_eq!(
        f.store
            .preview_goal_ready(&scope.id, 1)
            .unwrap()
            .budget_usage
            .root
            .runtime_ms,
        Some(0)
    );
    let intent = f
        .store
        .prepare_attempt(
            &f.task.id,
            f.store.get(&f.task.id).unwrap().revision,
            scope.binding.clone(),
            "later-legacy",
        )
        .unwrap();
    f.store
        .confirm_stop(&intent.id, StopEvidence::NeverDispatched)
        .unwrap();
    let preview = f.store.preview_goal_ready(&scope.id, 1).unwrap();
    assert_eq!(preview.budget_usage.root.attempts, 1);
    assert_eq!(preview.budget_usage.root.runtime_ms, None);
    assert!(preview.decision.is_none());
    assert_eq!(
        f.store.budget(false, &f.task.id).unwrap().unwrap().consumed,
        1
    );
}
#[test]
fn related_prepared_intent_and_even_dangling_command_fence_block_goal_approval() {
    let mut f = Fixture::new();
    let g = &f.grant.connection_bindings["fixture-connection"];
    let head = match &f.grant.workspace_bindings.values().next().unwrap().kind {
        agentbaton_workspaces::WorkspaceKind::Git { head, .. } => head.clone(),
        _ => panic!("Git fixture"),
    };
    let binding = agentbaton_core::execution::ExecutionBinding {
        connection_id: "fixture-connection".into(),
        environment_id: g.environment_id.clone(),
        identity_ref: g.identity_ref.clone(),
        host_id: g.host_id.clone(),
        workspace_id: f.grant.scope.workspace_ids[0].clone(),
        config_revision: g.config_revision.clone(),
        input_revision: head,
        capability_snapshot_id: g.capability_snapshot_id.clone(),
        policy_revision: format!("{}:1", f.grant.id),
    };
    f.store
        .configure_budget(true, &f.task.project_id, 8, 2)
        .unwrap();
    f.store.configure_budget(false, &f.task.id, 8, 2).unwrap();
    let intent = f
        .store
        .prepare_attempt(&f.task.id, 1, binding, "active-intent")
        .unwrap();
    f.selection.expected_task_revision = f.store.get(&f.task.id).unwrap().revision;
    assert_eq!(
        f.store
            .propose_goal_scope(&f.selection, "busy")
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    f.store
        .confirm_stop(&intent.id, StopEvidence::NeverDispatched)
        .unwrap();
    f.selection.expected_task_revision = f.store.get(&f.task.id).unwrap().revision;
    // Deliberately corrupt/dangling fence remains conservative rather than
    // becoming a generic trusted-reader exception. No command process is run.
    let fault_db = f.sql();
    fault_db.pragma_update(None, "foreign_keys", false).unwrap();
    fault_db
        .execute(
            "INSERT INTO command_workspace_fences VALUES(?1,'missing-run-must-still-block')",
            [&f.grant.scope.workspace_ids[0]],
        )
        .unwrap();
    fault_db.pragma_update(None, "foreign_keys", true).unwrap();
    assert_eq!(
        f.store
            .propose_goal_scope(&f.selection, "fenced")
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    assert_eq!(f.store.get_grant(&f.grant.id).unwrap().revision, 1);
}
#[test]
fn additional_native_goal_revision_preserves_full_grant_history_and_old_attempt_refs() {
    let mut f = Fixture::new();
    let first = f.approve();
    let revision2 = f.store.execution_grant_revision(&f.grant.id, 2).unwrap();
    f.store
        .revoke_goal_scope(&first.id, 1, "revoke-first")
        .unwrap();
    let mut next = f.selection.clone();
    next.expected_grant_revision = 2;
    next.new_max_children = 5;
    next.new_max_depth = 3;
    next.max_nodes = 6;
    let p = f.store.propose_goal_scope(&next, "second-goal").unwrap();
    let second: GoalScopeView =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    assert_eq!(second.root_grant_revision, 3);
    assert_eq!(
        f.store.execution_grant_revision(&f.grant.id, 1).unwrap(),
        f.grant
    );
    assert_eq!(
        f.store.execution_grant_revision(&f.grant.id, 2).unwrap(),
        revision2
    );
    assert!(f.store.get_goal_scope(&first.id).unwrap().revoked);
    assert_eq!(
        f.store.budget(false, &f.task.id).unwrap().unwrap().consumed,
        0
    );
}
#[test]
fn corrupted_oversize_stored_plan_is_not_decoded_or_silently_dropped() {
    let mut f = Fixture::new();
    let scope = f.approve();
    f.store
        .submit_goal_plan(&Fixture::plan(&scope), "plan", 0)
        .unwrap();
    f.sql()
        .execute(
            "UPDATE goal_plan_revisions SET document=?1",
            ["x".repeat(MAX_GOAL_PLAN_BYTES + 1)],
        )
        .unwrap();
    assert_eq!(
        f.store.get_goal_plan(&scope.id).unwrap_err().code,
        "RESOURCE_LIMIT"
    );
    assert_eq!(
        f.store
            .list_goal_plan_revisions(&scope.id)
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
}

#[test]
fn old_scope_evidence_cannot_be_bypassed_by_revision_revocation_or_new_scope_key() {
    let mut f = Fixture::new();
    let scope = f.approve();
    let mut plan = Fixture::plan(&scope);
    f.store.submit_goal_plan(&plan, "one", 0).unwrap();
    f.sql()
        .execute(
            "INSERT INTO goal_node_evidence VALUES(?1,'write-report',?2)",
            params![
                scope.id,
                json!({"state":"outcome_unknown","attempt_id":"retained-unknown"}).to_string()
            ],
        )
        .unwrap();
    let sequence = f.store.sequence().unwrap();
    plan.proposal.revision = 2;
    plan.proposal.expected_revision = 1;
    assert_eq!(
        f.store.submit_goal_plan(&plan, "two", 1).unwrap_err().code,
        "WORKSPACE_BUSY"
    );
    assert_eq!(f.store.sequence().unwrap(), sequence);
    assert!(f
        .store
        .preview_goal_ready(&scope.id, 1)
        .unwrap()
        .decision
        .is_none());
    f.store.revoke_goal_scope(&scope.id, 1, "revoke").unwrap();
    f.selection.expected_grant_revision = 2;
    assert_eq!(
        f.store
            .propose_goal_scope(&f.selection, "new-scope-key")
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    assert_eq!(
        f.sql()
            .query_row("SELECT count(*) FROM goal_node_evidence", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        1
    );
}
#[test]
fn committed_plan_replay_is_historical_data_after_revocation_not_new_authority() {
    let mut f = Fixture::new();
    let scope = f.approve();
    let plan = Fixture::plan(&scope);
    let stored = f.store.submit_goal_plan(&plan, "one", 0).unwrap();
    f.store.revoke_goal_scope(&scope.id, 1, "revoke").unwrap();
    let sequence = f.store.sequence().unwrap();
    assert_eq!(
        f.store.submit_goal_plan(&plan, "one", 0).unwrap().digest,
        stored.digest
    );
    assert_eq!(f.store.sequence().unwrap(), sequence);
    assert_eq!(
        f.store
            .submit_goal_plan(&plan, "brand-new-key", 0)
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    assert_eq!(
        f.store.preview_goal_ready(&scope.id, 1).unwrap_err().code,
        "PERMISSION_DENIED"
    );
}

#[test]
fn same_root_grant_consumption_in_another_task_is_not_reset_without_session_mapping() {
    let mut f = Fixture::new();
    let other = f
        .store
        .create_registered(
            &f.task.project_id,
            "Other root user of same grant",
            "other-task",
        )
        .unwrap();
    f.selection.project_budget.attempts = 16;
    f.store
        .configure_budget(true, &f.task.project_id, 16, 2)
        .unwrap();
    f.store.configure_budget(false, &other.id, 8, 2).unwrap();
    let c = &f.grant.connection_bindings["fixture-connection"];
    let input = match &f.grant.workspace_bindings.values().next().unwrap().kind {
        agentbaton_workspaces::WorkspaceKind::Git { head, .. } => head.clone(),
        _ => panic!("Git fixture"),
    };
    let binding = agentbaton_core::execution::ExecutionBinding {
        connection_id: "fixture-connection".into(),
        environment_id: c.environment_id.clone(),
        identity_ref: c.identity_ref.clone(),
        host_id: c.host_id.clone(),
        workspace_id: f.grant.scope.workspace_ids[0].clone(),
        config_revision: c.config_revision.clone(),
        input_revision: input,
        capability_snapshot_id: c.capability_snapshot_id.clone(),
        policy_revision: format!("{}:1", f.grant.id),
    };
    let attempt = f
        .store
        .prepare_attempt(&other.id, 1, binding, "other-attempt")
        .unwrap();
    f.store
        .confirm_stop(&attempt.id, StopEvidence::NeverDispatched)
        .unwrap();
    let proposal = f
        .store
        .propose_goal_scope(&f.selection, "shared-root")
        .unwrap();
    assert_eq!(proposal.snapshot["resolved"]["root_consumed"], 1);
    let scope: GoalScopeView =
        serde_json::from_value(f.store.finalize_proposal(&proposal.id, 1).unwrap()).unwrap();
    assert_eq!(
        f.store.budget(false, &f.task.id).unwrap().unwrap().consumed,
        1,
        "actual future-admission row must inherit shared root use"
    );
    assert_eq!(
        f.store
            .budget(true, &f.task.project_id)
            .unwrap()
            .unwrap()
            .consumed,
        1
    );
    f.store
        .submit_goal_plan(&Fixture::plan(&scope), "plan", 0)
        .unwrap();
    let preview = f.store.preview_goal_ready(&scope.id, 1).unwrap();
    assert_eq!(preview.budget_usage.root.attempts, 1);
    assert_eq!(preview.budget_usage.project.attempts, 1);
    assert_eq!(preview.budget_usage.root.runtime_ms, None);
    assert!(preview.decision.is_none());
    // Project still has room, so only the actual inherited root row can prevent
    // eight new attempts from spending a ninth shared-root attempt.
    for i in 0..7 {
        let expected = f.store.get(&f.task.id).unwrap().revision;
        let a = f
            .store
            .prepare_attempt(
                &f.task.id,
                expected,
                scope.binding.clone(),
                &format!("new-root-{i}"),
            )
            .unwrap();
        f.store
            .confirm_stop(&a.id, StopEvidence::NeverDispatched)
            .unwrap();
    }
    let expected = f.store.get(&f.task.id).unwrap().revision;
    assert_eq!(
        f.store
            .prepare_attempt(
                &f.task.id,
                expected,
                scope.binding.clone(),
                "ninth-shared-root"
            )
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    assert_eq!(
        f.store.budget(false, &f.task.id).unwrap().unwrap().consumed,
        8
    );
    assert_eq!(
        f.store
            .budget(true, &f.task.project_id)
            .unwrap()
            .unwrap()
            .consumed,
        8
    );
}

fn stopped_policy_session(f: &mut Fixture) -> String {
    f.store
        .configure_budget(true, &f.task.project_id, 8, 2)
        .unwrap();
    f.store.configure_budget(false, &f.task.id, 8, 2).unwrap();
    let workspace = f.grant.scope.workspace_ids[0].clone();
    let lease = f
        .store
        .acquire_workspace_lease(&workspace, "policy-session", 0, 60000)
        .unwrap();
    let c = &f.grant.connection_bindings["fixture-connection"];
    let input = match &f.grant.workspace_bindings[&workspace].kind {
        agentbaton_workspaces::WorkspaceKind::Git { head, .. } => head.clone(),
        _ => panic!("Git fixture"),
    };
    let binding = agentbaton_core::execution::ExecutionBinding {
        connection_id: "fixture-connection".into(),
        environment_id: c.environment_id.clone(),
        identity_ref: c.identity_ref.clone(),
        host_id: c.host_id.clone(),
        workspace_id: workspace.clone(),
        config_revision: c.config_revision.clone(),
        input_revision: input,
        capability_snapshot_id: c.capability_snapshot_id.clone(),
        policy_revision: format!("{}:1", f.grant.id),
    };
    let intent = f
        .store
        .prepare_attempt(&f.task.id, 1, binding, "policy-attempt")
        .unwrap();
    f.store
        .confirm_stop(&intent.id, StopEvidence::NeverDispatched)
        .unwrap();
    f.store
        .release_workspace_lease(&workspace, "policy-session", lease.fencing_token)
        .unwrap();
    let session = agentbaton_core::sessions::ManagedSession {
        id: "policy-session".into(),
        task_id: f.task.id.clone(),
        project_id: f.task.project_id.clone(),
        workspace_id: workspace,
        connection_id: "fixture-connection".into(),
        grant_id: f.grant.id.clone(),
        state: "failed".into(),
        revision: 1,
        native_session_id: None,
        created_at: "2026-10-27T00:00:00Z".into(),
        updated_at: "2026-10-27T00:00:00Z".into(),
        last_error: Some("Explicit synthetic never-dispatched session".into()),
        turn_state: None,
        attempt_id: intent.attempt_id,
        intent_id: intent.id,
        fencing_token: lease.fencing_token,
        runtime_snapshot: None,
    };
    f.sql()
        .execute(
            "INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6,?6,?7,?8,?9,?10)",
            params![
                session.id,
                session.task_id,
                session.project_id,
                session.workspace_id,
                session.connection_id,
                session.grant_id,
                session.state,
                serde_json::to_string(&session).unwrap(),
                "policy-session-key",
                "fixture-only"
            ],
        )
        .unwrap();
    fs::create_dir_all(f.root.join("outputs")).unwrap();
    fs::write(f.root.join("outputs/report.txt"), "generated report\n").unwrap();
    f.selection.expected_task_revision = f.store.get(&f.task.id).unwrap().revision;
    session.id
}
#[test]
fn actual_native_integrity_definition_is_additional_rule_never_goal_acceptance() {
    let mut f = Fixture::new();
    let session = stopped_policy_session(&mut f);
    let selection = agentbaton_core::validation::ValidationSelection {
        session_id: session,
        inputs: f.selection.inputs.clone(),
        outputs: vec![f.selection.outputs[0].artifact.clone()],
        analysis: None,
    };
    let p = f
        .store
        .propose_validation_policy(&selection, "integrity-proposal")
        .unwrap();
    let policy: agentbaton_core::validation::ValidationPolicy =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    let actual = f
        .store
        .run_artifact_validation(&policy.id, 1, "actual-integrity")
        .unwrap();
    assert_eq!(actual.state, "integrity_passed");
    f.selection.outputs[0].validators.push(GoalValidatorRef {
        kind: GoalValidatorKind::Integrity,
        policy_id: policy.id.clone(),
        revision: 1,
    });
    let scope = f.approve();
    assert_eq!(scope.required_acceptance_rules, ["goal-owner-review"]);
    assert!(scope.contracts["output:report"].contains(&format!("integrity:{}", policy.id)));
    assert!(scope.contracts["output:report"].contains(&"goal-owner-review".into()));
    f.store
        .submit_goal_plan(&Fixture::plan(&scope), "plan", 0)
        .unwrap();
    assert!(f
        .store
        .preview_goal_ready(&scope.id, 1)
        .unwrap()
        .decision
        .is_none());
    assert_eq!(
        f.store.get(&f.task.id).unwrap().state,
        agentbaton_core::storage::TaskState::Draft
    );
    f.store.revoke_validation_policy(&policy.id, 1).unwrap();
    assert_eq!(
        f.store.get_goal_plan(&scope.id).unwrap().proposal.revision,
        1
    );
    assert_eq!(
        f.store.preview_goal_ready(&scope.id, 1).unwrap_err().code,
        "INPUT_CHANGED"
    );
    let mut next = Fixture::plan(&scope);
    next.proposal.expected_revision = 1;
    next.proposal.revision = 2;
    assert_eq!(
        f.store
            .submit_goal_plan(&next, "revoked-source-new-action", 1)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
}
#[test]
fn actual_native_fixed_command_definition_projection_is_lossless_and_consumed_run_is_not_renewed() {
    let mut f = Fixture::new();
    let session = stopped_policy_session(&mut f);
    let script = b"printf generated-report\\n\n";
    fs::write(f.root.join("outputs/check.sh"), script).unwrap();
    let selection = agentbaton_core::command_validation::CommandValidationSelection {
        session_id: session,
        executable: Path::new("/bin/sh")
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
            .into(),
        mode: "posix_sh_stdin".into(),
        script: Some(ArtifactRequirement {
            artifact_id: "check-script".into(),
            relative_path: "outputs/check.sh".into(),
            max_bytes: 1024,
            expected_sha256: Some(sha256(script)),
        }),
        inputs: f.selection.inputs.clone(),
        outputs: vec![f.selection.outputs[0].artifact.clone()],
        arguments: vec![],
        environment: std::collections::BTreeMap::new(),
        expected_exit_codes: vec![0],
        wall_time_ms: 100,
        cleanup_time_ms: 100,
        stdout_limit: 1024,
        stderr_limit: 1024,
        require: Default::default(),
    };
    let p = f
        .store
        .propose_command_validation_policy(&selection, "command-proposal")
        .unwrap();
    let policy: agentbaton_core::command_validation::CommandValidationPolicy =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    f.selection.outputs[0].validators.push(GoalValidatorRef {
        kind: GoalValidatorKind::Command,
        policy_id: policy.id.clone(),
        revision: 1,
    });
    let goal_proposal = f
        .store
        .propose_goal_scope(&f.selection, "goal-scope")
        .unwrap();
    let approved_contract: Value = serde_json::from_str(
        goal_proposal.snapshot["resolved"]["validators"][0]["approved_contract_json"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(approved_contract, policy.contract);
    assert!(
        serde_json::to_string_pretty(&goal_proposal.snapshot)
            .unwrap()
            .len()
            <= 16384
    );
    assert_eq!(
        goal_proposal.snapshot["resolved"]["validators"][0]["source_runs_started"],
        0
    );
    // Alter a permission-bearing contract in the persisted proposal, not merely
    // a display label. Finalization must still compare actual original records.
    let mut tampered = goal_proposal.clone();
    let mut changed_contract = approved_contract.clone();
    changed_contract["arguments"] = json!(["--different-permission"]);
    tampered.snapshot["resolved"]["validators"][0]["approved_contract_json"] =
        serde_json::to_string(&changed_contract).unwrap().into();
    f.sql()
        .execute(
            "UPDATE approval_proposals SET document=?1 WHERE id=?2",
            params![serde_json::to_string(&tampered).unwrap(), goal_proposal.id],
        )
        .unwrap();
    assert_eq!(
        f.store
            .finalize_proposal(&goal_proposal.id, 1)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    assert_eq!(f.store.get_grant(&f.grant.id).unwrap().revision, 1);
    f.sql()
        .execute(
            "UPDATE approval_proposals SET document=?1 WHERE id=?2",
            params![
                serde_json::to_string(&goal_proposal).unwrap(),
                goal_proposal.id
            ],
        )
        .unwrap();
    let mut source_drift = serde_json::to_value(&policy).unwrap();
    source_drift["contract"]["require"]["network_isolation"] = true.into();
    f.sql()
        .execute(
            "UPDATE command_validation_policies SET document=?1 WHERE id=?2",
            params![source_drift.to_string(), policy.id],
        )
        .unwrap();
    assert_eq!(
        f.store
            .finalize_proposal(&goal_proposal.id, 1)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    f.sql()
        .execute(
            "UPDATE command_validation_policies SET document=?1 WHERE id=?2",
            params![serde_json::to_string(&policy).unwrap(), policy.id],
        )
        .unwrap();
    let scope = f.approve();
    assert!(scope.contracts["output:report"].contains(&format!("command:{}", policy.id)));
    assert_eq!(scope.required_acceptance_rules, ["goal-owner-review"]);
    assert_eq!(
        f.sql()
            .query_row("SELECT count(*) FROM command_validation_runs", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.store.get(&f.task.id).unwrap().state,
        agentbaton_core::storage::TaskState::Draft
    );
    let plan = Fixture::plan(&scope);
    let saved = f
        .store
        .submit_goal_plan(&plan, "plan-before-command-use", 0)
        .unwrap();
    let mut workers = agentbaton_core::command_workers::CommandWorkers::default();
    let run = workers
        .start(&mut f.store, &policy.id, 1, "actual-source-once")
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    while workers.has_workers() {
        assert!(
            std::time::Instant::now() < deadline,
            "owned generated source command did not finish"
        );
        workers.tick(&mut f.store).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        f.store
            .get_command_validation_record(&run.id)
            .unwrap()
            .state,
        "command_passed"
    );
    assert_eq!(
        f.store.get_goal_plan(&scope.id).unwrap().digest,
        saved.digest
    );
    assert_eq!(
        f.store.preview_goal_ready(&scope.id, 1).unwrap_err().code,
        "CAPABILITY_UNSUPPORTED"
    );
    let mut next = plan;
    next.proposal.expected_revision = 1;
    next.proposal.revision = 2;
    assert_eq!(
        f.store
            .submit_goal_plan(&next, "used-source-new-action", 1)
            .unwrap_err()
            .code,
        "CAPABILITY_UNSUPPORTED"
    );
    f.selection.expected_grant_revision = 2;
    f.selection.expected_task_revision = f.store.get(&f.task.id).unwrap().revision;
    assert_eq!(
        f.store
            .propose_goal_scope(&f.selection, "fresh-cannot-renew-source-run")
            .unwrap_err()
            .code,
        "CAPABILITY_UNSUPPORTED"
    );
    assert_eq!(
        f.sql()
            .query_row("SELECT count(*) FROM command_validation_runs", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        1
    );
}

fn native_grant_view(snapshot: &Value, which: &str) -> ExecutionGrant {
    let resolved = &snapshot["resolved"];
    let diff = &resolved[which];
    assert_eq!(diff["immutable_ref"], "resolved.grant_immutable");
    let mut full = resolved["grant_immutable"].clone();
    full["revision"] = diff["revision"].clone();
    full["scope"]["max_children"] = diff["scope"]["max_children"].clone();
    full["scope"]["max_depth"] = diff["scope"]["max_depth"].clone();
    serde_json::from_value(full).unwrap()
}

#[test]
fn oversize_existing_meter_cannot_become_an_absent_fresh_zero_ledger() {
    let mut f = Fixture::new();
    let scope = f.approve();
    f.store
        .submit_goal_plan(&Fixture::plan(&scope), "plan", 0)
        .unwrap();
    f.sql()
        .execute(
            "UPDATE goal_ledgers SET document=?1 WHERE scope=?2",
            params!["x".repeat(1_048_577), format!("root-grant:{}", f.grant.id)],
        )
        .unwrap();
    assert_eq!(
        f.store.preview_goal_ready(&scope.id, 1).unwrap_err().code,
        "RESOURCE_LIMIT"
    );
    assert_eq!(
        f.sql()
            .query_row(
                "SELECT octet_length(document) FROM goal_ledgers WHERE scope=?1",
                [format!("root-grant:{}", f.grant.id)],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1_048_577
    );
}

#[test]
fn actual_budget_history_reconciliation_preserves_limits_and_rolls_back_atomically() {
    let mut f = Fixture::new();
    stopped_policy_session(&mut f);
    // Simulate an admission ledger restored without its already durable attempt
    // count. Facts are recovered from actual attempts, not proposal JSON.
    f.sql()
        .execute("UPDATE budgets SET consumed=0", [])
        .unwrap();
    f.selection.root_budget.attempts = 4;
    f.selection.root_budget.parallel = 1;
    let p = f
        .store
        .propose_goal_scope(&f.selection, "reconcile")
        .unwrap();
    assert_eq!(p.snapshot["resolved"]["root_consumed"], 1);
    assert_eq!(p.snapshot["resolved"]["project_consumed"], 1);
    let sequence = f.store.sequence().unwrap();
    f.sql().execute_batch("CREATE TRIGGER reject_reconciled_goal BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='approval.confirmed' AND json_extract(NEW.document,'$.payload.kind')='goal' BEGIN SELECT RAISE(ABORT,'synthetic Goal rollback after budget reconciliation'); END;").unwrap();
    assert!(f.store.finalize_proposal(&p.id, 1).is_err());
    assert_eq!(f.store.sequence().unwrap(), sequence);
    assert_eq!(f.store.get_grant(&f.grant.id).unwrap(), f.grant);
    for (project, id) in [(true, &f.task.project_id), (false, &f.task.id)] {
        let b = f.store.budget(project, id).unwrap().unwrap();
        assert_eq!(
            (b.total_limit, b.parallel_limit, b.consumed, b.active),
            (8, 2, 0, 0)
        );
    }
    f.sql()
        .execute_batch("DROP TRIGGER reject_reconciled_goal")
        .unwrap();
    f.store.finalize_proposal(&p.id, 1).unwrap();
    for (project, id) in [(true, &f.task.project_id), (false, &f.task.id)] {
        let b = f.store.budget(project, id).unwrap().unwrap();
        // Existing ceilings remain intact even when this Goal is narrower.
        assert_eq!(
            (b.total_limit, b.parallel_limit, b.consumed, b.active),
            (8, 2, 1, 0)
        );
    }
    let reconciliation_events: u32 = f.sql().query_row(
        "SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.budget_history_reconciled' AND json_extract(document,'$.payload.goal_approval_id')=?1",
        [&p.id], |r| r.get(0),
    ).unwrap();
    assert_eq!(reconciliation_events, 2);
    let after = f.store.sequence().unwrap();
    f.store.finalize_proposal(&p.id, 1).unwrap();
    assert_eq!(f.store.sequence().unwrap(), after);
}

#[test]
fn oversize_idempotency_payload_or_result_is_never_treated_as_absent() {
    let mut f = Fixture::new();
    let proposal = f.store.propose_goal_scope(&f.selection, "scope").unwrap();
    let sequence = f.store.sequence().unwrap();
    for field in ["payload", "result"] {
        let original: String = f.sql().query_row(
            &format!("SELECT {field} FROM idempotency WHERE scope='local:approvals.propose:goal' AND key='scope'"),
            [], |r| r.get(0),
        ).unwrap();
        f.sql().execute(
            &format!("UPDATE idempotency SET {field}=?1 WHERE scope='local:approvals.propose:goal' AND key='scope'"),
            ["x".repeat(1_048_577)],
        ).unwrap();
        assert_eq!(
            f.store
                .propose_goal_scope(&f.selection, "scope")
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        assert_eq!(f.store.sequence().unwrap(), sequence);
        f.sql().execute(
            &format!("UPDATE idempotency SET {field}=?1 WHERE scope='local:approvals.propose:goal' AND key='scope'"),
            [original],
        ).unwrap();
    }
    let scope: GoalScopeView =
        serde_json::from_value(f.store.finalize_proposal(&proposal.id, 1).unwrap()).unwrap();
    let submission = Fixture::plan(&scope);
    f.store.submit_goal_plan(&submission, "plan", 0).unwrap();
    let sequence = f.store.sequence().unwrap();
    let idem = format!("goal:{}:plan", scope.id);
    for field in ["payload", "result"] {
        let original: String = f
            .sql()
            .query_row(
                &format!("SELECT {field} FROM idempotency WHERE scope=?1 AND key='plan'"),
                [&idem],
                |r| r.get(0),
            )
            .unwrap();
        f.sql()
            .execute(
                &format!("UPDATE idempotency SET {field}=?1 WHERE scope=?2 AND key='plan'"),
                params!["x".repeat(1_048_577), idem],
            )
            .unwrap();
        assert_eq!(
            f.store
                .submit_goal_plan(&submission, "plan", 0)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        assert_eq!(f.store.sequence().unwrap(), sequence);
        assert_eq!(
            f.store.list_goal_plan_revisions(&scope.id).unwrap().len(),
            1
        );
        f.sql()
            .execute(
                &format!("UPDATE idempotency SET {field}=?1 WHERE scope=?2 AND key='plan'"),
                params![original, idem],
            )
            .unwrap();
    }
}

#[test]
fn fresh_scope_lists_real_empty_plan_history_before_first_submission() {
    let mut f = Fixture::new();
    let scope = f.approve();
    let sequence = f.store.sequence().unwrap();
    assert!(f
        .store
        .list_goal_plan_revisions(&scope.id)
        .unwrap()
        .is_empty());
    assert_eq!(
        f.store.get_goal_plan(&scope.id).unwrap_err().code,
        "NOT_FOUND"
    );
    assert_eq!(f.store.sequence().unwrap(), sequence);
    f.store
        .submit_goal_plan(&Fixture::plan(&scope), "first", 0)
        .unwrap();
    assert_eq!(
        f.store.list_goal_plan_revisions(&scope.id).unwrap().len(),
        1
    );
}

#[test]
fn immutable_plan_history_remains_readable_without_reviving_current_authority() {
    for change in [
        "scope_revoke",
        "grant_revoke",
        "grant_expiry",
        "input_drift",
        "task_spec",
    ] {
        let mut f = Fixture::new();
        let scope = f.approve();
        let first = Fixture::plan(&scope);
        f.store.submit_goal_plan(&first, "first", 0).unwrap();
        let mut second = first.clone();
        second.proposal.expected_revision = 1;
        second.proposal.revision = 2;
        second.proposal.nodes[0].goal = "Refine the same approved report".into();
        let saved = f.store.submit_goal_plan(&second, "second", 1).unwrap();
        match change {
            "scope_revoke" => {
                f.store.revoke_goal_scope(&scope.id, 1, "revoke").unwrap();
            }
            "grant_revoke" => {
                f.store.revoke_grant(&f.grant.id, 2, "revoke").unwrap();
            }
            "grant_expiry" => {
                f.sql().execute("UPDATE execution_grants SET document=json_set(document,'$.expires_at_ms',0) WHERE id=?1", [&f.grant.id]).unwrap();
            }
            "input_drift" => {
                fs::write(f.root.join("input.txt"), "Changed after proposal commit\n").unwrap();
            }
            "task_spec" => {
                f.sql().execute("UPDATE tasks SET document=json_set(document,'$.spec_revision',2,'$.goal','New unapproved specification') WHERE id=?1", [&f.task.id]).unwrap();
            }
            _ => unreachable!(),
        }
        let sequence = f.store.sequence().unwrap();
        let history = f.store.list_goal_plan_revisions(&scope.id).unwrap();
        assert_eq!(history.len(), 2, "{change}");
        assert_eq!(history[0].proposal.revision, 1);
        assert_eq!(history[1].digest, saved.digest);
        assert_eq!(
            f.store.get_goal_plan(&scope.id).unwrap().digest,
            saved.digest
        );
        assert_eq!(
            f.store.sequence().unwrap(),
            sequence,
            "historical reads do not authorize/reserve: {change}"
        );
        let mut third = second.clone();
        third.proposal.expected_revision = 2;
        third.proposal.revision = 3;
        let expected = if ["input_drift", "task_spec"].contains(&change) {
            "INPUT_CHANGED"
        } else {
            "PERMISSION_DENIED"
        };
        assert_eq!(
            f.store
                .submit_goal_plan(&third, "new-action", 2)
                .unwrap_err()
                .code,
            expected,
            "{change}"
        );
        assert_eq!(
            f.store.preview_goal_ready(&scope.id, 2).unwrap_err().code,
            expected,
            "{change}"
        );
        assert_eq!(f.store.sequence().unwrap(), sequence);
        assert_eq!(
            f.store.budget(false, &f.task.id).unwrap().unwrap().consumed,
            0
        );
    }
}

#[test]
fn historical_reads_reject_orphans_and_tampered_commitments_instead_of_empty_success() {
    for corruption in [
        "no_head",
        "no_revisions",
        "no_proposals",
        "revision_plan_id",
        "proposal_plan_id",
        "proposal_body",
        "previous_digest",
        "head_digest",
        "plan_journal",
        "scope_journal",
        "scope_columns",
        "scope_approval",
        "oversize_proposal",
    ] {
        let mut f = Fixture::new();
        let scope = f.approve();
        f.store
            .submit_goal_plan(&Fixture::plan(&scope), "plan", 0)
            .unwrap();
        f.store.revoke_goal_scope(&scope.id, 1, "revoke").unwrap();
        match corruption {
            "no_head" => {
                f.sql().execute("DELETE FROM goal_plan_heads", []).unwrap();
            }
            "no_revisions" => {
                f.sql()
                    .execute("DELETE FROM goal_plan_revisions", [])
                    .unwrap();
            }
            "no_proposals" => {
                f.sql()
                    .execute("DELETE FROM goal_plan_proposals", [])
                    .unwrap();
            }
            "revision_plan_id" => {
                f.sql()
                    .execute(
                        "UPDATE goal_plan_revisions SET plan_id='uncommitted-other-plan'",
                        [],
                    )
                    .unwrap();
            }
            "proposal_plan_id" => {
                f.sql()
                    .execute(
                        "UPDATE goal_plan_proposals SET plan_id='uncommitted-other-plan'",
                        [],
                    )
                    .unwrap();
            }
            "proposal_body" => {
                f.sql().execute("UPDATE goal_plan_proposals SET document=json_set(document,'$.proposal.nodes[0].goal','Uncommitted replacement')", []).unwrap();
            }
            "previous_digest" => {
                f.sql()
                    .execute(
                        "UPDATE goal_plan_revisions SET previous_digest='uncommitted-predecessor'",
                        [],
                    )
                    .unwrap();
            }
            "head_digest" => {
                f.sql()
                    .execute("UPDATE goal_plan_heads SET digest='uncommitted-digest'", [])
                    .unwrap();
            }
            "plan_journal" => {
                f.sql().execute("DELETE FROM events WHERE json_extract(document,'$.kind')='goal.plan_validated'", []).unwrap();
            }
            "scope_journal" => {
                f.sql().execute("DELETE FROM events WHERE json_extract(document,'$.kind')='goal.scope_approved'", []).unwrap();
            }
            "scope_columns" => {
                f.sql()
                    .execute("UPDATE goal_scopes SET revision=99", [])
                    .unwrap();
            }
            "scope_approval" => {
                f.sql().execute("UPDATE approval_proposals SET document=json_set(document,'$.snapshot.intent.owner_acceptance_criteria','Uncommitted acceptance') WHERE id=?1", [&scope.id]).unwrap();
            }
            "oversize_proposal" => {
                f.sql()
                    .execute(
                        "UPDATE goal_plan_proposals SET document=?1",
                        ["x".repeat(1_048_577)],
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let sequence = f.store.sequence().unwrap();
        let expected = if corruption == "oversize_proposal" {
            "RESOURCE_LIMIT"
        } else {
            "INPUT_CHANGED"
        };
        assert_eq!(
            f.store
                .list_goal_plan_revisions(&scope.id)
                .unwrap_err()
                .code,
            expected,
            "{corruption}"
        );
        assert_eq!(
            f.store.get_goal_plan(&scope.id).unwrap_err().code,
            expected,
            "{corruption}"
        );
        assert_eq!(f.store.sequence().unwrap(), sequence);
    }
    // With every plan table empty, a retained plan-validated journal is still
    // evidence of corrupt/missing history, not an unused fresh Goal.
    let mut f = Fixture::new();
    let scope = f.approve();
    f.store
        .submit_goal_plan(&Fixture::plan(&scope), "plan", 0)
        .unwrap();
    f.sql().execute_batch("DELETE FROM goal_plan_heads;DELETE FROM goal_plan_revisions;DELETE FROM goal_plan_proposals;").unwrap();
    assert_eq!(
        f.store
            .list_goal_plan_revisions(&scope.id)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
}
