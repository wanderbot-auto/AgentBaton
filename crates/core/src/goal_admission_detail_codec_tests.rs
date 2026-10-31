// Source-only diagnostic codecs. No database, provider or execution witness is
// invented by these tests; the actual Store block path has separate tests.
fn transport_receipt(r: &GoalRun) -> Value {
    json!({
        "run_id":r.id,"intent_id":"11111111-1111-4111-8111-111111111111",
        "requested_run_revision":r.revision,"requested_ledger_revision":99,
        "observation":"advance_intent_recorded","execution_state_requires_get":true,
        "task_accepted":false
    })
}
fn marked_error(detail: &AncestorAdmissionDiagnostic, r: &GoalRun) -> CoreError {
    Store::recorded_goal_advance_error(
        detail.refusal(detail.guard_kind),
        &transport_receipt(r),
        &r.id,
        r.revision,
    )
}
fn split_recorded(reference: &str) -> (&str, &str) {
    reference.split_once(RECORDED_ADMISSION_SEPARATOR).unwrap()
}

#[test]
fn recorded_admission_transports_original_guard_and_marker_without_authority_fields() {
    let r = run_sample();
    let original = sample().refusal(AncestorAdmissionGuard::Runtime);
    let expected =
        ancestor_admission_diagnostic(&original.code, original.details_ref.as_deref(), &r).unwrap();
    let wrapped = Store::recorded_goal_advance_error(
        original.clone(),
        &transport_receipt(&r),
        &r.id,
        r.revision,
    );
    assert_eq!(wrapped.code, original.code);
    assert_eq!(wrapped.message, original.message);
    assert_eq!(wrapped.retryable, original.retryable);
    assert_eq!(wrapped.side_effect_state, "confirmed");
    let reference = wrapped.details_ref.as_ref().unwrap();
    assert!(reference.len() <= MAX_RECORDED_ADMISSION_BYTES);
    let (legacy, encoded) = split_recorded(reference);
    assert_eq!(
        legacy,
        "recorded-goal-advance:11111111-1111-4111-8111-111111111111"
    );
    let detail: RecordedAdmissionDetail = serde_json::from_str(encoded).unwrap();
    assert_eq!(detail.run_id, r.id);
    assert_eq!(detail.run_revision, r.revision);
    assert_eq!(detail.intent_id, "11111111-1111-4111-8111-111111111111");
    assert_eq!(Some(detail.original_guard), original.details_ref);
    let actual = ancestor_admission_diagnostic(&wrapped.code, Some(reference), &r).unwrap();
    assert_eq!(actual, expected);
    let public = serde_json::to_string(&actual).unwrap();
    assert!(!public.contains("CANARY"));
    assert!(!public.contains("11111111"));
    assert!(!public.contains("recorded-goal-advance"));
}

#[test]
fn recorded_non_guard_errors_keep_the_exact_original_error_wrapper_bytes() {
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
                    let actual = Store::recorded_goal_advance_error(
                        original.clone(),
                        &receipt,
                        &r.id,
                        r.revision,
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual).unwrap(),
                        serde_json::to_vec(&old).unwrap()
                    );
                }
            }
        }
    }
}

#[test]
fn recorded_encoder_requires_the_actual_closed_receipt_run_and_revision() {
    let r = run_sample();
    let original = sample().refusal(AncestorAdmissionGuard::Runtime);
    let mut cases = vec![];
    for (key, bad) in [
        ("run_id", json!("FOREIGN_RUN")),
        ("requested_run_revision", json!(14)),
        ("requested_run_revision", json!(13.0)),
        ("requested_ledger_revision", Value::Null),
        ("observation", json!("provider_started")),
        ("execution_state_requires_get", json!(false)),
        ("task_accepted", json!(true)),
        ("intent_id", json!("NOT_A_CANONICAL_UUID")),
        ("intent_id", json!("11111111-1111-4111-8111-111111111111\n")),
    ] {
        let mut value = transport_receipt(&r);
        value[key] = bad;
        cases.push(value);
    }
    let mut extra = transport_receipt(&r);
    extra["private"] = json!("CANARY");
    cases.push(extra);
    for key in [
        "run_id",
        "requested_run_revision",
        "requested_ledger_revision",
        "observation",
        "execution_state_requires_get",
        "task_accepted",
    ] {
        let mut value = transport_receipt(&r);
        value.as_object_mut().unwrap().remove(key);
        cases.push(value);
    }
    for receipt in cases {
        let wrapped =
            Store::recorded_goal_advance_error(original.clone(), &receipt, &r.id, r.revision);
        assert_eq!(
            wrapped.details_ref,
            receipt["intent_id"]
                .as_str()
                .map(|id| format!("recorded-goal-advance:{id}"))
        );
        assert!(
            ancestor_admission_diagnostic(&wrapped.code, wrapped.details_ref.as_deref(), &r)
                .is_none()
        );
    }
    for (run_id, revision) in [("FOREIGN_RUN", r.revision), (r.id.as_str(), r.revision + 1)] {
        let wrapped = Store::recorded_goal_advance_error(
            original.clone(),
            &transport_receipt(&r),
            run_id,
            revision,
        );
        assert!(!wrapped
            .details_ref
            .unwrap()
            .contains(RECORDED_ADMISSION_SEPARATOR));
    }
}

