// Real Core/SQLite, directly executed readonly worker-body and Git files. Explicit synthetic ready
// connection; no Provider process, MCP process, Native dialog or model call.
use super::*;
use crate::{factory::*, goal_plan::*};
use serde_json::json;
fn fixture() -> (Fixture, FactoryScope) {
    let mut f = Fixture::new();
    f.selection.capabilities.child_planning = true;
    f.selection.child_planning_policy = Some(FactoryChildPlanningPolicy {
        max_nodes: 8,
        max_revisions: 16,
        max_plan_bytes: 262144,
        outputs: f
            .selection
            .outputs
            .iter()
            .cloned()
            .map(|artifact| crate::goal_store::GoalOutputSelection {
                artifact,
                validators: vec![],
            })
            .collect(),
        writable_prefixes: vec!["output".into()],
        owner_acceptance_criteria:
            "Review the exact complete report; a stored plan is not acceptance".into(),
    });
    let grant_before =
        serde_json::to_value(f.store.get_grant(&f.selection.grant_id).unwrap()).unwrap();
    assert_eq!(grant_before["scope"]["max_children"], 0);
    let w = f.observe(f.plan());
    f.store.data_input = Some(w);
    let proposal = f
        .store
        .propose_factory_scope(&f.selection, "fixture-read", &f.companion)
        .unwrap();
    f.store.data_input = None;
    let raw = "private exact factory finalize";
    let read = f
        .store
        .prepare_factory_owner_read(
            "approvals.finalize",
            &json!({"proposal_id":proposal.id}),
            Some(proposal.revision),
            Some("finalize"),
            raw,
            Some(f.companion.clone()),
        )
        .unwrap()
        .unwrap();
    f.raw = raw.into();
    let w = f.observe(read);
    f.store.data_input = Some(w);
    let scope: FactoryScope = serde_json::from_value(
        f.store
            .finalize_factory_scope(&proposal.id, proposal.revision, &f.companion)
            .unwrap(),
    )
    .unwrap();
    f.store.data_input = None;
    let raw = "private exact factory start";
    let read = f
        .store
        .prepare_factory_owner_read(
            "factory_runs.start",
            &json!({"scope_id":scope.id}),
            Some(scope.revision),
            Some("planning-start"),
            raw,
            Some(f.companion.clone()),
        )
        .unwrap()
        .unwrap();
    f.raw = raw.into();
    let w = f.observe(read);
    f.store.data_input = Some(w);
    f.store
        .start_factory_run(&scope.id, scope.revision, "planning-start")
        .unwrap();
    f.store.data_input = None;
    assert_eq!(
        serde_json::to_value(f.store.get_grant(&f.selection.grant_id).unwrap()).unwrap(),
        grant_before
    );
    (f, scope)
}
fn observation(f: &mut Fixture, s: &FactoryScope) {
    f.raw = format!("exact planning observation {}", uuid::Uuid::new_v4());
    let read = f.store.prepare_factory_child_plan_read(s, &f.raw).unwrap();
    let w = f.observe(read);
    f.store.data_input = Some(w);
}
fn query(s: &FactoryScope) -> FactoryChildPlanQuery {
    FactoryChildPlanQuery {
        scope_id: s.id.clone(),
        task_id: s.root_task_id.clone(),
        plan_revision: None,
    }
}
fn plan(c: &FactoryChildPlanningContext, revision: u64) -> GoalPlanProposal {
    let output = c.outputs.iter().next().unwrap();
    let input = c.inputs.iter().next().unwrap();
    GoalPlanProposal {
        schema_version: 1,
        plan_id: "fixed-child-plan".into(),
        goal: c.goal.clone(),
        scope_revision: c.scope_revision,
        revision,
        expected_revision: revision - 1,
        acceptance_rules: c.required_acceptance_rules.clone(),
        nodes: vec![PlanNode {
            id: "work".into(),
            parent_id: None,
            kind: NodeKind::Work,
            goal: "Produce only the exact approved output".into(),
            dependencies: vec![],
            workspace_id: c.execution_bindings[0].workspace_id.clone(),
            executor: Executor::Connection {
                binding: Box::new(c.execution_bindings[0].clone()),
                program_sha256: c.program_sha256[&c.execution_bindings[0].connection_id].clone(),
            },
            recipients: vec![],
            required_inputs: vec![RequiredInput {
                goal_task_id: c.goal.task_id.clone(),
                artifact_id: input.0.clone(),
                contract_id: input.1.contract_id.clone(),
                source: InputSource::ApprovedExternal {
                    sha256: input.1.sha256.clone(),
                },
            }],
            outputs: vec![PlannedOutput {
                artifact_id: output.0.clone(),
                contract_id: output.1.clone(),
            }],
            required_validators: c.contracts[output.1].clone(),
            side_effect: SideEffect::WorkspaceWrite,
            write_set: vec![WriteTarget::Workspace {
                path: c.edit_context.writable_prefixes[0].clone(),
            }],
            budget: c.budget.clone(),
            reservation: AttemptReservation {
                runtime_ms: 1000,
                output_bytes: 1024,
                tokens: None,
                cost_microunits: None,
            },
            retry: RetryPolicy {
                max_attempts: 1,
                max_reworks: 0,
                retry_classes: vec![],
            },
            idempotency_key: "one-work".into(),
            external_action_id: None,
        }],
    }
}
fn update(s: &FactoryScope, p: &GoalPlanProposal) -> FactoryChildPlanUpdate {
    FactoryChildPlanUpdate {
        scope_id: s.id.clone(),
        task_id: s.root_task_id.clone(),
        plan_json: serde_json::to_string_pretty(p).unwrap() + "\n",
        expected_plan_revision: p.expected_revision,
    }
}
fn unchanged_execution(f: &Fixture) -> serde_json::Value {
    let mut value = serde_json::Map::new();
    for table in [
        "attempts",
        "sessions",
        "dispatch_intents",
        "goal_runs",
        "goal_run_nodes",
        "goal_graph_jobs",
        "goal_graph_acceptances",
    ] {
        let n: i64 = f
            .store
            .conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        value.insert(table.into(), json!(n));
    }
    let data:String=f.store.conn.query_row("SELECT COALESCE(json_group_array(json_array(scope,total_limit,parallel_limit,consumed,active)),'[]') FROM budgets",[],|r|r.get(0)).unwrap();
    value.insert("budgets".into(), json!(data));
    json!(value)
}
#[test]
fn child_plan_original_raw_revisions_cas_exact_replay_and_refs_never_mint_execution_or_expand_zero_child_grant(
) {
    let (mut f, s) = fixture();
    observation(&mut f, &s);
    let c = f.store.factory_child_planning_context(&query(&s)).unwrap();
    assert_eq!(c.current_plan_revision, 0);
    assert!(c.planning_only);
    assert!(!c.goal_execution_authorized);
    let counts = unchanged_execution(&f);
    let mut p = plan(&c, 1);
    let one = update(&s, &p);
    let v = f
        .store
        .submit_factory_child_plan(&one, s.revision, "plan-one")
        .unwrap();
    assert_eq!(v.plan_json, one.plan_json);
    assert_eq!(
        v.reference.raw_sha256,
        agentbaton_workspaces::validation::sha256(one.plan_json.as_bytes())
    );
    assert!(v.planning_only);
    assert!(!v.dispatched);
    assert!(!v.task_accepted);
    f.store.data_input = None;
    let seq = f.store.sequence().unwrap();
    assert_eq!(
        f.store
            .submit_factory_child_plan(&one, s.revision, "plan-one")
            .unwrap()
            .reference,
        v.reference
    );
    assert_eq!(f.store.sequence().unwrap(), seq);
    p.revision = 2;
    p.expected_revision = 1;
    p.nodes[0].goal = "Revised bounded report goal".into();
    let two = update(&s, &p);
    observation(&mut f, &s);
    let v2 = f
        .store
        .submit_factory_child_plan(&two, s.revision, "plan-two")
        .unwrap();
    assert_eq!(v2.reference.revision, 2);
    let refs = f.store.list_factory_child_plans(&query(&s)).unwrap();
    assert_eq!(refs, vec![v.reference.clone(), v2.reference.clone()]);
    assert_eq!(
        f.store
            .get_factory_child_plan(&FactoryChildPlanQuery {
                plan_revision: Some(1),
                ..query(&s)
            })
            .unwrap()
            .plan_json,
        one.plan_json
    );
    assert_eq!(unchanged_execution(&f), counts);
    assert_eq!(
        f.store
            .get_grant(&s.root_grant_id)
            .unwrap()
            .scope
            .max_children,
        0
    );

    if let Some(output) = std::env::var_os("AGENTBATON_CHILD_PLAN_FIXTURE_OUT") {
        let source_hashes = json!({"factory.rs":agentbaton_workspaces::validation::sha256(include_bytes!("factory.rs")),"factory_child_plan.rs":agentbaton_workspaces::validation::sha256(include_bytes!("factory_child_plan.rs")),"factory_reads.rs":agentbaton_workspaces::validation::sha256(include_bytes!("factory_reads.rs")),"goal_plan.rs":agentbaton_workspaces::validation::sha256(include_bytes!("goal_plan.rs")),"factory_child_plan_tests.rs":agentbaton_workspaces::validation::sha256(include_bytes!("factory_child_plan_tests.rs"))});
        let actual = json!({"dimension":"actual Core proposal/finalizer + SQLite + readonly worker-body on direct test thread; synthetic connection and host confirmation, no Provider/MCP/daemon worker join/native GUI","source_hashes":source_hashes,"scope":s,"context":c,"initial":v,"revised":v2,"history":refs,"execution_before_after":counts,"original_grant":f.store.get_grant(&s.root_grant_id).unwrap(),"journal_sequence":f.store.sequence().unwrap()});
        fs::write(output, serde_json::to_vec_pretty(&actual).unwrap()).unwrap();
    }
    f.store.data_input = None;
    let seq = f.store.sequence().unwrap();
    let mut conflict = two.clone();
    conflict.plan_json.push(' ');
    assert_eq!(
        f.store
            .submit_factory_child_plan(&conflict, s.revision, "plan-two")
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    assert_eq!(f.store.sequence().unwrap(), seq);
    // A damaged private database may violate a foreign key; reject its raw
    // oversized head before allocating the TEXT, even in that state.
    f.store
        .conn
        .execute_batch("PRAGMA foreign_keys=OFF")
        .unwrap();
    f.store
        .conn
        .execute(
            "UPDATE factory_child_plan_heads SET revision_id=?1 WHERE scope_id=?2",
            rusqlite::params!["oversized".repeat(600000), s.id],
        )
        .unwrap();
    assert_eq!(
        f.store.get_factory_child_plan(&query(&s)).unwrap_err().code,
        "RESOURCE_LIMIT"
    );
    assert_eq!(f.store.sequence().unwrap(), seq);
}
#[test]
fn child_plan_bad_raw_permission_references_cycles_cas_and_unsafe_numbers_leave_no_partial_rows_or_events(
) {
    let (mut f, s) = fixture();
    observation(&mut f, &s);
    let c = f.store.factory_child_planning_context(&query(&s)).unwrap();
    let base = plan(&c, 1);
    let seq = f.store.sequence().unwrap();
    let mut cases = vec![];
    let mut coherent = base.clone();
    coherent.goal.input_revision = "coherent-foreign-input".into();
    if let Executor::Connection { binding, .. } = &mut coherent.nodes[0].executor {
        binding.input_revision = "coherent-foreign-input".into();
    }
    cases.push(coherent);
    let mut changed_spec = base.clone();
    changed_spec.goal.spec_revision += 1;
    cases.push(changed_spec);
    let mut missing_inputs = base.clone();
    missing_inputs.nodes[0].required_inputs.clear();
    cases.push(missing_inputs);
    let mut missing_outputs = base.clone();
    missing_outputs.nodes[0].outputs.clear();
    cases.push(missing_outputs);
    let mut unapproved_output_alias = base.clone();
    unapproved_output_alias.nodes[0]
        .outputs
        .push(PlannedOutput {
            artifact_id: "unapproved-extra".into(),
            contract_id: base.nodes[0].outputs[0].contract_id.clone(),
        });
    cases.push(unapproved_output_alias);
    for path in ["output/unauthorized-child", "other-output"] {
        let mut widened_write = base.clone();
        widened_write.nodes[0].write_set = vec![WriteTarget::Workspace { path: path.into() }];
        cases.push(widened_write);
    }

    let mut p = base.clone();
    p.acceptance_rules.clear();
    cases.push(p);
    let mut p = base.clone();
    p.nodes[0].dependencies = vec!["work".into()];
    cases.push(p);
    let mut p = base.clone();
    p.nodes[0].workspace_id = "foreign".into();
    cases.push(p);
    let mut p = base.clone();
    p.nodes[0].budget.attempts += 1;
    cases.push(p);
    let mut p = base.clone();
    p.nodes[0].required_validators = vec!["agent-invented-rule".into()];
    cases.push(p);
    let mut p = base.clone();
    p.nodes[0].recipients = vec!["unapproved-recipient".into()];
    cases.push(p);
    let mut p = base.clone();
    p.goal.input_revision = "foreign-input".into();
    cases.push(p);
    let mut p = base.clone();
    p.revision = 2;
    p.expected_revision = 1;
    cases.push(p);
    for (i, p) in cases.iter().enumerate() {
        assert!(
            f.store
                .submit_factory_child_plan(&update(&s, p), s.revision, &format!("negative-{i}"))
                .is_err(),
            "unexpected acceptance of negative case {i}: {p:?}"
        );
        assert_eq!(f.store.sequence().unwrap(), seq);
    }
    let raw = update(&s, &base);
    for text in [
        raw.plan_json.replacen(
            "\"schema_version\": 1",
            "\"schema_version\": 1, \"schema_version\": 1",
            1,
        ),
        raw.plan_json
            .replacen("\"revision\": 1", "\"revision\": 9007199254740993", 1),
        " ".repeat(262145),
    ] {
        let p = FactoryChildPlanUpdate {
            plan_json: text,
            ..raw.clone()
        };
        assert!(f
            .store
            .submit_factory_child_plan(&p, s.revision, "raw-negative")
            .is_err());
        assert_eq!(f.store.sequence().unwrap(), seq)
    }
    assert!(f
        .store
        .list_factory_child_plans(&query(&s))
        .unwrap()
        .is_empty());
}
#[test]
fn child_plan_journal_insert_fault_rolls_back_head_revision_and_idempotency() {
    let (mut f, s) = fixture();
    observation(&mut f, &s);
    let c = f.store.factory_child_planning_context(&query(&s)).unwrap();
    let p = update(&s, &plan(&c, 1));
    let seq = f.store.sequence().unwrap();
    f.store.conn.execute_batch("CREATE TRIGGER refuse_child_plan BEFORE INSERT ON factory_child_plan_heads BEGIN SELECT RAISE(ABORT,'controlled head failure after journal'); END;").unwrap();
    assert!(f
        .store
        .submit_factory_child_plan(&p, s.revision, "atomic-plan")
        .is_err());
    assert_eq!(f.store.sequence().unwrap(), seq);
    assert!(f
        .store
        .list_factory_child_plans(&query(&s))
        .unwrap()
        .is_empty());
    f.store
        .conn
        .execute_batch("DROP TRIGGER refuse_child_plan;")
        .unwrap();
    let v = f
        .store
        .submit_factory_child_plan(&p, s.revision, "atomic-plan")
        .unwrap();
    assert_eq!(v.reference.revision, 1);
}
#[test]
fn child_plan_observation_must_be_request_owned_and_detects_post_worker_input_or_policy_drift() {
    let (mut f, s) = fixture();
    let q = query(&s);
    assert_eq!(
        f.store.factory_child_planning_context(&q).unwrap_err().code,
        "WORKSPACE_BUSY"
    );
    observation(&mut f, &s);
    let c = f.store.factory_child_planning_context(&q).unwrap();
    let p = update(&s, &plan(&c, 1));
    let seq = f.store.sequence().unwrap();
    fs::write(&f.input, b"changed in place").unwrap();
    assert!(f
        .store
        .submit_factory_child_plan(&p, s.revision, "input-drift")
        .is_err());
    assert_eq!(f.store.sequence().unwrap(), seq);
}
#[test]
fn child_plan_history_remains_readonly_after_revoke_exact_key_does_not_upgrade_authority() {
    let (mut f, s) = fixture();
    observation(&mut f, &s);
    let c = f.store.factory_child_planning_context(&query(&s)).unwrap();
    let p = update(&s, &plan(&c, 1));
    let one = f
        .store
        .submit_factory_child_plan(&p, s.revision, "historic-plan")
        .unwrap();
    f.store.data_input = None;
    f.store
        .revoke_factory_scope(&s.id, s.revision, "revoke")
        .unwrap();
    let seq = f.store.sequence().unwrap();
    assert_eq!(
        f.store
            .get_factory_child_plan(&query(&s))
            .unwrap()
            .reference,
        one.reference
    );
    assert_eq!(
        f.store
            .factory_child_planning_context(&query(&s))
            .unwrap()
            .scope_revision,
        1
    );
    assert_eq!(
        f.store
            .submit_factory_child_plan(&p, s.revision, "historic-plan")
            .unwrap()
            .reference,
        one.reference
    );
    assert!(f
        .store
        .submit_factory_child_plan(&p, 2, "new-after-revoke")
        .is_err());
    assert_eq!(f.store.sequence().unwrap(), seq);
}

mod child_goal_policy_tests {
    include!("factory_child_goal_policy_tests.rs");
}
