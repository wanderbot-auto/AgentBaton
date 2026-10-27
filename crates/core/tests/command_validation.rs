//! Explicit synthetic stopped-session storage fixtures; only generated local
//! validator scripts run. No Provider/model/account/native-dialog is invoked.
use agentbaton_core::{
    command_validation::*,
    command_workers::CommandWorkers,
    execution::{ExecutionBinding, StopEvidence},
    ipc::{self, Daemon},
    sessions::ManagedSession,
    storage::{Store, TaskState},
};
use agentbaton_workspaces::validation::{sha256, ArtifactRequirement};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
struct Fixture {
    _dir: tempfile::TempDir,
    runtime: PathBuf,
    db: PathBuf,
    root: PathBuf,
    store: Store,
    selection: CommandValidationSelection,
    task: String,
    workspace: String,
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("/usr/bin/git")
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
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
impl Fixture {
    fn new(script: &str) -> Self {
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
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&db)
                .unwrap(),
        );
        let source = dir.path().join("source");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        fs::write(source.join("input.txt"), "fixed input\n").unwrap();
        fs::write(
            source.join(".gitignore"),
            "result.txt\nmarker\nescape-release\nescaped-finished\n",
        )
        .unwrap();
        fs::write(source.join(".validator.sh"), script).unwrap();
        git(
            &source,
            &["add", "input.txt", ".gitignore", ".validator.sh"],
        );
        git(&source, &["commit", "-m", "generated input"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let managed = runtime.join("workspaces");
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        let mut store = Store::open(&db).unwrap();
        let project = store
            .import_project(&source, "Synthetic project", "import")
            .unwrap();
        let workspace = store
            .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
            .unwrap();
        let task = store
            .create_registered(&project.id, "Validate local command", "task")
            .unwrap();
        let conn = Connection::open(&db).unwrap();
        let connection = json!({"id":"fixture-connection","name":"Synthetic; no Provider","provider":"custom_acp","probe_state":"ready","host_id":"local","revision":1,"environment_id":"fixture-env","identity_ref":"unknown-fixture","config_revision":"fixture-config","executable_sha256":"0".repeat(64),"capability_snapshot_id":"fixture-capability"});
        conn.execute(
            "INSERT INTO connections VALUES(?1,?2)",
            params!["fixture-connection", connection.to_string()],
        )
        .unwrap();
        let grant = store
            .create_grant(
                &project.id,
                &workspace.id,
                "fixture-connection",
                4,
                1,
                60_000,
                true,
                "grant",
            )
            .unwrap();
        let c = &grant.connection_bindings["fixture-connection"];
        let binding = ExecutionBinding {
            connection_id: "fixture-connection".into(),
            environment_id: c.environment_id.clone(),
            identity_ref: c.identity_ref.clone(),
            host_id: c.host_id.clone(),
            workspace_id: workspace.id.clone(),
            config_revision: c.config_revision.clone(),
            input_revision: head,
            capability_snapshot_id: c.capability_snapshot_id.clone(),
            policy_revision: format!("{}:1", grant.id),
        };
        store.configure_budget(true, &project.id, 4, 1).unwrap();
        store.configure_budget(false, &task.id, 4, 1).unwrap();
        let lease = store
            .acquire_workspace_lease(&workspace.id, "synthetic-session", 0, 60_000)
            .unwrap();
        let intent = store
            .prepare_attempt(&task.id, task.revision, binding, "attempt")
            .unwrap();
        store
            .confirm_stop(&intent.id, StopEvidence::NeverDispatched)
            .unwrap();
        store
            .release_workspace_lease(&workspace.id, "synthetic-session", lease.fencing_token)
            .unwrap();
        let session = ManagedSession {
            id: "synthetic-session".into(),
            task_id: task.id.clone(),
            project_id: project.id,
            workspace_id: workspace.id.clone(),
            connection_id: "fixture-connection".into(),
            grant_id: grant.id.clone(),
            state: "failed".into(),
            revision: 1,
            native_session_id: None,
            created_at: task.created_at.clone(),
            updated_at: task.created_at.clone(),
            last_error: Some("Synthetic never-dispatched fixture".into()),
            turn_state: None,
            attempt_id: intent.attempt_id,
            intent_id: intent.id,
            fencing_token: lease.fencing_token,
            runtime_snapshot: None,
        };
        conn.execute(
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
                "fixture-session-key",
                "fixture-only"
            ],
        )
        .unwrap();
        drop(conn);
        fs::write(workspace.root.join(".validator.sh"), script).unwrap();
        let requirement = |id: &str, path: &str, bytes: &[u8]| ArtifactRequirement {
            artifact_id: id.into(),
            relative_path: path.into(),
            max_bytes: 4096,
            expected_sha256: Some(sha256(bytes)),
        };
        let selection = CommandValidationSelection {
            session_id: session.id,
            executable: Path::new("/bin/sh")
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
                .into(),
            mode: "posix_sh_stdin".into(),
            script: Some(requirement(
                "validator-script",
                ".validator.sh",
                script.as_bytes(),
            )),
            inputs: vec![requirement("input", "input.txt", b"fixed input\n")],
            outputs: vec![requirement("output", "result.txt", b"ok")],
            arguments: vec![],
            environment: BTreeMap::from([("LANG".into(), "C".into()), ("TZ".into(), "UTC".into())]),
            expected_exit_codes: vec![0],
            wall_time_ms: 1000,
            cleanup_time_ms: 100,
            stdout_limit: 4096,
            stderr_limit: 4096,
            require: Default::default(),
        };
        Self {
            _dir: dir,
            runtime,
            db,
            root: workspace.root,
            store,
            selection,
            task: task.id,
            workspace: workspace.id,
        }
    }
    fn approve(&mut self, key: &str) -> CommandValidationPolicy {
        let p = self
            .store
            .propose_command_validation_policy(&self.selection, key)
            .unwrap();
        assert_eq!(p.kind, "command_validation");
        serde_json::from_value(self.store.finalize_proposal(&p.id, p.revision).unwrap()).unwrap()
    }
    fn finished(&mut self, workers: &mut CommandWorkers, id: &str) -> CommandValidationRecord {
        let end = Instant::now() + Duration::from_secs(3);
        loop {
            workers.tick(&mut self.store).unwrap();
            let r = self.store.get_command_validation_record(id).unwrap();
            if !matches!(
                r.state.as_str(),
                "prepared" | "running" | "cancel_requested"
            ) {
                return r;
            }
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
#[test]
fn real_script_policy_worker_and_idempotent_history_are_scoped_and_persistent() {
    let mut f = Fixture::new("printf ok > result.txt\nprintf hello\nprintf diag >&2\n");
    let p = f.approve("proposal");
    assert_eq!(p.max_runs, 1);
    let mut w = CommandWorkers::default();
    let prepared = w.start(&mut f.store, &p.id, 1, "run").unwrap();
    assert_eq!(prepared.state, "prepared");
    let r = f.finished(&mut w, &prepared.id);
    assert_eq!(r.state, "command_passed");
    assert_eq!(r.evidence.as_ref().unwrap()["stdout"]["captured_bytes"], 5);
    assert_eq!(r.evidence.as_ref().unwrap()["stderr"]["captured_bytes"], 4);
    assert_eq!(r.task_acceptance, "not_established");
    assert!(!r.integration_authorized);
    assert!(!serde_json::to_string(&r).unwrap().contains("hello"));
    assert_eq!(w.start(&mut f.store, &p.id, 1, "run").unwrap().id, r.id);
    assert_eq!(
        w.start(&mut f.store, &p.id, 2, "run").unwrap_err().code,
        "CONFIG_CONFLICT"
    );
    assert_eq!(
        w.start(&mut f.store, &p.id, 1, "new-key").unwrap_err().code,
        "PERMISSION_DENIED"
    );
    let conn = Connection::open(&f.db).unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM command_workspace_fences", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
    let reopened = Store::open(&f.db).unwrap();
    assert_eq!(
        reopened.get_command_validation_record(&r.id).unwrap().state,
        "command_passed"
    );
    assert_eq!(reopened.get(&f.task).unwrap().state, TaskState::Draft);
}
#[test]
fn wrong_hash_and_unavailable_enforcement_fail_before_spawn_and_are_recorded() {
    let mut f = Fixture::new("printf ok > result.txt\n");
    let p = f.approve("proposal");
    fs::write(f.root.join(".validator.sh"), "printf unsafe > marker\n").unwrap();
    let mut w = CommandWorkers::default();
    assert_eq!(
        w.start(&mut f.store, &p.id, 1, "run").unwrap_err().code,
        "INPUT_CHANGED"
    );
    assert!(!f.root.join("marker").exists());
    assert!(f
        .store
        .list_command_validation_records(&f.task)
        .unwrap()
        .is_empty());
    let mut f = Fixture::new("printf unsafe > marker\n");
    f.selection.require.network_isolation = true;
    let p = f.approve("proposal");
    let r = w.start(&mut f.store, &p.id, 1, "run").unwrap();
    let r = f.finished(&mut w, &r.id);
    assert_eq!(r.state, "command_failed");
    assert_eq!(r.last_error.unwrap().code, "ENFORCEMENT_UNAVAILABLE");
    assert!(!f.root.join("marker").exists());
}
#[test]
fn timeout_cancel_and_revoke_keep_the_actor_control_path_live() {
    for operation in ["timeout", "cancel", "revoke", "task_cancel"] {
        let mut f = Fixture::new("printf started > marker\nwhile :; do :; done\n");
        if operation == "timeout" {
            f.selection.wall_time_ms = 30;
        }
        let p = f.approve("proposal");
        let mut w = CommandWorkers::default();
        let r = w.start(&mut f.store, &p.id, 1, "run").unwrap();
        let end = Instant::now() + Duration::from_secs(1);
        while operation != "timeout" && !f.root.join("marker").exists() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        }
        match operation {
            "cancel" => {
                let cancel = f.store.get_command_validation_record(&r.id).unwrap();
                assert!(cancel.revision >= r.revision); // Exercise actual IPC below; Store mutators are crate-private.
                w.cancel(&r.id);
            }
            "revoke" => {
                f.store
                    .revoke_command_validation_policy(&p.id, 1, "revoke")
                    .unwrap();
                w.cancel_policy(&f.store, &p.id).unwrap();
            }
            "task_cancel" => {
                let task = f.store.get(&f.task).unwrap();
                assert_eq!(
                    f.store.cancel(&f.task, task.revision).unwrap().state,
                    TaskState::CancelRequested
                );
            }
            _ => (),
        };
        let r = f.finished(&mut w, &r.id);
        assert_ne!(r.state, "command_passed");
        if operation == "task_cancel" {
            assert_eq!(f.store.get(&f.task).unwrap().state, TaskState::Cancelled);
        }
        assert!(r.evidence.unwrap()["cleanup"]["owned_leader_reaped"]
            .as_bool()
            .unwrap());
    }
}
#[test]
fn forbidden_environment_and_forged_fields_never_create_an_owner_proposal() {
    let mut f = Fixture::new("printf unsafe > marker\n");
    f.selection
        .environment
        .insert("API_KEY".into(), "not-a-credential-fixture".into());
    assert_eq!(
        f.store
            .propose_command_validation_policy(&f.selection, "bad")
            .unwrap_err()
            .code,
        "CAPABILITY_UNSUPPORTED"
    );
    let mut forged = json!(f.selection);
    forged["root"] = json!("/tmp");
    assert!(serde_json::from_value::<CommandValidationSelection>(forged).is_err());
    assert!(f
        .store
        .list_command_validation_policies(&f.task)
        .unwrap()
        .is_empty());
}
#[test]
fn actual_unix_ipc_run_cancel_and_replay_preserve_stale_observation_safety() {
    let f = Fixture::new("printf started > marker\nwhile :; do :; done\n");
    let selection = json!(f.selection);
    let runtime = f.runtime.clone();
    let task = f.task.clone();
    let root = f.root.clone();
    drop(f.store);
    let daemon = Daemon::bind(&runtime).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let control = stop.clone();
    let thread = std::thread::spawn(move || daemon.run(control).unwrap());
    let call = |request: Value| ipc::call(&runtime, request).unwrap();
    let proposal = call(
        json!({"method":"command_validation_policies.propose","params":selection,"idempotency_key":"proposal"}),
    );
    assert!(proposal.get("error").is_none(), "{proposal}");
    let p = &proposal["result"];
    let approval = call(
        json!({"method":"approvals.finalize","params":{"proposal_id":p["id"]},"expected_revision":p["revision"],"idempotency_key":"confirm"}),
    );
    let policy = &approval["result"];
    assert!(approval.get("error").is_none(), "{approval}");
    let request = json!({"method":"command_validations.run","params":{"policy_id":policy["id"]},"expected_revision":1,"idempotency_key":"run"});
    let response = call(request.clone());
    assert!(response.get("error").is_none(), "{response}");
    let r = &response["result"];
    let id = r["id"].as_str().unwrap();
    let end = Instant::now() + Duration::from_secs(1);
    while !root.join("marker").exists() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    let start = Instant::now();
    let status = call(json!({"method":"command_validations.get","params":{"run_id":id}}));
    assert!(start.elapsed() < Duration::from_millis(500));
    assert_eq!(status["result"]["state"], "running");
    let cancel = json!({"method":"command_validations.cancel","params":{"run_id":id},"expected_revision":1,"idempotency_key":"cancel"});
    let a = call(cancel.clone());
    assert_eq!(a["result"]["state"], "cancel_requested");
    assert_eq!(call(cancel)["result"]["id"], id);
    let end = Instant::now() + Duration::from_secs(2);
    loop {
        let r = call(json!({"method":"command_validations.get","params":{"run_id":id}}));
        if !matches!(
            r["result"]["state"].as_str(),
            Some("running" | "cancel_requested" | "prepared")
        ) {
            assert_ne!(r["result"]["state"], "command_passed");
            break;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(call(request)["result"]["id"], id);
    let p2 = call(
        json!({"method":"command_validation_policies.propose","params":selection,"idempotency_key":"second-policy"}),
    );
    let p2 = call(
        json!({"method":"approvals.finalize","params":{"proposal_id":p2["result"]["id"]},"expected_revision":1}),
    );
    let second = call(
        json!({"method":"command_validations.run","params":{"policy_id":p2["result"]["id"]},"expected_revision":1,"idempotency_key":"second-run"}),
    );
    assert!(second.get("error").is_none(), "{second}");
    let current = call(json!({"method":"tasks.get","params":{"task_id":task}}));
    let requested = call(
        json!({"method":"tasks.cancel","params":{"task_id":task},"expected_revision":current["result"]["revision"]}),
    );
    assert_eq!(requested["result"]["state"], "cancel_requested");
    let end = Instant::now() + Duration::from_secs(2);
    loop {
        let state = call(json!({"method":"tasks.get","params":{"task_id":task}}));
        if state["result"]["state"] == "cancelled" {
            break;
        }
        assert!(Instant::now() < end, "{state}");
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut forged = selection;
    forged["approval_id"] = json!("self-authorized");
    assert_eq!(
        call(
            json!({"method":"command_validation_policies.propose","params":forged,"idempotency_key":"forged"})
        )["error"]["code"],
        "INVALID_REQUEST"
    );
    assert_eq!(
        call(json!({"method":"command_validations.list","params":{"task_id":task}}))["result"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
}

#[test]
fn tracked_input_and_head_changes_require_a_new_owner_policy_without_dispatch() {
    for drift in ["input", "head"] {
        let mut f = Fixture::new("printf unsafe > marker\nprintf ok > result.txt\n");
        let p = f.approve("proposal");
        match drift {
            "input" => fs::write(f.root.join("input.txt"), "changed input").unwrap(),
            _ => {
                git(
                    &f.root,
                    &["commit", "--allow-empty", "-m", "changed candidate"],
                );
            }
        }
        let mut w = CommandWorkers::default();
        assert_eq!(
            w.start(&mut f.store, &p.id, 1, "run").unwrap_err().code,
            "INPUT_CHANGED"
        );
        assert!(!f.root.join("marker").exists());
        assert!(f
            .store
            .list_command_validation_records(&f.task)
            .unwrap()
            .is_empty());
    }
}
#[test]
fn durable_intent_restart_is_unknown_and_fences_new_keys_policies_and_provider_writers() {
    let mut f = Fixture::new("printf unsafe > marker\n");
    let p = f.approve("proposal");
    let new_policy = f.approve("new-proposal");
    let timestamp = f.store.get(&f.task).unwrap().created_at;
    let row = CommandValidationRecord {
        id: "synthetic-unexecuted-intent".into(),
        policy_id: p.id.clone(),
        task_id: f.task.clone(),
        session_id: p.session_id.clone(),
        workspace_id: f.workspace.clone(),
        attempt_id: p.attempt_id.clone(),
        revision: 1,
        state: "prepared".into(),
        scope: p.scope.clone(),
        correctness: "approved_fixed_command_observations_only".into(),
        task_acceptance: "not_established".into(),
        integration_authorized: false,
        evidence: None,
        last_error: None,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    };
    let conn = Connection::open(&f.db).unwrap();
    conn.execute(
        "INSERT INTO command_validation_runs VALUES(?1,?2,?3,?4,'prepared',?5,'lost',?6)",
        params![
            row.id,
            p.id,
            f.task,
            f.workspace,
            serde_json::to_string(&row).unwrap(),
            json!({"policy_id":p.id,"revision":1}).to_string()
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO command_workspace_fences VALUES(?1,?2)",
        params![f.workspace, row.id],
    )
    .unwrap();
    drop(conn);
    drop(f.store);
    f.store = Store::open(&f.db).unwrap();
    let recovered = f.store.get_command_validation_record(&row.id).unwrap();
    assert_eq!(recovered.state, "outcome_unknown");
    assert_eq!(
        recovered.last_error.as_ref().unwrap().code,
        "OUTCOME_UNKNOWN"
    );
    let mut w = CommandWorkers::default();
    assert_eq!(
        w.start(&mut f.store, &p.id, 1, "lost").unwrap().state,
        "outcome_unknown"
    );
    assert_eq!(
        w.start(&mut f.store, &p.id, 1, "new-key").unwrap_err().code,
        "PERMISSION_DENIED"
    );
    assert_eq!(
        f.store
            .propose_command_validation_policy(&f.selection, "after-unknown")
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    let integrity = agentbaton_core::validation::ValidationSelection {
        session_id: p.session_id.clone(),
        inputs: f.selection.inputs.clone(),
        outputs: f.selection.outputs.clone(),
        analysis: None,
    };
    assert_eq!(
        f.store
            .propose_validation_policy(&integrity, "unreconciled-read")
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    assert_eq!(
        w.start(&mut f.store, &new_policy.id, 1, "fresh-run")
            .unwrap_err()
            .code,
        "OUTCOME_UNKNOWN"
    );
    let s = f.store.get_session(&p.session_id).unwrap();
    let task = f.store.get(&f.task).unwrap();
    assert_eq!(
        f.store
            .create_session(
                &f.task,
                &f.workspace,
                &s.connection_id,
                &s.grant_id,
                task.revision,
                "new-session"
            )
            .unwrap_err()
            .code,
        "OUTCOME_UNKNOWN"
    );
    assert!(f
        .store
        .acquire_workspace_lease(&f.workspace, "rogue-next-writer", p.fencing_token, 60_000)
        .is_err());
    assert_eq!(
        f.store
            .workspace_lease(&f.workspace)
            .unwrap()
            .unwrap()
            .fencing_token,
        p.fencing_token
    );
    assert!(!f.root.join("marker").exists());
    let runtime = f.runtime.clone();
    drop(f.store);
    let daemon = Daemon::bind(&runtime).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || daemon.run(flag).unwrap());
    let cancel=ipc::call(&runtime,json!({"method":"command_validations.cancel","params":{"run_id":row.id},"expected_revision":recovered.revision,"idempotency_key":"cancel-unknown"})).unwrap();
    assert_eq!(cancel["result"]["state"], "outcome_unknown");
    assert!(cancel["result"]["evidence"].is_null());
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
}
#[test]
fn journal_failure_rolls_back_intent_key_and_fence_before_any_program_runs() {
    let mut f = Fixture::new("printf unsafe > marker\nprintf ok > result.txt\n");
    let p = f.approve("proposal");
    let conn = Connection::open(&f.db).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_command_journal BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'generated journal fault');END;").unwrap();
    let mut w = CommandWorkers::default();
    assert!(w.start(&mut f.store, &p.id, 1, "run").is_err());
    assert!(!w.has_workers());
    assert!(f
        .store
        .list_command_validation_records(&f.task)
        .unwrap()
        .is_empty());
    assert_eq!(
        conn.query_row("SELECT count(*) FROM command_workspace_fences", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
    assert!(!f.root.join("marker").exists());
    conn.execute_batch("DROP TRIGGER reject_command_journal;")
        .unwrap();
    let r = w.start(&mut f.store, &p.id, 1, "run").unwrap();
    assert_eq!(f.finished(&mut w, &r.id).state, "command_passed");
}
#[test]
fn policy_snapshot_drift_revocation_and_predecode_size_limits_are_explicit() {
    let mut f = Fixture::new("printf ok > result.txt\n");
    let proposal = f
        .store
        .propose_command_validation_policy(&f.selection, "proposal")
        .unwrap();
    fs::write(f.root.join("input.txt"), "changed").unwrap();
    assert_eq!(
        f.store.finalize_proposal(&proposal.id, 1).unwrap_err().code,
        "INPUT_CHANGED"
    );
    git(&f.root, &["restore", "input.txt"]);
    let p = f.approve("new");
    let revoked = f
        .store
        .revoke_command_validation_policy(&p.id, 1, "revoke")
        .unwrap();
    assert!(revoked.revoked);
    assert_eq!(
        f.store
            .revoke_command_validation_policy(&p.id, 1, "revoke")
            .unwrap()
            .revision,
        2
    );
    assert_eq!(
        f.store
            .revoke_command_validation_policy(&p.id, 2, "revoke")
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    let mut w = CommandWorkers::default();
    assert_eq!(
        w.start(&mut f.store, &p.id, 2, "run").unwrap_err().code,
        "PERMISSION_DENIED"
    );
    let conn = Connection::open(&f.db).unwrap();
    conn.execute(
        "UPDATE command_validation_policies SET document=?1 WHERE id=?2",
        params!["x".repeat(4 * 1024 * 1024 + 1), p.id],
    )
    .unwrap();
    assert_eq!(
        f.store
            .get_command_validation_policy(&p.id)
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
    assert_eq!(
        f.store
            .list_command_validation_policies(&f.task)
            .unwrap_err()
            .code,
        "RESOURCE_LIMIT"
    );
}

#[test]
fn terminal_journal_failure_never_frees_fence_or_reexecutes_after_restart() {
    let mut f = Fixture::new("printf x >> marker\nprintf ok > result.txt\n");
    let p = f.approve("proposal");
    let connection = Connection::open(&f.db).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_terminal_command BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='command_validation.result_recorded' BEGIN SELECT RAISE(ABORT,'generated terminal journal fault');END;").unwrap();
    let mut workers = CommandWorkers::default();
    let r = workers.start(&mut f.store, &p.id, 1, "run").unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while let Ok(()) = workers.tick(&mut f.store) {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(fs::read(f.root.join("marker")).unwrap(), b"x");
    assert_eq!(
        f.store.get_command_validation_record(&r.id).unwrap().state,
        "running"
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM command_workspace_fences", [], |r| r
                .get::<_, u32>(
                0
            ))
            .unwrap(),
        1
    );
    drop(workers);
    drop(f.store);
    f.store = Store::open(&f.db).unwrap();
    let mut workers = CommandWorkers::default();
    assert_eq!(
        workers.start(&mut f.store, &p.id, 1, "run").unwrap().state,
        "outcome_unknown"
    );
    assert_eq!(fs::read(f.root.join("marker")).unwrap(), b"x");
}
#[test]
fn actual_daemon_shutdown_retires_owned_command_before_writer_rebind() {
    let f = Fixture::new("printf started > marker\nwhile :; do :; done\n");
    let runtime = f.runtime.clone();
    let selection = json!(f.selection);
    let db = f.db.clone();
    drop(f.store);
    let daemon = Daemon::bind(&runtime).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || daemon.run(flag).unwrap());
    let call = |r: Value| ipc::call(&runtime, r).unwrap();
    let p=call(json!({"method":"command_validation_policies.propose","params":selection,"idempotency_key":"proposal"}))["result"].clone();
    let p=call(json!({"method":"approvals.finalize","params":{"proposal_id":p["id"]},"expected_revision":1}))["result"].clone();
    let r=call(json!({"method":"command_validations.run","params":{"policy_id":p["id"]},"expected_revision":1,"idempotency_key":"run"}))["result"].clone();
    let end = Instant::now() + Duration::from_secs(1);
    while !f.root.join("marker").exists() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    let end = Instant::now() + Duration::from_secs(2);
    stop.store(true, Ordering::Relaxed);
    while !thread.is_finished() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(2));
    }
    thread.join().unwrap();
    let daemon = Daemon::bind(&runtime).unwrap();
    let s = Store::open(&db).unwrap();
    let record = s
        .get_command_validation_record(r["id"].as_str().unwrap())
        .unwrap();
    assert_eq!(record.state, "command_failed");
    assert!(record.evidence.unwrap()["cleanup"]["owned_leader_reaped"]
        .as_bool()
        .unwrap());
    drop(s);
    drop(daemon);
}

#[test]
fn running_command_fences_integrity_and_data_proposals_and_unconsumed_approvals() {
    use agentbaton_core::{
        data_validation::DataValidationSelection, validation::ValidationSelection,
    };
    use agentbaton_workspaces::{
        deterministic::{Column, JsonLimits, JsonTableContract, ScalarType},
        validation::{AnalysisReproducibility, AnalysisScope},
    };
    let mut f = Fixture::new("printf started > marker\nwhile :; do :; done\n");
    let integrity = ValidationSelection {
        session_id: f.selection.session_id.clone(),
        inputs: f.selection.inputs.clone(),
        outputs: f.selection.outputs.clone(),
        analysis: None,
    };
    let mut outputs = f.selection.outputs.clone();
    outputs.push(ArtifactRequirement {
        artifact_id: "script-output".into(),
        relative_path: ".validator.sh".into(),
        max_bytes: 4096,
        expected_sha256: f.selection.script.as_ref().unwrap().expected_sha256.clone(),
    });
    outputs.push(ArtifactRequirement {
        artifact_id: "environment-output".into(),
        relative_path: "environment.json".into(),
        max_bytes: 4096,
        expected_sha256: Some(sha256(b"{}")),
    });
    let data = DataValidationSelection {
        session_id: f.selection.session_id.clone(),
        inputs: f.selection.inputs.clone(),
        outputs,
        table: JsonTableContract {
            table_artifact_id: "output".into(),
            dataset_rows: 1,
            columns: vec![Column {
                name: "x".into(),
                required: true,
                nullable: false,
                scalar: ScalarType::Number { min: 0.0, max: 1.0 },
            }],
            analysis: AnalysisReproducibility {
                script_artifact_id: "script-output".into(),
                environment_artifact_id: "environment-output".into(),
                units: BTreeMap::from([("x".into(), "m".into())]),
                timezone: "UTC".into(),
                seed: Some(7),
                scope: AnalysisScope::Full,
            },
            limits: JsonLimits::default(),
        },
    };
    let old_integrity = f
        .store
        .propose_validation_policy(&integrity, "old-integrity")
        .unwrap();
    let old_data = f
        .store
        .propose_data_validation_policy(&data, "old-data")
        .unwrap();
    let policy = f.approve("command");
    let mut workers = CommandWorkers::default();
    let r = workers.start(&mut f.store, &policy.id, 1, "run").unwrap();
    assert_eq!(
        f.store
            .propose_validation_policy(&integrity, "new-integrity")
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    assert_eq!(
        f.store
            .propose_data_validation_policy(&data, "new-data")
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    assert_eq!(
        f.store
            .finalize_proposal(&old_integrity.id, old_integrity.revision)
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    assert_eq!(
        f.store
            .finalize_proposal(&old_data.id, old_data.revision)
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    let c = Connection::open(&f.db).unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM validation_policies", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM data_validation_policies", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
    for id in [&old_integrity.id, &old_data.id] {
        assert!(c
            .query_row(
                "SELECT result IS NULL FROM approval_proposals WHERE id=?1",
                [id],
                |r| r.get::<_, bool>(0)
            )
            .unwrap());
    }
    workers.cancel(&r.id);
    assert_ne!(f.finished(&mut workers, &r.id).state, "command_passed");
}

#[test]
fn actual_reaped_failed_acp_lifecycle_can_approve_independent_command_while_task_result_stays_unknown(
) {
    let f = Fixture::new("printf ok > result.txt\n");
    let runtime = f.runtime.clone();
    let database = f.db.clone();
    let workspace = f.workspace.clone();
    let project = f
        .store
        .get_session(&f.selection.session_id)
        .unwrap()
        .project_id;
    let mut selection = json!(f.selection);
    // A generated account-free local executable: --version succeeds; ACP startup
    // actually handshakes, then session/new fails with malformed protocol. The
    // driver owns the process and confirms reap; no prompt/model call occurs.
    let executable = f.runtime.join("generated-malformed-acp");
    let python = Command::new("/usr/bin/env")
        .args(["which", "python3"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let python = String::from_utf8(python.stdout).unwrap();
    fs::write(&executable,format!("#!{}\nimport sys,json\nif '--version' in sys.argv:\n print('generated-account-free-fixture 0.1')\n sys.exit(0)\nfor line in sys.stdin:\n request=json.loads(line)\n if request.get('method')=='initialize':\n  print(json.dumps({{'jsonrpc':'2.0','id':request['id'],'result':{{'protocolVersion':1,'agentCapabilities':{{}}}}}}),flush=True)\n elif request.get('method')=='session/new':\n  print('not-valid-acp-json',flush=True)\n  sys.exit(0)\n",python.trim())).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    drop(f.store);
    let daemon = Daemon::bind(&runtime).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || daemon.run(flag).unwrap());
    let call = |r: Value| {
        let response = ipc::call(&runtime, r).unwrap();
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    };
    let p = call(
        json!({"method":"connections.propose","params":{"name":"Generated malformed ACP, no account","provider":"custom_acp","executable":executable,"arguments":[]},"idempotency_key":"real-connection"}),
    );
    let connection = call(
        json!({"method":"approvals.finalize","params":{"proposal_id":p["id"]},"expected_revision":1}),
    );
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        let c = Connection::open(&database).unwrap();
        let raw: String = c
            .query_row(
                "SELECT document FROM connections WHERE id=?1",
                [connection["id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();
        if value["probe_state"] == "ready" {
            break;
        }
        assert!(Instant::now() < end, "{value}");
        std::thread::sleep(Duration::from_millis(5));
    }
    let grant = call(
        json!({"method":"grants.propose","params":{"project_id":project,"workspace_id":workspace,"connection_id":connection["id"],"max_attempts":4,"max_concurrency":1,"max_duration_ms":1000},"idempotency_key":"real-grant"}),
    );
    let grant = call(
        json!({"method":"approvals.finalize","params":{"proposal_id":grant["id"]},"expected_revision":1}),
    );
    let task = call(
        json!({"method":"tasks.create","params":{"project_id":project,"goal":"Validate confirmed stopped generated runtime"},"idempotency_key":"real-task"}),
    );
    let session = call(
        json!({"method":"sessions.create","params":{"task_id":task["id"],"workspace_id":workspace,"connection_id":connection["id"],"grant_id":grant["id"]},"expected_revision":task["revision"],"idempotency_key":"real-session"}),
    );
    let end = Instant::now() + Duration::from_secs(3);
    let stopped = loop {
        let s = call(json!({"method":"sessions.get","params":{"session_id":session["id"]}}));
        if matches!(s["state"].as_str(), Some("failed" | "stopped")) {
            break s;
        }
        assert!(Instant::now() < end, "{s}");
        std::thread::sleep(Duration::from_millis(5));
    };
    let current = call(json!({"method":"tasks.get","params":{"task_id":task["id"]}}));
    assert_eq!(current["state"], "outcome_unknown");
    let c = Connection::open(&database).unwrap();
    let intent: String = c
        .query_row(
            "SELECT state FROM dispatch_intents WHERE id=?1",
            [stopped["intent_id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(intent, "stopped");
    assert_eq!(
        c.query_row(
            "SELECT holder FROM workspace_leases WHERE resource=?1",
            [workspace.as_str()],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        ""
    );
    drop(c);
    selection["session_id"] = session["id"].clone();
    let p = call(
        json!({"method":"command_validation_policies.propose","params":selection,"idempotency_key":"real-command"}),
    );
    let p = call(
        json!({"method":"approvals.finalize","params":{"proposal_id":p["id"]},"expected_revision":1}),
    );
    let r = call(
        json!({"method":"command_validations.run","params":{"policy_id":p["id"]},"expected_revision":1,"idempotency_key":"real-run"}),
    );
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        let r = call(json!({"method":"command_validations.get","params":{"run_id":r["id"]}}));
        if !matches!(
            r["state"].as_str(),
            Some("prepared" | "running" | "cancel_requested")
        ) {
            assert_eq!(r["state"], "command_passed", "{r}");
            assert_eq!(r["task_acceptance"], "not_established");
            break;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        call(json!({"method":"tasks.get","params":{"task_id":task["id"]}}))["state"],
        "outcome_unknown"
    );
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
}
