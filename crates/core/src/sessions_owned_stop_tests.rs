//! Actual generated ACP/process and original Store receipt regressions.
use super::*;
use crate::goal_plan::*;
use crate::goal_store::*;
use crate::policy::ExecutionGrant;
use agentbaton_workspaces::validation::ArtifactRequirement;
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

const GENERATED_ACP: &str = r#"import json,sys,os
if '--version' in sys.argv:
 print('Generated Goal owned stop 1');sys.exit(0)
os.makedirs('reports',exist_ok=True)
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={'protocolVersion':1,'agentCapabilities':{}}
 elif m=='session/new':
  assert r['params']['mcpServers']==[]
  result={'sessionId':'actual-generated-goal'}
 elif m=='session/prompt':
  with open('reports/prompt-count','a')as f:f.write('one\n')
  if MODE=='hold': continue
  with open('reports/report.txt','w')as f:f.write('real-owned-one-turn\n')
  if MODE=='update-complete':
   update={'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'actual-generated-goal','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'real update before end_turn'}}}}
   done={'jsonrpc':'2.0','id':r['id'],'result':{'stopReason':'end_turn'}}
   os.write(1,(json.dumps(update)+'\n'+json.dumps(done)+'\n').encode());continue
  result={'stopReason':'end_turn'}
 else: continue
 if m=='session/prompt' and MODE=='wrong-response': r['id']+=100
 if m=='session/prompt' and MODE=='unknown-turn': result={'stopReason':'unsupported-reason'}
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':result}),flush=True)
"#;
struct Fixture {
    store: Store,
    _directory: tempfile::TempDir,
    run: GoalRun,
}
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args([
            "-c",
            "user.name=Generated fixture",
            "-c",
            "user.email=fixture@invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-C",
        ])
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
fn fixture(capacity: u32) -> Fixture {
    fixture_with_outputs(capacity, false, "complete")
}
fn fixture_with_outputs(capacity: u32, two_outputs: bool, mode: &str) -> Fixture {
    let dir = match std::env::var_os("AB_GOAL_CRASH_TEST_PARENT") {
        Some(parent) => tempfile::tempdir_in(parent).unwrap(),
        None => tempfile::tempdir().unwrap(),
    };
    let base = dir.path().canonicalize().unwrap();
    let source = base.join("source");
    fs::create_dir(&source).unwrap();
    git(&source, &["init", "--initial-branch=main"]);
    fs::write(source.join("file"), b"baseline").unwrap();
    git(&source, &["add", "file"]);
    git(&source, &["commit", "-m", "generated"]);
    let head = git(&source, &["rev-parse", "HEAD"]);
    let runtime = base.join("runtime");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let mut store = Store::open(&runtime.join("baton.sqlite3")).unwrap();
    let project = store
        .import_project(&source, "Generated unit project", "project")
        .unwrap();
    let managed = runtime.join("workspaces");
    fs::create_dir(&managed).unwrap();
    fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
    let workspace = store
        .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
        .unwrap();
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let executable = base.join("generated-goal-acp");
    fs::write(
        &executable,
        format!(
            "#!{}\nMODE={}\n{}",
            String::from_utf8(python.stdout).unwrap().trim(),
            serde_json::to_string(mode).unwrap(),
            GENERATED_ACP
        ),
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let proposal = store
        .propose_connection(
            "Actual generated one-turn ACP",
            "custom_acp",
            &executable,
            &[],
            "connection",
        )
        .unwrap();
    let connection = store.finalize_proposal(&proposal.id, 1).unwrap();
    let full = store
        .get_connection(connection["id"].as_str().unwrap())
        .unwrap();
    let mut probing = Supervisor::new(&runtime);
    probing.probe(&store, &full.id).unwrap();
    let until = Instant::now() + Duration::from_secs(4);
    while store.get_connection(&full.id).unwrap().probe_state == "pending" {
        probing.tick(&mut store).unwrap();
        assert!(
            Instant::now() < until,
            "actual version probe did not finish"
        );
        std::thread::yield_now();
    }
    assert_eq!(store.get_connection(&full.id).unwrap().probe_state, "ready");
    let grant = store
        .propose_grant(&project.id, &workspace.id, &full.id, 4, 1, 60_000, "grant")
        .unwrap();
    let grant: ExecutionGrant =
        serde_json::from_value(store.finalize_proposal(&grant.id, 1).unwrap()).unwrap();
    let task = store
        .create_registered(
            &project.id,
            "Produce an independently reviewed report",
            "task",
        )
        .unwrap();
    let limits = BudgetLimits {
        attempts: 4,
        parallel: 1,
        runtime_ms: 20_000,
        output_bytes: 16384,
        children: capacity - 1,
        reworks: 4,
        tokens: None,
        cost_microunits: None,
    };
    let mut declared_outputs = vec![GoalOutputSelection {
        artifact: ArtifactRequirement {
            artifact_id: "report".into(),
            relative_path: "reports/report.txt".into(),
            max_bytes: 1024,
            expected_sha256: None,
        },
        validators: vec![],
    }];
    if two_outputs {
        declared_outputs.push(GoalOutputSelection {
            artifact: ArtifactRequirement {
                artifact_id: "intermediate".into(),
                relative_path: "reports/intermediate.json".into(),
                max_bytes: 1024,
                expected_sha256: None,
            },
            validators: vec![],
        });
    }
    let selection = GoalScopeSelection {
        task_id: task.id.clone(),
        expected_task_revision: task.revision,
        grant_id: grant.id.clone(),
        expected_grant_revision: grant.revision,
        max_nodes: capacity,
        new_max_children: capacity - 1,
        new_max_depth: 1,
        root_budget: limits.clone(),
        project_budget: limits,
        inputs: vec![ArtifactRequirement {
            artifact_id: "input".into(),
            relative_path: "file".into(),
            max_bytes: 1024,
            expected_sha256: Some(agentbaton_workspaces::validation::sha256(b"baseline")),
        }],
        outputs: declared_outputs,
        writable_prefixes: vec!["reports".into()],
        owner_acceptance_criteria: "Owner independently reviews exact bound artifacts".into(),
    };
    let approved = store.propose_goal_scope(&selection, "scope").unwrap();
    let scope: GoalScopeView =
        serde_json::from_value(store.finalize_proposal(&approved.id, 1).unwrap()).unwrap();
    let mut node = PlanNode {
        id: "produce-report".into(),
        parent_id: None,
        kind: NodeKind::Work,
        goal: "Produce a report".into(),
        dependencies: vec![],
        workspace_id: workspace.id,
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
            artifact_id: "report".into(),
            contract_id: scope.outputs["report"].clone(),
        }],
        required_validators: vec!["goal-owner-review".into()],
        side_effect: SideEffect::WorkspaceWrite,
        write_set: vec![WriteTarget::Workspace {
            path: "reports/report.txt".into(),
        }],
        budget: BudgetLimits {
            attempts: 4,
            parallel: 1,
            runtime_ms: 10000,
            output_bytes: 8192,
            children: 0,
            reworks: 4,
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
            max_attempts: 4,
            max_reworks: 4,
            retry_classes: vec![
                FailureClass::BeforeDispatchTransient,
                FailureClass::ValidationFailed,
            ],
        },
        idempotency_key: "produce-report".into(),
        external_action_id: None,
    };
    if two_outputs {
        node.outputs.push(PlannedOutput {
            artifact_id: "intermediate".into(),
            contract_id: scope.outputs["intermediate"].clone(),
        });
    }
    let plan = GoalPlanProposal {
        schema_version: 1,
        plan_id: "unit-runtime-plan".into(),
        goal: scope.goal.clone(),
        scope_revision: scope.authority_revision,
        revision: 1,
        expected_revision: 0,
        acceptance_rules: scope.required_acceptance_rules.clone(),
        nodes: vec![node],
    };
    store
        .submit_goal_plan(
            &GoalPlanSubmission {
                scope_id: scope.id.clone(),
                proposal: plan,
            },
            "plan",
            0,
        )
        .unwrap();
    let selection = GoalRunSelection {
        scope_id: scope.id.clone(),
        expected_plan_revision: 1,
        expected_ledger_revision: store.sequence().unwrap(),
    };
    let proposal = store.propose_goal_run(&selection, "execute").unwrap();
    let run: GoalRun =
        serde_json::from_value(store.finalize_proposal(&proposal.id, 1).unwrap()).unwrap();
    Fixture {
        store,
        _directory: dir,
        run,
    }
}
fn reserve(f: &mut Fixture) -> crate::sessions::ManagedSession {
    let p = GoalRunDispatch {
        run_id: f.run.id.clone(),
        expected_ledger_revision: f.store.sequence().unwrap(),
    };
    let context = f
        .store
        .goal_dispatch_context(&p, f.store.get_goal_run(&f.run.id).unwrap().revision)
        .unwrap()
        .unwrap();
    f.store.create_goal_session(context).unwrap()
}

