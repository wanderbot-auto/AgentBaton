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
fn first_counter_constructor_refuses_related_attempt_or_orphan_session_or_intent_history_atomically(
) {
    // Real approvals/queued Goal, then deliberate raw historical corruption.
    // No successful legacy attempt, stop, join or new authority is fabricated.
    for case in [
        "root_attempt",
        "root_attempt_task_mismatch",
        "project_foreign_root_attempt",
        "project_grant_task_attempt",
        "root_orphan_session",
        "project_orphan_session",
        "root_foreign_intent_document",
        "root_foreign_intent_task_document",
    ] {
        let grant_project_case = case == "project_grant_task_attempt";
        let (mut f, s, g) = if grant_project_case {
            queue_fixture(execution_fixture_with_root_budget(true))
        } else {
            queued()
        };
        assert_eq!(
            f.store
                .conn
                .query_row(
                    "SELECT count(*) FROM budgets WHERE scope=?1",
                    [format!("root:{}", s.root_task_id)],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            if grant_project_case { 1 } else { 0 }
        );
        let other = f
            .store
            .create_registered(
                &s.project_id,
                "Unmapped raw-history negative only",
                "foreign-history-task",
            )
            .unwrap();
        let foreign_binding = if grant_project_case {
            // A genuinely approved DIFFERENT root Grant still belongs to this
            // shared project. Raw-corrupt only the historical Task's project
            // and retained attempt; never manufacture successful execution.
            let p = f
                .store
                .propose_grant(
                    &s.project_id,
                    &s.selection.workspace_ids[0],
                    &s.selection.connection_ids[0],
                    4,
                    1,
                    30000,
                    "other-native-history-grant",
                )
                .unwrap();
            let grant = f.store.finalize_proposal(&p.id, p.revision).unwrap();
            f.store.conn.execute("UPDATE tasks SET document=json_set(document,'$.project_id','foreign-project-raw-corruption') WHERE id=?1",[&other.id]).unwrap();
            json!({"policy_revision":format!("{}:{}",grant["id"].as_str().unwrap(),grant["revision"].as_u64().unwrap())}).to_string()
        } else {
            "{}".into()
        };
        if case.starts_with("project_") {
            f.store
                .conn
                .execute(
                    "DELETE FROM budgets WHERE scope=?1",
                    [format!("project:{}", s.project_id)],
                )
                .unwrap();
        }
        if case.contains("orphan_session") {
            let task = if case.starts_with("project_") {
                &other.id
            } else {
                &s.root_task_id
            };
            let raw = json!({"id":"orphan-history-session","task_id":task,"project_id":s.project_id,"grant_id":s.root_grant_id,"attempt_id":"missing-history-attempt","state":"failed"});
            f.store.conn.execute("INSERT INTO sessions VALUES('orphan-history-session',?1,?2,?3,?4,?5,?5,'failed',?6,'raw-orphan-history','{}')",rusqlite::params![task,s.project_id,s.selection.workspace_ids[0],s.selection.connection_ids[0],s.root_grant_id,raw.to_string()]).unwrap();
        } else {
            let task = if matches!(case, "root_attempt" | "root_attempt_task_mismatch") {
                &s.root_task_id
            } else {
                &other.id
            };
            let root = if case == "root_attempt" {
                &s.root_task_id
            } else {
                &other.id
            };
            f.store.conn.execute("INSERT INTO attempts VALUES('raw-history-attempt',?1,?2,?3,'raw-history-only')",rusqlite::params![task,root,foreign_binding]).unwrap();
            if case.starts_with("root_foreign_intent") {
                let raw = json!({"id":"raw-history-intent","attempt_id":"raw-history-attempt","task_id":if case=="root_foreign_intent_task_document"{&s.root_task_id}else{&other.id},"root_task_id":if case=="root_foreign_intent_task_document"{&other.id}else{&s.root_task_id},"state":"stopped","binding":{}});
                f.store.conn.execute("INSERT INTO dispatch_intents VALUES('raw-history-intent','raw-history-attempt','stopped',NULL,?1)",[raw.to_string()]).unwrap();
            }
        }
        assert!(!f
            .store
            .conn
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
        let before = unchanged_execution(&f);
        let task_before: String = f
            .store
            .conn
            .query_row(
                "SELECT document FROM tasks WHERE id=?1",
                [&s.root_task_id],
                |r| r.get(0),
            )
            .unwrap();
        let run_before: String = f
            .store
            .conn
            .query_row(
                "SELECT document FROM goal_runs WHERE id=?1",
                [&g.goal.run.id],
                |r| r.get(0),
            )
            .unwrap();
        f.raw = format!("history guard {case} {}", uuid::Uuid::new_v4());
        let read = f
            .store
            .prepare_factory_child_goal_read(&s.id, &s.root_task_id, &f.raw, false)
            .unwrap();
        f.store.data_input = Some(f.observe(read));
        let seq = f.store.sequence().unwrap();
        let context=f.store.goal_dispatch_context(&crate::goal_store::GoalRunDispatch{run_id:g.goal.run.id.clone(),expected_ledger_revision:seq},g.goal.run.revision).unwrap().expect("fixture must reach actual private node selection, not an earlier permission/busy refusal");
        let error = f.store.create_goal_session(context).unwrap_err();
        assert_eq!(error.code, "INPUT_CHANGED", "{case}");
        assert_eq!(error.details_ref.as_deref(),Some("factory-goal-initial-counter-history"),"must hit actual constructor attribution guard, not an earlier input/permission/resource failure: {case}");
        assert_eq!(f.store.sequence().unwrap(), seq, "{case}");
        assert_eq!(unchanged_execution(&f), before, "{case}");
        assert_eq!(
            f.store
                .conn
                .query_row(
                    "SELECT document FROM tasks WHERE id=?1",
                    [&s.root_task_id],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            task_before,
            "{case}"
        );
        assert_eq!(
            f.store
                .conn
                .query_row(
                    "SELECT document FROM goal_runs WHERE id=?1",
                    [&g.goal.run.id],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            run_before,
            "{case}"
        );
        assert_eq!(
            f.store
                .conn
                .query_row("SELECT count(*) FROM goal_run_reservations", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0,
            "{case}"
        );
        assert!(!f
            .store
            .conn
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
    }
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

#[test]
fn budget_history_reuse_preserves_values_and_rechecks_each_new_invocation() {
    // Real approvals and Core-written reservation, not a fabricated successful
    // process/stop. The faults below deliberately corrupt otherwise private
    // history; they do not represent a supported write or create authority.
    let (f, s, _) = reserved();
    let project = || crate::factory::budget_records(&f.store.conn, &s.root_grant_id, &s.project_id);
    assert!(f.store.conn.is_autocommit());
    let original = serde_json::to_value(project().unwrap()).unwrap();
    assert_eq!(original.as_array().unwrap().len(), 1);
    let before = unchanged_execution(&f);
    let sequence = f.store.sequence().unwrap();
    f.store
        .conn
        .execute_batch("SAVEPOINT history_projection")
        .unwrap();
    assert!(!f.store.conn.is_autocommit());
    assert_eq!(serde_json::to_value(project().unwrap()).unwrap(), original);

    for (label, sql, code) in [
        ("origin_changed_after_prior_read", "UPDATE factory_child_goal_runs SET record_digest=(CASE substr(record_digest,1,1) WHEN '0' THEN '1' ELSE '0' END)||substr(record_digest,2)", "INPUT_CHANGED"),
        ("usage_changed_after_prior_read", "UPDATE goal_run_reservations SET document=json_set(document,'$.actual_runtime_ms',1)", "BUDGET_UNKNOWN"),
        ("node_state_changed_after_prior_read", "UPDATE goal_run_nodes SET state='foreign-state',document=json_set(document,'$.state','foreign-state') WHERE attempt_id IS NOT NULL", "INPUT_CHANGED"),
        ("session_binding_changed_after_prior_read", "UPDATE sessions SET document=json_set(document,'$.task_id','foreign-history-task')", "INPUT_CHANGED"),
        ("receipt_source_changed_after_prior_read", "UPDATE events SET document=json_set(document,'$.source','foreign-history-source') WHERE json_extract(document,'$.kind')='goal.node_reserved'", "PERMISSION_DENIED"),
        // A second, same-run reservation cannot borrow the first one's Node or
        // event/attempt binding merely because its run history was checked.
        ("second_row_foreign_node", "INSERT INTO attempts(id,task_id,root_task_id,binding,idempotency_key) SELECT 'foreign-history-attempt',task_id,root_task_id,binding,'foreign-history-key' FROM attempts LIMIT 1; INSERT INTO goal_run_reservations(attempt_id,run_id,node_id,parent_node_id,document) SELECT 'foreign-history-attempt',run_id,'foreign-budget-node',parent_node_id,json_set(document,'$.node_id','foreign-budget-node') FROM goal_run_reservations LIMIT 1", "INPUT_CHANGED"),
    ] {
        f.store.conn.execute_batch("SAVEPOINT history_fault").unwrap();
        // Each variant follows a successful projection in this SAME outer
        // transaction. A function-local read result must not hide later writes.
        assert_eq!(serde_json::to_value(project().unwrap()).unwrap(), original);
        f.store.conn.execute_batch(sql).unwrap();
        assert!(!f.store.conn.prepare("PRAGMA foreign_key_check").unwrap().exists([]).unwrap(), "{label}");
        let error = project().unwrap_err();
        assert_eq!(error.code, code, "{label}: {error}");
        assert_eq!(f.store.sequence().unwrap(), sequence, "{label}");
        f.store.conn.execute_batch("ROLLBACK TO history_fault;RELEASE history_fault").unwrap();
        assert_eq!(serde_json::to_value(project().unwrap()).unwrap(), original, "{label}");
        assert_eq!(unchanged_execution(&f), before, "{label}");
    }
    f.store
        .conn
        .execute_batch("RELEASE history_projection")
        .unwrap();
    assert!(f.store.conn.is_autocommit());
    assert_eq!(serde_json::to_value(project().unwrap()).unwrap(), original);
    assert_eq!(unchanged_execution(&f), before);
    assert_eq!(f.store.sequence().unwrap(), sequence);
}

// The original guard, recorded advance intent, wrapper, owned reply and event
// writer are real Core/SQLite paths. No Provider/Actor loop/process is claimed.
fn original_ancestor_failure_after_recorded_intent(
) -> (Fixture, String, serde_json::Value, crate::CoreError) {
    let (mut f, scope, goal) = queued();
    f.raw = format!("admission original request {}", uuid::Uuid::new_v4());
    let read = f
        .store
        .prepare_factory_child_goal_read(&scope.id, &scope.root_task_id, &f.raw, false)
        .unwrap();
    f.store.data_input = Some(f.observe(read));
    let request = crate::goal_store::GoalRunDispatch {
        run_id: goal.goal.run.id.clone(),
        expected_ledger_revision: f.store.sequence().unwrap(),
    };
    let (receipt, fresh) = f
        .store
        .admit_goal_graph_advance(
            &request,
            goal.goal.run.revision,
            "recorded-observation-test",
        )
        .unwrap();
    assert!(fresh);
    f.store.data_input = None;
    // Deliberately oversized negative input to the existing guard, not a new
    // plan or an execution authority. All existing approved caps are unchanged.
    let reservation = crate::goal_plan::AttemptReservation {
        runtime_ms: u64::MAX,
        output_bytes: 1,
        tokens: None,
        cost_microunits: None,
    };
    let error = f
        .store
        .test_factory_ancestor_refusal(&request.run_id, &reservation)
        .unwrap_err();
    assert_eq!(error.code, "RESOURCE_LIMIT");
    assert!(error
        .details_ref
        .as_deref()
        .unwrap()
        .starts_with("factory-ancestor-budget-v1:"));
    assert_eq!(
        f.store
            .conn
            .query_row("SELECT count(*) FROM goal_run_reservations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    (f, request.run_id, receipt, error)
}
fn latest_blocked_payload(f: &Fixture, id: &str) -> serde_json::Value {
    let raw: String = f.store.conn.query_row(
        "SELECT document FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.run_blocked' ORDER BY sequence DESC LIMIT 1", [id], |r|r.get(0)).unwrap();
    serde_json::from_str::<serde_json::Value>(&raw).unwrap()["payload"].clone()
}
#[test]
fn original_guard_sample_survives_recorded_intent_and_owned_reply_into_blocked_event() {
    let (mut f, id, receipt, error) = original_ancestor_failure_after_recorded_intent();
    let original = serde_json::to_value(&error).unwrap();
    let mut observation = None;
    let wrapped = crate::ipc::recorded_goal_dispatch_error(&id, &receipt, error, &mut observation);
    let mut expected = original;
    expected["side_effect_state"] = json!("confirmed");
    expected["details_ref"] = json!(format!(
        "recorded-goal-advance:{}",
        receipt["intent_id"].as_str().unwrap()
    ));
    assert_eq!(
        serde_json::to_value(&wrapped).unwrap(),
        expected,
        "original external error/recovery reference must remain exact"
    );
    let wire = json!({"error": wrapped, "revision": f.store.sequence().unwrap()});
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    crate::ipc::ReadyReply::Goal(send)
        .try_send_observed(crate::ipc::GoalAdvanceReply::new(wire.clone(), observation))
        .unwrap();
    let (received, owned) = receive.try_recv().unwrap().into_parts();
    assert_eq!(received, wire);
    assert!(
        receive.try_recv().is_err(),
        "one owned reply cannot be replayed"
    );
    f.store
        .block_goal_run_observed(
            &id,
            received["error"]["code"].as_str().unwrap(),
            received["error"]["details_ref"].as_str(),
            owned.as_ref(),
        )
        .unwrap();
    let payload = latest_blocked_payload(&f, &id);
    assert_eq!(payload["reason_code"], "RESOURCE_LIMIT");
    assert_eq!(payload["automatic_retry"], false);
    assert_eq!(payload["task_accepted"], false);
    let diagnostic = &payload["budget_admission"];
    assert_eq!(diagnostic["guard_kind"], "runtime");
    assert_eq!(diagnostic["requested_runtime_ms"], u64::MAX);
    assert_eq!(diagnostic["relevant_count"], 0);
    assert_eq!(diagnostic["settled_actual_runtime_ms"], 0);
    assert_eq!(diagnostic["unsettled_reserved_runtime_ms"], 0);
    assert_eq!(
        f.store
            .conn
            .query_row("SELECT count(*) FROM goal_run_reservations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn private_guard_observation_cannot_cross_run_or_relabel_a_stale_revision() {
    for kind in ["missing", "foreign_run", "stale_revision", "foreign_plan"] {
        let (mut f, id, receipt, mut error) = original_ancestor_failure_after_recorded_intent();
        if matches!(kind, "stale_revision" | "foreign_plan") {
            let prefix = "factory-ancestor-budget-v1:";
            let mut value: serde_json::Value = serde_json::from_str(
                error
                    .details_ref
                    .as_deref()
                    .unwrap()
                    .strip_prefix(prefix)
                    .unwrap(),
            )
            .unwrap();
            value[if kind == "stale_revision" {
                "run_revision"
            } else {
                "plan_revision"
            }] = json!(u64::MAX);
            error.details_ref = Some(format!(
                "{prefix}{}",
                serde_json::to_string(&value).unwrap()
            ));
        }
        let mut observation = None;
        let wrapped = crate::ipc::recorded_goal_dispatch_error(
            if kind == "foreign_run" {
                "foreign-generated-run"
            } else {
                &id
            },
            &receipt,
            error,
            &mut observation,
        );
        if kind == "missing" {
            observation = None;
        }
        f.store
            .block_goal_run_observed(
                &id,
                &wrapped.code,
                wrapped.details_ref.as_deref(),
                observation.as_ref(),
            )
            .unwrap();
        let payload = latest_blocked_payload(&f, &id);
        assert_eq!(payload["reason_code"], "RESOURCE_LIMIT");
        assert_eq!(payload["automatic_retry"], false);
        assert_eq!(payload["task_accepted"], false);
        assert!(payload.get("budget_admission").is_none(), "{kind}");
    }
}
#[test]
fn public_wire_reply_drops_private_observation_and_keeps_original_value() {
    let (_f, id, receipt, error) = original_ancestor_failure_after_recorded_intent();
    let mut observation = None;
    let wrapped = crate::ipc::recorded_goal_dispatch_error(&id, &receipt, error, &mut observation);
    assert!(observation.is_some());
    let wire = json!({"error":wrapped});
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    crate::ipc::ReadyReply::Wire(send)
        .try_send_observed(crate::ipc::GoalAdvanceReply::new(wire.clone(), observation))
        .unwrap();
    assert_eq!(receive.try_recv().unwrap(), wire);
}
