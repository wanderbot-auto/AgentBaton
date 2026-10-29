// Actual Core approvals -> derived scope -> atomic session/attempt/reservation
// -> real Core event writer -> shared accounting. The connection is synthetic
// ready metadata; no Provider process, Supervisor launch or owned join is claimed.
use super::*;

fn reserved() -> (Fixture, FactoryScope, FactoryChildGoalSummary) {
    reserved_with_prior_root_counter(true)
}
fn reserved_with_prior_root_counter(
    prior_counter: bool,
) -> (Fixture, FactoryScope, FactoryChildGoalSummary) {
    let (mut f, s, g) = queue_fixture(execution_fixture_with_root_budget(prior_counter));
    // Receipt controls use real prior Native approval to establish counters;
    // the separate Owner-first control intentionally retains counter absence.
    // Neither branch fabricates a counter or a process/join receipt.
    let present: i64 = f
        .store
        .conn
        .query_row(
            "SELECT count(*) FROM budgets WHERE scope=?1",
            [format!("root:{}", s.root_task_id)],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(present, if prior_counter { 1 } else { 0 });
    if prior_counter {
        let usage: (i64, i64) = f
            .store
            .conn
            .query_row(
                "SELECT consumed,active FROM budgets WHERE scope=?1",
                [format!("root:{}", s.root_task_id)],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            usage,
            (0, 0),
            "prior approved counter must retain its exact initial usage assertion"
        );
    }
    assert_eq!(
        f.store
            .conn
            .query_row("SELECT count(*) FROM attempts", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let unrelated_before: String = f.store.conn.query_row(
        "SELECT COALESCE(json_group_array(json_array(scope,total_limit,parallel_limit,consumed,active)),'[]') FROM (SELECT * FROM budgets WHERE scope NOT IN(?1,?2) ORDER BY scope)",
        rusqlite::params![format!("root:{}",s.root_task_id),format!("project:{}",s.project_id)], |row|row.get(0),
    ).unwrap();
    f.raw = format!("actual direct reservation {}", uuid::Uuid::new_v4());
    let read = f
        .store
        .prepare_factory_child_goal_read(&s.id, &s.root_task_id, &f.raw, false)
        .unwrap();
    f.store.data_input = Some(f.observe(read));
    let session = f
        .store
        .goal_dispatch_session(
            &crate::goal_store::GoalRunDispatch {
                run_id: g.goal.run.id.clone(),
                expected_ledger_revision: f.store.sequence().unwrap(),
            },
            g.goal.run.revision,
        )
        .unwrap()
        .expect("real direct reservation must not block on its own Core-written receipt");
    assert_eq!(session.state, "starting");
    let unrelated_after: String = f.store.conn.query_row(
        "SELECT COALESCE(json_group_array(json_array(scope,total_limit,parallel_limit,consumed,active)),'[]') FROM (SELECT * FROM budgets WHERE scope NOT IN(?1,?2) ORDER BY scope)",
        rusqlite::params![format!("root:{}",s.root_task_id),format!("project:{}",s.project_id)], |row|row.get(0),
    ).unwrap();
    assert_eq!(
        unrelated_after, unrelated_before,
        "unrelated approved counters remain exactly unchanged"
    );
    f.store.data_input = None;
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT count(*) FROM dispatch_intents WHERE state='prepared'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT count(*) FROM factory_budget_reservations",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert!(!f
        .store
        .conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
    (f, s, g)
}

#[test]
fn owner_first_goal_reservation_initializes_only_its_original_root_counter_atomically() {
    let (f, s, g) = reserved_with_prior_root_counter(false);
    let current: (u32, u32, u32, u32) = f
        .store
        .conn
        .query_row(
            "SELECT total_limit,parallel_limit,consumed,active FROM budgets WHERE scope=?1",
            [format!("root:{}", s.root_task_id)],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        current,
        (
            s.bindings.approved_grant.scope.max_attempts,
            s.bindings.approved_grant.scope.max_concurrency,
            1,
            1
        )
    );
    assert_eq!(
        crate::factory::budget_records(&f.store.conn, &s.root_grant_id, &s.project_id)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT count(*) FROM goal_run_reservations WHERE run_id=?1",
                [&g.goal.run.id],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn owner_first_counter_initializer_rolls_back_when_session_launch_journal_fails() {
    let (mut f, s, g) = queued();
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT count(*) FROM budgets WHERE scope=?1",
                [format!("root:{}", s.root_task_id)],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    let before = unchanged_execution(&f);
    let task_before: String = f
        .store
        .conn
        .query_row(
            "SELECT document FROM tasks WHERE id=?1",
            [&s.root_task_id],
            |row| row.get(0),
        )
        .unwrap();
    let run_before: String = f
        .store
        .conn
        .query_row(
            "SELECT document FROM goal_runs WHERE id=?1",
            [&g.goal.run.id],
            |row| row.get(0),
        )
        .unwrap();
    f.raw = format!("counter rollback {}", uuid::Uuid::new_v4());
    let read = f
        .store
        .prepare_factory_child_goal_read(&s.id, &s.root_task_id, &f.raw, false)
        .unwrap();
    f.store.data_input = Some(f.observe(read));
    f.store.conn.execute_batch("CREATE TRIGGER refuse_counter_launch_journal BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='session.launch_reserved' BEGIN SELECT RAISE(ABORT,'controlled launch journal failure'); END;").unwrap();
    let seq = f.store.sequence().unwrap();
    let error = f
        .store
        .goal_dispatch_session(
            &crate::goal_store::GoalRunDispatch {
                run_id: g.goal.run.id.clone(),
                expected_ledger_revision: seq,
            },
            g.goal.run.revision,
        )
        .unwrap_err();
    assert_eq!(error.code, "INTERNAL_ERROR");
    assert_eq!(
        error.details_ref.as_deref(),
        Some("sqlite-code:1811"),
        "must reach the intended SQLite trigger, not an earlier authority/readset refusal"
    );
    assert_eq!(f.store.sequence().unwrap(), seq);
    assert_eq!(unchanged_execution(&f), before);
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT document FROM tasks WHERE id=?1",
                [&s.root_task_id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        task_before
    );
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT document FROM goal_runs WHERE id=?1",
                [&g.goal.run.id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        run_before
    );
    for table in [
        "attempts",
        "sessions",
        "dispatch_intents",
        "goal_run_reservations",
    ] {
        assert_eq!(
            f.store
                .conn
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0,
            "{table}"
        );
    }
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT count(*) FROM budgets WHERE scope=?1",
                [format!("root:{}", s.root_task_id)],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert!(!f
        .store
        .conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
}

#[test]
fn exact_core_written_reservation_receipt_is_not_rejected_for_json_object_field_order() {
    let (f, s, g) = reserved();
    let rows =
        crate::factory::budget_records(&f.store.conn, &s.root_grant_id, &s.project_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].authority_kind, "factory_child_goal");
    assert_eq!(rows[0].scope_id.as_deref(), Some(s.id.as_str()));
    assert_eq!(rows[0].task_id, g.goal.run.task_id);
    assert!(!rows[0].settled);
    assert_eq!(rows[0].actual_runtime_ms, None);
    assert_eq!(rows[0].actual_output_bytes, None);
    let (raw, reservation): (String, String) = f.store.conn.query_row(
        "SELECT e.document,r.document FROM events e JOIN goal_run_reservations r ON r.attempt_id=json_extract(e.document,'$.payload.attempt_id') WHERE json_extract(e.document,'$.kind')='goal.node_reserved' AND r.run_id=?1",
        [&g.goal.run.id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    let event: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let record: serde_json::Value = serde_json::from_str(&reservation).unwrap();
    let typed: AttemptReservation = serde_json::from_value(record["reservation"].clone()).unwrap();
    assert_eq!(event["payload"]["reservation"], json!(typed));
    let derived_struct = serde_json::to_string(&typed).unwrap();
    let event_value = serde_json::to_string(&event["payload"]["reservation"]).unwrap();
    let old_text_equal: bool = f
        .store
        .conn
        .query_row(
            "SELECT json(?1)=json(?2)",
            rusqlite::params![event_value, derived_struct],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        !old_text_equal,
        "SQLite json() normalizes spacing, not object key order"
    );
}

#[test]
fn shared_reservation_projection_still_rejects_missing_foreign_changed_or_double_classified_receipts(
) {
    let (f, s, g) = reserved();
    let before = unchanged_execution(&f);
    for (label, sql) in [
        ("missing", "DELETE FROM events WHERE json_extract(document,'$.kind')='goal.node_reserved'"),
        ("wrong_amount", "UPDATE events SET document=json_set(document,'$.payload.reservation.output_bytes',1025) WHERE json_extract(document,'$.kind')='goal.node_reserved'"),
        ("wrong_source", "UPDATE events SET document=json_set(document,'$.source','user_action') WHERE json_extract(document,'$.kind')='goal.node_reserved'"),
        ("wrong_run", "UPDATE events SET document=json_set(document,'$.aggregate_id','foreign-run') WHERE json_extract(document,'$.kind')='goal.node_reserved'"),
        ("wrong_node", "UPDATE events SET document=json_set(document,'$.payload.node_id','foreign-node') WHERE json_extract(document,'$.kind')='goal.node_reserved'"),
        ("wrong_attempt", "UPDATE events SET document=json_set(document,'$.payload.attempt_id','foreign-attempt') WHERE json_extract(document,'$.kind')='goal.node_reserved'"),
        ("wrong_plan", "UPDATE events SET document=json_set(document,'$.payload.plan_revision',2) WHERE json_extract(document,'$.kind')='goal.node_reserved'"),
        ("missing_reservation", "UPDATE events SET document=json_remove(document,'$.payload.reservation') WHERE json_extract(document,'$.kind')='goal.node_reserved'"),
        ("wrong_node_binding", "UPDATE goal_run_reservations SET node_id='foreign-node'"),
    ] {
        f.store.conn.execute_batch("SAVEPOINT receipt_fault").unwrap();
        f.store.conn.execute_batch(sql).unwrap();
        let seq = f.store.sequence().unwrap();
        let error = crate::factory::budget_records(&f.store.conn, &s.root_grant_id, &s.project_id).unwrap_err();
        assert!(matches!(error.code.as_str(), "BUDGET_UNKNOWN" | "PERMISSION_DENIED" | "INPUT_CHANGED"), "{label}: {}", error.code);
        assert_eq!(f.store.sequence().unwrap(), seq, "{label}");
        f.store.conn.execute_batch("ROLLBACK TO receipt_fault;RELEASE receipt_fault").unwrap();
        assert_eq!(unchanged_execution(&f), before, "{label}");
    }
    let mut duplicate =
        crate::factory::budget_records(&f.store.conn, &s.root_grant_id, &s.project_id)
            .unwrap()
            .remove(0);
    duplicate.authority_kind = "legacy_shared_session".into();
    duplicate.scope_id = None;
    duplicate.run_id = None;
    duplicate.measurement_scope = "empty_selected_artifacts_no_validator_streams".into();
    duplicate.reservation.output_bytes = 0;
    let seq = f.store.sequence().unwrap();
    f.store
        .conn
        .execute_batch("SAVEPOINT duplicate_class")
        .unwrap();
    f.store
        .conn
        .execute(
            "INSERT INTO factory_budget_reservations VALUES(?1,NULL,?2,?3,?4)",
            rusqlite::params![
                duplicate.attempt_id,
                duplicate.root_task_id,
                duplicate.project_id,
                serde_json::to_string(&duplicate).unwrap()
            ],
        )
        .unwrap();
    assert!(!f
        .store
        .conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
    let error =
        crate::factory::budget_records(&f.store.conn, &s.root_grant_id, &s.project_id).unwrap_err();
    assert_eq!(error.code, "BUDGET_UNKNOWN");
    assert_eq!(f.store.sequence().unwrap(), seq);
    f.store
        .conn
        .execute_batch("ROLLBACK TO duplicate_class;RELEASE duplicate_class")
        .unwrap();
    assert_eq!(unchanged_execution(&f), before);
    assert_eq!(
        crate::factory::budget_records(&f.store.conn, &s.root_grant_id, &s.project_id).unwrap()[0]
            .task_id,
        g.goal.run.task_id
    );
}