fn launched() -> (Fixture, Supervisor, ManagedSession) {
    let mut f = fixture(2);
    let session = reserve(&mut f);
    let runtime = f._directory.path().canonicalize().unwrap().join("runtime");
    let mut supervisor = Supervisor::new(&runtime);
    supervisor.launch(&mut f.store, &session.id).unwrap();
    let until = Instant::now() + Duration::from_secs(4);
    while f.store.get_session(&session.id).unwrap().state != "ready" {
        supervisor.tick(&mut f.store).unwrap();
        assert!(Instant::now() < until, "actual provider never became ready");
        std::thread::yield_now();
    }
    assert!(supervisor.workers[&session.id].owned_stop.is_some());
    (f, supervisor, session)
}
fn queue_original_prompt(f: &mut Fixture, supervisor: &mut Supervisor, session: &ManagedSession) {
    let revision = f.store.get_goal_run(&f.run.id).unwrap().revision;
    let mut dispatch = GoalRunDispatch {
        run_id: f.run.id.clone(),
        expected_ledger_revision: f.store.sequence().unwrap(),
    };
    f.store
        .admit_goal_advance(&dispatch, revision, "one-actual-dispatch")
        .unwrap();
    dispatch.expected_ledger_revision = f.store.sequence().unwrap();
    assert!(f
        .store
        .goal_dispatch_session(&dispatch, revision)
        .unwrap()
        .is_none());
    let (id, text) = f.store.goal_prompt(&dispatch, revision).unwrap().unwrap();
    assert_eq!(id, session.id);
    supervisor.send_goal(&mut f.store, &id, &text).unwrap();
}
fn joined_without_actor(supervisor: &mut Supervisor, session: &ManagedSession) -> WorkerReceipt {
    let until = Instant::now() + Duration::from_secs(4);
    while !supervisor.workers[&session.id].thread.is_finished() {
        assert!(
            Instant::now() < until,
            "owned process still waiting for Actor after completed turn"
        );
        std::thread::yield_now();
    }
    let worker = supervisor.workers.remove(&session.id).unwrap();
    let receipt = worker.thread.join().unwrap();
    assert!(receipt.process_reaped && !receipt.never_started);
    assert!(
        receipt.elapsed_ms > 0 && receipt.elapsed_ms < 4000,
        "original4s reservation must really be met: {}",
        receipt.elapsed_ms
    );
    assert_eq!(receipt.reason, "cancelled");
    assert!(receipt.completed_turn.is_some());
    receipt
}
fn event_count(store: &Store, kind: &str) -> u32 {
    store
        .conn
        .query_row(
            "SELECT count(*) FROM events WHERE json_extract(document,'$.kind')=?1",
            [kind],
            |r| r.get(0),
        )
        .unwrap()
}
#[test]
fn actual_goal_completed_turn_reaps_before_actor_and_then_settles_once() {
    let (mut f, mut supervisor, session) = launched();
    queue_original_prompt(&mut f, &mut supervisor, &session);
    let receipt = joined_without_actor(&mut supervisor, &session);
    assert_eq!(
        event_count(&f.store, "session.turn_ended"),
        0,
        "Actor has deliberately not run"
    );
    let charged = receipt.elapsed_ms;
    supervisor
        .retained_receipts
        .insert(session.id.clone(), RetainedReceipt::new(receipt));
    assert!(supervisor.has_workers() && supervisor.has_owned_io() && supervisor.owns(&session.id));
    supervisor.flush_retained_receipts(&mut f.store).unwrap();
    assert!(!supervisor.has_workers());
    assert_eq!(event_count(&f.store, "session.turn_ended"), 1);
    assert_eq!(event_count(&f.store, "session.stopped"), 1);
    let details = f.store.goal_run_details(&f.run.id).unwrap();
    assert_eq!(details["nodes"][0]["actual_runtime_ms"], charged);
    assert_eq!(details["nodes"][0]["state"], "candidate");
    assert_eq!(details["run"]["task_accepted"], false);
    let root = f
        .store
        .list_workspaces(&session.project_id)
        .unwrap()
        .into_iter()
        .find(|w| w.id == session.workspace_id)
        .unwrap()
        .root;
    assert_eq!(
        fs::read_to_string(root.join("reports/prompt-count")).unwrap(),
        "one\n"
    );
}
#[test]
fn actual_completion_survives_full64_queue_and_failed_original_session_journal() {
    let (mut f, mut supervisor, session) = launched();
    for _ in 0..64 {
        assert!(supervisor
            .sender
            .try_send(Outcome::Update(
                session.id.clone(),
                json!({"generated":"queue saturation"})
            ))
            .is_ok());
    }
    queue_original_prompt(&mut f, &mut supervisor, &session);
    let receipt = joined_without_actor(&mut supervisor, &session);
    supervisor
        .retained_receipts
        .insert(session.id.clone(), RetainedReceipt::new(receipt));
    f.store.conn.execute_batch("CREATE TEMP TRIGGER stop_journal_failure BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='session.stopped' BEGIN SELECT RAISE(ABORT,'actual generated journal failure'); END;").unwrap();
    assert!(supervisor.flush_retained_receipts(&mut f.store).is_err());
    let pending = &supervisor.retained_receipts[&session.id];
    assert!(
        pending.progress.terminal
            && pending.progress.stop
            && pending.progress.lease
            && !pending.progress.session
    );
    assert!(supervisor.has_workers() && supervisor.has_owned_io());
    assert_eq!(event_count(&f.store, "session.turn_ended"), 1);
    assert_eq!(event_count(&f.store, "workspace.lease_released"), 1);
    f.store
        .conn
        .execute_batch("DROP TRIGGER stop_journal_failure")
        .unwrap();
    supervisor.flush_retained_receipts(&mut f.store).unwrap();
    assert_eq!(
        event_count(&f.store, "workspace.lease_released"),
        1,
        "non-idempotent original lease release was not repeated"
    );
    assert_eq!(event_count(&f.store, "session.turn_ended"), 1);
    assert_eq!(event_count(&f.store, "session.stopped"), 1);
    supervisor.tick(&mut f.store).unwrap();
    assert_eq!(f.store.get_session(&session.id).unwrap().state, "stopped");
}
#[test]
fn committed_ticket_rejects_changed_turn_payload_native_and_other_launch() {
    let (mut f, mut supervisor, session) = launched();
    let revision = f.store.get_goal_run(&f.run.id).unwrap().revision;
    let p = GoalRunDispatch {
        run_id: f.run.id.clone(),
        expected_ledger_revision: f.store.sequence().unwrap(),
    };
    let (_, text) = f.store.goal_prompt(&p, revision).unwrap().unwrap();
    let origin = supervisor.workers[&session.id].owned_stop.as_ref().unwrap();
    assert!(committed_ticket(&f.store, origin, "changed payload").is_err());
    let ticket = committed_ticket(&f.store, origin, &text).unwrap();
    assert!(ticket.matches(Some(origin), &session.id, "actual-generated-goal"));
    assert!(!ticket.matches(Some(origin), &session.id, "another-native"));
    assert!(!ticket.matches(None, &session.id, "actual-generated-goal"));
    f.store
        .conn
        .execute(
            "UPDATE session_turns SET payload='{}' WHERE session_id=?1",
            [&session.id],
        )
        .unwrap();
    assert!(ticket
        .completed()
        .persist(&mut f.store, &session.id)
        .is_err());
    supervisor.cancel(&session.id);
    supervisor.shutdown(&mut f.store).unwrap();
}
#[test]
fn actual_pending_goal_keeps_original4second_deadline_and_no_terminal_proof() {
    let mut f = fixture_with_outputs(2, false, "hold");
    let session = reserve(&mut f);
    let runtime = f._directory.path().canonicalize().unwrap().join("runtime");
    let mut supervisor = Supervisor::new(&runtime);
    supervisor.launch(&mut f.store, &session.id).unwrap();
    let ready_until = Instant::now() + Duration::from_secs(4);
    while f.store.get_session(&session.id).unwrap().state != "ready" {
        supervisor.tick(&mut f.store).unwrap();
        assert!(Instant::now() < ready_until);
        std::thread::yield_now();
    }
    queue_original_prompt(&mut f, &mut supervisor, &session);
    // No Actor servicing: original worker deadline, not a new test timeout, stops it.
    let until = Instant::now() + Duration::from_secs(5);
    while !supervisor.workers[&session.id].thread.is_finished() {
        assert!(Instant::now() < until);
        std::thread::yield_now();
    }
    let w = supervisor.workers.remove(&session.id).unwrap();
    let receipt = w.thread.join().unwrap();
    assert!(receipt.process_reaped && receipt.completed_turn.is_none());
    assert!(
        matches!(
            receipt.reason.as_str(),
            "duration_limit" | "provider_protocol_or_timeout_failure"
        ),
        "only original deadline/protocol failure, not completion: {}",
        receipt.reason
    );
    assert!(
        receipt.elapsed_ms >= 4000,
        "no refund or shifted original runtime origin"
    );
    supervisor
        .retained_receipts
        .insert(session.id.clone(), RetainedReceipt::new(receipt));
    supervisor.tick(&mut f.store).unwrap();
    assert_ne!(
        f.store
            .get_session(&session.id)
            .unwrap()
            .turn_state
            .as_deref(),
        Some("completed")
    );
    assert_eq!(event_count(&f.store, "session.turn_ended"), 0);
}

