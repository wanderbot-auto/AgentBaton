// These use the existing real Native-approved, no-Provider Store fixture.
// A controlled finite guard tests transport only; it is not evidence of the
// original guard in the failed Intel run or a forged execution/stop witness.
fn detail_test_counts(store: &Store) -> Vec<i64> {
    [
        "sessions",
        "attempts",
        "dispatch_intents",
        "goal_graph_jobs",
        "goal_run_reservations",
        "factory_budget_reservations",
        "goal_ledgers",
        "approval_proposals",
        "execution_grants",
        "idempotency",
    ]
    .iter()
    .map(|table| {
        store
            .conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    })
    .collect()
}
fn controlled_admission_error(r: &GoalRun) -> CoreError {
    let mut error = limited();
    error.details_ref = Some(format!(
        "factory-ancestor-budget-v1:{}",
        json!({
            "guard_kind":"runtime","run_revision":r.revision,"plan_revision":r.plan_revision,
            "relevant_count":4,"active_count":0,"attempt_cap":5,"parallel_cap":1,
            "runtime_accounted_count":4,"output_accounted_count":4,
            "settled_actual_runtime_ms":14029,"unsettled_reserved_runtime_ms":0,
            "requested_runtime_ms":6000,"runtime_cap_ms":20000,
            "settled_actual_output_bytes":62,"unsettled_reserved_output_bytes":0,
            "requested_output_bytes":1024,"output_cap_bytes":8192,"aggregation_complete":true
        })
    ));
    error
}
fn blocked_detail_event(store: &Store, id: &str) -> Value {
    let events: Vec<String> = store.conn.prepare(
        "SELECT document FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.run_blocked' ORDER BY sequence"
    ).unwrap().query_map([id], |row| row.get(0)).unwrap()
        .collect::<std::result::Result<_, _>>().unwrap();
    assert_eq!(events.len(), 1);
    serde_json::from_str(&events[0]).unwrap()
}

#[test]
fn real_recorded_intent_error_wrapper_reaches_existing_block_event_without_new_dispatch() {
    let mut f = fixture(4);
    let before = f.store.get_goal_run(&f.run.id).unwrap();
    let request = GoalRunDispatch {
        run_id: before.id.clone(),
        expected_ledger_revision: f.store.sequence().unwrap(),
    };
    let (receipt, new) = f
        .store
        .admit_goal_advance(&request, before.revision, "detail-intent")
        .unwrap();
    assert!(new);
    assert_eq!(receipt["requested_run_revision"], before.revision);
    assert_eq!(
        f.store.get_goal_run(&before.id).unwrap().revision,
        before.revision,
        "Observe actual admit semantics; do not assume it advances the run revision"
    );
    let counts = detail_test_counts(&f.store);
    let sequence = f.store.sequence().unwrap();
    let original = controlled_admission_error(&before);
    // Call the exact production error wrapper used by the IPC dispatch branch,
    // then round-trip its actual CoreError DTO like the pending response path.
    let wrapped = Store::recorded_goal_advance_error(
        original.clone(),
        &receipt,
        &request.run_id,
        before.revision,
    );
    let transported: CoreError =
        serde_json::from_slice(&serde_json::to_vec(&wrapped).unwrap()).unwrap();
    assert_eq!(transported.code, original.code);
    assert_eq!(transported.message, original.message);
    assert_eq!(transported.retryable, original.retryable);
    assert_eq!(transported.side_effect_state, "confirmed");
    assert!(transported
        .details_ref
        .as_ref()
        .unwrap()
        .starts_with(&format!(
            "recorded-goal-advance:{}|budget-admission-v1:",
            receipt["intent_id"].as_str().unwrap()
        )));
    f.store
        .block_goal_run(
            &before.id,
            &transported.code,
            transported.details_ref.as_deref(),
        )
        .unwrap();
    let event = blocked_detail_event(&f.store, &before.id);
    assert_eq!(event["revision"], before.revision + 1);
    assert_eq!(event["payload"]["reason_code"], "RESOURCE_LIMIT");
    assert_eq!(event["payload"]["automatic_retry"], false);
    assert_eq!(event["payload"]["task_accepted"], false);
    let detail = &event["payload"]["budget_admission"];
    assert_eq!(detail["run_revision"], before.revision);
    assert_eq!(detail["plan_revision"], before.plan_revision);
    assert_eq!(detail["settled_actual_runtime_ms"], 14029);
    assert_eq!(detail["requested_runtime_ms"], 6000);
    assert_eq!(detail["runtime_cap_ms"], 20000);
    let public = serde_json::to_string(detail).unwrap();
    assert!(!public.contains(&before.id));
    assert!(!public.contains(receipt["intent_id"].as_str().unwrap()));
    assert_eq!(
        f.store.sequence().unwrap(),
        sequence + 1,
        "Only the original blocked event is written"
    );
    assert_eq!(detail_test_counts(&f.store), counts);
    let mut after = f.store.get_goal_run(&before.id).unwrap();
    assert_eq!(after.state, "blocked");
    assert!(!after.task_accepted);
    assert_eq!(after.revision, before.revision + 1);
    after.state = before.state.clone();
    after.revision = before.revision;
    after.updated_at = before.updated_at.clone();
    assert_eq!(json!(after), json!(before));
    let (replay, new) = f
        .store
        .admit_goal_advance(&request, before.revision, "detail-intent")
        .unwrap();
    assert!(!new);
    assert_eq!(replay, receipt);
    assert_eq!(f.store.sequence().unwrap(), sequence + 1);
    assert_eq!(detail_test_counts(&f.store), counts);
}

