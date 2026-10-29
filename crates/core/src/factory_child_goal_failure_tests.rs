// Real Core approvals and derived provenance, with deliberately corrupted raw
// rows. No Provider process, actual failed worker, owned join or replay claim.
use super::*;

#[test]
fn reserved_unknown_task_without_normal_factory_handoff_cannot_be_projected_running() {
    // Genuine queued approvals and actual Store reservation writes, then a
    // cfg(test)-only one-shot, deliberately impossible production mutation inside
    // that same transaction. There
    // is deliberately no original planner, joined worker or successful stop.
    for variant in [
        "missing_planner",
        "fake_confirmed_stop",
        "goal_session_is_not_factory_planner",
    ] {
        let (mut f, s, g) = queue_fixture(execution_fixture_with_root_budget(true));
        assert_eq!(
            f.store
                .conn
                .query_row("SELECT count(*) FROM factory_attempts", [], |r| r
                    .get::<_, i64>(0))
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
                |r| r.get(0),
            )
            .unwrap();
        let binding_before: String = f
            .store
            .conn
            .query_row(
                "SELECT document FROM factory_task_bindings WHERE task_id=?1",
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
        f.raw = format!("normal-handoff negative {variant} {}", uuid::Uuid::new_v4());
        let read = f
            .store
            .prepare_factory_child_goal_read(&s.id, &s.root_task_id, &f.raw, false)
            .unwrap();
        f.store.data_input = Some(f.observe(read));
        let reached = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reached_hook = reached.clone();
        let _fault = crate::sessions::goal_reservation_fault_fixture::install(move |db, task| {
            assert_eq!(task.state, crate::storage::TaskState::Draft);
            for table in ["sessions", "attempts", "goal_run_reservations"] {
                assert_eq!(
                    db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    1,
                    "guard must run after the genuine own reservation writes: {table}"
                );
            }
            task.state = crate::storage::TaskState::OutcomeUnknown;
            db.execute("UPDATE tasks SET document=json_set(document,'$.state','outcome_unknown') WHERE id=?1",[&task.id]).unwrap();
            if variant != "missing_planner" {
                // Keep the relational/document binding coherent so this
                // deliberately fake stop reaches the original normal-handoff
                // semantic verifier, not an earlier column/doc mismatch.
                db.execute("UPDATE factory_task_bindings SET state='waiting_child',document=json_set(document,'$.handle.state','waiting_child','$.handle.confirmed_stop',json('true'),'$.stop_reason','completed_turn_handoff') WHERE task_id=?1",[&task.id]).unwrap();
                assert!(db.query_row("SELECT state=json_extract(document,'$.handle.state') FROM factory_task_bindings WHERE task_id=?1",[&task.id],|row|row.get::<_,bool>(0)).unwrap());
            }
            if variant == "goal_session_is_not_factory_planner" {
                let (session,attempt):(String,String)=db.query_row("SELECT id,json_extract(document,'$.attempt_id') FROM sessions WHERE task_id=?1",[&task.id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
                db.execute("UPDATE factory_task_bindings SET document=json_set(document,'$.session_id',?1,'$.handle.attempt_id',?2) WHERE task_id=?3",rusqlite::params![session,attempt,task.id]).unwrap();
            }
            reached_hook.store(true, std::sync::atomic::Ordering::SeqCst);
        });
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
        assert!(
            reached.load(std::sync::atomic::Ordering::SeqCst),
            "must not count early empty-counter/permission/input refusal as this control"
        );
        assert_eq!(
            error.code, "AUTH_REQUIRED",
            "must reach the explicit normal Factory handoff policy: {variant}"
        );
        assert_eq!(f.store.sequence().unwrap(), seq);
        assert_eq!(unchanged_execution(&f), before);
        assert_eq!(
            f.store
                .conn
                .query_row(
                    "SELECT document FROM tasks WHERE id=?1",
                    [&s.root_task_id],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            task_before
        );
        assert_eq!(
            f.store
                .conn
                .query_row(
                    "SELECT document FROM factory_task_bindings WHERE task_id=?1",
                    [&s.root_task_id],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            binding_before
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
            run_before
        );
        for table in [
            "sessions",
            "attempts",
            "goal_run_reservations",
            "dispatch_intents",
        ] {
            assert_eq!(
                f.store
                    .conn
                    .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0,
                "{table}"
            );
        }
        assert!(!f
            .store
            .conn
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
    }
}

fn assert_no_failure_or_rework_receipt(f: &Fixture, run_id: &str) {
    let count: i64 = f.store.conn.query_row(
        "SELECT count(*) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind') IN('goal.factory_failure_confirmed','goal.factory_rework_reserved','factory.child_goal_planner_recovery_prepared')",
        [run_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn failed_blocked_unknown_or_cancel_labels_cannot_mint_failure_or_rework_permission() {
    for (state, cancel) in [
        ("blocked", false),
        ("failed", false),
        ("outcome_unknown", false),
        ("cancel_requested", true),
        ("cancelled", true),
    ] {
        // Confirmation owns BEGIN IMMEDIATE. Isolate each corrupt variant in
        // a fresh approved fixture, not an outer transaction or SAVEPOINT.
        let (mut f, _s, g) = queued();
        f.store.conn.execute(
            "UPDATE goal_runs SET state=?1,document=json_set(document,'$.state',?1,'$.cancel_requested',json(?2)) WHERE id=?3",
            rusqlite::params![state, if cancel { "true" } else { "false" }, g.goal.run.id],
        ).unwrap();
        let before = unchanged_execution(&f);
        let seq = f.store.sequence().unwrap();
        assert!(
            !f.store
                .confirm_factory_child_goal_failure(&g.goal.run.id)
                .unwrap(),
            "{state}"
        );
        assert!(
            crate::goal_store::reserve_factory_child_goal_rework(&f.store.conn, &g.goal.run.id)
                .is_err(),
            "{state}"
        );
        assert_eq!(f.store.sequence().unwrap(), seq);
        assert_eq!(unchanged_execution(&f), before);
        assert_no_failure_or_rework_receipt(&f, &g.goal.run.id);
    }
}

#[test]
fn fabricated_failed_node_stop_and_measurements_cannot_replace_owned_stop_evidence() {
    for confirmed in [false, true] {
        let (mut f, _s, g) = queued();
        f.store.conn.execute(
            "UPDATE goal_runs SET state='blocked',document=json_set(document,'$.state','blocked') WHERE id=?1",
            [&g.goal.run.id],
        ).unwrap();
        // Keep relational/document state coherent to reach the actual stop
        // verifier. A true flag and zero actuals do not invent a session,
        // reservation, supervisor receipt, capture job or intrinsic validator.
        f.store.conn.execute(
            "UPDATE goal_run_nodes SET state='failed',document=json_set(document,'$.state','failed','$.stop_confirmed',json(?1),'$.actual_runtime_ms',?2,'$.actual_output_bytes',?2) WHERE run_id=?3 AND ordinal=0",
            rusqlite::params![if confirmed { "true" } else { "false" }, if confirmed { Some(0_i64) } else { None }, g.goal.run.id],
        ).unwrap();
        let before = unchanged_execution(&f);
        let seq = f.store.sequence().unwrap();
        let error = f
            .store
            .confirm_factory_child_goal_failure(&g.goal.run.id)
            .unwrap_err();
        assert_eq!(error.code, "INPUT_CHANGED", "claimed_stop={confirmed}");
        assert!(crate::goal_store::reserve_factory_child_goal_rework(
            &f.store.conn,
            &g.goal.run.id
        )
        .is_err());
        assert_eq!(f.store.sequence().unwrap(), seq);
        assert_eq!(unchanged_execution(&f), before);
        assert_no_failure_or_rework_receipt(&f, &g.goal.run.id);
    }
}