#[test]
fn exact_terminal_receipt_replays_readonly_but_never_overwrites_a_future_turn() {
    let (mut f, mut supervisor, session) = launched();
    queue_original_prompt(&mut f, &mut supervisor, &session);
    let receipt = joined_without_actor(&mut supervisor, &session);
    let proof = receipt.completed_turn.as_ref().unwrap();
    proof.persist(&mut f.store, &session.id).unwrap();
    proof.persist(&mut f.store, &session.id).unwrap();
    assert_eq!(event_count(&f.store, "session.turn_ended"), 1);
    f.store.conn.execute("INSERT INTO session_turns VALUES('foreign-future-turn',?1,'pending','{}','future-key','{}')", [&session.id]).unwrap();
    assert!(proof.persist(&mut f.store, &session.id).is_err());
    assert_eq!(event_count(&f.store, "session.turn_ended"), 1);
    f.store
        .conn
        .execute(
            "DELETE FROM session_turns WHERE id='foreign-future-turn'",
            [],
        )
        .unwrap();
    let mut changed = f.store.get_session(&session.id).unwrap();
    changed.native_session_id = Some("another-runtime".into());
    f.store
        .conn
        .execute(
            "UPDATE sessions SET document=?1 WHERE id=?2",
            params![encode(&changed).unwrap(), session.id],
        )
        .unwrap();
    assert!(proof.persist(&mut f.store, &session.id).is_err());
}

