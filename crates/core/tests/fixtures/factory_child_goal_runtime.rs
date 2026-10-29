//! Generated ACP -> real fixed MCP -> Actor/Graph -> actual private Owner receipt.
//! Uses an explicitly frozen schema15 companion. No model, paid API or OS dialog.
use super::*;
use agentbaton_core::ipc::{call, Daemon};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
struct RunningDaemon {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<agentbaton_core::Result<()>>>,
    db: PathBuf,
    workspace: PathBuf,
    evidence: PathBuf,
}
impl Drop for RunningDaemon {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let joined = match self.thread.take() {
            Some(thread) => match thread.join() {
                Ok(Ok(())) => json!({"joined":true,"daemon_result":"ok"}),
                Ok(Err(e)) => json!({"joined":true,"daemon_result":"error","error":e.to_string()}),
                Err(_) => json!({"joined":true,"daemon_result":"panic"}),
            },
            None => json!({"joined":false,"daemon_result":"missing_owned_thread"}),
        };
        // Do not mask an original assertion panic with a cleanup assertion.
        // Preserve private raw DB/stderr before Fixture's TempDir is removed.
        let captured = (|| -> std::io::Result<()> {
            let copy = self.evidence.join("runtime.raw.sqlite3");
            fs::copy(&self.db, &copy)?;
            fs::set_permissions(&copy, fs::Permissions::from_mode(0o600))?;
            let stderr = self.workspace.join("reports/fixture-stderr.txt");
            if stderr.exists() {
                let dst = self.evidence.join("generated-provider.stderr.log");
                fs::copy(stderr, &dst)?;
                fs::set_permissions(dst, fs::Permissions::from_mode(0o600))?;
            }
            let mut report = joined.clone();
            report["dimension"]=json!("current Core daemon actor thread; actual owned Provider/MCP process evidence remains in DB");
            report["raw_db_sha256"] = json!(sha256(&fs::read(&copy)?));
            report["raw_db_bytes"] = json!(fs::metadata(&copy)?.len());
            report["test_panicking"] = json!(std::thread::panicking());
            fs::write(
                self.evidence.join("owned-cleanup.json"),
                serde_json::to_vec_pretty(&report)?,
            )?;
            Ok(())
        })();
        if let Err(error) = captured {
            eprintln!(
                "CHILD_GOAL_EVIDENCE_CAPTURE_FAILED path={} error={error}",
                self.evidence.display()
            );
        }
        eprintln!(
            "CHILD_GOAL_OWNED_CLEANUP evidence={} result={joined}",
            self.evidence.display()
        );
    }
}
#[test]
fn actual_factory_child_goal_scoped_execute_owned_stop_owner_acceptance_parent_resume() {
    actual_generated_child_goal(false, false);
}
#[test]
fn actual_factory_root_goal_owned_stop_owner_acceptance_completes_without_cancel_or_resume() {
    actual_generated_child_goal(true, false);
}
#[test]
fn actual_factory_child_goal_one_second_deadline_retains_unknown_without_replay_or_refund() {
    actual_generated_child_goal(false, true);
}
fn actual_generated_child_goal(root_self: bool, deadline_probe: bool) {
    let mut f = Fixture::new();
    let companion=PathBuf::from(std::env::var_os("AGENTBATON_CHILD_GOAL_COMPANION").expect("Provide an explicitly frozen matching schema15 Baton companion; schema14 tools are not this test"));
    assert!(companion.is_absolute());
    fs::copy(&companion, &f.companion).unwrap();
    fs::set_permissions(&f.companion, fs::Permissions::from_mode(0o700)).unwrap();
    let provider = f
        ._temp
        .path()
        .canonicalize()
        .unwrap()
        .join("generated-child-goal-acp");
    let script = child_goal_provider(
        &f.selection.connection_ids[0],
        &f.selection.workspace_ids[0],
        root_self,
        deadline_probe,
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
    let mut connection: serde_json::Value = serde_json::from_str(&raw).unwrap();
    connection["executable"] = json!(provider);
    connection["executable_sha256"] = json!(sha256(script.as_bytes()));
    let plan = RuntimePlan::empty(RuntimeProvider::CustomAcp);
    connection["config_revision"]=json!(sha256(&serde_json::to_vec(&json!({"arguments":[],"protocol_route":"custom_acp","runtime_plan_digest":plan.plan_digest().unwrap(),"host_capabilities":{"fs":false,"terminal":false}})).unwrap()));
    db.execute(
        "UPDATE connections SET document=?1 WHERE id=?2",
        params![connection.to_string(), f.selection.connection_ids[0]],
    )
    .unwrap();
    // Independent real Native grant+Goal-scope approvals establish their own
    // original children/depth ceilings before the separate Factory approval.
    // The base children0 Grant is never edited or silently upgraded.
    let base_grant = f.store.get_grant(&f.selection.grant_id).unwrap();
    // The independent Native Goal approval establishes the same shared meter
    // ceiling BEFORE Factory approval; a later scope must not raise/reset it.
    f.selection.root_budget.output_bytes = 12288;
    f.selection.project_budget.output_bytes = 12288;
    let _unexecuted_native = prepare_mixed_goal(&mut f);
    assert_eq!(
        json!(f.store.get_grant(&base_grant.id).unwrap()),
        json!(base_grant)
    );
    f.selection.capabilities.child_planning = true;
    f.selection.capabilities.child_goal_execution = true;
    f.selection.child_planning_policy = Some(FactoryChildPlanningPolicy {
        max_nodes: 4,
        max_revisions: 8,
        max_plan_bytes: 262144,
        outputs: f
            .selection
            .outputs
            .iter()
            .cloned()
            .map(
                |artifact| agentbaton_core::goal_store::GoalOutputSelection {
                    artifact,
                    validators: vec![],
                },
            )
            .collect(),
        writable_prefixes: vec!["reports/report.txt".into()],
        owner_acceptance_criteria:
            "Review the exact generated complete child report and its SHA/root/version".into(),
    });
    f.selection.child_goal_execution_policy = Some(FactoryChildGoalExecutionPolicy {
        max_nodes: 2,
        max_runs_per_task: 1,
        max_total_runs: 2,
        root_budget: f.selection.root_budget.clone(),
        project_budget: f.selection.project_budget.clone(),
        require_owner_acceptance: true,
    });
    let scope = f.scope();
    let grant = f.store.get_grant(&scope.root_grant_id).unwrap();
    assert_eq!(grant.scope.max_children, 3);
    assert_eq!(grant.revision, scope.root_grant_revision);
    let workspace = scope.bindings.workspaces[0].root.clone();
    let runtime = f._temp.path().canonicalize().unwrap();
    drop(db);
    drop(f.store);
    let daemon = Daemon::bind(&runtime).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let workerstop = stop.clone();
    let evidence = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../artifacts/factory-child-goal-core/runtime-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&evidence).unwrap();
    fs::set_permissions(&evidence, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(evidence.join("started.json"),serde_json::to_vec_pretty(&json!({"runtime":runtime,"source_fixture_sha256":sha256(include_bytes!("factory_child_goal_runtime.rs")),"companion_sha256":sha256(&fs::read(&companion).unwrap()),"in_process_daemon_actor":true})).unwrap()).unwrap();
    let running = RunningDaemon {
        stop,
        thread: Some(std::thread::spawn(move || daemon.run(workerstop))),
        db: f.path.clone(),
        workspace: workspace.clone(),
        evidence: evidence.clone(),
    };
    let reply=call(&runtime,json!({"method":"factory_runs.start","params":{"scope_id":scope.id},"expected_revision":scope.revision,"idempotency_key":"child-goal-root-start"})).unwrap();
    assert!(reply.get("error").is_none(), "{reply}");
    let until = Instant::now() + Duration::from_secs(15);
    let (before, proposal, decision) = loop {
        let runs = mixed_call(
            &runtime,
            json!({"method":"factory_runs.get","params":{"scope_id":scope.id}}),
        );
        if let Some(child) = runs["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["parent_task_id"].is_null() == root_self)
        {
            let list = mixed_call(
                &runtime,
                json!({"method":"factory_child_goals.list","params":{"scope_id":scope.id,"task_id":child["task_id"],"goal_run_id":null}}),
            );
            if let Some(g) = list.as_array().unwrap().first() {
                let view = mixed_call(
                    &runtime,
                    json!({"method":"factory_child_goals.get","params":{"scope_id":scope.id,"task_id":child["task_id"],"goal_run_id":g["goal"]["run"]["id"]}}),
                );
                assert_eq!(view["origin"]["factory_scope_id"], scope.id);
                assert_eq!(view["origin"]["root_grant_id"], scope.root_grant_id);
                if deadline_probe {
                    let work = view["details"]["nodes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|n| n["node"]["id"] == "fixed-work")
                        .unwrap();
                    if work["state"] == "outcome_unknown"
                        && work["stop_confirmed"] == true
                        && runs["run"]["state"] == "outcome_unknown"
                    {
                        assert_eq!(work["node"]["reservation"]["runtime_ms"], 1000);
                        assert!(work["actual_runtime_ms"].as_u64().unwrap() >= 1000);
                        assert!(work["actual_output_bytes"].is_null());
                        assert_eq!(view["details"]["task_accepted"], false);
                        assert!(view["details"]["candidates"].as_array().unwrap().is_empty());
                        let hold_reached = workspace.join("reports/goal-deadline-started").exists();
                        // Stage is observed, never assumed: the original 1s
                        // budget may expire before the first prompt reaches
                        // the Provider. A delivered prompt intentionally waits.
                        let stable_until = Instant::now() + Duration::from_millis(150);
                        while Instant::now() < stable_until {
                            assert!(
                                Instant::now() < until,
                                "original 15s observer expired during no-replay observation"
                            );
                            let current = mixed_call(
                                &runtime,
                                json!({"method":"factory_runs.get","params":{"scope_id":scope.id}}),
                            );
                            assert_eq!(current["run"]["state"], "outcome_unknown");
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        drop(running);
                        let db = Connection::open(&f.path).unwrap();
                        for (sql, expected) in [
                            ("SELECT count(*) FROM factory_attempts", 1),
                            ("SELECT count(*) FROM goal_run_reservations", 1),
                            ("SELECT count(*) FROM attempts", 2),
                            ("SELECT count(*) FROM goal_graph_acceptances", 0),
                            ("SELECT count(*) FROM goal_graph_owner_decisions", 0),
                            ("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'", 0),
                            ("SELECT count(*) FROM workspace_leases WHERE holder!=''", 0),
                            ("SELECT count(*) FROM tasks WHERE json_extract(document,'$.state')='accepted'", 0),
                        ] { assert_eq!(db.query_row(sql,[],|row|row.get::<_,i64>(0)).unwrap(),expected,"{sql}"); }
                        let stopped: i64=db.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.reason')='duration_limit' AND json_extract(document,'$.payload.process_reaped')=1 AND json_extract(document,'$.payload.no_process_started')=0",[work["session_id"].as_str().unwrap()],|row|row.get(0)).unwrap();
                        assert_eq!(
                            stopped, 1,
                            "a real owned process must be reaped, not a synthetic stopped label"
                        );
                        let reservation: String = db
                            .query_row("SELECT document FROM goal_run_reservations", [], |row| {
                                row.get(0)
                            })
                            .unwrap();
                        let reservation: serde_json::Value =
                            serde_json::from_str(&reservation).unwrap();
                        assert_eq!(reservation["settled"], true);
                        assert!(reservation["actual_runtime_ms"].as_u64().unwrap() >= 1000);
                        assert!(reservation["actual_output_bytes"].is_null());
                        for key in [
                            format!("root:{}", scope.root_task_id),
                            format!("project:{}", scope.project_id),
                        ] {
                            let usage: (i64, i64) = db
                                .query_row(
                                    "SELECT consumed,active FROM budgets WHERE scope=?1",
                                    [key],
                                    |row| Ok((row.get(0)?, row.get(1)?)),
                                )
                                .unwrap();
                            assert_eq!(usage, (2,0), "owned stop releases activity, never cumulative attempt consumption");
                        }
                        let grant_after: String = db
                            .query_row(
                                "SELECT document FROM execution_grants WHERE id=?1",
                                [&scope.root_grant_id],
                                |row| row.get(0),
                            )
                            .unwrap();
                        assert_eq!(
                            serde_json::from_str::<serde_json::Value>(&grant_after).unwrap(),
                            json!(grant)
                        );
                        let cleanup: serde_json::Value = serde_json::from_slice(
                            &fs::read(evidence.join("owned-cleanup.json")).unwrap(),
                        )
                        .unwrap();
                        assert_eq!(cleanup["joined"], true);
                        assert_eq!(cleanup["daemon_result"], "ok");
                        let body = json!({"dimension":"actual generated local ACP/MCP and original 1s node deadline; no paid Provider/OS dialog","deadline_probe":true,"provider_hold_marker_observed":hold_reached,"stage":if hold_reached {"prompt_reached_provider_before_deadline"}else{"deadline_before_provider_prompt_marker"},"source_sha256":sha256(include_bytes!("factory_child_goal_runtime.rs")),"companion_sha256":sha256(&fs::read(&companion).unwrap()),"scope":scope,"unknown_view":view,"factory":runs,"retained_reservation":reservation,"owned_cleanup":cleanup,"journal":db_events(&f.path)});
                        let bytes = serde_json::to_vec_pretty(&body).unwrap();
                        let hash = sha256(&bytes);
                        fs::write(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../artifacts/factory-runtime-core/actual-child-goal-timeout-{hash}.json")),bytes).unwrap();
                        return;
                    }
                }
                if view["details"]["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|n| n["node"]["id"] == "owner-report" && n["state"] == "awaiting_owner")
                {
                    assert!(
                        !deadline_probe,
                        "original 1s timeout probe must not reach private Owner acceptance"
                    );
                    let g = &view["details"];
                    assert_eq!(g["task_accepted"], false);
                    assert_eq!(g["candidates"].as_array().unwrap().len(), 1);
                    let producer = &g["nodes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|n| n["node"]["id"] == "fixed-work")
                        .unwrap();
                    assert_eq!(producer["stop_confirmed"], true);
                    let proposal = mixed_call(
                        &runtime,
                        json!({"method":"goal_node_approvals.propose","params":{"run_id":g["run"]["id"],"node_id":"owner-report","expected_plan_revision":1,"expected_ledger_revision":g["ledger_revision"]},"expected_revision":g["run"]["revision"],"idempotency_key":"child-goal-private-owner-review"}),
                    );
                    assert_eq!(proposal["kind"], "goal_node_acceptance");
                    assert_eq!(
                        proposal["snapshot"]["run_binding"]["factory_origin"],
                        view["origin"]
                    );
                    assert_eq!(proposal["snapshot"]["candidate_receipts"], g["candidates"]);
                    let decision = mixed_call(
                        &runtime,
                        json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":"child-goal-private-owner-finalize"}),
                    );
                    assert_eq!(decision["task_accepted"], true);
                    break (view, proposal, decision);
                }
            }
        }
        assert!(Instant::now()<until,"actual child Goal did not reach owned candidate/private Owner review; run={runs}; stderr={:?}",fs::read_to_string(workspace.join("reports/fixture-stderr.txt")));
        std::thread::sleep(Duration::from_millis(10));
    };
    let final_run = loop {
        let r = mixed_call(
            &runtime,
            json!({"method":"factory_runs.get","params":{"scope_id":scope.id}}),
        );
        if r["run"]["state"] == "completed" {
            break r;
        }
        assert!(
            Instant::now() < until,
            "parent did not resume after actual child acceptance: {r}"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(final_run["run"]["cancel_requested"], false);
    if root_self {
        assert_eq!(final_run["tasks"].as_array().unwrap().len(), 1);
        assert_eq!(final_run["tasks"][0]["task_id"], scope.root_task_id);
        assert_eq!(final_run["tasks"][0]["result_available"], true);
        assert_eq!(final_run["tasks"][0]["cancel_requested"], false);
    }
    assert_eq!(
        fs::read_to_string(workspace.join("reports/mcp-stages.txt")).unwrap(),
        if root_self {
            "parent-initial\n"
        } else {
            "parent-initial\nparent-resumed\n"
        }
    );
    drop(running); // Actual daemon shutdown / owned reap before DB assertions.
    let cleanup: serde_json::Value =
        serde_json::from_slice(&fs::read(evidence.join("owned-cleanup.json")).unwrap()).unwrap();
    assert_eq!(cleanup["joined"], true);
    assert_eq!(cleanup["daemon_result"], "ok");
    assert_eq!(cleanup["test_panicking"], false);
    let db = Connection::open(&f.path).unwrap();
    assert_eq!(
        db.pragma_query_value::<u32, _>(None, "user_version", |row| row.get(0))
            .unwrap(),
        15
    );
    for (sql,expected) in [("SELECT count(*) FROM factory_child_goal_runs",1),("SELECT count(*) FROM factory_attempts",if root_self {1}else{2}),("SELECT count(*) FROM goal_run_reservations",1),("SELECT count(*) FROM goal_graph_capabilities",0),("SELECT count(*) FROM dispatch_intents WHERE state!='stopped'",0),("SELECT count(*) FROM workspace_leases WHERE holder!=''",0),("SELECT count(*) FROM tasks WHERE json_extract(document,'$.state')='accepted'",1),("SELECT count(*) FROM attempts WHERE root_task_id!=(SELECT root_task_id FROM factory_scopes LIMIT 1)",0)]{assert_eq!(db.query_row(sql,[],|row|row.get::<_,i64>(0)).unwrap(),expected,"{sql}");}
    if root_self {
        let root_result: String = db.query_row("SELECT json_extract(document,'$.result.acceptance_state') FROM factory_task_bindings WHERE task_id=?1",[&scope.root_task_id],|row|row.get(0)).unwrap();
        assert_eq!(root_result, "task_accepted");
        let task_state: String = db
            .query_row(
                "SELECT json_extract(document,'$.state') FROM tasks WHERE id=?1",
                [&scope.root_task_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(task_state, "accepted");
    }
    let stored_grant: serde_json::Value = serde_json::from_str(
        &db.query_row(
            "SELECT document FROM execution_grants WHERE id=?1",
            [&scope.root_grant_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(stored_grant, json!(grant));
    let body = json!({"dimension":"generated local ACP + actual fixed schema15 MCP + Actor UnixIPC + actual joined workers/private Owner finalizer; no paid Provider or OS dialog","root_self":root_self,"source_hashes":{"factory_child_goal_runtime.rs":sha256(include_bytes!("factory_child_goal_runtime.rs")),"core_goal_factory.rs":sha256(include_bytes!("../../src/goal_factory.rs")),"core_graph_stop.rs":sha256(include_bytes!("../../src/goal_graph_stop.rs"))},"companion_sha256":sha256(&fs::read(companion).unwrap()),"scope":scope,"before_owner":before,"owner_proposal":proposal,"owner_decision":decision,"final_factory":final_run,"journal":db_events(&f.path)});
    let bytes = serde_json::to_vec_pretty(&body).unwrap();
    let hash = sha256(&bytes);
    fs::write(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../artifacts/factory-runtime-core/actual-child-goal-{hash}.json"
        )),
        bytes,
    )
    .unwrap();
}

fn child_goal_provider(
    connection: &str,
    workspace: &str,
    root_self: bool,
    deadline_probe: bool,
) -> String {
    let mut script = ACP_FACTORY.replace(
        "mcp=None;mid=0",
        &format!(
            "status_connection={}\nstatus_workspace={}\nfixture_goal_deadline_probe={}\nfixture_mode='child_goal'\nmcp=None;mid=0",
            serde_json::to_string(connection).unwrap(),
            serde_json::to_string(workspace).unwrap(),
            if deadline_probe { "True" } else { "False" }
        ),
    );
    script=script.replace("names==['delegate_task','get_task_status','get_task_result','cancel_task']","names==['delegate_task','get_task_status','get_task_result','cancel_task','submit_child_plan','execute_child_goal','advance_child_goal']");
    script = script
        .replace(
            "assert goal['outputs'][0]['artifact']['relative_path']=='reports/goal-report.txt'",
            "assert goal['outputs'][0]['artifact']['relative_path']=='reports/report.txt'",
        )
        .replace(
            "open('reports/goal-report.txt','w')",
            "open('reports/report.txt','w')",
        );
    script = script.replace("    assert goal['outputs'][0]['artifact']['relative_path']=='reports/report.txt'", "    if fixture_goal_deadline_probe:\n     open('reports/goal-deadline-started','w').write('actual Goal prompt received')\n     import time\n     time.sleep(60)\n    assert goal['outputs'][0]['artifact']['relative_path']=='reports/report.txt'");
    script=script.replace("  if stage=='parent-initial':",r#"  if stage=='parent-initial':
   import copy
   child=tool('delegate_task',{'goal':'Produce and accept only the actual fixed child report','connection_id':status_connection,'workspace_id':status_workspace,'input_artifact_ids':['input'],'output_artifact_ids':['report'],'budget':{'attempts':2,'parallel':1,'runtime_ms':5000,'output_bytes':4096,'children':1,'reworks':0,'tokens':None,'cost_microunits':None},'wait_for_child':True,'idempotency_key':'actual-executing-child'})
   childstatus=tool('get_task_status',{'task_id':child['task_id']});ctx=childstatus['child_planning_context']
   assert ctx['goal_execution_authorized']is False
   p=child_plan(ctx,1);work=p['nodes'][0];work['budget']['children']=0
   owner=copy.deepcopy(work);owner.update({'id':'owner-report','kind':'owner_approval','goal':'Review the exact candidate','dependencies':[work['id']],'executor':{'kind':'owner','rule_id':ctx['required_acceptance_rules'][0]},'outputs':[],'required_inputs':[{'goal_task_id':ctx['goal']['task_id'],'artifact_id':'report','contract_id':'output:report','source':{'kind':'node','node_id':work['id'],'candidate_for_validation':True}}],'required_validators':ctx['required_acceptance_rules'],'side_effect':'advisory_read_only','write_set':[],'idempotency_key':'owner-report-once'})
   work['reservation']['runtime_ms']=1000 if fixture_goal_deadline_probe else 4000
   p['nodes'].append(owner)
   saved=tool('submit_child_plan',{'task_id':child['task_id'],'plan_json':json.dumps(p,indent=2)+'\n','expected_plan_revision':0,'idempotency_key':'actual-child-executable-plan'})
   childstatus=tool('get_task_status',{'task_id':child['task_id']})
   queued=tool('execute_child_goal',{'task_id':child['task_id'],'plan':saved['reference'],'expected_factory_run_revision':childstatus['factory_run_revision'],'idempotency_key':'actual-child-goal-queue'})
   assert queued['goal']['task_accepted']is False and queued['goal']['run']['task_id']==child['task_id']
   open('reports/actual-child-goal-queued.json','w').write(json.dumps(queued))
   response(r,{'stopReason':'end_turn'});continue
  if stage=='parent-resumed':
   assert c['handoff_result']['acceptance_state']=='task_accepted'
   assert c['handoff_result']['task']['confirmed_stop']is True
   assert any(x.startswith('goal-task-acceptance:')for x in c['handoff_result']['validation_refs'])
   output=c['output_artifacts'][0]['requirement']['relative_path']
   with open(output,'w')as f:f.write(stage+' '+c['output_artifacts'][0]['requirement']['artifact_id']+' report\n')
   response(r,{'stopReason':'end_turn'});continue
"#);
    if root_self {
        let marker = "   child=tool('delegate_task',{'goal':'Produce and accept only the actual fixed child report'";
        assert_eq!(
            script
                .lines()
                .filter(|line| line.starts_with(marker))
                .count(),
            1
        );
        script = script
            .lines()
            .map(|line| {
                if line.starts_with(marker) {
                    "   child={'task_id':c['task_id']}"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
    }
    // Each inserted hook carries the real same GoalPlan model. Graph workers
    // have zero MCP servers and never receive a separate Factory credential.
    script = script.replace(
        "mcp=None;mid=0",
        &format!(
            "{}\nmcp=None;mid=0",
            include_str!("factory_child_planning_acp.py")
        ),
    );
    script
}
