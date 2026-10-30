//! Real generated ACP/MCP/Actor plus a precise post-stop SQL duration override.
//! This tests refusal containment, not a natural performance/CI reproduction.
use super::*;
use serde_json::Value;

#[derive(Clone, Copy, Debug)]
enum Fault {
    RetainedOverrun,
    MissingReservation,
    MissingStop,
    OversizeNodeRecords,
}
struct OwnedActor {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<agentbaton_core::Result<()>>>,
    db: PathBuf,
    evidence: PathBuf,
    outcome: Value,
}
impl OwnedActor {
    fn finish(&mut self) -> agentbaton_core::Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        match self.thread.take().unwrap().join() {
            Ok(result) => {
                self.outcome = json!({"joined":true,"daemon_result":if result.is_ok(){"ok"}else{"error"},"error_code":result.as_ref().err().map(|e|e.code.as_str())});
                result
            }
            Err(panic) => {
                self.outcome = json!({"joined":true,"daemon_result":"panic"});
                std::panic::resume_unwind(panic)
            }
        }
    }
}
impl Drop for OwnedActor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            self.outcome = match thread.join() {
                Ok(result) => {
                    json!({"joined":true,"daemon_result":if result.is_ok(){"ok"}else{"error"},"error_code":result.err().map(|e|e.code)})
                }
                Err(_) => json!({"joined":true,"daemon_result":"panic"}),
            };
        }
        // Preserve private failure evidence without masking the first panic.
        // This main-file copy is not a WAL snapshot or a new stop proof.
        let _ = fs::copy(&self.db, self.evidence.join("runtime.raw.sqlite3")).and_then(|_| {
            fs::set_permissions(
                self.evidence.join("runtime.raw.sqlite3"),
                fs::Permissions::from_mode(0o600),
            )
        });
        let _ = fs::write(self.evidence.join("actor-outcome.json"), serde_json::to_vec_pretty(&json!({"actual_owned_actor":self.outcome,"test_panicking":std::thread::panicking(),"raw_main_copy_not_snapshot_proof":true})).unwrap_or_default());
    }
}