#[test]
fn actual_completed_stop_does_not_bypass_revoked_grant_or_cancelled_run() {
    let (mut f, mut supervisor, session) = launched();
    queue_original_prompt(&mut f, &mut supervisor, &session);
    let receipt = joined_without_actor(&mut supervisor, &session);
    let run = f.store.get_goal_run(&f.run.id).unwrap();
    f.store
        .cancel_goal_with_stops(&run.id, run.revision, "actual-cancel-race")
        .unwrap();
    let grant = f.store.get_grant(&session.grant_id).unwrap();
    f.store
        .revoke_grant(&grant.id, grant.revision, "actual-revocation")
        .unwrap();
    supervisor
        .retained_receipts
        .insert(session.id.clone(), RetainedReceipt::new(receipt));
    supervisor.flush_retained_receipts(&mut f.store).unwrap();
    let run = f.store.get_goal_run(&f.run.id).unwrap();
    assert!(run.cancel_requested && !run.task_accepted);
    assert_eq!(run.state, "cancelled");
    assert_eq!(
        f.store
            .get_session(&session.id)
            .unwrap()
            .turn_state
            .as_deref(),
        Some("completed"),
        "real earlier turn is historical, never erased by later cancellation"
    );
    assert_eq!(event_count(&f.store, "session.turn_ended"), 1);
}