#[test]
fn recorded_decoder_refuses_foreign_stale_extra_missing_and_duplicate_bindings_whole() {
    let r = run_sample();
    let good = marked_error(&sample(), &r).details_ref.unwrap();
    let (legacy, raw) = split_recorded(&good);
    let parsed: Value = serde_json::from_str(raw).unwrap();
    let mut cases = vec![];
    for (key, bad) in [
        ("run_id", json!("FOREIGN_RUN")),
        ("run_revision", json!(14)),
        ("run_revision", json!(13.0)),
        ("intent_id", json!("22222222-2222-4222-8222-222222222222")),
        ("private", json!("CANARY")),
    ] {
        let mut value = parsed.clone();
        value[key] = bad;
        cases.push(format!("{legacy}{RECORDED_ADMISSION_SEPARATOR}{value}"));
    }
    for key in ["run_id", "run_revision", "intent_id", "original_guard"] {
        let mut value = parsed.clone();
        value.as_object_mut().unwrap().remove(key);
        cases.push(format!("{legacy}{RECORDED_ADMISSION_SEPARATOR}{value}"));
    }
    cases.push(format!(
        "{legacy}{RECORDED_ADMISSION_SEPARATOR}{}",
        raw.replacen(
            "\"run_revision\":13",
            "\"run_revision\":13,\"run_revision\":13",
            1
        )
    ));
    cases.push(good.replace("11111111-1111-4111-8111-111111111111", "bad-intent"));
    cases.push(good.replacen(
        "11111111-1111-4111-8111-111111111111",
        "22222222-2222-4222-8222-222222222222",
        1,
    ));
    cases.push(good.replace("budget-admission-v1", "budget-admission-v2"));
    cases.push(format!("{good}{RECORDED_ADMISSION_SEPARATOR}{raw}"));
    cases.push(format!("PRIVATE_PREFIX{good}"));
    for value in cases {
        assert!(
            ancestor_admission_diagnostic("RESOURCE_LIMIT", Some(&value), &r).is_none(),
            "invalid complete reference accepted"
        );
    }
    let mut foreign = r.clone();
    foreign.id = "FOREIGN_RUN".into();
    assert!(ancestor_admission_diagnostic("RESOURCE_LIMIT", Some(&good), &foreign).is_none());
    let mut stale = r.clone();
    stale.revision += 1;
    assert!(ancestor_admission_diagnostic("RESOURCE_LIMIT", Some(&good), &stale).is_none());
    stale = r;
    stale.plan_revision += 1;
    assert!(ancestor_admission_diagnostic("RESOURCE_LIMIT", Some(&good), &stale).is_none());
}

#[test]
fn recorded_guard_has_required_nullable_fields_and_rejects_inner_duplicates_or_extras() {
    let r = run_sample();
    let original = sample().refusal(AncestorAdmissionGuard::Runtime);
    let valid: Value = serde_json::from_str(
        original
            .details_ref
            .as_ref()
            .unwrap()
            .strip_prefix(ANCESTOR_ADMISSION_PREFIX)
            .unwrap(),
    )
    .unwrap();
    let mut cases = vec![];
    for key in [
        "settled_actual_runtime_ms",
        "unsettled_reserved_runtime_ms",
        "settled_actual_output_bytes",
        "unsettled_reserved_output_bytes",
    ] {
        let mut value = valid.clone();
        value.as_object_mut().unwrap().remove(key);
        cases.push(value.to_string());
    }
    let mut extra = valid.clone();
    extra["path"] = json!("PRIVATE_PATH_CANARY");
    cases.push(extra.to_string());
    cases.push(valid.to_string().replacen(
        "\"run_revision\":13",
        "\"run_revision\":13,\"run_revision\":13",
        1,
    ));
    for raw in cases {
        let mut error = original.clone();
        error.details_ref = Some(format!("{ANCESTOR_ADMISSION_PREFIX}{raw}"));
        assert!(
            ancestor_admission_diagnostic(&error.code, error.details_ref.as_deref(), &r).is_none()
        );
        let wrapped =
            Store::recorded_goal_advance_error(error, &transport_receipt(&r), &r.id, r.revision);
        assert!(!wrapped
            .details_ref
            .unwrap()
            .contains(RECORDED_ADMISSION_SEPARATOR));
    }
}

