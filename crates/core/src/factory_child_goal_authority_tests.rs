fn expect_refusal<T>(result: std::result::Result<T, crate::CoreError>, message: &str) -> crate::CoreError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("{message}"),
    }
}

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
fn missing_or_corrupt_derived_provenance_cannot_fall_back_to_legacy_or_mint_mutation() {
    let (mut f, s, g) = queued();
    assert!(f.store.list_goal_runs(&s.root_task_id).unwrap().is_empty());
    let before = unchanged_execution(&f);
    let mut unexpected_legacy_lists = vec![];
    for missing in [true, false] {
        f.store
            .conn
            .execute_batch("SAVEPOINT issuer_fault")
            .unwrap();
        if missing {
            f.store
                .conn
                .execute(
                    "DELETE FROM factory_child_goal_runs WHERE goal_run_id=?1",
                    [&g.goal.run.id],
                )
                .unwrap();
        } else {
            f.store
                .conn
                .execute(
                    "UPDATE factory_child_goal_runs SET record_digest=?1 WHERE goal_run_id=?2",
                    rusqlite::params!["0".repeat(64), g.goal.run.id],
                )
                .unwrap();
        }
        assert!(!f
            .store
            .conn
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
        let seq = f.store.sequence().unwrap();
        let source_error = |route: &str, e: crate::CoreError| {
            // Preserve the existing distinct refusal contracts. GoalStore's
            // private denied() is PERMISSION_DENIED; legacy graph classification
            // is CAPABILITY_UNSUPPORTED; malformed real issuer proof is INPUT_CHANGED.
            let expected = match route {
                "legacy_get" | "legacy_cancel" => "CAPABILITY_UNSUPPORTED",
                "fixed_get" | "graph_details" | "fixed_advance" | "owner" if !missing => {
                    "INPUT_CHANGED"
                }
                "fixed_get" | "graph_details" | "fixed_advance" | "owner" | "native_cancel"
                | "legacy_advance" | "native_advance" => "PERMISSION_DENIED",
                _ => panic!("unknown fixed test route"),
            };
            assert_eq!(e.code, expected, "missing={missing},route={route}");
        };
        source_error(
            "fixed_get",
            expect_refusal(
            f.store
                .get_factory_child_goal(&goal_query(&s, &g)),
                "fixed Factory get needs its real issuer",
            ),
        );
        source_error(
            "legacy_get",
            expect_refusal(
            f.store
                .get_goal_run(&g.goal.run.id),
                "derived source never becomes legacy",
            ),
        );
        source_error(
            "graph_details",
            expect_refusal(
            f.store
                .goal_graph_details(&g.goal.run.id),
                "shared Graph details validates the complete issuer",
            ),
        );
        source_error(
            "legacy_cancel",
            expect_refusal(
            f.store
                .cancel_goal_run(
                    &g.goal.run.id,
                    g.goal.run.revision,
                    "issuer-not-legacy-cancel",
                ),
                "no legacy cancel",
            ),
        );
        source_error(
            "native_cancel",
            expect_refusal(
            f.store
                .cancel_goal_graph_with_stops(
                    &g.goal.run.id,
                    g.goal.run.revision,
                    "issuer-not-native-cancel",
                ),
                "no native cancel",
            ),
        );
        let native_advance = crate::goal_store::GoalPlanAdvance {
            run_id: g.goal.run.id.clone(),
            expected_plan_revision: g.plan.revision,
            expected_ledger_revision: seq,
        };
        source_error(
            "legacy_advance",
            expect_refusal(
            f.store
                .advance_goal_plan(
                    &native_advance,
                    g.goal.run.revision,
                    "issuer-not-legacy-advance",
                ),
                "no legacy advance",
            ),
        );
        source_error(
            "native_advance",
            expect_refusal(
            f.store
                .advance_goal_graph_plan(
                    &native_advance,
                    g.goal.run.revision,
                    "issuer-not-native-advance",
                ),
                "no native advance",
            ),
        );
        let request = FactoryChildGoalSubmission {
            task_id: s.root_task_id.clone(),
            plan: g.plan.clone(),
            expected_factory_run_revision: f.store.get_factory_run(&s.id).unwrap().run.revision,
        };
        let advance = FactoryChildGoalAdvance {
            scope_id: s.id.clone(),
            goal_run_id: g.goal.run.id.clone(),
            request,
            expected_goal_run_revision: g.goal.run.revision,
        };
        source_error(
            "fixed_advance",
            expect_refusal(
            f.store
                .advance_factory_child_goal(&advance, s.revision, "issuer-fixed-advance"),
                "fixed advance needs real source before revision import",
            ),
        );
        let owner = crate::goal_store::GraphNodeApprovalSelection {
            run_id: g.goal.run.id.clone(),
            node_id: "owner-fixed-output".into(),
            expected_plan_revision: g.plan.revision,
            expected_ledger_revision: seq,
        };
        source_error(
            "owner",
            expect_refusal(
            f.store
                .propose_goal_node_acceptance(&owner, g.goal.run.revision, "issuer-owner-proposal"),
                "Owner needs real source before candidate/decision",
            ),
        );
        // Present corrupt rows stay excluded from legacy list. Missing rows
        // still marked Factory in their scope must fail closed, not be listed
        // as old execution. This assertion is intentionally last so the other
        // independent source-denial checks run even when the list defect exists.
        let legacy = f.store.list_goal_runs(&s.root_task_id);
        if missing {
            if legacy.is_ok() {
                unexpected_legacy_lists.push("missing derived row returned a legacy list result");
            }
        } else {
            assert!(legacy.unwrap().is_empty());
        }
        assert_eq!(f.store.sequence().unwrap(), seq);
        f.store
            .conn
            .execute_batch("ROLLBACK TO issuer_fault;RELEASE issuer_fault")
            .unwrap();
        assert!(f.store.get_factory_child_goal(&goal_query(&s, &g)).is_ok());
        assert_eq!(unchanged_execution(&f), before);
    }
    assert!(
        unexpected_legacy_lists.is_empty(),
        "{unexpected_legacy_lists:?}"
    );
}