#[test]
fn changed_input_and_expired_or_revoked_grant_still_refuse_the_original_prompt() {
    for negative in ["input", "expired", "revoked"] {
        let (mut f, mut supervisor, session) = launched();
        if negative == "input" {
            let root = f
                .store
                .list_workspaces(&session.project_id)
                .unwrap()
                .into_iter()
                .find(|w| w.id == session.workspace_id)
                .unwrap()
                .root;
            fs::write(root.join("file"), b"changed after Ready").unwrap();
        } else if negative == "revoked" {
            let g = f.store.get_grant(&session.grant_id).unwrap();
            f.store
                .revoke_grant(&g.id, g.revision, "fresh-revoke-before-turn")
                .unwrap();
        } else {
            let mut g = f.store.get_grant(&session.grant_id).unwrap();
            g.expires_at_ms = 1;
            f.store
                .conn
                .execute(
                    "UPDATE execution_grants SET document=?1 WHERE id=?2",
                    params![encode(&g).unwrap(), g.id],
                )
                .unwrap();
        }
        let revision = f.store.get_goal_run(&f.run.id).unwrap().revision;
        let dispatch = GoalRunDispatch {
            run_id: f.run.id.clone(),
            expected_ledger_revision: f.store.sequence().unwrap(),
        };
        assert!(
            f.store.goal_prompt(&dispatch, revision).is_err(),
            "{negative} cannot get a prompt or stop ticket"
        );
        assert_eq!(event_count(&f.store, "goal.node_prompt_queued"), 0);
        supervisor.shutdown(&mut f.store).unwrap();
        assert_eq!(event_count(&f.store, "session.turn_ended"), 0);
    }
}