#[test]
fn recorded_transport_retains_all_six_original_guard_branch_shapes() {
    let r = run_sample();
    let mut count = sample();
    count.aggregation_complete = false;
    count.runtime_accounted_count = 0;
    count.output_accounted_count = 0;
    count.settled_actual_runtime_ms = None;
    count.unsettled_reserved_runtime_ms = None;
    count.settled_actual_output_bytes = None;
    count.unsettled_reserved_output_bytes = None;
    count.guard_kind = AncestorAdmissionGuard::Attempts;
    count.relevant_count = count.attempt_cap;
    let mut parallel = count.clone();
    parallel.guard_kind = AncestorAdmissionGuard::Parallel;
    parallel.relevant_count = 4;
    parallel.active_count = 1;
    let mut output = sample();
    output.guard_kind = AncestorAdmissionGuard::Output;
    output.runtime_cap_ms = 30000;
    output.output_cap_bytes = 1000;
    let mut runtime_overflow = sample();
    runtime_overflow.guard_kind = AncestorAdmissionGuard::RuntimeOverflow;
    runtime_overflow.aggregation_complete = false;
    runtime_overflow.runtime_accounted_count = 1;
    runtime_overflow.output_accounted_count = 0;
    runtime_overflow.settled_actual_runtime_ms = Some(u64::MAX);
    runtime_overflow.settled_actual_output_bytes = Some(0);
    let mut output_overflow = sample();
    output_overflow.guard_kind = AncestorAdmissionGuard::OutputOverflow;
    output_overflow.aggregation_complete = false;
    output_overflow.runtime_accounted_count = 1;
    output_overflow.output_accounted_count = 1;
    output_overflow.settled_actual_runtime_ms = Some(0);
    output_overflow.settled_actual_output_bytes = Some(u64::MAX);
    for detail in [
        count,
        parallel,
        sample(),
        output,
        runtime_overflow,
        output_overflow,
    ] {
        let wrapped = marked_error(&detail, &r);
        assert!(wrapped
            .details_ref
            .as_ref()
            .unwrap()
            .contains(RECORDED_ADMISSION_SEPARATOR));
        assert!(
            ancestor_admission_diagnostic(&wrapped.code, wrapped.details_ref.as_deref(), &r)
                .is_some()
        );
    }
    let mut invalid = sample();
    invalid.relevant_count = 100_001;
    assert!(!marked_error(&invalid, &r)
        .details_ref
        .unwrap()
        .contains(RECORDED_ADMISSION_SEPARATOR));
    let mut earlier_count_refusal = sample();
    earlier_count_refusal.attempt_cap = earlier_count_refusal.relevant_count;
    assert!(!marked_error(&earlier_count_refusal, &r)
        .details_ref
        .unwrap()
        .contains(RECORDED_ADMISSION_SEPARATOR));
    let mut earlier_parallel_refusal = sample();
    earlier_parallel_refusal.parallel_cap = 0;
    assert!(!marked_error(&earlier_parallel_refusal, &r)
        .details_ref
        .unwrap()
        .contains(RECORDED_ADMISSION_SEPARATOR));
    let mut not_a_refusal = sample();
    not_a_refusal.runtime_cap_ms = 30000;
    assert!(!marked_error(&not_a_refusal, &r)
        .details_ref
        .unwrap()
        .contains(RECORDED_ADMISSION_SEPARATOR));
}

#[test]
fn recorded_admission_bounds_apply_before_optional_decoding_without_truncation() {
    let r = run_sample();
    let good = marked_error(&sample(), &r).details_ref.unwrap();
    assert!(good.len() < MAX_RECORDED_ADMISSION_BYTES);
    let bounded = format!(
        "{good}{}",
        " ".repeat(MAX_RECORDED_ADMISSION_BYTES - good.len())
    );
    assert_eq!(bounded.len(), MAX_RECORDED_ADMISSION_BYTES);
    assert!(ancestor_admission_diagnostic("RESOURCE_LIMIT", Some(&bounded), &r).is_some());
    assert!(ancestor_admission_diagnostic("RESOURCE_LIMIT", Some(&(bounded + " ")), &r).is_none());
    let mut error = sample().refusal(AncestorAdmissionGuard::Runtime);
    let padding = 1025
        - error
            .details_ref
            .as_ref()
            .unwrap()
            .strip_prefix(ANCESTOR_ADMISSION_PREFIX)
            .unwrap()
            .len();
    error
        .details_ref
        .as_mut()
        .unwrap()
        .push_str(&" ".repeat(padding));
    assert!(ancestor_admission_diagnostic(&error.code, error.details_ref.as_deref(), &r).is_none());
    let wrapped =
        Store::recorded_goal_advance_error(error, &transport_receipt(&r), &r.id, r.revision);
    assert!(!wrapped
        .details_ref
        .unwrap()
        .contains(RECORDED_ADMISSION_SEPARATOR));
}
