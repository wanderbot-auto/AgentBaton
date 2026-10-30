//! Real Core approvals, generated account-free ACP and owned Supervisor threads.
//! Tests drive private Actor helpers directly; no OS dialog or Daemon.run claim.
use super::*;
use crate::{
    goal_plan::*,
    goal_store::{
        GoalOutputSelection, GoalPlanSubmission, GoalRun, GoalRunDispatch, GoalRunSelection,
        GoalScopeSelection, GoalScopeView,
    },
    policy::ExecutionGrant,
    registry::ManagedWorkspace,
    sessions::ManagedSession,
    storage::Task,
};
use agentbaton_workspaces::validation::{sha256, ArtifactRequirement};
use std::process::Command;

const PROVIDER: &str = r#"import json,os,sys
if '--version' in sys.argv:
 print('Generated Goal stop ACP 1');sys.exit(0)
def emit(v):print(json.dumps(v),flush=True)
def response(r,v):emit({'jsonrpc':'2.0','id':r['id'],'result':v})
for line in sys.stdin:
 r=json.loads(line);method=r.get('method')
 if method=='initialize':response(r,{'protocolVersion':1,'agentCapabilities':{}})
 elif method=='session/new':
  assert r['params']['mcpServers']==[]
  response(r,{'sessionId':'generated-stop-service'})
 elif method=='session/prompt':
  text=r['params']['prompt'][0]['text']
  if text.startswith('{'):
   request=json.loads(text);assert request['kind']=='agentbaton_goal_task_request'
   if HOLD:
    emit({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'generated-stop-service','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'controlled pending Goal prompt'}}}})
    continue
   os.makedirs('reports',exist_ok=True)
   with open('reports/report.txt','w')as out:out.write('actual owned Goal report\n')
  response(r,{'stopReason':'end_turn'})
"#;

fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "init.templateDir=",
            "-c",
            "user.name=Generated fixture",
            "-c",
            "user.email=fixture@invalid",
            "-C",
        ])
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "generated source Git operation failed"
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
// Field/local declaration order keeps this wrapper after the Daemon. A panic
// therefore retains only after any constructed Daemon has cancelled/joined its
// owned workers and closed its Store. This is not a panic-instant DB snapshot.
struct RetainedDirectory(Option<tempfile::TempDir>);
impl RetainedDirectory {
    fn path(&self) -> &Path {
        self.0.as_ref().expect("live test directory").path()
    }
}
impl Drop for RetainedDirectory {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(directory) = self.0.take() {
                let path = directory.keep();
                // Evidence logging must not replace the original panic.
                use std::io::Write;
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "Retained failed Goal stop fixture after owned cleanup: {}",
                    path.display()
                );
            }
        }
    }
}
struct Fixture {
    // The daemon must reap its real owned processes before the private tree goes.
    daemon: Daemon,
    _directory: RetainedDirectory,
    task: Task,
    workspace: ManagedWorkspace,
    connection: String,
    grant: ExecutionGrant,
}
impl Fixture {
    fn new(hold: bool) -> Self {
        let directory = RetainedDirectory(Some(
            tempfile::Builder::new()
                .prefix("ab-stop-")
                .permissions(fs::Permissions::from_mode(0o700))
                .tempdir_in(Path::new("/tmp").canonicalize().unwrap())
                .unwrap(),
        ));
        let base = directory.path();
        let source = base.join("s");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        fs::write(source.join("input.txt"), b"fixed input\n").unwrap();
        fs::write(source.join(".gitignore"), b"reports/\n").unwrap();
        git(&source, &["add", "input.txt", ".gitignore"]);
        git(&source, &["commit", "-m", "generated stop-service input"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let mut daemon = Daemon::bind(&base.join("r")).unwrap();
        let project = daemon
            .store
            .import_project(&source, "Generated stop service", "project")
            .unwrap();
        let managed = base.join("w");
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        let workspace = daemon
            .store
            .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
            .unwrap();
        let python = Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .unwrap();
        assert!(python.status.success());
        let executable = base.join("provider");
        fs::write(
            &executable,
            format!(
                "#!{}\nHOLD={}\n{}",
                String::from_utf8(python.stdout).unwrap().trim(),
                if hold { "True" } else { "False" },
                PROVIDER
            ),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let proposal = daemon
            .store
            .propose_connection(
                "Generated account-free stop ACP",
                "custom_acp",
                &executable,
                &[],
                "connection",
            )
            .unwrap();
        let connection = daemon
            .store
            .finalize_proposal(&proposal.id, proposal.revision)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        daemon.supervisor.probe(&daemon.store, &connection).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while daemon
            .store
            .get_connection(&connection)
            .unwrap()
            .probe_state
            != "ready"
        {
            daemon.supervisor.tick(&mut daemon.store).unwrap();
            assert!(
                Instant::now() < until,
                "real generated version probe must finish"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let proposal = daemon
            .store
            .propose_grant(
                &project.id,
                &workspace.id,
                &connection,
                4,
                1,
                30_000,
                "grant",
            )
            .unwrap();
        let grant: ExecutionGrant = serde_json::from_value(
            daemon
                .store
                .finalize_proposal(&proposal.id, proposal.revision)
                .unwrap(),
        )
        .unwrap();
        let task = daemon
            .store
            .create_registered(&project.id, "Produce a separately reviewed report", "task")
            .unwrap();
        Self {
            daemon,
            _directory: directory,
            task,
            workspace,
            connection,
            grant,
        }
    }
    fn goal_session(&mut self) -> (GoalRun, ManagedSession) {
        let limits = BudgetLimits {
            attempts: 4,
            parallel: 1,
            runtime_ms: 20_000,
            output_bytes: 8192,
            children: 0,
            reworks: 0,
            tokens: None,
            cost_microunits: None,
        };
        let selection = GoalScopeSelection {
            task_id: self.task.id.clone(),
            expected_task_revision: self.task.revision,
            grant_id: self.grant.id.clone(),
            expected_grant_revision: self.grant.revision,
            max_nodes: 1,
            new_max_children: 0,
            new_max_depth: 1,
            root_budget: limits.clone(),
            project_budget: limits,
            inputs: vec![ArtifactRequirement {
                artifact_id: "input".into(),
                relative_path: "input.txt".into(),
                max_bytes: 1024,
                expected_sha256: Some(sha256(b"fixed input\n")),
            }],
            outputs: vec![GoalOutputSelection {
                artifact: ArtifactRequirement {
                    artifact_id: "report".into(),
                    relative_path: "reports/report.txt".into(),
                    max_bytes: 1024,
                    expected_sha256: None,
                },
                validators: vec![],
            }],
            writable_prefixes: vec!["reports".into()],
            owner_acceptance_criteria:
                "Review the exact output independently; stop is never acceptance".into(),
        };
        let proposal = self
            .daemon
            .store
            .propose_goal_scope(&selection, "scope")
            .unwrap();
        let scope: GoalScopeView = serde_json::from_value(
            self.daemon
                .store
                .finalize_proposal(&proposal.id, proposal.revision)
                .unwrap(),
        )
        .unwrap();
        let contract = scope.outputs["report"].clone();
        let node = PlanNode {
            id: "produce-report".into(),
            parent_id: None,
            kind: NodeKind::Work,
            goal: "Produce the exact requested report".into(),
            dependencies: vec![],
            workspace_id: self.workspace.id.clone(),
            executor: Executor::Connection {
                binding: Box::new(scope.binding.clone()),
                program_sha256: scope.program_sha256.clone(),
            },
            recipients: vec![],
            required_inputs: vec![RequiredInput {
                goal_task_id: self.task.id.clone(),
                artifact_id: "input".into(),
                contract_id: scope.inputs["input"].contract_id.clone(),
                source: InputSource::ApprovedExternal {
                    sha256: scope.inputs["input"].sha256.clone(),
                },
            }],
            outputs: vec![PlannedOutput {
                artifact_id: "report".into(),
                contract_id: contract.clone(),
            }],
            required_validators: scope.contracts[&contract].clone(),
            side_effect: SideEffect::WorkspaceWrite,
            write_set: vec![WriteTarget::Workspace {
                path: "reports/report.txt".into(),
            }],
            budget: BudgetLimits {
                attempts: 1,
                parallel: 1,
                runtime_ms: 5000,
                output_bytes: 1024,
                children: 0,
                reworks: 0,
                tokens: None,
                cost_microunits: None,
            },
            reservation: AttemptReservation {
                runtime_ms: 4000,
                output_bytes: 1024,
                tokens: None,
                cost_microunits: None,
            },
            retry: RetryPolicy {
                max_attempts: 1,
                max_reworks: 0,
                retry_classes: vec![],
            },
            idempotency_key: "produce-report".into(),
            external_action_id: None,
        };
        self.daemon
            .store
            .submit_goal_plan(
                &GoalPlanSubmission {
                    scope_id: scope.id.clone(),
                    proposal: GoalPlanProposal {
                        schema_version: 1,
                        plan_id: "stop-service-plan".into(),
                        goal: scope.goal.clone(),
                        scope_revision: scope.authority_revision,
                        revision: 1,
                        expected_revision: 0,
                        acceptance_rules: scope.required_acceptance_rules.clone(),
                        nodes: vec![node],
                    },
                },
                "plan",
                0,
            )
            .unwrap();
        let proposal = self
            .daemon
            .store
            .propose_goal_run(
                &GoalRunSelection {
                    scope_id: scope.id,
                    expected_plan_revision: 1,
                    expected_ledger_revision: self.daemon.store.sequence().unwrap(),
                },
                "execute",
            )
            .unwrap();
        let run: GoalRun = serde_json::from_value(
            self.daemon
                .store
                .finalize_proposal(&proposal.id, proposal.revision)
                .unwrap(),
        )
        .unwrap();
        let session = self
            .daemon
            .store
            .goal_dispatch_session(
                &GoalRunDispatch {
                    run_id: run.id.clone(),
                    expected_ledger_revision: self.daemon.store.sequence().unwrap(),
                },
                run.revision,
            )
            .unwrap()
            .unwrap();
        self.daemon
            .supervisor
            .launch(&mut self.daemon.store, &session.id)
            .unwrap();
        self.wait_ready(&session.id);
        assert!(
            !self.daemon.supervisor.take_goal_stop_hint(),
            "Ready is not a terminal Goal outcome"
        );
        let run = self.daemon.store.get_goal_run(&run.id).unwrap();
        let (id, text) = self
            .daemon
            .store
            .goal_prompt(
                &GoalRunDispatch {
                    run_id: run.id.clone(),
                    expected_ledger_revision: self.daemon.store.sequence().unwrap(),
                },
                run.revision,
            )
            .unwrap()
            .unwrap();
        self.daemon.supervisor.send(&id, &text).unwrap();
        (run, self.daemon.store.get_session(&session.id).unwrap())
    }
    fn wait_ready(&mut self, id: &str) {
        let until = Instant::now() + Duration::from_secs(5);
        while self.daemon.store.get_session(id).unwrap().state != "ready" {
            self.daemon.supervisor.tick(&mut self.daemon.store).unwrap();
            assert!(
                Instant::now() < until,
                "real owned Provider must become ready"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn wait_turn(&mut self, id: &str) {
        let until = Instant::now() + Duration::from_secs(5);
        while self
            .daemon
            .store
            .get_session(id)
            .unwrap()
            .turn_state
            .as_deref()
            != Some("completed")
        {
            self.daemon.supervisor.tick(&mut self.daemon.store).unwrap();
            assert!(
                Instant::now() < until,
                "real correlated end_turn must be committed"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn wait_stopped(&mut self, id: &str) {
        let until = Instant::now() + Duration::from_secs(5);
        while self.daemon.store.get_session(id).unwrap().state != "stopped" {
            self.daemon.supervisor.tick(&mut self.daemon.store).unwrap();
            assert!(
                Instant::now() < until,
                "owned process must actually stop and reap"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(self.daemon.store.conn.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.source')='baton_supervisor' AND json_extract(document,'$.payload.process_reaped')=1 AND json_extract(document,'$.payload.no_process_started')=0", [id], |r|r.get::<_,i64>(0)).unwrap(),1);
        assert_eq!(
            self.daemon
                .store
                .workspace_lease(&self.workspace.id)
                .unwrap()
                .unwrap()
                .holder,
            ""
        );
    }
    fn pending_turn_before(&self, id: &str) -> Option<String> {
        let state = self.daemon.store.get_session(id).unwrap().turn_state;
        // send_goal_session commits managed turn_state=running while its
        // session_turns row is pending; neither representation is a terminal turn.
        assert_eq!(state.as_deref(), Some("running"));
        self.assert_pending_turn_unchanged(id, &state);
        state
    }
    fn assert_pending_turn_unchanged(&self, id: &str, before: &Option<String>) {
        let current = self.daemon.store.get_session(id).unwrap().turn_state;
        assert_eq!(current.as_ref(), before.as_ref());
        let rows: (i64, i64, i64) = self.daemon.store.conn.query_row(
            "SELECT count(*),COALESCE(sum(state='pending'),0),COALESCE(sum(state='ended'),0) FROM session_turns WHERE session_id=?1",
            [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        ).unwrap();
        assert_eq!(rows, (1, 1, 0));
        assert_eq!(
            self.daemon.store.conn.query_row(
                "SELECT count(*) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='session.turn_ended'",
                [id], |r| r.get::<_, i64>(0)
            ).unwrap(),
            0,
            "no terminal event may be persisted for this pending turn"
        );
    }
    fn no_extra_execution_or_task_acceptance(&self, attempts: i64, since: u64) {
        assert_eq!(
            self.daemon
                .store
                .conn
                .query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            attempts,
            "real reap/settlement must not admit another attempt"
        );
        let task = self.daemon.store.get(&self.task.id).unwrap();
        assert_ne!(task.state, crate::storage::TaskState::Accepted);
        assert!(task.acceptance_ref.is_none());
        assert!(task.acceptance_sequence.is_none());
        assert_eq!(
            self.daemon
                .store
                .conn
                .query_row(
                    "SELECT count(*) FROM goal_graph_acceptances WHERE task_id=?1",
                    [&self.task.id],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0,
            "a stop must not create an acceptance receipt without accepting its Task"
        );
        // Use the actual event-writer kinds, not a frozen journal length:
        // session.stopped/attempt.stopped/goal.node_settled may legitimately grow.
        assert_eq!(
            self.daemon.store.conn.query_row(
                "SELECT count(*) FROM events WHERE sequence>?1 AND json_extract(document,'$.kind') IN('attempt.prepared','attempt.dispatch_started','goal.run_authorized','goal.node_reserved','goal.node_prompt_queued','session.message_queued','task.accepted')",
                [i64::try_from(since).unwrap()], |r| r.get::<_, i64>(0)
            ).unwrap(),
            0,
            "stop-only service and actual reap must not dispatch, prompt or accept"
        );
    }
    fn stop_requests(&self, id: &str) -> i64 {
        self.daemon.store.conn.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.node_stop_requested' AND json_extract(document,'$.payload.session_id')=?1",[id],|r|r.get(0)).unwrap()
    }
}

#[test]
fn owned_goal_terminal_commit_is_stopped_by_hint_before_an_unrelated_read_without_admission() {
    let mut f = Fixture::new(false);
    let (run, session) = f.goal_session();
    f.wait_turn(&session.id);
    assert_eq!(f.stop_requests(&session.id), 0);
    assert_ne!(
        f.daemon.store.get_session(&session.id).unwrap().state,
        "stopped"
    );
    let attempts: i64 = f
        .daemon
        .store
        .conn
        .query_row("SELECT count(*) FROM attempts", [], |r| r.get(0))
        .unwrap();
    let stop_tail = f.daemon.store.sequence().unwrap();
    f.daemon.service_hinted_goal_stops().unwrap();
    assert_eq!(f.stop_requests(&session.id), 1);
    // A real stop was requested, but neither hint nor end_turn can fake its receipt.
    assert_ne!(
        f.daemon.store.get_session(&session.id).unwrap().state,
        "stopped"
    );
    let response = f.daemon.handle(&json!({"api_version":1,"request_id":"unrelated-after-stop-service","token":f.daemon.token,"method":"tasks.get","params":{"task_id":f.task.id}}).to_string());
    assert!(response.get("error").is_none());
    assert_eq!(f.stop_requests(&session.id), 1);
    let after = f.daemon.store.sequence().unwrap();
    f.daemon.service_hinted_goal_stops().unwrap();
    assert_eq!(
        f.daemon.store.sequence().unwrap(),
        after,
        "consumed/coalesced hint must not rescan or duplicate writes"
    );
    assert_eq!(
        f.daemon
            .store
            .conn
            .query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        attempts
    );
    f.wait_stopped(&session.id);
    f.no_extra_execution_or_task_acceptance(attempts, stop_tail);
    assert!(!f.daemon.store.get_goal_run(&run.id).unwrap().task_accepted);
    let actual: Option<i64> = f.daemon.store.conn.query_row("SELECT json_extract(document,'$.actual_runtime_ms') FROM goal_run_reservations WHERE attempt_id=?1",[&session.attempt_id],|r|r.get(0)).unwrap();
    assert!(actual.is_some_and(|ms| ms > 0 && ms <= 4000));
}

#[test]
fn ordinary_ready_and_completed_turn_do_not_hint_or_automatically_stop() {
    let mut f = Fixture::new(false);
    let session = f
        .daemon
        .store
        .create_session(
            &f.task.id,
            &f.workspace.id,
            &f.connection,
            &f.grant.id,
            f.task.revision,
            "ordinary",
        )
        .unwrap();
    f.daemon
        .supervisor
        .launch(&mut f.daemon.store, &session.id)
        .unwrap();
    f.wait_ready(&session.id);
    assert!(!f.daemon.supervisor.take_goal_stop_hint());
    let current = f.daemon.store.get_session(&session.id).unwrap();
    let (_, send) = f
        .daemon
        .store
        .send_session(
            &session.id,
            "ordinary generated turn",
            current.revision,
            "ordinary-turn",
        )
        .unwrap();
    assert!(send);
    f.daemon
        .supervisor
        .send(&session.id, "ordinary generated turn")
        .unwrap();
    f.wait_turn(&session.id);
    assert!(!f.daemon.supervisor.take_goal_stop_hint());
    let sequence = f.daemon.store.sequence().unwrap();
    f.daemon.service_hinted_goal_stops().unwrap();
    assert_eq!(f.daemon.store.sequence().unwrap(), sequence);
    assert_eq!(f.stop_requests(&session.id), 0);
    assert!(f.daemon.supervisor.owns(&session.id));
    assert_ne!(
        f.daemon.store.get_session(&session.id).unwrap().state,
        "stopped"
    );
    let current = f.daemon.store.get_session(&session.id).unwrap();
    f.daemon
        .store
        .cancel_session(&session.id, current.revision, "ordinary-owner-cancel", true)
        .unwrap();
    f.daemon.supervisor.cancel(&session.id);
    f.wait_stopped(&session.id);
}

#[test]
fn failed_terminal_turn_transaction_publishes_no_hint_or_false_stop() {
    let mut f = Fixture::new(false);
    let (_run, session) = f.goal_session();
    let turn_before = f.pending_turn_before(&session.id);
    f.daemon.store.conn.execute_batch("CREATE TRIGGER refuse_terminal_turn BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='session.turn_ended' BEGIN SELECT RAISE(ABORT,'owned test terminal persistence fault'); END;").unwrap();
    let sequence = f.daemon.store.sequence().unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let error = loop {
        match f.daemon.supervisor.tick(&mut f.daemon.store) {
            Err(error) => break error,
            Ok(()) => {
                assert!(
                    Instant::now() < until,
                    "must reach the exact terminal persistence fault"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    };
    assert_eq!(error.code, "INTERNAL_ERROR");
    assert_eq!(f.daemon.store.sequence().unwrap(), sequence);
    f.assert_pending_turn_unchanged(&session.id, &turn_before);
    assert!(!f.daemon.supervisor.take_goal_stop_hint());
    assert_eq!(f.stop_requests(&session.id), 0);
    assert_ne!(
        f.daemon.store.get_session(&session.id).unwrap().state,
        "stopped"
    );
    // Fatal caller cleanup retains the actual owned worker; no fake receipt is installed.
    drop(f);
}

#[test]
fn committed_hint_does_not_bypass_current_goal_node_provenance() {
    let mut f = Fixture::new(false);
    let (_run, session) = f.goal_session();
    f.wait_turn(&session.id);
    // Deliberate raw-corruption negative after a real approved/owned turn.
    assert_eq!(f.daemon.store.conn.execute("UPDATE goal_run_nodes SET document=json_set(document,'$.node.goal','foreign unapproved replacement') WHERE session_id=?1",[&session.id]).unwrap(),1);
    let sequence = f.daemon.store.sequence().unwrap();
    assert_eq!(
        f.daemon.service_hinted_goal_stops().unwrap_err().code,
        "INPUT_CHANGED"
    );
    assert_eq!(f.stop_requests(&session.id), 0);
    assert_eq!(f.daemon.store.sequence().unwrap(), sequence);
    assert_ne!(
        f.daemon.store.get_session(&session.id).unwrap().state,
        "stopped"
    );
    assert!(f.daemon.supervisor.owns(&session.id));
    assert_eq!(
        f.daemon.service_goal_stop_requests().unwrap_err().code,
        "INPUT_CHANGED",
        "outer fresh sweep still rejects the corrupt source after the hint is consumed"
    );
    assert_eq!(f.daemon.store.sequence().unwrap(), sequence);
    drop(f);
}

#[test]
fn pending_goal_cancel_still_uses_the_outer_fresh_sweep_without_a_hint() {
    let mut f = Fixture::new(true);
    let (run, session) = f.goal_session();
    let turn_before = f.pending_turn_before(&session.id);
    let until = Instant::now() + Duration::from_secs(5);
    while f.daemon.store.conn.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='session.update'",[&session.id],|r|r.get::<_,i64>(0)).unwrap()==0 {
        f.daemon.supervisor.tick(&mut f.daemon.store).unwrap();
        assert!(Instant::now()<until,"real pending prompt must be observed");
        std::thread::sleep(Duration::from_millis(5));
    }
    f.assert_pending_turn_unchanged(&session.id, &turn_before);
    assert!(!f.daemon.supervisor.take_goal_stop_hint());
    let attempts: i64 = f
        .daemon
        .store
        .conn
        .query_row("SELECT count(*) FROM attempts", [], |r| r.get(0))
        .unwrap();
    let stop_tail = f.daemon.store.sequence().unwrap();
    let current = f.daemon.store.get_goal_run(&run.id).unwrap();
    f.daemon
        .store
        .cancel_goal_run(&run.id, current.revision, "cancel-pending-goal")
        .unwrap();
    assert_eq!(f.stop_requests(&session.id), 0);
    f.daemon.tick_goals().unwrap();
    assert_eq!(f.stop_requests(&session.id), 1);
    f.wait_stopped(&session.id);
    f.no_extra_execution_or_task_acceptance(attempts, stop_tail);
    let cancelled = f.daemon.store.get_goal_run(&run.id).unwrap();
    assert!(cancelled.cancel_requested);
    assert!(!cancelled.task_accepted);
}

#[test]
fn no_hint_skips_the_stop_scan_instead_of_masking_its_required_failure() {
    let directory = RetainedDirectory(Some(
        tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(Path::new("/tmp").canonicalize().unwrap())
            .unwrap(),
    ));
    let mut daemon = Daemon::bind(&directory.path().join("r")).unwrap();
    // Empty owned DB corruption is only a read-path probe; no authority is built.
    daemon
        .store
        .conn
        .execute_batch("DROP TABLE goal_runs;")
        .unwrap();
    let sequence = daemon.store.sequence().unwrap();
    daemon.service_hinted_goal_stops().unwrap();
    assert_eq!(daemon.store.sequence().unwrap(), sequence);
    assert_eq!(
        daemon.service_goal_stop_requests().unwrap_err().code,
        "INTERNAL_ERROR"
    );
    assert_eq!(daemon.store.sequence().unwrap(), sequence);
}

#[path = "factory_history_actor_tests.rs"]
mod history_actor_tests;