#[test]
fn actual_ordinary_session_can_complete_two_turns_without_one_turn_stop() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    let exe = root.join("actual-ordinary-acp");
    fs::write(
        &exe,
        format!(
            "#!{}\nMODE='complete'\n{}",
            String::from_utf8(python.stdout).unwrap().trim(),
            GENERATED_ACP
        ),
    )
    .unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    let (events, receive) = mpsc::sync_channel(64);
    let (commands, worker_commands) = mpsc::sync_channel(8);
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let installation = ApprovedInstallation {
        executable: exe.clone(),
        expected_sha256: executable_digest(&exe).unwrap(),
        arguments: vec![],
        workspace: root,
        approval_reference: "actual-generated-local-only".into(),
        environment: BTreeMap::new(),
    };
    let thread = std::thread::spawn(move || {
        run_worker(
            "ordinary".into(),
            WorkerLaunch {
                route: ProtocolRoute::CustomAcp,
                installation,
                configuration: None,
                factory_mcp: None,
                owned_stop: None,
                planning: None,
                prelaunch_ms: 0,
                duration: Duration::from_secs(4),
            },
            worker_commands,
            worker_cancel,
            events,
        )
    });
    assert!(matches!(
        receive.recv_timeout(Duration::from_secs(3)).unwrap(),
        Outcome::Ready(_, _)
    ));
    for _ in 0..2 {
        commands
            .send(Work::Prompt("actual ordinary turn".into()))
            .unwrap();
        assert!(
            matches!(receive.recv_timeout(Duration::from_secs(3)).unwrap(),Outcome::Turn(_,s) if s=="completed")
        );
        assert!(
            !thread.is_finished(),
            "ordinary multi-turn ownership remains alive after end_turn"
        );
    }
    cancel.store(true, Ordering::Relaxed);
    let receipt = thread.join().unwrap();
    assert!(receipt.process_reaped && receipt.completed_turn.is_none());
}

