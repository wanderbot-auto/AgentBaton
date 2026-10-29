// Real Core approvals and derived provenance, with deliberately corrupted raw
// rows. No Provider process, actual failed worker, owned join or replay claim.
use super::*;

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
