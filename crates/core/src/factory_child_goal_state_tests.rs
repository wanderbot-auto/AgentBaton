//! Actual Core/SQLite Native approvals and derived Graph provenance. Readonly
//! I/O worker bodies run on this test thread; no process/daemon/MCP claim.
use super::*;
use crate::goal_store::{GoalOutputSelection, GoalScopeSelection};
fn execution_fixture() -> (Fixture, FactoryScope) {
    let mut f = Fixture::new();
    let project = f.store.get(&f.selection.root_task_id).unwrap().project_id;
    let p = f
        .store
        .propose_grant(
            &project,
            &f.selection.workspace_ids[0],
            &f.selection.connection_ids[0],
            4,
            1,
            30000,
            "queued-real-grant",
        )
        .unwrap();
    let grant = f.store.finalize_proposal(&p.id, p.revision).unwrap();
    f.selection.grant_id = grant["id"].as_str().unwrap().into();
    f.selection.expected_grant_revision = grant["revision"].as_u64().unwrap();
    let separate = f
        .store
        .create_registered(
            &project,
            "Separately approve original Grant child/depth ceilings",
            "separate-native-scope",
        )
        .unwrap();
    let p = f
        .store
        .propose_goal_scope(
            &GoalScopeSelection {
                task_id: separate.id,
                expected_task_revision: separate.revision,
                grant_id: f.selection.grant_id.clone(),
                expected_grant_revision: f.selection.expected_grant_revision,
                max_nodes: 2,
                new_max_children: 1,
                new_max_depth: 1,
                root_budget: f.selection.root_budget.clone(),
                project_budget: f.selection.project_budget.clone(),
                inputs: f.selection.inputs.clone(),
                outputs: f
                    .selection
                    .outputs
                    .iter()
                    .cloned()
                    .map(|artifact| GoalOutputSelection {
                        artifact,
                        validators: vec![],
                    })
                    .collect(),
                writable_prefixes: vec!["output".into()],
                owner_acceptance_criteria:
                    "Review original fixed output under the original approved ceiling".into(),
            },
            "queued-native-scope",
        )
        .unwrap();
    f.store.finalize_proposal(&p.id, p.revision).unwrap();
    f.selection.expected_grant_revision =
        f.store.get_grant(&f.selection.grant_id).unwrap().revision;
    f.selection.capabilities.child_goal_execution = true;
    f.selection.child_goal_execution_policy = Some(FactoryChildGoalExecutionPolicy {
        max_nodes: 2,
        max_runs_per_task: 1,
        max_total_runs: 1,
        root_budget: f.selection.root_budget.clone(),
        project_budget: f.selection.project_budget.clone(),
        require_owner_acceptance: true,
    });
    fixture_from(f)
}
fn execution_plan(context: &FactoryChildPlanningContext, revision: u64) -> GoalPlanProposal {
    let mut p = plan(context, revision);
    p.nodes[0].budget.children = 0;
    let mut owner = p.nodes[0].clone();
    owner.id = "owner-fixed-output".into();
    owner.kind = NodeKind::OwnerApproval;
    owner.goal = "Review this exact produced candidate".into();
    owner.dependencies = vec![p.nodes[0].id.clone()];
    owner.executor = Executor::Owner {
        rule_id: context.required_acceptance_rules[0].clone(),
    };
    owner.outputs.clear();
    owner.required_inputs = vec![RequiredInput {
        goal_task_id: context.goal.task_id.clone(),
        artifact_id: "output".into(),
        contract_id: "output:output".into(),
        source: InputSource::Node {
            node_id: p.nodes[0].id.clone(),
            candidate_for_validation: true,
        },
    }];
    owner.required_validators = context.required_acceptance_rules.clone();
    owner.side_effect = SideEffect::AdvisoryReadOnly;
    owner.write_set.clear();
    owner.idempotency_key = "fixed-owner-review".into();
    p.nodes.push(owner);
    p
}
fn queued() -> (Fixture, FactoryScope, FactoryChildGoalSummary) {
    let (mut f, s) = execution_fixture();
    observation(&mut f, &s);
    let c = f.store.factory_child_planning_context(&query(&s)).unwrap();
    let p = execution_plan(&c, 1);
    let saved = f
        .store
        .submit_factory_child_plan(&update(&s, &p), s.revision, "first-executable-plan")
        .unwrap();
    observation(&mut f, &s);
    let start = FactoryChildGoalStart {
        scope_id: s.id.clone(),
        request: FactoryChildGoalSubmission {
            task_id: s.root_task_id.clone(),
            plan: saved.reference,
            expected_factory_run_revision: f.store.get_factory_run(&s.id).unwrap().run.revision,
        },
    };
    let g = f
        .store
        .start_factory_child_goal(&start, s.revision, "queued-root-goal")
        .unwrap();
    f.store.data_input = None;
    assert_eq!(g.goal.run.state, "queued");
    assert_eq!(
        f.store.get_factory_run(&s.id).unwrap().tasks[0].state,
        "waiting_goal"
    );
    (f, s, g)
}
#[test]
fn queued_never_started_goal_can_save_and_explicitly_adopt_new_head_without_new_execution() {
    let (mut f, s, original) = queued();
    let before = unchanged_execution(&f);
    observation(&mut f, &s);
    let c = f.store.factory_child_planning_context(&query(&s)).unwrap();
    let mut p = execution_plan(&c, 2);
    p.nodes[0].goal = "New bounded work goal before any attempt".into();
    let update = update(&s, &p);
    let saved = f
        .store
        .submit_factory_child_plan(&update, s.revision, "second-queued-plan")
        .unwrap();
    observation(&mut f, &s);
    let advance = FactoryChildGoalAdvance {
        scope_id: s.id.clone(),
        goal_run_id: original.goal.run.id.clone(),
        request: FactoryChildGoalSubmission {
            task_id: s.root_task_id.clone(),
            plan: saved.reference.clone(),
            expected_factory_run_revision: f.store.get_factory_run(&s.id).unwrap().run.revision,
        },
        expected_goal_run_revision: original.goal.run.revision,
    };
    let adopted = f
        .store
        .advance_factory_child_goal(&advance, s.revision, "queued-explicit-adopt")
        .unwrap();
    f.store.data_input = None;
    assert_eq!(adopted.goal.run.id, original.goal.run.id);
    assert_eq!(adopted.plan, saved.reference);
    let view = f
        .store
        .get_factory_child_goal(&FactoryChildGoalQuery {
            scope_id: s.id.clone(),
            task_id: s.root_task_id.clone(),
            goal_run_id: Some(original.goal.run.id.clone()),
        })
        .unwrap();
    assert_eq!(view.origin.initial_plan, original.plan);
    assert_eq!(view.summary.plan, adopted.plan);
    assert!(view
        .details
        .nodes
        .iter()
        .all(|n| n.state == "not_started" && n.session_id.is_none() && n.attempt_id.is_none()));
    let after = unchanged_execution(&f);
    // Only versioned Goal Node metadata grows; no execution/reservation charge.
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
    let seq = f.store.sequence().unwrap();
    assert_eq!(
        f.store
            .advance_factory_child_goal(&advance, s.revision, "queued-explicit-adopt")
            .unwrap()
            .plan,
        adopted.plan
    );
    assert_eq!(f.store.sequence().unwrap(), seq);
    assert!(f
        .store
        .advance_factory_child_goal(&advance, s.revision, "stale-new-key-adopt")
        .is_err());
    assert_eq!(f.store.sequence().unwrap(), seq);
    assert!(f
        .store
        .submit_factory_child_plan(&update, s.revision + 1, "wrong-scope-cas")
        .is_err());
    assert_eq!(f.store.sequence().unwrap(), seq);
}
#[test]
fn queued_label_cannot_repair_unknown_goal_factory_run_stop_claim_or_missing_issuer() {
    let (mut f, s, g) = queued();
    let before = unchanged_execution(&f);
    observation(&mut f, &s);
    let context = f.store.factory_child_planning_context(&query(&s)).unwrap();
    let next = update(&s, &execution_plan(&context, 2));
    for sql in [
        "UPDATE goal_runs SET state='outcome_unknown',document=json_set(document,'$.state','outcome_unknown')",
        "UPDATE factory_runs SET state='outcome_unknown',document=json_set(document,'$.state','outcome_unknown')",
        "UPDATE factory_task_bindings SET document=json_set(document,'$.handle.confirmed_stop',json('true'))",
        "DELETE FROM factory_child_goal_runs",
    ] {
        observation(&mut f,&s);
        f.store.conn.execute_batch("SAVEPOINT corrupt").unwrap();
        f.store.conn.execute_batch(sql).unwrap();
        let seq=f.store.sequence().unwrap();
        assert!(f.store.submit_factory_child_plan(&next,s.revision,"refused-queued-mutation").is_err(),"{sql}");
        assert_eq!(f.store.sequence().unwrap(),seq);
        f.store.data_input=None;
        f.store.conn.execute_batch("ROLLBACK TO corrupt;RELEASE corrupt").unwrap();
    }
    observation(&mut f, &s);
    assert!(f.store.factory_child_planning_context(&query(&s)).is_ok());
    assert_eq!(
        f.store
            .get_factory_child_goal(&FactoryChildGoalQuery {
                scope_id: s.id.clone(),
                task_id: s.root_task_id.clone(),
                goal_run_id: Some(g.goal.run.id)
            })
            .unwrap()
            .origin
            .initial_plan,
        g.plan
    );
    assert_eq!(unchanged_execution(&f), before);
}

