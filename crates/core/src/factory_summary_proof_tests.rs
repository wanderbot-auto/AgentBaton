// Real approved Core/SQLite history, with deliberate owned-row corruption.
// No Actor, Provider, process-stop or performance claim.
use super::*;

fn goal_query(s: &FactoryScope, g: &FactoryChildGoalSummary) -> FactoryChildGoalQuery {
    FactoryChildGoalQuery {
        scope_id: s.id.clone(),
        task_id: s.root_task_id.clone(),
        goal_run_id: Some(g.goal.run.id.clone()),
    }
}
fn bytes(value: &impl serde::Serialize) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}
fn summary_pair(f: &Fixture, id: &str) -> FactoryChildGoalSummary {
    let (strict, reused) = f.store.factory_summary_snapshot_pair_for_test(id).unwrap();
    assert!(bytes(&strict) == bytes(&reused), "summary byte equivalence");
    strict
}
fn refusal<T>(result: std::result::Result<T, crate::CoreError>) -> crate::CoreError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("corrupt history was accepted"),
    }
}
fn changes(f: &Fixture) -> i64 {
    f.store
        .conn
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .unwrap()
}
fn corrupt_digest(f: &Fixture, id: &str) {
    assert_eq!(f.store.conn.execute(
        "UPDATE factory_child_goal_runs SET record_digest=(CASE substr(record_digest,1,1) WHEN '0' THEN '1' ELSE '0' END)||substr(record_digest,2) WHERE goal_run_id=?1",
        [id],
    ).unwrap(), 1);
}

#[test]
fn request_snapshot_summary_and_complete_view_match_original_strict_bytes() {
    let (f, s, g) = queued();
    let before = unchanged_execution(&f);
    let writes = changes(&f);
    let seq = f.store.sequence().unwrap();
    let strict = summary_pair(&f, &g.goal.run.id);
    let q = goal_query(&s, &g);
    let snapshot_view = f.store.get_factory_child_goal(&q).unwrap();
    assert!(bytes(&snapshot_view.summary) == bytes(&strict));
    let mut list_query = q.clone();
    list_query.goal_run_id = None;
    let snapshot_list = f.store.list_factory_child_goals(&list_query).unwrap();
    assert_eq!(snapshot_list.len(), 1);
    assert!(bytes(&snapshot_list[0]) == bytes(&strict));
    assert!(f.store.conn.is_autocommit());

    // The same public methods in an existing transaction use the unchanged
    // strict path. Compare the entire View, not merely its summary fields.
    let tx = f.store.conn.unchecked_transaction().unwrap();
    let original_view = f.store.get_factory_child_goal(&q).unwrap();
    let original_list = f.store.list_factory_child_goals(&list_query).unwrap();
    assert!(bytes(&original_view) == bytes(&snapshot_view));
    assert!(bytes(&original_list) == bytes(&snapshot_list));
    tx.rollback().unwrap();
    assert!(f.store.conn.is_autocommit());
    assert_eq!(f.store.sequence().unwrap(), seq);
    assert_eq!(unchanged_execution(&f), before);
    assert_eq!(changes(&f), writes);
}

#[test]
fn later_request_rechecks_real_adopted_plan_and_preserves_initial_origin() {
    let (mut f, s, g) = queued();
    let initial = summary_pair(&f, &g.goal.run.id);
    let before = unchanged_execution(&f);
    observation(&mut f, &s);
    let context = f.store.factory_child_planning_context(&query(&s)).unwrap();
    let mut p = execution_plan(&context, 2);
    p.nodes[0].goal = "New bounded summary revision before execution".into();
    let saved = f
        .store
        .submit_factory_child_plan(&update(&s, &p), s.revision, "summary-real-plan-two")
        .unwrap();
    observation(&mut f, &s);
    let advance = FactoryChildGoalAdvance {
        scope_id: s.id.clone(),
        goal_run_id: g.goal.run.id.clone(),
        request: FactoryChildGoalSubmission {
            task_id: s.root_task_id.clone(),
            plan: saved.reference.clone(),
            expected_factory_run_revision: f.store.get_factory_run(&s.id).unwrap().run.revision,
        },
        expected_goal_run_revision: g.goal.run.revision,
    };
    f.store
        .advance_factory_child_goal(&advance, s.revision, "summary-real-adoption")
        .unwrap();
    f.store.data_input = None;
    let current = summary_pair(&f, &g.goal.run.id);
    assert_eq!(current.plan, saved.reference);
    assert_ne!(bytes(&current.plan), bytes(&initial.plan));
    let view = f.store.get_factory_child_goal(&goal_query(&s, &g)).unwrap();
    assert_eq!(view.origin.initial_plan, initial.plan);
    assert!(bytes(&view.summary) == bytes(&current));
    let after = unchanged_execution(&f);
    for key in [
        "attempts",
        "sessions",
        "dispatch_intents",
        "goal_runs",
        "goal_graph_jobs",
        "goal_graph_acceptances",
        "budgets",
    ] {
        assert_eq!(after[key], before[key], "{key}");
    }
    // Historical display after actual revocation retains the original strict
    // semantics. This read neither renews the scope nor permits fresh dispatch.
    assert!(
        f.store
            .revoke_factory_scope(&s.id, s.revision, "summary-real-revoke")
            .unwrap()
            .revoked
    );
    let seq = f.store.sequence().unwrap();
    let revoked = summary_pair(&f, &g.goal.run.id);
    let history = f.store.get_factory_child_goal(&goal_query(&s, &g)).unwrap();
    assert!(bytes(&history.summary) == bytes(&revoked));
    assert_eq!(history.origin.initial_plan, initial.plan);
    assert_eq!(f.store.sequence().unwrap(), seq);
}

