// The local compound codec is retired. These controls exercise Main's sole
// typed observation and unchanged legacy wire. Original 5810 stays in Git.
fn transport_receipt(r: &GoalRun) -> Value {
    json!({"intent_id":"11111111-1111-4111-8111-111111111111", "run_id":r.id,
        "requested_run_revision":r.revision,"requested_ledger_revision":21,
        "observation":"advance_intent_recorded","execution_state_requires_get":true,
        "task_accepted":false})
}
#[test]
fn typed_admission_preserves_original_guard_and_exact_legacy_wrapper() {
    let r = run_sample();
    let original = sample().refusal(AncestorAdmissionGuard::Runtime);
    let expected =
        ancestor_admission_diagnostic(&original.code, original.details_ref.as_deref(), &r).unwrap();
    let mut observation = None;
    let wrapped = crate::ipc::recorded_goal_dispatch_error(
        &r.id,
        &transport_receipt(&r),
        original.clone(),
        &mut observation,
    );
    let mut legacy = original;
    legacy.side_effect_state = "confirmed".into();
    legacy.details_ref = Some("recorded-goal-advance:11111111-1111-4111-8111-111111111111".into());
    assert_eq!(
        serde_json::to_vec(&wrapped).unwrap(),
        serde_json::to_vec(&legacy).unwrap()
    );
    let actual = observation.unwrap().for_run(&wrapped.code, &r).unwrap();
    assert_eq!(actual, expected);
    let public = serde_json::to_string(&actual).unwrap();
    assert!(!public.contains("CANARY"));
    assert!(!public.contains("11111111"));
    assert!(!public.contains("recorded-goal-advance"));
}
#[test]
fn typed_non_guard_errors_keep_exact_original_wrapper_bytes_without_a_sample() {
    let r = run_sample();
    for code in [
        "RESOURCE_LIMIT",
        "BUDGET_UNKNOWN",
        "INPUT_CHANGED",
        "OUTCOME_UNKNOWN",
    ] {
        for state in ["not_started", "confirmed", "unknown"] {
            for reference in [
                None,
                Some("PRIVATE_CANARY".to_string()),
                Some(format!("{ANCESTOR_ADMISSION_PREFIX}{{}}")),
            ] {
                let mut original = CoreError::new(code, "original message");
                original.retryable = true;
                original.side_effect_state = state.into();
                original.details_ref = reference;
                for receipt in [
                    transport_receipt(&r),
                    json!({}),
                    json!({"intent_id":"opaque-original-id"}),
                ] {
                    let mut old = original.clone();
                    if old.side_effect_state == "not_started" {
                        old.side_effect_state = "confirmed".into();
                    }
                    old.details_ref = receipt["intent_id"]
                        .as_str()
                        .map(|id| format!("recorded-goal-advance:{id}"));
                    let mut observation = None;
                    let actual = crate::ipc::recorded_goal_dispatch_error(
                        &r.id,
                        &receipt,
                        original.clone(),
                        &mut observation,
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual).unwrap(),
                        serde_json::to_vec(&old).unwrap()
                    );
                    assert!(observation.is_none());
                }
            }
        }
    }
}
#[test]
fn typed_observation_cannot_cross_run_revision_plan_or_refusal_code() {
    let r = run_sample();
    let original = sample().refusal(AncestorAdmissionGuard::Runtime);
    let observation = GoalAdmissionObservation::capture(&r.id, &original).unwrap();
    assert!(observation.for_run("RESOURCE_LIMIT", &r).is_some());
    let mut foreign = r.clone();
    foreign.id = "FOREIGN_RUN".into();
    assert!(observation.for_run("RESOURCE_LIMIT", &foreign).is_none());
    let mut stale = r.clone();
    stale.revision += 1;
    assert!(observation.for_run("RESOURCE_LIMIT", &stale).is_none());
    stale = r.clone();
    stale.plan_revision += 1;
    assert!(observation.for_run("RESOURCE_LIMIT", &stale).is_none());
    for code in [
        "INPUT_CHANGED",
        "BUDGET_UNKNOWN",
        "OUTCOME_UNKNOWN",
        "WORKSPACE_BUSY",
    ] {
        assert!(observation.for_run(code, &r).is_none());
        let mut other = original.clone();
        other.code = code.into();
        assert!(GoalAdmissionObservation::capture(&r.id, &other).is_none());
    }
}
#[test]
fn typed_inner_sample_rejects_duplicates_extras_and_invalid_finite_counts() {
    let r = run_sample();
    let good = serde_json::to_value(sample()).unwrap();
    let mut cases = vec![];
    for (key, value) in [
        ("private_path", json!("PRIVATE_PATH_CANARY")),
        ("relevant_count", json!(100_001)),
        ("active_count", json!(5)),
        ("runtime_accounted_count", json!(5)),
        ("output_accounted_count", json!(5)),
        ("run_revision", json!(14)),
        ("plan_revision", json!(3)),
    ] {
        let mut invalid = good.clone();
        invalid[key] = value;
        cases.push((invalid.to_string(), key == "private_path"));
    }
    cases.push((
        good.to_string().replacen(
            "\"run_revision\":13",
            "\"run_revision\":13,\"run_revision\":13",
            1,
        ),
        true,
    ));
    for (raw, capture_rejected) in cases {
        let mut error = sample().refusal(AncestorAdmissionGuard::Runtime);
        error.details_ref = Some(format!("{ANCESTOR_ADMISSION_PREFIX}{raw}"));
        assert!(
            ancestor_admission_diagnostic(&error.code, error.details_ref.as_deref(), &r).is_none()
        );
        let observation = GoalAdmissionObservation::capture(&r.id, &error);
        if capture_rejected {
            assert!(observation.is_none());
        }
        if let Some(observation) = observation {
            assert!(observation.for_run(&error.code, &r).is_none());
        }
    }
    // Main keeps Option decoding. Missing nullable keys are not reclassified
    // as invalid here, and no observation supplies admission authority.
}
#[test]
fn typed_observation_preserves_all_six_original_guard_constructor_outputs() {
    let r = run_sample();
    let mut count = sample();
    count.aggregation_complete = false;
    count.runtime_accounted_count = 0;
    count.output_accounted_count = 0;
    count.settled_actual_runtime_ms = None;
    count.unsettled_reserved_runtime_ms = None;
    count.settled_actual_output_bytes = None;
    count.unsettled_reserved_output_bytes = None;
    count.relevant_count = count.attempt_cap;
    let mut parallel = count.clone();
    parallel.relevant_count = 4;
    parallel.active_count = 1;
    let mut output = sample();
    output.runtime_cap_ms = 30000;
    output.output_cap_bytes = 1000;
    let mut runtime_overflow = sample();
    runtime_overflow.aggregation_complete = false;
    runtime_overflow.runtime_accounted_count = 1;
    runtime_overflow.output_accounted_count = 0;
    runtime_overflow.settled_actual_runtime_ms = Some(u64::MAX);
    runtime_overflow.settled_actual_output_bytes = Some(0);
    let mut output_overflow = sample();
    output_overflow.aggregation_complete = false;
    output_overflow.runtime_accounted_count = 1;
    output_overflow.output_accounted_count = 1;
    output_overflow.settled_actual_runtime_ms = Some(0);
    output_overflow.settled_actual_output_bytes = Some(u64::MAX);
    // Real error constructors, not a claim of six DB guard-branch executions.
    for (detail, guard) in [
        (count, AncestorAdmissionGuard::Attempts),
        (parallel, AncestorAdmissionGuard::Parallel),
        (sample(), AncestorAdmissionGuard::Runtime),
        (output, AncestorAdmissionGuard::Output),
        (runtime_overflow, AncestorAdmissionGuard::RuntimeOverflow),
        (output_overflow, AncestorAdmissionGuard::OutputOverflow),
    ] {
        let error = detail.refusal(guard);
        let observation = GoalAdmissionObservation::capture(&r.id, &error).unwrap();
        let projected = observation.for_run(&error.code, &r).unwrap();
        assert_eq!(projected["guard_kind"], json!(guard));
        assert_eq!(
            projected,
            serde_json::to_value(AncestorAdmissionDiagnostic {
                guard_kind: guard,
                ..detail
            })
            .unwrap()
        );
        let plain = limited();
        assert_eq!(error.code, plain.code);
        assert_eq!(error.message, plain.message);
        assert_eq!(error.retryable, plain.retryable);
        assert_eq!(error.side_effect_state, plain.side_effect_state);
    }
}
#[test]
fn typed_admission_bounds_apply_before_decode_without_truncation() {
    let r = run_sample();
    let mut error = sample().refusal(AncestorAdmissionGuard::Runtime);
    let raw = error
        .details_ref
        .as_ref()
        .unwrap()
        .strip_prefix(ANCESTOR_ADMISSION_PREFIX)
        .unwrap();
    let padded = format!("{raw}{}", " ".repeat(1024 - raw.len()));
    assert_eq!(padded.len(), 1024);
    error.details_ref = Some(format!("{ANCESTOR_ADMISSION_PREFIX}{padded}"));
    assert!(GoalAdmissionObservation::capture(&r.id, &error)
        .unwrap()
        .for_run(&error.code, &r)
        .is_some());
    error.details_ref.as_mut().unwrap().push(' ');
    assert!(GoalAdmissionObservation::capture(&r.id, &error).is_none());
    assert!(ancestor_admission_diagnostic(&error.code, error.details_ref.as_deref(), &r).is_none());
    error = sample().refusal(AncestorAdmissionGuard::Runtime);
    assert!(GoalAdmissionObservation::capture("", &error).is_none());
    assert!(GoalAdmissionObservation::capture(&"x".repeat(512), &error).is_some());
    assert!(GoalAdmissionObservation::capture(&"x".repeat(513), &error).is_none());
    error.details_ref = Some("PRIVATE_PATH_CANARY".into());
    assert!(GoalAdmissionObservation::capture(&r.id, &error).is_none());
}
#[test]
fn recorded_wire_cannot_recreate_the_private_owned_observation() {
    let r = run_sample();
    let original = sample().refusal(AncestorAdmissionGuard::Runtime);
    let mut observation = None;
    let wrapped = crate::ipc::recorded_goal_dispatch_error(
        &r.id,
        &transport_receipt(&r),
        original,
        &mut observation,
    );
    assert!(observation.is_some());
    let wire = json!({"error":wrapped});
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    crate::ipc::ReadyReply::Wire(send)
        .try_send_observed(crate::ipc::GoalAdvanceReply::new(wire.clone(), observation))
        .unwrap();
    let received = receive.try_recv().unwrap();
    assert_eq!(received, wire);
    assert!(receive.try_recv().is_err());
    assert!(ancestor_admission_diagnostic(
        "RESOURCE_LIMIT",
        received["error"]["details_ref"].as_str(),
        &r
    )
    .is_none());
    assert!(ancestor_admission_diagnostic(
        "RESOURCE_LIMIT",
        Some("recorded-goal-advance:11111111-1111-4111-8111-111111111111|budget-admission-v1:{}"),
        &r
    )
    .is_none());
}
