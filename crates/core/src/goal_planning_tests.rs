//! Real Store/native approval transitions and generated local stdio ACP child.
//! No vendor executable, external network, daemon socket, or paid model call.
use super::*;
use crate::conversation_tests::{fixture_with_body, Fixture};
use crate::conversations::ConversationSupervisor;
use std::time::{Duration, Instant};
const PEER: &str = r#"
import sys,json,time
for line in sys.stdin:
 r=json.loads(line);m=r.get('method');i=r.get('id')
 if m=='initialize':result={'protocolVersion':1,'agentCapabilities':{}}
 elif m=='session/new':result={'sessionId':'planning-native'}
 elif m=='session/prompt':
  prompt=r['params']['prompt'][0]['text']
  context=json.loads(prompt.split('Approved context follows:\n',1)[1])
  advice={'schema_version':1,'source':context['source'],'steps':[{'id':'work','goal':'Produce approved report','dependencies':[],'input_artifact_ids':list(context['inputs'].keys()),'output_artifact_ids':[x['artifact_id'] for x in context['outputs']]}]}
  print(json.dumps({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'planning-native','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':json.dumps(advice)}}}}),flush=True)
  result={'stopReason':'end_turn'}
 elif m=='session/cancel':continue
 else:raise AssertionError('Unexpected native method')
 print(json.dumps({'jsonrpc':'2.0','id':i,'result':result}),flush=True)
