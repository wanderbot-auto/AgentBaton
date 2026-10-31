use super::*;
const CANARY: &str = "PRIVATE_CANARY_DO_NOT_PUBLISH";
#[test]
fn private_pins_refuse_aliases_replacement_special_files_and_journals() {
    use std::os::unix::{
        ffi::OsStrExt,
        fs::{symlink, PermissionsExt},
    };
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = temp.path().join("data");
    fs::write(&path, [0u8; 100]).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let pin = Pin::open(&path, false).unwrap();
    assert!(Pin::open(temp.path(), false).is_err());
    let alias = temp.path().join("alias");
    symlink(&path, &alias).unwrap();
    assert!(Pin::open(&alias, false).is_err());
    fs::remove_file(alias).unwrap();
    let hard = temp.path().join("hard");
    fs::hard_link(&path, &hard).unwrap();
    assert!(Pin::open(&path, false).is_err());
    fs::remove_file(hard).unwrap();
    fs::rename(&path, temp.path().join("old")).unwrap();
    fs::write(&path, [0u8; 100]).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(pin.check().is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Pin::open(&path, false).is_err());
    let fifo = temp.path().join("fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(Pin::open(&fifo, false).is_err());
    assert!(no_journal(temp.path()).is_ok());
    fs::write(temp.path().join("baton.sqlite3-wal"), []).unwrap();
    assert!(no_journal(temp.path()).is_err());
    assert_eq!(
        uri(Path::new("/tmp/a b?é")).unwrap(),
        "file:/tmp/a%20b%3F%C3%A9?mode=ro&immutable=1"
    );
}
#[test]
fn actor_observation_preserves_original_finish_join_and_panic_behavior() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    for outcome in ["ok", "error", "panic"] {
        let mut server = super::super::super::Server {
            runtime: PathBuf::from("/never-opened-unit-server"),
            stop: Arc::new(AtomicBool::new(false)),
            thread: Some(std::thread::spawn(move || match outcome {
                "ok" => Ok(()),
                "error" => Err(CoreError::new("INTERNAL_ERROR", "unit-only failure")),
                _ => panic!("unit-only panic"),
            })),
            join_outcome: None,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| server.finish()));
        let expected = match outcome {
            "ok" => ActorJoinOutcome::Ok,
            "error" => ActorJoinOutcome::Error,
            _ => ActorJoinOutcome::Panic,
        };
        assert!(server.join_outcome == Some(expected));
        assert!(server.stop.load(Ordering::Acquire));
        assert!(server.thread.is_none());
        assert_eq!(result.is_ok(), outcome == "ok");
        server.finish(); // Consumed join stays consumed, including an observed error.
        assert!(server.join_outcome == Some(expected));
    }
}
#[test]
fn join_and_binding_gates_never_open_records_for_unknown_outcome() {
    let (b, _, _, _) = fixture();
    for actor in [
        None,
        Some(ActorJoinOutcome::Error),
        Some(ActorJoinOutcome::Panic),
    ] {
        let value = diagnostic(actor, Some(b.clone()), Some(&b.task), |_| {
            panic!("must not read")
        });
        assert_eq!(value["status"], "unobserved_actor_join");
    }
    for observed in [
        None,
        Some(Binding {
            task: "foreign".into(),
            ..b.clone()
        }),
    ] {
        let value = diagnostic(Some(ActorJoinOutcome::Ok), observed, Some(&b.task), |_| {
            panic!("must not read")
        });
        assert_eq!(value["status"], "unobserved_run_binding");
    }
    let value = diagnostic(
        Some(ActorJoinOutcome::Ok),
        Some(b.clone()),
        Some(&b.task),
        |_| Err(()),
    );
    assert_eq!(value["status"], "unobserved_records");
    let value = diagnostic(
        Some(ActorJoinOutcome::Ok),
        Some(b.clone()),
        Some(&b.task),
        |_| Ok(envelope("collected", "ok", json!([]))),
    );
    assert_eq!(value["status"], "collected");
    assert_eq!(value["process_stop_proven"], false);
}
fn fixture() -> (Binding, String, Vec<AttemptRow>, Vec<(i64, String)>) {
    let b = Binding {
        id: "run-private".into(),
        task: "task-private".into(),
        policy: "policy-private".into(),
    };
    let error = json!({"code":"OUTCOME_UNKNOWN","message":CANARY,"retryable":false,"side_effect_state":"unknown","details_ref":CANARY});
    let run = json!({"id":b.id,"policy_id":b.policy,"policy_revision":1,"revision":4,"task_id":b.task,"spec_revision":1,
        "goal_scope_id":CANARY,"scope_digest":CANARY,"acceptance_ref":CANARY,"acceptance_digest":CANARY,
        "source_commit":null,"source_tree":null,"target_ref":CANARY,"expected_target_head":CANARY,
        "integration_commit":null,"integration_tree":null,"round":1,"state":"outcome_unknown","cancel_requested":false,
        "active_stage":null,"io_completion":"interrupted_no_join_receipt","validation_receipts":[],"last_error":error,
        "created_at":CANARY,"updated_at":CANARY});
    let record = json!({"view":{"attempt_id":"attempt-private","round":1,"kind":"preparation","validator":null,"state":"outcome_unknown",
        "reservation":{"runtime_ms":2000,"output_bytes":1024,"tokens":null,"cost_microunits":null},"settled":false,
        "actual_runtime_ms":null,"actual_output_bytes":null,"io_completion":"interrupted_no_join_receipt","last_error":error,"receipt_sequence":null},
        "integration_id":b.id,"policy_id":b.policy,"policy_snapshot_digest":CANARY,"root_task_id":CANARY,"root_grant_id":CANARY,
        "project_id":CANARY,"task_id":b.task,"factory_scope_id":null,"ancestors":[],"intent_id":CANARY,
        "binding":{"connection_id":CANARY,"environment_id":CANARY,"identity_ref":CANARY,"host_id":CANARY,"workspace_id":CANARY,"config_revision":CANARY,"input_revision":CANARY,"capability_snapshot_id":CANARY,"policy_revision":CANARY},"prepared_at_ms":1,"queued_ms":2});
    let row = AttemptRow {
        id: "attempt-private".into(),
        run: b.id.clone(),
        round: 1,
        kind: "preparation".into(),
        state: "outcome_unknown".into(),
        authority: "integration".into(),
        root_task: CANARY.into(),
        root_grant: CANARY.into(),
        project: CANARY.into(),
        document: record.to_string(),
    };
    let unknown = event(
        &b,
        UNKNOWN,
        json!({"attempt_id":row.id,"observed_runtime_lower_bound_ms":2003,"historical_cas_confirmed":false,"worker_thread_joined":true,"reservation_refunded":false,"effects_admitted":true,"joined_error":{"code":"RESOURCE_LIMIT","retryable":false,"side_effect_state":"not_started"}}),
    );
    let preparation = event(
        &b,
        PREPARATION,
        json!({"attempt_id":row.id,"io":{"elapsed_ms":1880,"git_calls_attempted":4,"git_calls_complete":3,"git_stdout_observed_bytes":null,"git_stderr_observed_bytes":null,"effects_may_have_started":true,"interrupted":true,"io_complete":false},"measurement_not_authority":true,"whole_filesystem_output":null}),
    );
    (
        b,
        run.to_string(),
        vec![row],
        vec![(3, preparation.to_string()), (4, unknown.to_string())],
    )
}
fn event(b: &Binding, kind: &str, payload: Value) -> Value {
    json!({"sequence":0,"event_id":CANARY,"aggregate_id":b.id,"revision":4,"kind":kind,"occurred_at":CANARY,"payload":payload,"source":"integration_core"})
}
fn mutate_text(text: &str, f: impl FnOnce(&mut Value)) -> String {
    let mut v: Value = serde_json::from_str(text).unwrap();
    f(&mut v);
    v.to_string()
}
#[test]
fn unknown_is_not_missing_evidence_and_private_fields_never_escape() {
    let (b, run, rows, events) = fixture();
    let value = project(&b, &run, &rows, &events).unwrap();
    assert_eq!(value["status"], "collected");
    assert_eq!(value["run_state"], "outcome_unknown");
    assert_eq!(
        value["rows"][0]["stage_unknown"]["worker_thread_joined"],
        true
    );
    assert_eq!(value["rows"][0]["preparation_io"]["io_complete"], false);
    assert_eq!(value["rows"][0]["actual_runtime_ms"], Value::Null);
    assert_eq!(value["process_stop_proven"], false);
    let safe = value.to_string();
    for private in [
        CANARY,
        "run-private",
        "task-private",
        "policy-private",
        "attempt-private",
    ] {
        assert!(!safe.contains(private));
    }
    let missing = project(&b, &run, &rows, &[]).unwrap();
    assert_eq!(missing["rows"][0]["stage_unknown"], Value::Null);
    assert_eq!(missing["rows"][0]["preparation_io"], Value::Null);
    assert_eq!(
        envelope("unobserved_records", "ok", json!([]))["status"],
        "unobserved_records"
    );
}
#[test]
fn complete_records_preserve_metrics_and_missing_attempts_are_unobserved() {
    let (b, run, mut rows, mut events) = fixture();
    rows[0].state = "finished".into();
    rows[0].document = mutate_text(&rows[0].document, |v| {
        v["view"]["state"] = json!("finished");
        v["view"]["settled"] = json!(true);
        v["view"]["actual_runtime_ms"] = json!(1800);
        v["view"]["actual_output_bytes"] = json!(12);
        v["view"]["io_completion"] = json!("worker_joined");
        v["view"]["last_error"] = Value::Null;
    });
    events.truncate(1);
    events[0].1 = mutate_text(&events[0].1, |v| {
        v["payload"]["io"]["git_calls_complete"] = json!(4);
        v["payload"]["io"]["io_complete"] = json!(true);
        v["payload"]["io"]["interrupted"] = json!(false);
        v["payload"]["io"]["git_stdout_observed_bytes"] = json!(12);
        v["payload"]["io"]["git_stderr_observed_bytes"] = json!(0);
    });
    assert_eq!(
        project(&b, &run, &rows, &events).unwrap()["rows"][0]["actual_runtime_ms"],
        1800
    );
    assert!(project(&b, &run, &[], &[]).is_err());
}
#[test]
fn bindings_reverse_events_duplicate_and_wrong_stage_are_rejected_whole() {
    let (b, run, rows, events) = fixture();
    for field in ["id", "task_id", "policy_id"] {
        let wrong = mutate_text(&run, |v| v[field] = json!("foreign"));
        assert!(project(&b, &wrong, &rows, &events).is_err());
    }
    for field in [
        "integration_id",
        "policy_id",
        "task_id",
        "root_task_id",
        "root_grant_id",
        "project_id",
    ] {
        let mut wrong = rows.clone();
        wrong[0].document = mutate_text(&wrong[0].document, |v| v[field] = json!("foreign"));
        assert!(project(&b, &run, &wrong, &events).is_err());
    }
    let mut wrong = rows.clone();
    wrong[0].run = "foreign".into();
    assert!(project(&b, &run, &wrong, &events).is_err());
    let mut duplicate = rows.clone();
    duplicate.push(rows[0].clone());
    assert!(project(&b, &run, &duplicate, &events).is_err());
    for field in ["aggregate_id", "source"] {
        let mut wrong = events.clone();
        wrong[0].1 = mutate_text(&wrong[0].1, |v| v[field] = json!("foreign"));
        assert!(project(&b, &run, &rows, &wrong).is_err());
    }
    let mut orphan = events.clone();
    orphan[0].1 = mutate_text(&orphan[0].1, |v| {
        v["payload"]["attempt_id"] = json!("other-attempt")
    });
    assert!(project(&b, &run, &rows, &orphan).is_err());
    let mut duplicate = events.clone();
    duplicate.push((5, events[0].1.clone()));
    assert!(project(&b, &run, &rows, &duplicate).is_err());
    let mut validator = rows.clone();
    validator[0].kind = "validator".into();
    validator[0].document = mutate_text(&validator[0].document, |v| {
        v["view"]["kind"] = json!("validator")
    });
    assert!(project(&b, &run, &validator, &events).is_err());
}
#[test]
fn malformed_shapes_duplicate_keys_bool_numbers_and_inconsistent_io_refuse() {
    let (b, run, rows, events) = fixture();
    for bad in [json!(true), json!(-1), json!(1.5), json!("2"), Value::Null] {
        let wrong = mutate_text(&run, |v| v["round"] = bad);
        assert!(project(&b, &wrong, &rows, &events).is_err());
    }
    let extra = mutate_text(&run, |v| v[CANARY] = json!(CANARY));
    assert!(project(&b, &extra, &rows, &events).is_err());
    let missing = mutate_text(&run, |v| {
        v.as_object_mut().unwrap().remove("source_commit");
    });
    assert!(project(&b, &missing, &rows, &events).is_err());
    let duplicate = run.replacen('{', "{\"round\":1,", 1);
    assert!(project(&b, &duplicate, &rows, &events).is_err());
    let mut extra = events.clone();
    extra[0].1 = mutate_text(&extra[0].1, |v| v["payload"][CANARY] = json!(CANARY));
    assert!(project(&b, &run, &rows, &extra).is_err());
    let mut missing = events.clone();
    missing[0].1 = mutate_text(&missing[0].1, |v| {
        v["payload"]["io"]
            .as_object_mut()
            .unwrap()
            .remove("git_stdout_observed_bytes");
    });
    assert!(project(&b, &run, &rows, &missing).is_err());
    for (field, bad) in [
        ("git_calls_complete", json!(5)),
        ("io_complete", json!(1)),
        ("git_stdout_observed_bytes", json!(1)),
    ] {
        let mut wrong = events.clone();
        wrong[0].1 = mutate_text(&wrong[0].1, |v| v["payload"]["io"][field] = bad);
        assert!(project(&b, &run, &rows, &wrong).is_err());
    }
    let mut wrong = events.clone();
    wrong[1].1 = wrong[1].1.replacen(
        "\"worker_thread_joined\":true",
        "\"worker_thread_joined\":true,\"worker_thread_joined\":false",
        1,
    );
    assert!(project(&b, &run, &rows, &wrong).is_err());
}
#[test]
fn unknown_error_text_is_closed_and_all_input_output_bounds_hold() {
    let (b, run, rows, mut events) = fixture();
    events[1].1 = mutate_text(&events[1].1, |v| {
        v["payload"]["joined_error"]["code"] = json!(CANARY);
        v["payload"]["joined_error"]["side_effect_state"] = json!(CANARY);
    });
    let value = project(&b, &run, &rows, &events).unwrap();
    assert_eq!(
        value["rows"][0]["stage_unknown"]["joined_error"]["code"],
        "unrecognized"
    );
    assert!(!value.to_string().contains(CANARY));
    assert!(project(&b, &run, &vec![rows[0].clone(); 33], &[]).is_err());
    assert!(project(&b, &run, &rows, &vec![events[0].clone(); 65]).is_err());
    assert!(project(&b, &" ".repeat(MAX_DOCUMENT + 1), &rows, &[]).is_err());
    let mut many = Vec::new();
    for i in 0..32 {
        let mut row = rows[0].clone();
        row.id = format!("attempt-{i}");
        row.document = mutate_text(&row.document, |v| v["view"]["attempt_id"] = json!(row.id));
        many.push(row);
    }
    assert!(project(&b, &run, &many, &[]).is_ok());
    let mut large_events = Vec::new();
    for (i, row) in many.iter().enumerate() {
        let mut e: Value = serde_json::from_str(&events[1].1).unwrap();
        e["payload"]["attempt_id"] = json!(row.id);
        large_events.push(((i + 1) as i64, e.to_string()));
    }
    assert!(project(&b, &run, &many, &large_events).is_err()); // 32 detailed rows exceed public byte cap, never truncate.
}
#[test]
fn observation_conflicts_are_sticky_and_only_successful_replies_bind() {
    let root = Path::new("/never-opened-unit-runtime");
    OBSERVED.with(|v| {
        v.borrow_mut().remove(root);
    });
    let (b, _, _, _) = fixture();
    let reply = json!({"result":{"run":{"id":b.id,"task_id":b.task,"policy_id":b.policy}}});
    observe_reply(root, "integrations.start", &reply);
    assert_eq!(
        OBSERVED.with(|v| v.borrow().get(root).cloned().flatten()),
        Some(b)
    );
    let foreign = json!({"result":{"run":{"id":"foreign","task_id":"task-private","policy_id":"policy-private"}}});
    observe_reply(root, "integrations.get", &foreign);
    observe_reply(root, "integrations.get", &reply);
    assert_eq!(OBSERVED.with(|v| v.borrow_mut().remove(root)), Some(None));
    observe_reply(
        root,
        "integrations.start",
        &json!({"error":{},"result":reply["result"]}),
    );
    assert!(!OBSERVED.with(|v| v.borrow().contains_key(root)));
}