#[test]
fn owned_snapshot_and_strict_fallback_reject_corrupt_history_on_new_calls() {
    for fault in [
        "digest",
        "mirror",
        "source",
        "raw_plan",
        "run_binding",
        "fake_failed",
    ] {
        let (f, s, g) = queued();
        let q = goal_query(&s, &g);
        f.store.get_factory_child_goal(&q).unwrap();
        match fault {
            "digest" => corrupt_digest(&f, &g.goal.run.id),
            "mirror" => assert_eq!(f.store.conn.execute(
                "UPDATE factory_child_goal_runs SET node_count=node_count+1 WHERE goal_run_id=?1",
                [&g.goal.run.id],
            ).unwrap(), 1),
            "source" => assert_eq!(f.store.conn.execute(
                "UPDATE events SET document=json_set(document,'$.source','foreign_source') WHERE sequence=(SELECT receipt_sequence FROM factory_child_goal_runs WHERE goal_run_id=?1)",
                [&g.goal.run.id],
            ).unwrap(), 1),
            "raw_plan" => assert_eq!(f.store.conn.execute(
                "UPDATE factory_child_plan_revisions SET plan_json=plan_json||' ' WHERE id=?1",
                [&g.plan.id],
            ).unwrap(), 1),
            "run_binding" => assert_eq!(f.store.conn.execute(
                "UPDATE goal_runs SET document=json_set(document,'$.root_grant_id','foreign_root_grant') WHERE id=?1",
                [&g.goal.run.id],
            ).unwrap(), 1),
            // This is an inconsistent raw label, not a genuine failed/stopped
            // Graph. A full stop/failure witness must still be required.
            "fake_failed" => assert_eq!(f.store.conn.execute(
                "UPDATE goal_runs SET state='failed',document=json_set(document,'$.state','failed') WHERE id=?1",
                [&g.goal.run.id],
            ).unwrap(), 1),
            _ => unreachable!(),
        }
        let before = unchanged_execution(&f);
        let writes = changes(&f);
        let seq = f.store.sequence().unwrap();
        let original = refusal(
            f.store
                .factory_summary_strict_reference_for_test(&g.goal.run.id),
        );
        assert!(
            matches!(
                original.code.as_str(),
                "INPUT_CHANGED" | "PERMISSION_DENIED"
            ),
            "fault={fault}"
        );
        let get_error = refusal(f.store.get_factory_child_goal(&q));
        assert_eq!(get_error.code, original.code, "get fault={fault}");
        let mut list_query = q;
        list_query.goal_run_id = None;
        let list_error = refusal(f.store.list_factory_child_goals(&list_query));
        assert_eq!(list_error.code, original.code, "list fault={fault}");
        assert!(f.store.conn.is_autocommit());
        assert_eq!(f.store.sequence().unwrap(), seq, "fault={fault}");
        assert_eq!(unchanged_execution(&f), before, "fault={fault}");
        assert_eq!(changes(&f), writes, "fault={fault}");
    }
}

#[test]
fn existing_transaction_never_reuses_proof_after_own_write_or_across_requests() {
    let (f, s, g) = queued();
    let q = goal_query(&s, &g);
    let before = f.store.get_factory_child_goal(&q).unwrap();
    let counts = unchanged_execution(&f);
    let seq = f.store.sequence().unwrap();
    let tx = f.store.conn.unchecked_transaction().unwrap();
    let same = f.store.get_factory_child_goal(&q).unwrap();
    assert!(bytes(&same) == bytes(&before));
    corrupt_digest(&f, &g.goal.run.id);
    assert_eq!(
        refusal(f.store.get_factory_child_goal(&q)).code,
        "INPUT_CHANGED"
    );
    let mut list_query = q.clone();
    list_query.goal_run_id = None;
    assert_eq!(
        refusal(f.store.list_factory_child_goals(&list_query)).code,
        "INPUT_CHANGED"
    );
    assert!(
        !f.store.conn.is_autocommit(),
        "callee does not end caller transaction"
    );
    tx.rollback().unwrap();
    let after = f.store.get_factory_child_goal(&q).unwrap();
    assert!(bytes(&after) == bytes(&before));
    assert_eq!(f.store.sequence().unwrap(), seq);
    assert_eq!(unchanged_execution(&f), counts);
    assert!(f.store.conn.is_autocommit());
}