#[test]
fn actual_retained_overrun_blocks_factory_but_actor_reads_and_owner_cancel_survive() {
    actual_containment(Fault::RetainedOverrun);
}
#[test]
fn actual_missing_reservation_is_not_disguised_as_confirmed_budget_block() {
    actual_containment(Fault::MissingReservation);
}
#[test]
fn actual_missing_stop_proof_is_not_disguised_as_confirmed_budget_block() {
    actual_containment(Fault::MissingStop);
}
#[test]
fn actual_raw_resource_limit_is_not_caught_as_confirmed_budget_block() {
    actual_containment(Fault::OversizeNodeRecords);
}
fn actual_containment(fault: Fault) {
    let mut f = Fixture::new();
    let companion = PathBuf::from(std::env::var_os("AGENTBATON_CHILD_GOAL_COMPANION").unwrap());
    assert!(companion.is_absolute());
    fs::copy(&companion, &f.companion).unwrap();
    fs::set_permissions(&f.companion, fs::Permissions::from_mode(0o500)).unwrap();
    assert_eq!(
        sha256(&fs::read(&companion).unwrap()),
        sha256(&fs::read(&f.companion).unwrap())
    );
    let provider = f
        ._temp
        .path()
        .canonicalize()
        .unwrap()
        .join("generated-containment-acp");
    let script = child_goal_provider(
        &f.selection.connection_ids[0],
        &f.selection.workspace_ids[0],
        false,
        false,
    );
    fs::write(&provider, &script).unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o700)).unwrap();
    let db = Connection::open(&f.path).unwrap();
    let raw: String = db
        .query_row(
            "SELECT document FROM connections WHERE id=?1",
            [&f.selection.connection_ids[0]],
            |r| r.get(0),
        )
        .unwrap();
    let mut connection: Value = serde_json::from_str(&raw).unwrap();
    connection["executable"] = json!(provider);
    connection["executable_sha256"] = json!(sha256(script.as_bytes()));
    let plan = RuntimePlan::empty(RuntimeProvider::CustomAcp);
    connection["config_revision"] = json!(sha256(&serde_json::to_vec(&json!({"arguments":[],"protocol_route":"custom_acp","runtime_plan_digest":plan.plan_digest().unwrap(),"host_capabilities":{"fs":false,"terminal":false}})).unwrap()));
    db.execute(
        "UPDATE connections SET document=?1 WHERE id=?2",
        params![connection.to_string(), f.selection.connection_ids[0]],
    )
    .unwrap();
    // Same approved basic child Goal fixture; do not enlarge any runtime cap.
    f.selection.root_budget.output_bytes = 12288;
    f.selection.project_budget.output_bytes = 12288;
    let _unexecuted_native = prepare_mixed_goal(&mut f);
    f.selection.capabilities.child_planning = true;
    f.selection.capabilities.child_goal_execution = true;
    f.selection.child_planning_policy = Some(FactoryChildPlanningPolicy {
        max_nodes: 4,
        max_revisions: 8,
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
            "Reject this exact candidate solely to exercise stopped-failure containment".into(),
    });
    f.selection.child_goal_execution_policy = Some(FactoryChildGoalExecutionPolicy {
        max_nodes: 2,
        max_runs_per_task: 1,
        max_total_runs: 2,
        root_budget: f.selection.root_budget.clone(),
        project_budget: f.selection.project_budget.clone(),
        require_owner_acceptance: true,
    });
    let scope = f.scope();
    let runtime = f._temp.path().canonicalize().unwrap();
    drop(f.store);
    let daemon = Daemon::bind(&runtime).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let workerstop = stop.clone();
    let evidence = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../artifacts/factory-child-goal-core/containment-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&evidence).unwrap();
    fs::set_permissions(&evidence, fs::Permissions::from_mode(0o700)).unwrap();
    let mut actor = OwnedActor {
        stop,
        thread: Some(std::thread::spawn(move || daemon.run(workerstop))),
        db: f.path.clone(),
        evidence: evidence.clone(),
        outcome: json!({"joined":false,"daemon_result":"not_observed"}),
    };
    let started = mixed_call(
        &runtime,
        json!({"method":"factory_runs.start","params":{"scope_id":scope.id},"expected_revision":scope.revision,"idempotency_key":"containment-start"}),
    );
    assert_eq!(started["scope_id"], scope.id);
    let until = Instant::now() + Duration::from_secs(15);
    let view = loop {
        let detail = mixed_call(
            &runtime,
            json!({"method":"factory_runs.get","params":{"scope_id":scope.id}}),
        );
        let child = detail["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| !t["parent_task_id"].is_null());
        if let Some(child) = child {
            let summaries = mixed_call(
                &runtime,
                json!({"method":"factory_child_goals.list","params":{"scope_id":scope.id,"task_id":child["task_id"],"goal_run_id":null}}),
            );
            if let Some(summary) = summaries.as_array().unwrap().first() {
                if summary["goal"]["run"]["state"] == "awaiting_acceptance" {
                    break mixed_call(
                        &runtime,
                        json!({"method":"factory_child_goals.get","params":{"scope_id":scope.id,"task_id":child["task_id"],"goal_run_id":summary["goal"]["run"]["id"]}}),
                    );
                }
            }
        }
        assert!(
            Instant::now() < until,
            "original15s candidate observation expired"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let details = &view["details"];
    let goal = details["run"]["id"].as_str().unwrap();
    assert!(uuid::Uuid::parse_str(goal).is_ok());
    let work = details["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["node"]["id"] == "fixed-work")
        .unwrap();
    assert_eq!(work["stop_confirmed"], true);
    assert_eq!(work["node"]["reservation"]["runtime_ms"], 4000);
    let actual = work["actual_runtime_ms"].as_u64().unwrap();
    assert!(
        actual <= 4000,
        "natural overrun before deliberate control is not this test's evidence"
    );
    assert!(work["actual_output_bytes"].is_u64());
    assert_eq!(details["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(details["task_accepted"], false);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM dispatch_intents WHERE state!='stopped'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
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
    let before_attempts: i64 = db
        .query_row("SELECT count(*) FROM attempts", [], |r| r.get(0))
        .unwrap();
    let before_keys: i64 = db
        .query_row("SELECT count(*) FROM idempotency", [], |r| r.get(0))
        .unwrap();
    let before_budget_rows: Vec<(String, i64, i64)> = db
        .prepare("SELECT scope,consumed,active FROM budgets ORDER BY scope")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let before_fees: i64 = db
        .query_row(
            "SELECT sum(json_extract(document,'$.runtime_ms')) FROM goal_ledgers",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let proposal = mixed_call(
        &runtime,
        json!({"method":"goal_node_approvals.propose","params":{"run_id":goal,"node_id":"owner-report","expected_plan_revision":1,"expected_ledger_revision":details["ledger_revision"]},"expected_revision":details["run"]["revision"],"idempotency_key":"containment-owner-review"}),
    );
    // Execute only after the real Owner rejection, itself after real end_turn,
    // owned reap and candidate capture. No production delay/test RPC exists.
    let delta = 4001 - actual;
    let injected = match fault {
        Fault::RetainedOverrun => format!(r#"
UPDATE goal_run_reservations SET document=json_set(document,'$.actual_runtime_ms',4001) WHERE run_id='{goal}' AND node_id='fixed-work' AND json_extract(document,'$.settled')=1;
UPDATE goal_run_nodes SET document=json_set(document,'$.actual_runtime_ms',4001) WHERE run_id='{goal}' AND node_id='fixed-work' AND plan_revision=1;
UPDATE events SET document=json_set(document,'$.payload.actual_runtime_ms',4001,'$.payload.observed_runtime_lower_bound_ms',4001,'$.payload.budget_overrun',json('true')) WHERE json_extract(document,'$.aggregate_id')='{goal}' AND json_extract(document,'$.kind')='goal.node_settled' AND json_extract(document,'$.payload.node_id')='fixed-work';
UPDATE goal_ledgers SET document=json_set(document,'$.runtime_ms',json_extract(document,'$.runtime_ms')+{delta}) WHERE scope IN(SELECT 'root-grant:'||json_extract(document,'$.root_grant_id') FROM goal_runs WHERE id='{goal}' UNION SELECT 'project:'||project_id FROM goal_runs WHERE id='{goal}');
"#),
        Fault::MissingReservation => format!("DELETE FROM goal_run_reservations WHERE run_id='{goal}' AND node_id='fixed-work';"),
        Fault::MissingStop => format!("UPDATE events SET document=json_set(document,'$.kind','fixture.stop_proof_missing') WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.source')='baton_supervisor' AND json_extract(document,'$.aggregate_id') IN(SELECT session_id FROM goal_run_nodes WHERE run_id='{goal}' AND node_id='fixed-work' AND plan_revision=1);"),
        Fault::OversizeNodeRecords => format!("UPDATE goal_run_nodes SET document=json_set(document,'$.fixture_padding',hex(zeroblob(600000))) WHERE run_id='{goal}' AND node_id='fixed-work' AND plan_revision=1;"),
    };
    db.execute_batch(&format!(r#"CREATE TRIGGER fixture_post_stop_containment AFTER INSERT ON events
WHEN json_extract(NEW.document,'$.kind')='goal.owner_node_settled' AND json_extract(NEW.document,'$.aggregate_id')='{goal}' AND json_extract(NEW.document,'$.payload.decision')='rejected'
BEGIN {injected} END;"#)).unwrap();
    // Fault cases may destroy the just-created response's private evidence;
    // their assertion concerns exact Actor failure, not a successful response.
    let decision = call(
        &runtime,
        json!({"method":"approvals.decline","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"]}),
    );
    if matches!(fault, Fault::RetainedOverrun) {
        let decision = decision.unwrap();
        assert!(
            decision.get("error").is_none(),
            "Owner decline unexpectedly failed"
        );
        assert_eq!(decision["result"]["task_accepted"], false);
        let blocked = loop {
            let value = mixed_call(
                &runtime,
                json!({"method":"factory_runs.get","params":{"scope_id":scope.id}}),
            );
            if value["run"]["state"] == "blocked" {
                break value;
            }
            assert!(
                Instant::now() < until,
                "original15s containment observation expired"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(blocked["run"]["last_error"]["code"], "RESOURCE_LIMIT");
        assert_eq!(
            blocked["run"]["last_error"]["side_effect_state"],
            "confirmed"
        );
        let child = blocked["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["task_id"] == details["run"]["task_id"])
            .unwrap();
        assert_eq!(child["state"], "blocked");
        for _ in 0..3 {
            mixed_call(&runtime, json!({"method":"system.snapshot","params":{}}));
            let same = mixed_call(
                &runtime,
                json!({"method":"factory_runs.get","params":{"scope_id":scope.id}}),
            );
            assert_eq!(same["run"], blocked["run"]);
        }
        assert!(!actor.thread.as_ref().unwrap().is_finished());
        let cancelled = mixed_call(
            &runtime,
            json!({"method":"factory_runs.cancel","params":{"scope_id":scope.id},"expected_revision":blocked["run"]["revision"],"idempotency_key":"containment-owner-stop"}),
        );
        assert_eq!(cancelled["cancel_requested"], true);
        mixed_call(&runtime, json!({"method":"system.snapshot","params":{}}));
        actor.finish().unwrap();
        assert_eq!(
            db.query_row(
                "SELECT sum(json_extract(document,'$.runtime_ms')) FROM goal_ledgers",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            before_fees + i64::try_from(delta * 2).unwrap()
        );
        assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='factory.child_goal_rework_blocked' AND json_extract(document,'$.payload.reason_code')='RESOURCE_LIMIT' AND json_extract(document,'$.payload.planner_attempt_started')=0 AND json_extract(document,'$.payload.reservation_refunded')=0", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
    } else {
        while !actor.thread.as_ref().unwrap().is_finished() {
            assert!(
                Instant::now() < until,
                "original15s negative observation expired"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let error = actor
            .finish()
            .expect_err("corrupt/missing evidence must not become a budget-block success");
        assert_eq!(
            error.code,
            match fault {
                Fault::MissingReservation => "INTERNAL_ERROR",
                Fault::MissingStop => "PERMISSION_DENIED",
                Fault::OversizeNodeRecords => "RESOURCE_LIMIT",
                _ => unreachable!(),
            }
        );
        if matches!(fault, Fault::MissingReservation) {
            assert_eq!(error.side_effect_state, "unknown");
        }
        assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='factory.child_goal_rework_blocked'", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    }
    assert_eq!(
        db.query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        before_attempts
    );
    assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.factory_rework_reserved'", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    assert_eq!(db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.factory_failure_confirmed'", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM tasks WHERE json_extract(document,'$.state')='accepted'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    let after_budgets: Vec<(String, i64, i64)> = db
        .prepare("SELECT scope,consumed,active FROM budgets ORDER BY scope")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(after_budgets, before_budget_rows);
    let after_keys: i64 = db
        .query_row("SELECT count(*) FROM idempotency", [], |r| r.get(0))
        .unwrap();
    // Exactly one explicit proposal and, in the positive case, Owner cancel.
    assert_eq!(
        after_keys - before_keys,
        if matches!(fault, Fault::RetainedOverrun) {
            2
        } else {
            1
        }
    );
    fs::write(evidence.join("control.json"), serde_json::to_vec_pretty(&json!({"dimension":"generated ACP/MCP/real Actor; explicit post-stop SQL override, not natural overrun performance proof","fault":format!("{fault:?}"),"original_actual_runtime_ms":actual,"original_reservation_ms":4000,"injected_runtime_ms":if matches!(fault,Fault::RetainedOverrun){Some(4001)}else{None},"owned_actor_joined":true,"new_attempts":0,"budget_counters_unchanged":true,"task_accepted":false})).unwrap()).unwrap();
}