#[test]
fn real_block_keeps_resource_refusal_but_omits_a_foreign_optional_recorded_sample() {
    let mut f = fixture(4);
    let before = f.store.get_goal_run(&f.run.id).unwrap();
    let request = GoalRunDispatch {
        run_id: before.id.clone(),
        expected_ledger_revision: f.store.sequence().unwrap(),
    };
    let (receipt, _) = f
        .store
        .admit_goal_advance(&request, before.revision, "foreign-detail-intent")
        .unwrap();
    let error = Store::recorded_goal_advance_error(
        controlled_admission_error(&before),
        &receipt,
        &request.run_id,
        before.revision,
    );
    let reference = error.details_ref.unwrap();
    let (legacy, raw) = reference.split_once("|budget-admission-v1:").unwrap();
    let mut value: Value = serde_json::from_str(raw).unwrap();
    value["run_id"] = json!("foreign-recorded-run");
    let foreign = format!("{legacy}|budget-admission-v1:{value}");
    let sequence = f.store.sequence().unwrap();
    let counts = detail_test_counts(&f.store);
    f.store
        .block_goal_run(&before.id, "RESOURCE_LIMIT", Some(&foreign))
        .unwrap();
    let event = blocked_detail_event(&f.store, &before.id);
    assert!(event["payload"].get("budget_admission").is_none());
    assert_eq!(event["payload"]["reason_code"], "RESOURCE_LIMIT");
    assert_eq!(event["payload"]["automatic_retry"], false);
    assert_eq!(f.store.get_goal_run(&before.id).unwrap().state, "blocked");
    assert_eq!(f.store.sequence().unwrap(), sequence + 1);
    assert_eq!(detail_test_counts(&f.store), counts);
}

#[test]
fn real_legacy_recorded_intent_still_replays_without_an_optional_guard_sample() {
    let mut f = fixture(4);
    let before = f.store.get_goal_run(&f.run.id).unwrap();
    let request = GoalRunDispatch {
        run_id: before.id.clone(),
        expected_ledger_revision: f.store.sequence().unwrap(),
    };
    let (receipt, _) = f
        .store
        .admit_goal_advance(&request, before.revision, "legacy-detail-intent")
        .unwrap();
    let original = CoreError::new("RESOURCE_LIMIT", "Original plain refusal");
    let wrapped = Store::recorded_goal_advance_error(
        original.clone(),
        &receipt,
        &request.run_id,
        before.revision,
    );
    let mut old = original;
    old.side_effect_state = "confirmed".into();
    old.details_ref = Some(format!(
        "recorded-goal-advance:{}",
        receipt["intent_id"].as_str().unwrap()
    ));
    assert_eq!(
        serde_json::to_vec(&wrapped).unwrap(),
        serde_json::to_vec(&old).unwrap()
    );
    let sequence = f.store.sequence().unwrap();
    let counts = detail_test_counts(&f.store);
    f.store
        .block_goal_run(&before.id, &wrapped.code, wrapped.details_ref.as_deref())
        .unwrap();
    assert!(blocked_detail_event(&f.store, &before.id)["payload"]
        .get("budget_admission")
        .is_none());
    let (again, new) = f
        .store
        .admit_goal_advance(&request, before.revision, "legacy-detail-intent")
        .unwrap();
    assert!(!new);
    assert_eq!(again, receipt);
    assert_eq!(f.store.sequence().unwrap(), sequence + 1);
    assert_eq!(detail_test_counts(&f.store), counts);
}
