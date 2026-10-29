// Actual approved Core/SQLite derived authority records and deliberate raw-row
// fault controls. No Provider, process join, OS dialog or migration claim.
use super::*;
fn goal_query(s: &FactoryScope, g: &FactoryChildGoalSummary) -> FactoryChildGoalQuery {
    FactoryChildGoalQuery {
        scope_id: s.id.clone(),
        task_id: s.root_task_id.clone(),
        goal_run_id: Some(g.goal.run.id.clone()),
    }
}
#[test]
fn revoked_execution_scope_preserves_exact_receipts_and_stop_without_new_admission() {
    let (mut f, s, g) = queued();
    let initial_start = FactoryChildGoalStart {
        scope_id: s.id.clone(),
        request: FactoryChildGoalSubmission {
            task_id: s.root_task_id.clone(),
            plan: g.plan.clone(),
            expected_factory_run_revision: 1,
        },
    };
    let before = unchanged_execution(&f);
    let revoked = f
        .store
        .revoke_factory_scope(&s.id, s.revision, "explicit-derived-revoke")
        .unwrap();
    assert!(revoked.revoked);
    f.store.data_input = None;
    let seq = f.store.sequence().unwrap();
    let history = f.store.get_factory_child_goal(&goal_query(&s, &g)).unwrap();
    assert_eq!(history.origin.initial_plan, g.plan);
    let recovered = f
        .store
        .start_factory_child_goal(&initial_start, s.revision, "queued-root-goal")
        .unwrap();
    assert_eq!(recovered.goal.run.id, g.goal.run.id);
    assert_eq!(recovered.goal.run.revision, g.goal.run.revision);
    assert_eq!(
        f.store.sequence().unwrap(),
        seq,
        "historical same-key start does not renew or dispatch"
    );
    let mut fresh = initial_start;
    fresh.request.expected_factory_run_revision =
        f.store.get_factory_run(&s.id).unwrap().run.revision;
    assert!(f
        .store
        .start_factory_child_goal(&fresh, revoked.revision, "never-fresh-after-revoke")
        .is_err());
    assert!(f
        .store
        .cancel_goal_run(&g.goal.run.id, g.goal.run.revision, "not-a-legacy-run")
        .is_err());
    assert!(f
        .store
        .cancel_goal_graph_with_stops(&g.goal.run.id, g.goal.run.revision, "not-a-native-graph")
        .is_err());
    assert!(f
        .store
        .revoke_goal_scope(&g.goal.run.scope_id, 1, "not-a-native-scope")
        .is_err());
    assert_eq!(f.store.sequence().unwrap(), seq);
    let stop = FactoryChildGoalCancel {
        scope_id: s.id.clone(),
        task_id: s.root_task_id.clone(),
        goal_run_id: g.goal.run.id.clone(),
        expected_goal_run_revision: g.goal.run.revision,
    };
    let (cancelled, stops) = f
        .store
        .cancel_factory_child_goal_with_stops(&stop, revoked.revision, "actual-derived-stop")
        .unwrap();
    assert!(
        stops.is_empty(),
        "all Nodes are genuinely not_started, not secretly joined workers"
    );
    assert_eq!(cancelled.goal.run.state, "cancelled");
    assert!(!cancelled.goal.task_accepted);
    let stopped_seq = f.store.sequence().unwrap();
    let (replayed, stops) = f
        .store
        .cancel_factory_child_goal_with_stops(&stop, revoked.revision, "actual-derived-stop")
        .unwrap();
    assert_eq!(replayed.goal.run.revision, cancelled.goal.run.revision);
    assert!(stops.is_empty());
    assert_eq!(f.store.sequence().unwrap(), stopped_seq);
    assert!(f
        .store
        .cancel_factory_child_goal_with_stops(&stop, revoked.revision, "stale-stop-new-key")
        .is_err());
    assert_eq!(f.store.sequence().unwrap(), stopped_seq);
    let after = unchanged_execution(&f);
    for key in [
        "attempts",
        "sessions",
        "dispatch_intents",
        "goal_graph_jobs",
        "budgets",
    ] {
        assert_eq!(after[key], before[key], "{key}");
    }
    let invented_join:i64=f.store.conn.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.payload.worker_joined')=1 OR json_extract(document,'$.payload.process_reaped')=1",[],|r|r.get(0)).unwrap();
    assert_eq!(invented_join, 0);
}
#[test]
fn missing_derived_provenance_cannot_fall_back_to_legacy_read_list_or_cancel() {
    let (mut f, s, g) = queued();
    assert!(f.store.list_goal_runs(&s.root_task_id).unwrap().is_empty());
    let before = unchanged_execution(&f);
    f.store
        .conn
        .execute_batch("SAVEPOINT missing_issuer")
        .unwrap();
    f.store
        .conn
        .execute(
            "DELETE FROM factory_child_goal_runs WHERE goal_run_id=?1",
            [&g.goal.run.id],
        )
        .unwrap();
    assert!(!f
        .store
        .conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
    let seq = f.store.sequence().unwrap();
    assert!(f.store.get_factory_child_goal(&goal_query(&s, &g)).is_err());
    assert!(f.store.get_goal_run(&g.goal.run.id).is_err());
    assert!(
        f.store.list_goal_runs(&s.root_task_id).is_err(),
        "an issuer-marked corrupt run must not become a legacy row"
    );
    assert!(f
        .store
        .cancel_goal_run(
            &g.goal.run.id,
            g.goal.run.revision,
            "missing-issuer-not-legacy"
        )
        .is_err());
    assert_eq!(f.store.sequence().unwrap(), seq);
    f.store
        .conn
        .execute_batch("ROLLBACK TO missing_issuer;RELEASE missing_issuer")
        .unwrap();
    assert!(f.store.get_factory_child_goal(&goal_query(&s, &g)).is_ok());
    assert_eq!(unchanged_execution(&f), before);
}