#[test]
fn actual_actor_service_before_owned_join_keeps_original_terminal_order() {
    let (mut f, mut supervisor, session) = launched();
    queue_original_prompt(&mut f, &mut supervisor, &session);
    let until = Instant::now() + Duration::from_secs(4);
    while supervisor.has_workers() {
        supervisor.tick(&mut f.store).unwrap();
        assert!(
            Instant::now() < until,
            "normal Actor service must not wait for duration_limit"
        );
        std::thread::yield_now();
    }
    let kinds: Vec<String> = {
        let mut q=f.store.conn.prepare("SELECT json_extract(document,'$.kind') FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind') IN('session.turn_ended','session.stopped') ORDER BY sequence").unwrap();
        let out = q
            .query_map([&session.id], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        out
    };
    assert_eq!(kinds, vec!["session.turn_ended", "session.stopped"]);
    assert_eq!(f.store.get_session(&session.id).unwrap().last_error, None);
    assert!(!f.store.get_goal_run(&f.run.id).unwrap().task_accepted);
}

#[test]
fn actual_worker_rejects_wrong_native_control_ticket_without_prompt_or_completion() {
    let (mut f, mut supervisor, session) = launched();
    let revision = f.store.get_goal_run(&f.run.id).unwrap().revision;
    let p = GoalRunDispatch {
        run_id: f.run.id.clone(),
        expected_ledger_revision: f.store.sequence().unwrap(),
    };
    let (_, text) = f.store.goal_prompt(&p, revision).unwrap().unwrap();
    let worker = &supervisor.workers[&session.id];
    let mut ticket =
        committed_ticket(&f.store, worker.owned_stop.as_ref().unwrap(), &text).unwrap();
    ticket.native = "not-this-owned-native-session".into();
    worker
        .commands
        .send(Work::GoalPrompt { text, stop: ticket })
        .unwrap();
    let until = Instant::now() + Duration::from_secs(4);
    while !supervisor.workers[&session.id].thread.is_finished() {
        assert!(Instant::now() < until);
        std::thread::yield_now();
    }
    let w = supervisor.workers.remove(&session.id).unwrap();
    let receipt = w.thread.join().unwrap();
    assert_eq!(receipt.reason, "goal_turn_binding_changed");
    assert!(receipt.process_reaped && receipt.completed_turn.is_none());
    supervisor
        .retained_receipts
        .insert(session.id.clone(), RetainedReceipt::new(receipt));
    supervisor.tick(&mut f.store).unwrap();
    assert_ne!(
        f.store
            .get_session(&session.id)
            .unwrap()
            .turn_state
            .as_deref(),
        Some("completed")
    );
    assert_eq!(event_count(&f.store, "session.turn_ended"), 0);
}

#[test]
fn actual_uncorrelated_or_unknown_provider_result_never_becomes_completed_proof() {
    for mode in ["wrong-response", "unknown-turn"] {
        let mut f = fixture_with_outputs(2, false, mode);
        let session = reserve(&mut f);
        let runtime = f._directory.path().canonicalize().unwrap().join("runtime");
        let mut supervisor = Supervisor::new(&runtime);
        supervisor.launch(&mut f.store, &session.id).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while f.store.get_session(&session.id).unwrap().state != "ready" {
            supervisor.tick(&mut f.store).unwrap();
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
        queue_original_prompt(&mut f, &mut supervisor, &session);
        while !supervisor.workers[&session.id].thread.is_finished() {
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
        let w = supervisor.workers.remove(&session.id).unwrap();
        let receipt = w.thread.join().unwrap();
        assert!(
            receipt.process_reaped && receipt.completed_turn.is_none(),
            "{mode}"
        );
        assert_ne!(receipt.reason, "cancelled");
        supervisor
            .retained_receipts
            .insert(session.id.clone(), RetainedReceipt::new(receipt));
        supervisor.tick(&mut f.store).unwrap();
        assert_eq!(f.store.conn.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='session.turn_ended' AND json_extract(document,'$.payload.turn_state')='completed'",[],|r|r.get::<_,u32>(0)).unwrap(),0);
        assert_ne!(
            f.store
                .get_session(&session.id)
                .unwrap()
                .turn_state
                .as_deref(),
            Some("completed")
        );
        assert!(!f.store.get_goal_run(&f.run.id).unwrap().task_accepted);
    }
}

#[test]
fn actual_failed_terminal_commit_retains_joined_proof_before_any_stop_journal() {
    let (mut f, mut supervisor, session) = launched();
    queue_original_prompt(&mut f, &mut supervisor, &session);
    let receipt = joined_without_actor(&mut supervisor, &session);
    supervisor
        .retained_receipts
        .insert(session.id.clone(), RetainedReceipt::new(receipt));
    f.store.conn.execute_batch("CREATE TEMP TRIGGER terminal_journal_failure BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='session.turn_ended' BEGIN SELECT RAISE(ABORT,'actual terminal commit failure'); END;").unwrap();
    assert!(supervisor.flush_retained_receipts(&mut f.store).is_err());
    let retained = &supervisor.retained_receipts[&session.id];
    assert!(retained.receipt.completed_turn.is_some());
    assert!(!retained.progress.terminal && !retained.progress.stop && !retained.progress.lease);
    assert!(supervisor.has_owned_io() && supervisor.has_workers());
    assert_eq!(
        f.store
            .workspace_lease(&session.workspace_id)
            .unwrap()
            .unwrap()
            .holder,
        session.id
    );
    assert_eq!(event_count(&f.store, "session.turn_ended"), 0);
    assert_eq!(event_count(&f.store, "session.stopped"), 0);
    f.store
        .conn
        .execute_batch("DROP TRIGGER terminal_journal_failure")
        .unwrap();
    supervisor.flush_retained_receipts(&mut f.store).unwrap();
    assert_eq!(event_count(&f.store, "session.turn_ended"), 1);
    assert_eq!(event_count(&f.store, "session.stopped"), 1);
    assert!(!supervisor.has_workers());
}

#[test]
fn actual_update_and_completed_with_full64_keeps_history_but_preserves_backpressure_failure() {
    for failure_already_committed in [false, true] {
        let mut f = fixture_with_outputs(2, false, "update-complete");
        let session = reserve(&mut f);
        let mut unrepresentable_revision = session.clone();
        unrepresentable_revision.revision = u64::MAX;
        assert_eq!(
            original_backpressure_event(&f.store.conn, &unrepresentable_revision)
                .unwrap_err()
                .code,
            "INPUT_CHANGED",
            "SQLite binding cannot truncate an authorization revision"
        );
        let runtime = f._directory.path().canonicalize().unwrap().join("runtime");
        let mut supervisor = Supervisor::new(&runtime);
        supervisor.launch(&mut f.store, &session.id).unwrap();
        let until = Instant::now() + Duration::from_secs(4);
        while f.store.get_session(&session.id).unwrap().state != "ready" {
            supervisor.tick(&mut f.store).unwrap();
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
        for _ in 0..64 {
            assert!(supervisor
                .sender
                .try_send(Outcome::Update(
                    session.id.clone(),
                    json!({"generated":"actual full queue before real update and end_turn"})
                ))
                .is_ok());
        }
        queue_original_prompt(&mut f, &mut supervisor, &session);
        while !supervisor.workers[&session.id].thread.is_finished() {
            assert!(Instant::now() < until);
            std::thread::yield_now();
        }
        let worker = supervisor.workers.remove(&session.id).unwrap();
        let receipt = worker.thread.join().unwrap();
        assert_eq!(receipt.reason, "event_backpressure");
        assert!(receipt.process_reaped && !receipt.never_started);
        assert!(receipt.elapsed_ms > 0 && receipt.elapsed_ms < 4000);
        assert!(
            receipt.completed_turn.is_some(),
            "a real correlated result survives the failed update forwarding"
        );
        if failure_already_committed {
            // All original queued updates are processed before the exact real
            // failure callback. This is the other legal Actor/worker schedule.
            supervisor.tick(&mut f.store).unwrap();
            f.store
                .session_failure(&session.id, "event_backpressure")
                .unwrap();
            let (seq, raw): (i64, String) = f
                .store
                .conn
                .query_row(
                    "SELECT sequence,document FROM events ORDER BY sequence DESC LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            f.store.conn.execute("UPDATE events SET document=json_set(document,'$.source','forged-source') WHERE sequence=?1",[seq]).unwrap();
            assert!(
                receipt
                    .completed_turn
                    .as_ref()
                    .unwrap()
                    .persist_failed_backpressure(&mut f.store, &session.id)
                    .is_err(),
                "last_error alone is not durable same-turn failure provenance"
            );
            f.store
                .conn
                .execute(
                    "UPDATE events SET document=?1 WHERE sequence=?2",
                    params![raw, seq],
                )
                .unwrap();
        }
        let proof = receipt.completed_turn.as_ref().unwrap();
        proof
            .persist_failed_backpressure(&mut f.store, &session.id)
            .unwrap();
        proof
            .persist_failed_backpressure(&mut f.store, &session.id)
            .unwrap();
        assert_eq!(
            event_count(&f.store, "session.turn_ended"),
            1,
            "same exact historical turn is immutable"
        );
        assert_eq!(
            event_count(&f.store, "session.outcome_unknown"),
            1,
            "backpressure failure is recorded even when its queue notification was refused"
        );
        let (sequence,historical):(i64,String)=f.store.conn.query_row("SELECT sequence,document FROM events WHERE json_extract(document,'$.kind')='session.turn_ended' AND json_extract(document,'$.aggregate_id')=?1",[&session.id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        let history: Event = decode(&historical).unwrap();
        assert_eq!(history.payload["turn_state"], "completed");
        assert_eq!(history.payload["post_turn_failure"], "event_backpressure");
        assert!(history.payload["owned_turn_id"].is_string());
        assert_eq!(
            history.payload["owned_turn_proof_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        f.store.conn.execute("INSERT INTO session_turns VALUES('already-ended-future-turn',?1,'ended','{}','future-ended-key','{}')",[&session.id]).unwrap();
        assert!(
            proof
                .persist_failed_backpressure(&mut f.store, &session.id)
                .is_err(),
            "future ended turn cannot be overwritten either"
        );
        f.store
            .conn
            .execute(
                "DELETE FROM session_turns WHERE id='already-ended-future-turn'",
                [],
            )
            .unwrap();
        f.store.conn.execute("UPDATE events SET document=json_set(document,'$.payload.owned_turn_proof_sha256','forged') WHERE sequence=?1",[sequence]).unwrap();
        assert!(
            proof
                .persist_failed_backpressure(&mut f.store, &session.id)
                .is_err(),
            "ended state does not substitute for exact terminal evidence"
        );
        f.store
            .conn
            .execute(
                "UPDATE events SET document=?1 WHERE sequence=?2",
                params![historical, sequence],
            )
            .unwrap();
        f.store
            .conn
            .execute("DELETE FROM events WHERE sequence=?1", [sequence])
            .unwrap();
        assert!(
            proof
                .persist_failed_backpressure(&mut f.store, &session.id)
                .is_err(),
            "missing historical event refuses replay"
        );
        f.store
            .conn
            .execute(
                "INSERT INTO events(sequence,document) VALUES(?1,?2)",
                params![sequence, historical],
            )
            .unwrap();
        let mut current = f.store.get_session(&session.id).unwrap();
        current.last_error = Some("another-unknown-cause".into());
        f.store
            .conn
            .execute(
                "UPDATE sessions SET document=?1 WHERE id=?2",
                params![encode(&current).unwrap(), session.id],
            )
            .unwrap();
        assert!(
            proof
                .persist_failed_backpressure(&mut f.store, &session.id)
                .is_err(),
            "other unknown causes are not backpressure proof"
        );
        current.last_error = Some("event_backpressure".into());
        f.store
            .conn
            .execute(
                "UPDATE sessions SET document=?1 WHERE id=?2",
                params![encode(&current).unwrap(), session.id],
            )
            .unwrap();
        supervisor
            .retained_receipts
            .insert(session.id.clone(), RetainedReceipt::new(receipt));
        supervisor.flush_retained_receipts(&mut f.store).unwrap();
        supervisor.tick(&mut f.store).unwrap();
        let stopped = f.store.get_session(&session.id).unwrap();
        assert_eq!(stopped.state, "stopped");
        assert_eq!(stopped.last_error.as_deref(), Some("event_backpressure"));
        assert_eq!(stopped.turn_state.as_deref(), Some("outcome_unknown"));
        let details = f.store.goal_run_details(&f.run.id).unwrap();
        assert_ne!(details["nodes"][0]["state"], "candidate");
        assert_eq!(details["run"]["state"], "outcome_unknown");
        assert_eq!(details["run"]["task_accepted"], false);
        assert_eq!(event_count(&f.store, "session.turn_ended"), 1);
        assert_eq!(event_count(&f.store, "session.stopped"), 1);
        assert!(!supervisor.has_workers());
    }
}