"#;
fn fixture(body: &str) -> (Fixture, Task, GoalScopeView, GoalPlanningPrepareRequest) {
    let mut f = fixture_with_body(Some(body));
    let grant_proposal = f
        .store
        .propose_grant(
            &f.project.id,
            &f.workspace.id,
            &f.connection.id,
            8,
            4,
            10000,
            "planning-native-grant",
        )
        .unwrap();
    f.grant =
        serde_json::from_value(f.store.finalize_proposal(&grant_proposal.id, 1).unwrap()).unwrap();
    let task = f
        .store
        .create_registered(&f.project.id, "Produce an approved report", "planning-task")
        .unwrap();
    let budget = BudgetLimits {
        attempts: 8,
        parallel: 4,
        runtime_ms: 10000,
        output_bytes: 262144,
        children: 0,
        reworks: 0,
        tokens: None,
        cost_microunits: None,
    };
    let selection = GoalScopeSelection {
        task_id: task.id.clone(),
        expected_task_revision: task.revision,
        grant_id: f.grant.id.clone(),
        expected_grant_revision: f.grant.revision,
        max_nodes: 8,
        new_max_children: 7,
        new_max_depth: 1,
        root_budget: budget.clone(),
        project_budget: budget,
        inputs: vec![ArtifactRequirement {
            artifact_id: "input".into(),
            relative_path: "input".into(),
            max_bytes: 1024,
            expected_sha256: Some(format!("{:x}", Sha256::digest(b"fixture"))),
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
        owner_acceptance_criteria: "Owner verifies the report meets the requested goal".into(),
    };
    let p = f
        .store
        .propose_goal_scope(&selection, "planning-scope")
        .unwrap();
    let scope: GoalScopeView =
        serde_json::from_value(f.store.finalize_proposal(&p.id, 1).unwrap()).unwrap();
    let request = GoalPlanningPrepareRequest {
        query: GoalPlanAdviceQuery {
            scope_id: scope.id.clone(),
            task_id: task.id.clone(),
            expected_task_revision: task.revision,
            expected_spec_revision: task.spec_revision,
            expected_scope_revision: scope.revision,
            expected_scope_digest: scope.scope_digest.clone(),
            expected_authority_revision: scope.authority_revision,
        },
        runtime_ms: 5000,
        response_bytes: 8192,
    };
    (f, task, scope, request)
}
fn start(
    f: &mut Fixture,
    r: &GoalPlanningPrepareRequest,
    key: &str,
) -> (
    GoalPlanningPrepared,
    GoalPlanningStartRequest,
    GoalPlanningRun,
) {
    let p = f.store.prepare_goal_planning(r).unwrap();
    let request = GoalPlanningStartRequest {
        preparation: r.clone(),
        expected_digest: p.digest.clone(),
    };
    let (run, new) = f
        .store
        .start_goal_planning(&request, r.query.expected_task_revision, key)
        .unwrap();
    assert!(new);
    (p, request, run)
}
fn wait(f: &mut Fixture, s: &mut ConversationSupervisor, id: &str) -> GoalPlanningRun {
    let at = Instant::now();
    loop {
        s.tick(&mut f.store).unwrap();
        let run = f.store.get_goal_planning(id).unwrap();
        if matches!(run.conversation.state.as_str(), "stopped" | "failed") {
            assert!(!s.owns(id));
            return run;
        }
        assert!(
            at.elapsed() < Duration::from_secs(8),
            "state {} {:?}",
            run.conversation.state,
            run.conversation.last_error
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn database_snapshot(f: &Fixture) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
    let db = &f.store.conn;
    let names: Vec<String> = db
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    names
        .into_iter()
        .map(|name| {
            let mut statement = db
                .prepare(&format!(
                    "SELECT * FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                ))
                .unwrap();
            let columns = statement.column_count();
            let rows = statement
                .query_map([], |r| {
                    (0..columns)
                        .map(|i| r.get(i))
                        .collect::<rusqlite::Result<Vec<rusqlite::types::Value>>>()
                })
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap();
            (name, rows)
        })
        .collect()
}
fn running(f: &mut Fixture, s: &mut ConversationSupervisor, id: &str) {
    let at = Instant::now();
    loop {
        s.tick(&mut f.store).unwrap();
        if f.store.get_conversation(id).unwrap().state == "running" {
            return;
        }
        assert!(at.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn task_bytes(f: &Fixture, id: &str) -> String {
    f.store
        .conn
        .query_row("SELECT document FROM tasks WHERE id=?1", [id], |r| r.get(0))
        .unwrap()
}
#[test]
fn planning_generated_turn_owned_join_exact_reply_import_and_replay() {
    let (mut f, task, scope, r) = fixture(PEER);
    let original = task_bytes(&f, &task.id);
    let before = database_snapshot(&f);
    let prepared = f.store.prepare_goal_planning(&r).unwrap();
    assert_eq!(before, database_snapshot(&f));
    let (p, request, run) = start(&mut f, &r, "start");
    assert_eq!(prepared.digest, p.digest);
    assert_eq!(original, task_bytes(&f, &task.id));
    assert!(f
        .store
        .send_conversation(
            &run.conversation_id,
            run.conversation.revision,
            "do anything",
            "send"
        )
        .is_err());
    assert!(f
        .store
        .import_goal_plan_advice(&GoalPlanAdviceImportRequest {
            query: r.query.clone(),
            candidate_json: "{}".into()
        })
        .is_err());
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    supervisor
        .launch(&mut f.store, &run.conversation_id)
        .unwrap();
    let result = wait(&mut f, &mut supervisor, &run.conversation_id);
    assert!(result.reply_complete, "{:?}", result);
    assert!(result.worker_joined && result.process_reaped);
    assert!(result.observed_output_bytes.unwrap() > result.reply.as_ref().unwrap().len() as u64);
    let imported = f
        .store
        .import_goal_plan_advice(&GoalPlanAdviceImportRequest {
            query: r.query.clone(),
            candidate_json: result.reply.clone().unwrap(),
        })
        .unwrap();
    assert_eq!(imported.steps.len(), 1);
    assert_eq!(original, task_bytes(&f, &task.id));
    let replay = f
        .store
        .start_goal_planning(&request, task.revision, "start")
        .unwrap();
    assert!(!replay.1);
    assert_eq!(replay.0.conversation_id, result.conversation_id);
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT count(*) FROM conversation_turns WHERE conversation_id=?1",
                [&result.conversation_id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    let raw: String = f
        .store
        .conn
        .query_row(
            "SELECT document FROM conversation_budgets WHERE conversation_id=?1",
            [&result.conversation_id],
            |r| r.get(0),
        )
        .unwrap();
    let b: crate::conversations::ConversationBudget = decode(&raw).unwrap();
    assert_eq!(b.actual_output_bytes, result.observed_output_bytes);
    assert!(b.settled && b.metered);
    if let Some(path) = std::env::var_os("AGENTBATON_GOAL_PLANNING_FIXTURE") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(
                &json!({"task":task,"scope":scope,"request":r,"prepared":p,"run":result}),
            )
            .unwrap(),
        )
        .unwrap();
    }
    std::fs::write(f.workspace.root.join("input"), "changed after planning").unwrap();
    assert!(f
        .store
        .import_goal_plan_advice(&GoalPlanAdviceImportRequest {
            query: r.query,
            candidate_json: result.reply.clone().unwrap()
        })
        .is_err());
    assert_eq!(
        f.store
            .get_goal_planning(&result.conversation_id)
            .unwrap()
            .reply,
        result.reply
    );
}
#[test]
fn planning_stale_prepare_capacity_and_no_arbitrary_prompt() {
    let (mut f, task, _, mut r) = fixture(PEER);
    for (runtime, bytes) in [(0, 8192), (10001, 8192), (5000, 0), (5000, 65537)] {
        r.runtime_ms = runtime;
        r.response_bytes = bytes;
        assert!(f.store.prepare_goal_planning(&r).is_err());
    }
    r.runtime_ms = 5000;
    r.response_bytes = 8192;
    let p = f.store.prepare_goal_planning(&r).unwrap();
    let mut request = GoalPlanningStartRequest {
        preparation: r,
        expected_digest: p.digest,
    };
    request.expected_digest = "0".repeat(64);
    assert!(f
        .store
        .start_goal_planning(&request, task.revision, "bad")
        .is_err());
    assert_eq!(f.store.list_goal_planning(&task.id).unwrap().len(), 0);
    let extra = json!({"preparation":request.preparation,"expected_digest":request.expected_digest,"prompt":"execute arbitrary command"});
    assert!(serde_json::from_value::<GoalPlanningStartRequest>(extra).is_err());
}
#[test]
fn planning_output_limit_and_cancel_keep_unknown_meter_and_task_unchanged() {
    for cancel in [false, true] {
        let body = if cancel {
            PEER.replace("context=json.loads", "time.sleep(2)\n  context=json.loads")
        } else {
            PEER.to_owned()
        };
        let (mut f, task, _, mut r) = fixture(&body);
        if !cancel {
            r.response_bytes = 32
        }
        let original = task_bytes(&f, &task.id);
        let (_, _, run) = start(&mut f, &r, "start");
        let mut s = ConversationSupervisor::new(&f.runtime);
        s.launch(&mut f.store, &run.conversation_id).unwrap();
        if cancel {
            let at = Instant::now();
            loop {
                s.tick(&mut f.store).unwrap();
                let c = f.store.get_conversation(&run.conversation_id).unwrap();
                if c.state == "running" {
                    f.store
                        .cancel_goal_planning(&c.id, c.revision, "cancel")
                        .unwrap();
                    s.cancel(&c.id);
                    break;
                }
                assert!(at.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        let result = wait(&mut f, &mut s, &run.conversation_id);
        assert!(!result.reply_complete);
        assert!(result.reply.is_none());
        assert!(result.process_reaped && result.worker_joined);
        assert_eq!(original, task_bytes(&f, &task.id));
        let raw: String = f
            .store
            .conn
            .query_row(
                "SELECT document FROM conversation_budgets WHERE conversation_id=?1",
                [&result.conversation_id],
                |r| r.get(0),
            )
            .unwrap();
        let b: crate::conversations::ConversationBudget = decode(&raw).unwrap();
        assert!(b.actual_output_bytes.is_none());
    }
}
#[test]
fn planning_missing_purpose_and_forged_completion_fail_closed() {
    let (mut f, task, _, r) = fixture(PEER);
    let (_, request, run) = start(&mut f, &r, "start");
    f.store
        .conn
        .execute(
            "DELETE FROM idempotency WHERE scope=?1 AND key=?2",
            params![PURPOSE, run.conversation_id],
        )
        .unwrap();
    assert!(f.store.get_goal_planning(&run.conversation_id).is_err());
    assert!(f
        .store
        .send_conversation(
            &run.conversation_id,
            run.conversation.revision,
            "bad",
            "key"
        )
        .is_err());
    assert!(has_active_goal_planning(&f.store.conn, &task.id).is_err());
    assert_eq!(
        f.store
            .goal_planning_start_replay(&request, "start")
            .unwrap_err()
            .side_effect_state,
        "confirmed"
    );
    let (mut f, _, _, r) = fixture(PEER);
    let (_, _, run) = start(&mut f, &r, "start");
    let mut s = ConversationSupervisor::new(&f.runtime);
    s.launch(&mut f.store, &run.conversation_id).unwrap();
    wait(&mut f, &mut s, &run.conversation_id);
    let completion: String = f
        .store
        .conn
        .query_row(
            "SELECT result FROM idempotency WHERE scope=?1 AND key=?2",
            params![COMPLETION, run.conversation_id],
            |r| r.get(0),
        )
        .unwrap();
    f.store.conn.execute("UPDATE idempotency SET result=json_set(result,'$.reply','forged') WHERE scope=?1 AND key=?2",params![COMPLETION,run.conversation_id]).unwrap();
    assert!(f.store.get_goal_planning(&run.conversation_id).is_err());
    f.store
        .conn
        .execute(
            "UPDATE idempotency SET result=?1 WHERE scope=?2 AND key=?3",
            params![completion, COMPLETION, run.conversation_id],
        )
        .unwrap();
    f.store.conn.execute("UPDATE conversation_budgets SET document=json_set(document,'$.actual_output_bytes',0) WHERE conversation_id=?1",[&run.conversation_id]).unwrap();
    assert!(crate::conversations::budget_records(
        &f.store.conn,
        &f.grant.root_grant_id,
        &f.project.id
    )
    .is_err());
    assert!(f.store.get_goal_planning(&run.conversation_id).is_err());
}
#[test]
fn planning_task_cancel_requires_owned_stop_and_preserves_historical_replay() {
    let body = PEER.replace("context=json.loads", "time.sleep(2)\n  context=json.loads");
    let (mut f, task, scope, r) = fixture(&body);
    let (_, request, run) = start(&mut f, &r, "start");
    let mut s = ConversationSupervisor::new(&f.runtime);
    s.launch(&mut f.store, &run.conversation_id).unwrap();
    running(&mut f, &mut s, &run.conversation_id);
    let cancelled = f.store.cancel(&task.id, task.revision).unwrap();
    assert_eq!(cancelled.state, TaskState::CancelRequested);
    // Historical reads and exact original replay survive source cancellation.
    assert_eq!(
        f.store
            .goal_planning_start_replay(&request, "start")
            .unwrap()
            .unwrap()
            .conversation_id,
        run.conversation_id
    );
    assert!(f.store.get_goal_planning(&run.conversation_id).is_ok());
    let result = wait(&mut f, &mut s, &run.conversation_id);
    assert!(!result.reply_complete);
    let actual: Task = decode(&task_bytes(&f, &task.id)).unwrap();
    assert_eq!(actual.state, TaskState::Cancelled);
    assert!(f.store.get_goal_scope(&scope.id).is_ok());
}

#[test]
fn planning_restart_unknown_never_relaunches_and_revocation_blocks_first_turn() {
    let (mut f, task, _, r) = fixture(PEER);
    let (_, request, run) = start(&mut f, &r, "restart");
    f.store.recover_conversations().unwrap();
    let recovered = f.store.get_goal_planning(&run.conversation_id).unwrap();
    assert_eq!(recovered.conversation.state, "outcome_unknown");
    assert!(recovered.reply.is_none());
    let replay = f
        .store
        .start_goal_planning(&request, task.revision, "restart")
        .unwrap();
    assert!(!replay.1);
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    supervisor
        .launch(&mut f.store, &run.conversation_id)
        .unwrap();
    assert!(!supervisor.has_workers());
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT count(*) FROM conversation_turns WHERE conversation_id=?1",
                [&run.conversation_id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert!(has_active_goal_planning(&f.store.conn, &task.id).unwrap());
    let raw: String = f
        .store
        .conn
        .query_row(
            "SELECT document FROM conversation_budgets WHERE conversation_id=?1",
            [&run.conversation_id],
            |r| r.get(0),
        )
        .unwrap();
    let budget: crate::conversations::ConversationBudget = decode(&raw).unwrap();
    assert!(!budget.settled && budget.actual_output_bytes.is_none());
    let (mut f, task, scope, r) = fixture(PEER);
    let (_, request, run) = start(&mut f, &r, "revoke");
    f.store
        .revoke_goal_scope(&scope.id, scope.revision, "revoke-scope")
        .unwrap();
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    assert!(supervisor
        .launch(&mut f.store, &run.conversation_id)
        .is_err());
    supervisor
        .record_local_launch_failure(&mut f.store, &run.conversation_id)
        .unwrap();
    let stopped = f.store.get_goal_planning(&run.conversation_id).unwrap();
    assert_eq!(stopped.conversation.state, "failed");
    assert!(!stopped.reply_complete);
    assert_eq!(stopped.observed_output_bytes, Some(0));
    assert!(f
        .store
        .goal_planning_start_replay(&request, "revoke")
        .unwrap()
        .is_some());
    assert_eq!(task_bytes(&f, &task.id), encode(&task).unwrap());
    let (mut f, task, _, r) = fixture(PEER);
    let p = f.store.prepare_goal_planning(&r).unwrap();
    std::fs::write(f.workspace.root.join("input"), "changed before start").unwrap();
    assert!(f
        .store
        .start_goal_planning(
            &GoalPlanningStartRequest {
                preparation: r,
                expected_digest: p.digest
            },
            task.revision,
            "stale"
        )
        .is_err());
    assert!(f.store.list_goal_planning(&task.id).unwrap().is_empty());
}
#[test]
fn planning_owned_settlement_retries_task_cancel_after_local_database_error() {
    let body = PEER.replace("context=json.loads", "time.sleep(2)\n  context=json.loads");
    let (mut f, task, _, r) = fixture(&body);
    let (_, _, run) = start(&mut f, &r, "start");
    let mut supervisor = ConversationSupervisor::new(&f.runtime);
    supervisor
        .launch(&mut f.store, &run.conversation_id)
        .unwrap();
    running(&mut f, &mut supervisor, &run.conversation_id);
    f.store.cancel(&task.id, task.revision).unwrap();
    f.store.conn.execute_batch("CREATE TRIGGER test_cancel_finalize_failure BEFORE UPDATE ON tasks WHEN json_extract(NEW.document,'$.state')='cancelled' BEGIN SELECT RAISE(ABORT,'generated transient failure'); END;").unwrap();
    let clock = Instant::now();
    loop {
        if supervisor.tick(&mut f.store).is_err() {
            break;
        }
        assert!(clock.elapsed() < Duration::from_secs(8));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        f.store
            .get_goal_planning(&run.conversation_id)
            .unwrap()
            .conversation
            .state,
        "stopped"
    );
    assert_eq!(
        f.store.get(&task.id).unwrap().state,
        TaskState::CancelRequested
    );
    let meter: String = f
        .store
        .conn
        .query_row(
            "SELECT document FROM conversation_budgets WHERE conversation_id=?1",
            [&run.conversation_id],
            |r| r.get(0),
        )
        .unwrap();
    f.store
        .conn
        .execute_batch("DROP TRIGGER test_cancel_finalize_failure")
        .unwrap();
    supervisor.tick(&mut f.store).unwrap();
    assert_eq!(f.store.get(&task.id).unwrap().state, TaskState::Cancelled);
    assert!(!supervisor.has_workers());
    assert_eq!(
        meter,
        f.store
            .conn
            .query_row(
                "SELECT document FROM conversation_budgets WHERE conversation_id=?1",
                [&run.conversation_id],
                |r| r.get::<_, String>(0)
            )
            .unwrap()
    );
}