#[test]
fn queued_exact_issuer_still_refuses_dangling_target_work_and_foreign_writer() {
    let (f, s, g) = queued();
    let check = || {
        crate::goal_store::require_factory_queued_unstarted_goal(
            &f.store.conn,
            &g.goal.run.id,
            &s.id,
            &g.factory_run_id,
            &s.root_task_id,
        )
    };
    check().unwrap();
    let before = unchanged_execution(&f);
    for (sql, args) in [
        ("INSERT INTO attempts VALUES('orphan-target-attempt',?1,?1,'{}','orphan-negative')", vec![s.root_task_id.clone()]),
        ("INSERT INTO sessions VALUES('orphan-target-session',?1)", vec![json!({"task_id":s.root_task_id}).to_string()]),
        ("INSERT INTO workspace_leases(resource,holder,fencing_token,expires_at) VALUES(?1,'foreign-writer',1,0) ON CONFLICT(resource) DO UPDATE SET holder='foreign-writer'",vec![s.selection.workspace_ids[0].clone()]),
    ] {
        f.store.conn.execute_batch("SAVEPOINT corrupt").unwrap();
        f.store.conn.execute(sql,rusqlite::params_from_iter(args.iter())).unwrap();
        assert!(check().is_err(),"{sql}");
        f.store.conn.execute_batch("ROLLBACK TO corrupt;RELEASE corrupt").unwrap();
    }
    // The dispatch row is independently checked by its document task even if
    // its real FK attempt belongs to a different existing task.
    f.store.conn.execute_batch("SAVEPOINT corrupt").unwrap();
    let other = f
        .store
        .conn
        .query_row(
            "SELECT id FROM tasks WHERE id!=?1 LIMIT 1",
            [&s.root_task_id],
            |r| r.get::<_, String>(0),
        )
        .unwrap();
    f.store
        .conn
        .execute(
            "INSERT INTO attempts VALUES('foreign-attempt',?1,?1,'{}','foreign-key')",
            [&other],
        )
        .unwrap();
    f.store.conn.execute("INSERT INTO dispatch_intents VALUES('foreign-intent','foreign-attempt','prepared',NULL,?1)",[json!({"task_id":s.root_task_id}).to_string()]).unwrap();
    assert!(check().is_err());
    f.store
        .conn
        .execute_batch("ROLLBACK TO corrupt;RELEASE corrupt")
        .unwrap();
    check().unwrap();
    assert_eq!(unchanged_execution(&f), before);
}
