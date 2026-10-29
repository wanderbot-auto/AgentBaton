//! Real deterministic failure, joined planner recovery and same-Graph adoption.
//! This generated local ACP fixture never calls a model or OS approval dialog.
use super::*;
use agentbaton_core::{goal_plan::*, goal_store::*};
use serde_json::Value;

fn finalize(runtime: &Path, proposal: &Value, key: &str) -> Value {
    mixed_call(
        runtime,
        json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":key}),
    )
}

fn wait_until(
    runtime: &Path,
    method: &str,
    params: Value,
    until: Instant,
    ready: impl Fn(&Value) -> bool,
) -> Value {
    loop {
        let value = mixed_call(runtime, json!({"method":method,"params":params}));
        if ready(&value) {
            return value;
        }
        assert!(
            Instant::now() < until,
            "{method} did not reach its declared milestone: {value}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

// Establish all shared ceilings before either execution approval. The original
// children0 fixture Grant is retained untouched; this is a separate real Native
// Grant and Goal scope, not a synthetic approval or an edited stored policy.
fn bootstrap_scope(f: &mut Fixture) -> GoalScopeView {
    let project = f.store.get(&f.selection.root_task_id).unwrap().project_id;
    let p = f
        .store
        .propose_grant(
            &project,
            &f.selection.workspace_ids[0],
            &f.selection.connection_ids[0],
            8,
            1,
            30000,
            "rework-initial-native-grant",
        )
        .unwrap();
    let grant = f.store.finalize_proposal(&p.id, p.revision).unwrap();
    f.selection.grant_id = grant["id"].as_str().unwrap().into();
    f.selection.expected_grant_revision = grant["revision"].as_u64().unwrap();
    let task = f
        .store
        .create_registered(
            &project,
            "Create the real owned source session for a fixed Command definition",
            "rework-source-task",
        )
        .unwrap();
    let selection=GoalScopeSelection {
        task_id:task.id.clone(),expected_task_revision:task.revision,
        grant_id:f.selection.grant_id.clone(),expected_grant_revision:f.selection.expected_grant_revision,
        max_nodes:1,new_max_children:6,new_max_depth:2,
        root_budget:f.selection.root_budget.clone(),project_budget:f.selection.project_budget.clone(),
        inputs:f.selection.inputs.clone(),outputs:vec![GoalOutputSelection {artifact:f.selection.outputs[0].clone(),validators:vec![]}],
        writable_prefixes:vec!["reports/report.txt".into()],
        owner_acceptance_criteria:"The source candidate only establishes a real owned definition subject; it is not Task acceptance".into(),
    };
    let p = f
        .store
        .propose_goal_scope(&selection, "rework-source-native-scope")
        .unwrap();
    let scope: GoalScopeView =
        serde_json::from_value(f.store.finalize_proposal(&p.id, p.revision).unwrap()).unwrap();
    let contract = &scope.outputs["report"];
    let node = PlanNode {
        id: "bootstrap-report".into(),
        parent_id: None,
        kind: NodeKind::Work,
        goal: "Write the bootstrap source report".into(),
        dependencies: vec![],
        workspace_id: scope.workspace_id.clone(),
        executor: Executor::Connection {
            binding: Box::new(scope.binding.clone()),
            program_sha256: scope.program_sha256.clone(),
        },
        recipients: vec![],
        required_inputs: vec![RequiredInput {
            goal_task_id: task.id,
            artifact_id: "input".into(),
            contract_id: scope.inputs["input"].contract_id.clone(),
            source: InputSource::ApprovedExternal {
                sha256: scope.inputs["input"].sha256.clone(),
            },
        }],
        outputs: vec![PlannedOutput {
            artifact_id: "report".into(),
            contract_id: contract.clone(),
        }],
        required_validators: scope.contracts[contract].clone(),
        side_effect: SideEffect::WorkspaceWrite,
        write_set: vec![WriteTarget::Workspace {
            path: "reports/report.txt".into(),
        }],
        budget: BudgetLimits {
            attempts: 1,
            parallel: 1,
            runtime_ms: 5000,
            output_bytes: 4096,
            children: 0,
            reworks: 0,
            tokens: None,
            cost_microunits: None,
        },
        reservation: AttemptReservation {
            runtime_ms: 4000,
            output_bytes: 1024,
            tokens: None,
            cost_microunits: None,
        },
        retry: RetryPolicy {
            max_attempts: 1,
            max_reworks: 0,
            retry_classes: vec![],
        },
        idempotency_key: "bootstrap-report-once".into(),
        external_action_id: None,
    };
    let proposal = GoalPlanProposal {
        schema_version: 1,
        plan_id: "rework-definition-source-plan".into(),
        goal: scope.goal.clone(),
        scope_revision: scope.authority_revision,
        revision: 1,
        expected_revision: 0,
        acceptance_rules: scope.required_acceptance_rules.clone(),
        nodes: vec![node],
    };
    f.store
        .submit_goal_plan(
            &GoalPlanSubmission {
                scope_id: scope.id.clone(),
                proposal,
            },
            "rework-source-plan",
            0,
        )
        .unwrap();
    f.selection.expected_grant_revision =
        f.store.get_grant(&f.selection.grant_id).unwrap().revision;
    scope
}

#[test]
fn actual_factory_child_goal_failed_command_joined_rework_same_graph_revision_owner_and_parent_resume(
) {
    let mut f = Fixture::new();
    let companion = PathBuf::from(
        std::env::var_os("AGENTBATON_CHILD_GOAL_COMPANION")
            .expect("Provide the frozen source-matching schema15 companion"),
    );
    assert!(companion.is_absolute());
    let companion_sha = sha256(&fs::read(&companion).unwrap());
    fs::copy(&companion, &f.companion).unwrap();
    fs::set_permissions(&f.companion, fs::Permissions::from_mode(0o700)).unwrap();
    let provider = f
        ._temp
        .path()
        .canonicalize()
        .unwrap()
        .join("generated-bounded-rework-acp");
    let script = generated_rework_provider(
        &f.selection.connection_ids[0],
        &f.selection.workspace_ids[0],
    );
    fs::write(&provider, &script).unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o700)).unwrap();
    let db = Connection::open(&f.path).unwrap();
    let raw: String = db
        .query_row(
            "SELECT document FROM connections WHERE id=?1",
            [&f.selection.connection_ids[0]],
            |r| r.get(0),
        )
        .unwrap();
    let mut connection: Value = serde_json::from_str(&raw).unwrap();
    connection["executable"] = json!(provider);
    connection["executable_sha256"] = json!(sha256(script.as_bytes()));
    let plan = RuntimePlan::empty(RuntimeProvider::CustomAcp);
    connection["config_revision"]=json!(sha256(&serde_json::to_vec(&json!({"arguments":[],"protocol_route":"custom_acp","runtime_plan_digest":plan.plan_digest().unwrap(),"host_capabilities":{"fs":false,"terminal":false}})).unwrap()));
    db.execute(
        "UPDATE connections SET document=?1 WHERE id=?2",
        params![connection.to_string(), f.selection.connection_ids[0]],
    )
    .unwrap();
    // Eight shared attempts are initially declared: source1, parent2,
    // Work2, sessionless Command2 and one real recovery planner1.
    f.selection.root_budget.output_bytes = 12288;
    f.selection.project_budget.output_bytes = 12288;
    f.selection.root_budget.children = 6;
    f.selection.project_budget.children = 6;
    // The new case's upfront root child ceiling must fit its full Factory
    // node envelope (the unchanged baseline fixture has only four nodes).
    f.selection.max_total_nodes = 7;
    f.selection.root_budget.reworks = 1;
    f.selection.project_budget.reworks = 1;
    let original_base_grant = f.store.get_grant(&f.selection.grant_id).unwrap();
    let source = bootstrap_scope(&mut f);
    assert_eq!(
        json!(f.store.get_grant(&original_base_grant.id).unwrap()),
        json!(original_base_grant)
    );
    let grant = f.store.get_grant(&f.selection.grant_id).unwrap();
    assert_eq!(grant.scope.max_attempts, 8);
    assert_eq!(grant.scope.max_children, 6);
    let grant_raw: String = db
        .query_row(
            "SELECT document FROM execution_grants WHERE id=?1",
            [&grant.id],
            |r| r.get(0),
        )
        .unwrap();
    let workspace = source.binding.workspace_id.clone();
    let workspace_root: PathBuf = serde_json::from_str::<Value>(
        &db.query_row(
            "SELECT document FROM managed_workspaces WHERE id=?1",
            [&workspace],
            |r| r.get::<_, String>(0),
        )
        .unwrap(),
    )
    .unwrap()["root"]
        .as_str()
        .unwrap()
        .into();
    let runtime = f._temp.path().canonicalize().unwrap();
    drop(db);
    drop(f.store);
    let daemon = Daemon::bind(&runtime).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let workerstop = stop.clone();
    let evidence = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../artifacts/factory-child-goal-core/rework-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&evidence).unwrap();
    fs::set_permissions(&evidence, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(evidence.join("started.json"),serde_json::to_vec_pretty(&json!({"dimension":"generated ACP/real scoped MCP/Core actor; no model or OS dialog","test_source_sha256":sha256(include_bytes!("factory_child_goal_rework.rs")),"provider_source_sha256":sha256(include_bytes!("factory_child_goal_rework_acp.py")),"companion_sha256":companion_sha,"upfront_shared_attempts":8,"upfront_reworks":1,"max_plan_revisions":2})).unwrap()).unwrap();
    let running = RunningDaemon {
        stop,
        thread: Some(std::thread::spawn(move || daemon.run(workerstop))),
        db: f.path.clone(),
        workspace: workspace_root.clone(),
        evidence: evidence.clone(),
    };
    let snapshot = mixed_call(&runtime, json!({"method":"system.snapshot","params":{}}));
    let p = mixed_call(
        &runtime,
        json!({"method":"goal_runs.propose","params":{"scope_id":source.id,"expected_plan_revision":1,"expected_ledger_revision":snapshot["sequence"]},"idempotency_key":"rework-source-execution-review"}),
    );
    let run = finalize(&runtime, &p, "rework-source-execution-consent");
    let done = wait_until(
        &runtime,
        "goal_runs.get",
        json!({"run_id":run["id"]}),
        Instant::now() + Duration::from_secs(5),
        |v| v["run"]["state"] == "awaiting_acceptance",
    );
    assert_eq!(done["nodes"][0]["stop_confirmed"], true);
    // A deterministic fixed executable consumes the actual selected report:
    // first contents fail grep, second contents pass. The command, argv and
    // executable digest are identical for both Graph revisions.
    let executable = fs::canonicalize("/usr/bin/grep").unwrap();
    let executable_sha = sha256(&fs::read(&executable).unwrap());
    let p = mixed_call(
        &runtime,
        json!({"method":"command_validation_policies.propose","params":{"session_id":done["nodes"][0]["session_id"],"executable":executable,"mode":"native_executable","script":null,"inputs":f.selection.inputs,"outputs":f.selection.outputs,"arguments":["-q","-x","rework-second-valid","reports/report.txt"],"environment":{},"expected_exit_codes":[0],"wall_time_ms":1000,"cleanup_time_ms":500,"stdout_limit":512,"stderr_limit":512,"require":agentbaton_core::command_validation::CommandEnforcement::default()},"idempotency_key":"rework-fixed-grep-definition"}),
    );
    let definition = finalize(&runtime, &p, "rework-native-command-definition-consent");
    mixed_call(
        &runtime,
        json!({"method":"goal_runs.cancel","params":{"run_id":run["id"]},"expected_revision":done["run"]["revision"],"idempotency_key":"rework-source-candidate-cancel"}),
    );
    wait_until(
        &runtime,
        "goal_runs.get",
        json!({"run_id":run["id"]}),
        Instant::now() + Duration::from_secs(5),
        |v| v["run"]["state"] == "cancelled",
    );
    f.selection.capabilities.child_planning = true;
    f.selection.capabilities.child_goal_execution = true;
    f.selection.child_planning_policy=Some(FactoryChildPlanningPolicy {max_nodes:3,max_revisions:2,max_plan_bytes:262144,outputs:vec![GoalOutputSelection {artifact:f.selection.outputs[0].clone(),validators:vec![GoalValidatorRef {kind:GoalValidatorKind::Command,policy_id:definition["id"].as_str().unwrap().into(),revision:definition["revision"].as_u64().unwrap()}]}],writable_prefixes:vec!["reports/report.txt".into()],owner_acceptance_criteria:"Owner must accept the new exact repaired report after the same approved deterministic command passes; first-version evidence never accepts the second".into()});
    f.selection.child_goal_execution_policy = Some(FactoryChildGoalExecutionPolicy {
        max_nodes: 3,
        max_runs_per_task: 1,
        max_total_runs: 1,
        root_budget: f.selection.root_budget.clone(),
        project_budget: f.selection.project_budget.clone(),
        require_owner_acceptance: true,
    });
    let p = mixed_call(
        &runtime,
        json!({"method":"factory_scopes.propose","params":f.selection,"idempotency_key":"rework-full-initial-factory-review"}),
    );
    let scope = finalize(&runtime, &p, "rework-full-native-execution-consent");
    assert_eq!(scope["root_grant_id"], grant.id);
    assert_eq!(scope["selection"]["root_budget"]["reworks"], 1);
    assert_eq!(
        scope["selection"]["child_planning_policy"]["max_revisions"],
        2
    );
    mixed_call(
        &runtime,
        json!({"method":"factory_runs.start","params":{"scope_id":scope["id"]},"expected_revision":scope["revision"],"idempotency_key":"rework-start-once"}),
    );
    // Independent new case's declared observer, within the original 30s scope;
    // existing normal/1s tests retain their original15s observer unchanged.
    let until = Instant::now() + Duration::from_secs(25);
    let before = loop {
        let factory = mixed_call(
            &runtime,
            json!({"method":"factory_runs.get","params":{"scope_id":scope["id"]}}),
        );
        if let Some(child) = factory["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| !t["parent_task_id"].is_null())
        {
            let list = mixed_call(
                &runtime,
                json!({"method":"factory_child_goals.list","params":{"scope_id":scope["id"],"task_id":child["task_id"],"goal_run_id":null}}),
            );
            if let Some(summary) = list.as_array().unwrap().first() {
                let view = mixed_call(
                    &runtime,
                    json!({"method":"factory_child_goals.get","params":{"scope_id":scope["id"],"task_id":child["task_id"],"goal_run_id":summary["goal"]["run"]["id"]}}),
                );
                assert_ne!(
                    view["summary"]["goal"]["run"]["state"], "outcome_unknown",
                    "unknown must never become an automatic rework: {view}"
                );
                if view["details"]["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|n| {
                        n["plan_revision"] == 2
                            && n["node"]["id"] == "owner-report"
                            && n["state"] == "awaiting_owner"
                    })
                {
                    break view;
                }
            }
        }
        assert!(
            Instant::now() < until,
            "failed Validator did not produce the same-Graph joined rework: {factory}; stderr={:?}",
            fs::read_to_string(workspace_root.join("reports/fixture-stderr.txt"))
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let details = &before["details"];
    let goal_id = details["run"]["id"].as_str().unwrap();
    assert_eq!(before["origin"]["initial_plan"]["revision"], 1);
    assert_eq!(before["summary"]["plan"]["revision"], 2);
    assert_eq!(details["task_accepted"], false);
    let failed = details["validations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["plan_revision"] == 1)
        .unwrap();
    let passed = details["validations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["plan_revision"] == 2)
        .unwrap();
    assert_eq!(failed["passed"], false);
    assert_eq!(passed["passed"], true);
    assert_eq!(failed["observation"]["exit_code"], 1);
    assert_eq!(passed["observation"]["exit_code"], 0);
    for v in [failed, passed] {
        assert_eq!(v["io_completion"], "worker_joined");
        assert_eq!(v["stop_confirmed"], true);
        assert_eq!(v["scope"], "approved_deterministic_command_only");
        assert_eq!(v["task_acceptance"], "not_established");
        assert_eq!(v["observation"]["process_started"], true);
        assert_eq!(v["observation"]["cleanup"]["owned_leader_reaped"], true);
        assert_eq!(
            v["observation"]["cleanup"]["all_capture_pipes_observed_eof"],
            true
        );
        assert_eq!(v["observation"]["goal_io"]["state"], "observed");
    }
    assert_eq!(failed["definition_digest"], passed["definition_digest"]);
    assert_ne!(
        failed["candidate_refs"][0]["sha256"],
        passed["candidate_refs"][0]["sha256"]
    );
    let p = mixed_call(
        &runtime,
        json!({"method":"goal_node_approvals.propose","params":{"run_id":goal_id,"node_id":"owner-report","expected_plan_revision":2,"expected_ledger_revision":details["ledger_revision"]},"expected_revision":details["run"]["revision"],"idempotency_key":"rework-exact-second-owner-review"}),
    );
    assert_eq!(
        p["snapshot"]["run_binding"]["factory_origin"],
        before["origin"]
    );
    let current_candidates = details["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["plan_revision"] == 2)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(current_candidates.len(), 1);
    assert_eq!(
        p["snapshot"]["candidate_receipts"],
        json!(current_candidates)
    );
    let decision = finalize(&runtime, &p, "rework-exact-second-owner-consent");
    assert_eq!(decision["task_accepted"], true);
    let final_factory = wait_until(
        &runtime,
        "factory_runs.get",
        json!({"scope_id":scope["id"]}),
        until,
        |v| v["run"]["state"] == "completed",
    );
    assert_eq!(
        fs::read_to_string(workspace_root.join("reports/mcp-stages.txt")).unwrap(),
        "parent-initial\nchild-recovery\nparent-resumed\n"
    );
    assert_eq!(sha256(&fs::read(&executable).unwrap()), executable_sha);
    drop(running);
    let db = Connection::open(&f.path).unwrap();
    for (sql,n) in [
        ("SELECT count(*) FROM factory_child_goal_runs",1),
        ("SELECT count(*) FROM factory_attempts",3),
        ("SELECT count(*) FROM attempts",8),
        ("SELECT count(*) FROM goal_run_reservations",5),
        ("SELECT count(*) FROM goal_graph_owner_decisions",1),
        ("SELECT count(*) FROM goal_graph_acceptances",1),
        ("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'",0),
        ("SELECT count(*) FROM workspace_leases WHERE holder!=''",0),
        ("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.factory_failure_confirmed'",1),
        ("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.factory_rework_reserved'",1),
        ("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='goal.factory_plan_adopted'",1),
    ] {assert_eq!(db.query_row(sql,[],|r|r.get::<_,i64>(0)).unwrap(),n,"{sql}");}
    let grant_after: String = db
        .query_row(
            "SELECT document FROM execution_grants WHERE id=?1",
            [&grant.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(grant_after, grant_raw);
    for (key, consumed) in [
        (
            format!("root:{}", scope["root_task_id"].as_str().unwrap()),
            7,
        ),
        (
            format!("project:{}", scope["project_id"].as_str().unwrap()),
            8,
        ),
    ] {
        assert_eq!(
            db.query_row(
                "SELECT consumed,active FROM budgets WHERE scope=?1",
                [&key],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
            )
            .unwrap(),
            (consumed, 0),
            "{key}"
        );
    }
    let recovery: Value = serde_json::from_slice(
        &fs::read(workspace_root.join("reports/rework-recovery.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(recovery["context"]["all_graph_attempts_stopped"], true);
    assert_eq!(recovery["context"]["unknown_effects"], false);
    assert_eq!(recovery["before"]["details"]["run"]["id"], goal_id);
    assert_eq!(recovery["adopted"]["goal"]["run"]["id"], goal_id);
    assert_eq!(recovery["before"]["origin"], before["origin"]);
    assert_eq!(recovery["saved"]["reference"], before["summary"]["plan"]);
    assert_eq!(recovery["saved"]["reference"]["revision"], 2);
    assert_eq!(
        recovery["before"]["summary"]["plan"],
        before["origin"]["initial_plan"]
    );
    assert_eq!(recovery["before"]["details"]["owner_decisions"], json!([]));
    let failed_nodes = recovery["before"]["details"]["nodes"].as_array().unwrap();
    for node in failed_nodes.iter().filter(|n| !n["attempt_id"].is_null()) {
        assert_eq!(node["stop_confirmed"], true);
        assert!(node["actual_runtime_ms"].is_u64());
        assert!(node["actual_output_bytes"].is_u64());
        assert_ne!(node["state"], "outcome_unknown");
    }
    let sequence = |kind: &str| {
        db.query_row("SELECT sequence FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')=?2",params![goal_id,kind],|r|r.get::<_,i64>(0)).unwrap()
    };
    assert!(sequence("goal.factory_failure_confirmed") < sequence("goal.factory_rework_reserved"));
    assert!(sequence("goal.factory_rework_reserved") < sequence("goal.factory_plan_adopted"));
    assert!(
        recovery["context"]["failure_receipt_sequence"]
            .as_u64()
            .unwrap()
            < recovery["context"]["rework_receipt_sequence"]
                .as_u64()
                .unwrap()
    );
    let journal = db_events(&f.path);
    let body = json!({"dimension":"real generated ACP/private scoped MCP7/Actor/owned Graph and sessionless fixed Command/joined bounded rework/private Owner/parent resume; no model or OS dialog","test_source_sha256":sha256(include_bytes!("factory_child_goal_rework.rs")),"provider_source_sha256":sha256(include_bytes!("factory_child_goal_rework_acp.py")),"companion_sha256":companion_sha,"fixed_command_sha256":executable_sha,"initial_scope":scope,"definition":definition,"recovery":recovery,"before_owner":before,"owner_proposal":p,"owner_decision":decision,"final_factory":final_factory,"journal":journal});
    fs::write(
        evidence.join("complete.json"),
        serde_json::to_vec_pretty(&body).unwrap(),
    )
    .unwrap();
    let cleanup: Value =
        serde_json::from_slice(&fs::read(evidence.join("owned-cleanup.json")).unwrap()).unwrap();
    assert_eq!(cleanup["joined"], true);
    assert_eq!(cleanup["daemon_result"], "ok");
    assert_eq!(cleanup["test_panicking"], false);
}

fn generated_rework_provider(connection: &str, workspace: &str) -> String {
    let prefix = ACP_FACTORY
        .split_once(" elif method=='session/prompt':")
        .expect("one fixed base prompt branch")
        .0;
    let mut script=prefix.replace("names==['delegate_task','get_task_status','get_task_result','cancel_task']","names==['delegate_task','get_task_status','get_task_result','cancel_task','submit_child_plan','execute_child_goal','advance_child_goal']");
    script = script.replace(
        "mcp=None;mid=0",
        &format!(
            "status_connection={}\nstatus_workspace={}\n{}\n{}\nmcp=None;mid=0",
            serde_json::to_string(connection).unwrap(),
            serde_json::to_string(workspace).unwrap(),
            include_str!("factory_child_planning_acp.py"),
            include_str!("factory_child_goal_rework_acp.py")
        ),
    );
    script.push_str(r#" elif method=='session/prompt':
  c=json.loads(r['params']['prompt'][0]['text'])
  if mcp is None:
   assert c['kind']=='agentbaton_goal_task_request'
   assert c['outputs'][0]['artifact']['relative_path']=='reports/report.txt'
   text='bootstrap-real-source' if c['node']['id']=='bootstrap-report' else ('rework-first-invalid' if c['plan_revision']==1 else 'rework-second-valid')
   with open('reports/report.txt','w')as out:out.write(text+'\n')
   response(r,{'stopReason':'end_turn'});continue
  assert c['kind']=='agentbaton_factory_task_request'
  status=tool('get_task_status',{'task_id':c['task_id']})
  assert status['task_id']==c['task_id'] and status['child_goal_execution_binding']['new_workspace_authorized']is False
  if c.get('child_goal_recovery'):
   stage='child-recovery'
   context=c['child_goal_recovery'];assert context['all_graph_attempts_stopped']is True and context['unknown_effects']is False and context['plan_revision']==1
   result=tool('get_task_result',{'task_id':c['task_id']});before=result['child_goal']
   assert before['details']['run']['id']==context['goal_run_id'] and before['details']['run']['state']=='failed'
   assert before['details']['task_accepted']is False and before['details']['owner_decisions']==[]
   assert any(v['plan_revision']==1 and v['passed']is False and v['io_completion']=='worker_joined'for v in before['details']['validations'])
   saved=rework_save(status['child_planning_context'],2)
   current=tool('get_task_status',{'task_id':c['task_id']})
   args={'goal_run_id':context['goal_run_id'],'request':{'task_id':c['task_id'],'plan':saved['reference'],'expected_factory_run_revision':current['factory_run_revision']},'expected_goal_run_revision':before['details']['run']['revision'],'idempotency_key':'rework-adopt-same-run-r2'}
   adopted=tool('advance_child_goal',args)
   assert adopted['goal']['run']['id']==context['goal_run_id'] and adopted['plan']==saved['reference'] and adopted['goal']['task_accepted']is False
   assert tool('advance_child_goal',args)==adopted
   with open('reports/rework-recovery.json','w')as out:json.dump({'context':context,'before':before,'saved':saved,'adopted':adopted},out)
  elif c['handoff_result']or c.get('handoff_results'):
   stage='parent-resumed'
   assert c['handoff_result']['acceptance_state']=='task_accepted' and c['handoff_result']['task']['confirmed_stop']is True
   assert any(x.startswith('goal-task-acceptance:')for x in c['handoff_result']['validation_refs'])
   with open('reports/report.txt','w')as out:out.write('original parent resumed after repaired child acceptance\n')
  else:
   stage='parent-initial';assert c['parent_task_id']is None
   child=tool('delegate_task',{'goal':'Repair only the fixed exact report after a real failed deterministic validator','connection_id':status_connection,'workspace_id':status_workspace,'input_artifact_ids':['input'],'output_artifact_ids':['report'],'budget':{'attempts':5,'parallel':1,'runtime_ms':20000,'output_bytes':8192,'children':2,'reworks':1,'tokens':None,'cost_microunits':None},'wait_for_child':True,'idempotency_key':'rework-delegate-original-child'})
   child_status=tool('get_task_status',{'task_id':child['task_id']});ctx=child_status['child_planning_context'];assert ctx['goal_execution_authorized']is False
   saved=rework_save(ctx,1);child_status=tool('get_task_status',{'task_id':child['task_id']})
   queued=tool('execute_child_goal',{'task_id':child['task_id'],'plan':saved['reference'],'expected_factory_run_revision':child_status['factory_run_revision'],'idempotency_key':'rework-execute-original-graph'})
   assert queued['goal']['task_accepted']is False and queued['plan']==saved['reference']
  with open('reports/mcp-stages.txt','a')as out:out.write(stage+'\n')
  response(r,{'stopReason':'end_turn'});continue
 elif method=='session/cancel':
  if mcp is not None:mcp.stdin.close();mcp.wait(timeout=1)
"#);
    script
}
