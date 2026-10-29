//! Real deterministic failure, joined planner recovery and same-Graph adoption.
//! This generated local ACP fixture never calls a model or OS approval dialog.
use super::*;
use agentbaton_core::{goal_plan::*, goal_store::*};
use serde_json::Value;
use std::os::unix::fs::MetadataExt;

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

// Preserve the original RunningDaemon Drop (including panic unwind) first.
// Only then record this generated fixture's stable main-file/no-sidecar
// observation. This is not a live SQLite snapshot or process-tree stop proof.
struct ReworkRetainedDaemon(Option<RunningDaemon>, (u64, u64, u32));
impl Drop for ReworkRetainedDaemon {
    fn drop(&mut self) {
        let Some(running) = self.0.take() else { return };
        let db = running.db.clone();
        let evidence = running.evidence.clone();
        drop(running);
        let checked = (|| -> std::io::Result<bool> {
            let read_private = |path: &Path, cap: u64| -> std::io::Result<Vec<u8>> {
                use std::io::Read;
                let mut file = fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                    .open(path)?;
                let before = file.metadata()?;
                let stamp = |m: &fs::Metadata| {
                    (
                        m.dev(),
                        m.ino(),
                        m.mode(),
                        m.uid(),
                        m.nlink(),
                        m.len(),
                        m.mtime(),
                        m.mtime_nsec(),
                        m.ctime(),
                        m.ctime_nsec(),
                    )
                };
                if !before.is_file()
                    || before.nlink() != 1
                    || before.uid() != self.1 .2
                    || !matches!(before.mode() & 0o777, 0o600 | 0o644)
                    || before.len() == 0
                    || before.len() > cap
                {
                    return Err(std::io::ErrorKind::InvalidData.into());
                }
                let mut raw = Vec::new();
                Read::by_ref(&mut file)
                    .take(cap + 1)
                    .read_to_end(&mut raw)?;
                if raw.len() as u64 != before.len()
                    || stamp(&before) != stamp(&file.metadata()?)
                    || stamp(&before) != stamp(&fs::symlink_metadata(path)?)
                {
                    return Err(std::io::ErrorKind::InvalidData.into());
                }
                Ok(raw)
            };
            let absent = || {
                [db.clone(), evidence.join("runtime.raw.sqlite3")]
                    .iter()
                    .all(|base| {
                        ["-wal", "-shm", "-journal"].iter().all(|suffix| {
                            let mut path = base.as_os_str().to_owned();
                            path.push(suffix);
                            matches!(fs::symlink_metadata(PathBuf::from(path)),Err(e)
                            if e.kind()==std::io::ErrorKind::NotFound)
                        })
                    })
            };
            let cleanup: Value = serde_json::from_slice(&read_private(
                &evidence.join("owned-cleanup.json"),
                16384,
            )?)?;
            if cleanup["joined"] != true || cleanup["daemon_result"] != "ok" || !absent() {
                return Ok(false);
            }
            let before = fs::symlink_metadata(&db)?;
            if (before.dev(), before.ino(), before.uid()) != self.1 {
                return Ok(false);
            }
            let source = read_private(&db, 16 * 1024 * 1024)?;
            let copied = read_private(&evidence.join("runtime.raw.sqlite3"), 16 * 1024 * 1024)?;
            let after = fs::symlink_metadata(&db)?;
            Ok(absent()
                && (
                    before.dev(),
                    before.ino(),
                    before.len(),
                    before.mtime(),
                    before.mtime_nsec(),
                    before.ctime(),
                    before.ctime_nsec(),
                ) == (
                    after.dev(),
                    after.ino(),
                    after.len(),
                    after.mtime(),
                    after.mtime_nsec(),
                    after.ctime(),
                    after.ctime_nsec(),
                )
                && source == copied
                && cleanup["raw_db_bytes"] == copied.len() as u64
                && cleanup["raw_db_sha256"] == sha256(&copied))
        })();
        // A diagnostic failure must not replace the original assertion panic.
        let consistent = matches!(checked, Ok(true));
        let record = json!({"contract":"rework-post-join-main-file.v1",
            "main_file_copy_consistent":consistent,
            "status":if consistent{"observed_consistent"}else{"incomplete"},
            "snapshot_time":"post_owned_shutdown_not_deadline_instant",
            "not_a_general_sqlite_snapshot_or_process_stop_proof":true});
        let saved = (|| -> std::io::Result<()> {
            use std::io::Write;
            let raw = serde_json::to_vec(&record)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(evidence.join("post-shutdown-snapshot.json"))?;
            file.write_all(&raw)
        })();
        if saved.is_err() {
            eprintln!("REWORK_SNAPSHOT status=incomplete code=SNAPSHOT_RECEIPT_NOT_SAVED");
        }
    }
}

// Finite diagnostic projection only. IDs, messages and raw list items never
// enter public evidence. Revision1's failed Validator is expected rework input;
// revision2 must still reach the original full-view Owner assertions below.
fn rework_list_progress(result: &Value) -> Value {
    let Some(items) = result.as_array() else {
        return json!({"summary_count":null,"failure_code":"REWORK_LIST_INVALID"});
    };
    if items.is_empty() {
        return json!({"summary_count":0,"failure_code":null});
    }
    if items.len() != 1 {
        return json!({"summary_count":items.len(),"failure_code":"REWORK_LIST_INVALID"});
    }
    let head = &items[0];
    let run = &head["goal"]["run"];
    let plan_revision = head["plan"]["revision"].as_u64();
    let run_plan_revision = run["plan_revision"].as_u64();
    let state = run["state"].as_str().filter(|state| {
        matches!(
            *state,
            "queued"
                | "running"
                | "awaiting_acceptance"
                | "blocked"
                | "failed"
                | "cancel_requested"
                | "cancelled"
                | "outcome_unknown"
                | "accepted"
                | "completed"
        )
    });
    let task_accepted = head["goal"]["task_accepted"].as_bool();
    let run_task_accepted = run["task_accepted"].as_bool();
    let cancel_requested = run["cancel_requested"].as_bool();
    let digest_matches = head["plan"]["plan_digest"]
        .as_str()
        .zip(run["plan_digest"].as_str())
        .map(|(a, b)| a == b);
    let failure = if !matches!(plan_revision, Some(1 | 2))
        || state.is_none()
        || plan_revision != run_plan_revision
        || digest_matches != Some(true)
        || run["revision"].as_u64().is_none()
        || task_accepted.is_none()
        || run_task_accepted.is_none()
        || cancel_requested.is_none()
    {
        Some("REWORK_LIST_INVALID")
    } else if state == Some("outcome_unknown") {
        Some("REWORK_OUTCOME_UNKNOWN")
    } else if task_accepted == Some(true)
        || run_task_accepted == Some(true)
        || matches!(state, Some("accepted" | "completed"))
    {
        Some("REWORK_EARLY_ACCEPTANCE")
    } else if plan_revision == Some(2)
        && (cancel_requested == Some(true)
            || matches!(
                state,
                Some("blocked" | "failed" | "cancel_requested" | "cancelled")
            ))
    {
        Some("REWORK_REVISION2_TERMINAL")
    } else {
        None
    };
    json!({"summary_count":1,"plan_revision":plan_revision,
        "run_plan_revision":run_plan_revision,"run_revision":run["revision"].as_u64(),
        "run_state":state,"task_accepted":task_accepted,
        "run_task_accepted":run_task_accepted,"cancel_requested":cancel_requested,
        "plan_digest_matches":digest_matches,"failure_code":failure})
}

// Only this scenario's fixed, already-bound readonly observations may recover
// from a transport failure. In particular this never retries an Owner write.
struct ReworkReadReconciliation {
    scope: String,
    child: Option<String>,
    goal: Option<String>,
    deadline: Instant,
    origin: Instant,
    phase: &'static str,
    scope_expires_at_ms: i64,
    calls: u64,
    method_calls: [u64; 3],
    method_elapsed_ms: [u64; 3],
    retry_budget: u8,
    errors: Vec<Value>,
    evidence: PathBuf,
}
impl ReworkReadReconciliation {
    fn permits(&self, method: &str, params: &Value) -> bool {
        match method {
            "factory_runs.get" => *params == json!({"scope_id":self.scope}),
            "factory_child_goals.list" => self.child.as_ref().is_some_and(|child| {
                *params == json!({"scope_id":self.scope,"task_id":child,"goal_run_id":null})
            }),
            "factory_child_goals.get" => {
                self.child
                    .as_ref()
                    .zip(self.goal.as_ref())
                    .is_some_and(|(child, goal)| {
                        *params == json!({"scope_id":self.scope,"task_id":child,"goal_run_id":goal})
                    })
            }
            _ => false,
        }
    }
    // A single overwritten scalar record bounds evidence independently of
    // polling frequency. No raw request, result, credential, ID or Grant is
    // copied. This is test instrumentation, not an Actor latency measurement.
    fn observation(
        &self,
        method: &'static str,
        stage: &'static str,
        started: Instant,
        reply: Option<&Value>,
    ) -> Value {
        let now = Instant::now();
        let wall_ms = i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap();
        let result = reply.and_then(|v| v.get("result"));
        // Milestones may omit the already-received reply object. An absent
        // run state is unobserved, not evidence that a run is incomplete/known.
        let run_state = result
            .and_then(|v| v.pointer("/run/state"))
            .and_then(Value::as_str);
        json!({"phase":self.phase,"method":method,"stage":stage,"call_number":self.calls,
            "wall_ms":wall_ms,"scenario_elapsed_ms":self.origin.elapsed().as_millis(),
            "call_elapsed_ms":started.elapsed().as_millis(),
            "method_calls":{"factory_runs.get":self.method_calls[0],"factory_child_goals.list":self.method_calls[1],"factory_child_goals.get":self.method_calls[2]},
            "method_total_elapsed_ms":{"factory_runs.get":self.method_elapsed_ms[0],"factory_child_goals.list":self.method_elapsed_ms[1],"factory_child_goals.get":self.method_elapsed_ms[2]},
            "observation_remaining_ms":self.deadline.saturating_duration_since(now).as_millis(),
            "observation_overdue_ms":now.saturating_duration_since(self.deadline).as_millis(),
            "approved_scope_remaining_ms":self.scope_expires_at_ms.saturating_sub(wall_ms),
            "configured_client_read_timeout_ms":5000,"configured_actor_request_budget_ms":60000,
            "reply_object_supplied":reply.is_some(),"reply_is_error":reply.is_some_and(|v|v.get("error").is_some()),
            "reply_revision":reply.and_then(|v|v.get("revision")).and_then(Value::as_u64),
            "run_revision":result.and_then(|v|v.pointer("/run/revision")).and_then(Value::as_u64),
            "run_completed":run_state.map(|state|state=="completed"),
            "run_outcome_unknown":run_state.map(|state|state=="outcome_unknown"),
            "plan_revision":result.and_then(|v|v.pointer("/summary/plan/revision")).and_then(Value::as_u64),
            "node_count":result.and_then(|v|v.pointer("/details/nodes")).and_then(Value::as_array).map(Vec::len),
            "list_progress":if method=="factory_child_goals.list" {
                result.map(rework_list_progress)
            } else {None}})
    }
    fn record_observation(&self, value: &Value) {
        let raw = serde_json::to_vec_pretty(value).unwrap();
        assert!(raw.len() <= 4096, "scalar evidence has a fixed 4KiB bound");
        fs::write(self.evidence.join("readonly-observation-last.json"), raw)
            .expect("preserve bounded readonly phase evidence");
    }
    fn call(&mut self, runtime: &Path, method: &'static str, params: Value) -> Value {
        assert!(
            self.permits(method, &params),
            "only the original bound readonly observation may be reconciled"
        );
        loop {
            assert!(
                Instant::now() < self.deadline,
                "the original observation deadline cannot be renewed by reconciliation; phase={} method={method} calls={}", self.phase, self.calls
            );
            self.calls += 1;
            let method_index = match method {
                "factory_runs.get" => 0,
                "factory_child_goals.list" => 1,
                "factory_child_goals.get" => 2,
                _ => unreachable!("the existing readonly whitelist was checked above"),
            };
            self.method_calls[method_index] = self.method_calls[method_index].saturating_add(1);
            self.record_observation(&self.observation(
                method,
                "request_begin",
                Instant::now(),
                None,
            ));
            let started = Instant::now();
            let reply = call(runtime, json!({"method":method,"params":params}));
            // Keep the original response deadline sample before accounting/I/O.
            let received_before_deadline = Instant::now() < self.deadline;
            self.method_elapsed_ms[method_index] = self.method_elapsed_ms[method_index]
                .saturating_add(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
            match reply {
                Ok(reply) => {
                    // Recording cannot extend or turn a late reply into success.
                    let mut observed =
                        self.observation(method, "response_received", started, Some(&reply));
                    observed["received_before_original_deadline"] = json!(received_before_deadline);
                    self.record_observation(&observed);
                    assert!(
                        received_before_deadline,
                        "readonly response arrived after the original observation deadline; scalars={observed}"
                    );
                    assert!(reply.get("error").is_none(), "{reply}");
                    if method == "factory_child_goals.list" {
                        let progress = rework_list_progress(&reply["result"]);
                        assert!(progress["failure_code"].is_null(),
                            "unexpected rework progress before Owner acceptance; scalars={progress}");
                    }
                    return reply["result"].clone();
                }
                Err(error) => {
                    self.record_observation(&self.observation(
                        method,
                        "transport_failure",
                        started,
                        None,
                    ));
                    self.errors.push(json!({"method":method,"elapsed_ms":started.elapsed().as_millis(),"actual_transport_error_code":error.code,"actual_side_effect_state":error.side_effect_state,"request_was_readonly":true,"write_retried":false}));
                    fs::write(self.evidence.join("readonly-transport-reconciliation.json"),serde_json::to_vec_pretty(&json!({"dimension":"actual readonly client transport errors; receipt reconciliation is not a product latency PASS","errors":self.errors})).unwrap()).expect("preserve the first readonly transport failure before any retry");
                    assert_eq!(
                        error.code, "PROVIDER_DISCONNECTED",
                        "only this exact transport failure permits a readonly re-read"
                    );
                    assert!(self.retry_budget>0,"fixed readonly reconciliation attempts exhausted; original transport error remains recorded");
                    assert!(
                        Instant::now() < self.deadline,
                        "original observation expired after transport failure"
                    );
                    self.retry_budget -= 1;
                }
            }
        }
    }
}

#[test]
fn rework_list_progress_distinguishes_revision1_recovery_and_revision2_terminal() {
    let mut summary = json!({"id":"not-public","plan":{"revision":1,"plan_digest":"same"},
        "goal":{"task_accepted":false,"run":{"plan_revision":1,"plan_digest":"same",
            "revision":9,"state":"failed","task_accepted":false,"cancel_requested":false}}});
    let first = rework_list_progress(&json!([summary]));
    assert!(first["failure_code"].is_null());
    assert_eq!(first["run_state"], "failed");
    assert!(!serde_json::to_string(&first)
        .unwrap()
        .contains("not-public"));
    summary["plan"]["revision"] = json!(2);
    summary["goal"]["run"]["plan_revision"] = json!(2);
    for state in [
        "blocked",
        "failed",
        "cancel_requested",
        "cancelled",
        "completed",
        "outcome_unknown",
        "accepted",
    ] {
        summary["goal"]["run"]["state"] = json!(state);
        let projected = rework_list_progress(&json!([summary]));
        assert!(projected["failure_code"].is_string(), "{state}");
    }
    for state in ["queued", "running", "awaiting_acceptance"] {
        summary["goal"]["run"]["state"] = json!(state);
        assert!(rework_list_progress(&json!([summary]))["failure_code"].is_null());
    }
    summary["goal"]["run"]["state"] = json!("arbitrary-private-message");
    let invalid = rework_list_progress(&json!([summary]));
    assert_eq!(invalid["failure_code"], "REWORK_LIST_INVALID");
    assert!(invalid["run_state"].is_null());
    assert_eq!(
        rework_list_progress(&json!({}))["failure_code"],
        "REWORK_LIST_INVALID"
    );
    assert_eq!(rework_list_progress(&json!([]))["summary_count"], 0);
}

#[test]
fn readonly_reconciliation_whitelist_rejects_writes_and_other_bindings() {
    let reads = ReworkReadReconciliation {
        scope: "scope".into(),
        child: Some("child".into()),
        goal: Some("goal".into()),
        deadline: Instant::now(),
        origin: Instant::now(),
        phase: "whitelist_control",
        scope_expires_at_ms: 0,
        calls: 0,
        method_calls: [0; 3],
        method_elapsed_ms: [0; 3],
        retry_budget: 2,
        errors: vec![],
        evidence: PathBuf::new(),
    };
    assert!(reads.permits("factory_runs.get", &json!({"scope_id":"scope"})));
    assert!(reads.permits(
        "factory_child_goals.list",
        &json!({"scope_id":"scope","task_id":"child","goal_run_id":null})
    ));
    assert!(reads.permits(
        "factory_child_goals.get",
        &json!({"scope_id":"scope","task_id":"child","goal_run_id":"goal"})
    ));
    for (method, params) in [
        ("approvals.finalize", json!({"scope_id":"scope"})),
        (
            "factory_child_goals.advance_plan",
            json!({"scope_id":"scope"}),
        ),
        ("sessions.create", json!({"scope_id":"scope"})),
        ("factory_runs.get", json!({"scope_id":"foreign"})),
        (
            "factory_runs.get",
            json!({"scope_id":"scope","idempotency_key":"write"}),
        ),
        (
            "factory_child_goals.list",
            json!({"scope_id":"scope","task_id":"foreign","goal_run_id":null}),
        ),
        (
            "factory_child_goals.get",
            json!({"scope_id":"scope","task_id":"child","goal_run_id":"foreign"}),
        ),
        (
            "factory_child_goals.get",
            json!({"scope_id":"scope","task_id":"foreign","goal_run_id":"goal"}),
        ),
    ] {
        assert!(!reads.permits(method, &params), "{method} {params}");
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
            60000,
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
    let scenario_started = Instant::now();
    let mut f = Fixture::new();
    let companion = PathBuf::from(
        std::env::var_os("AGENTBATON_CHILD_GOAL_COMPANION")
            .expect("Provide the frozen source-matching schema15 companion"),
    );
    assert!(companion.is_absolute());
    let companion_sha = sha256(&fs::read(&companion).unwrap());
    fs::copy(&companion, &f.companion).unwrap();
    fs::set_permissions(&f.companion, fs::Permissions::from_mode(0o500)).unwrap();
    assert_eq!(fs::metadata(&f.companion).unwrap().nlink(), 1);
    assert_eq!(sha256(&fs::read(&f.companion).unwrap()), companion_sha);
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
    assert_eq!(original_base_grant.scope.max_children, 0);
    let unused_grant_raw: String = db
        .query_row(
            "SELECT document FROM execution_grants WHERE id=?1",
            [&original_base_grant.id],
            |row| row.get(0),
        )
        .unwrap();
    // This NEW positive scenario reviews a60s Owner wall-clock window. The
    // existing Grant API couples expiry to max_duration_ms; all effective
    // cumulative and Node execution caps stay narrower, as asserted below.
    f.selection.ttl_ms = 60000;
    let source = bootstrap_scope(&mut f);
    assert_eq!(
        json!(f.store.get_grant(&original_base_grant.id).unwrap()),
        json!(original_base_grant)
    );
    let grant = f.store.get_grant(&f.selection.grant_id).unwrap();
    assert_eq!(grant.scope.max_attempts, 8);
    assert_eq!(grant.scope.max_children, 6);
    assert_eq!(grant.scope.max_duration_ms, 60000);
    assert_eq!(f.selection.root_budget.runtime_ms, 30000);
    assert_eq!(f.selection.project_budget.runtime_ms, 30000);
    eprintln!("REWORK_PHASE stage=bootstrap_approved scenario_elapsed_ms={} grant_expires_at_ms={} grant_revoked={} grant_revision={} root_runtime_ms={} project_runtime_ms={}",
        scenario_started.elapsed().as_millis(), grant.expires_at_ms, grant.revoked, grant.revision,
        f.selection.root_budget.runtime_ms, f.selection.project_budget.runtime_ms);
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
    // Keep this original Store and observer connection until the trusted
    // fixture's Factory approval below. Do not reopen Store (which performs
    // recovery), or derive its private companion from a Cargo debug/deps ELF.
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
    let original_db = fs::symlink_metadata(&f.path).unwrap();
    let running = ReworkRetainedDaemon(
        Some(RunningDaemon {
            stop,
            thread: Some(std::thread::spawn(move || daemon.run(workerstop))),
            db: f.path.clone(),
            workspace: workspace_root.clone(),
            evidence: evidence.clone(),
        }),
        (original_db.dev(), original_db.ino(), original_db.uid()),
    );
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
    // The bootstrap candidate was genuinely stopped before cancellation. Its
    // owned intent/lease and all test jobs must be idle before the original
    // fixture Store prepares authority; an unknown/active source cannot pass.
    for sql in [
        "SELECT count(*) FROM sessions WHERE state!='stopped' OR json_extract(document,'$.state')!='stopped'",
        "SELECT count(*) FROM dispatch_intents WHERE state!='stopped'",
        "SELECT count(*) FROM workspace_leases WHERE holder!=''",
        "SELECT count(*) FROM goal_graph_jobs WHERE state IN('prepared','running','cancel_requested','outcome_unknown')",
        "SELECT count(*) FROM command_validation_runs WHERE state IN('prepared','running','cancel_requested','outcome_unknown')",
        "SELECT count(*) FROM factory_attempts",
    ] {
        assert_eq!(db.query_row(sql, [], |row| row.get::<_, i64>(0)).unwrap(),0,"bootstrap must be truly idle before fixture approval: {sql}");
    }
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
    // Existing trusted-host test approval path. Both proposal and finalizer
    // recheck this private0500/single-link exact companion. The production IPC
    // companion_path and all executable permission/link guards are unchanged.
    let p = f
        .store
        .propose_factory_scope(
            &f.selection,
            "rework-full-initial-factory-review",
            &f.companion,
        )
        .unwrap();
    let scope = f.store.finalize_proposal(&p.id, p.revision).unwrap();
    assert_eq!(
        scope["bindings"]["companion"]["executable"],
        json!(f.companion)
    );
    assert_eq!(
        scope["bindings"]["companion"]["executable_sha256"],
        companion_sha
    );
    // Close the retained connections before autonomous work. Daemon shutdown
    // remains the last Store close/checkpoint before existing private evidence
    // capture; no extra open/recovery or lingering observer masks WAL state.
    drop(db);
    drop(f.store);
    assert_eq!(scope["root_grant_id"], grant.id);
    assert_eq!(scope["selection"]["root_budget"]["reworks"], 1);
    assert_eq!(scope["selection"]["ttl_ms"], 60000);
    for budget in ["root_budget", "project_budget"] {
        assert_eq!(scope["selection"][budget]["runtime_ms"], 30000);
        assert_eq!(
            scope["selection"]["child_goal_execution_policy"][budget]["runtime_ms"],
            30000
        );
    }
    assert_eq!(scope["expires_at_ms"], grant.expires_at_ms);
    assert_eq!(
        scope["selection"]["child_planning_policy"]["max_revisions"],
        2
    );
    eprintln!("REWORK_PHASE stage=factory_approved scenario_elapsed_ms={} scope_expires_at_ms={} scope_revision={} scope_ttl_ms={}",
        scenario_started.elapsed().as_millis(),scope["expires_at_ms"],scope["revision"],scope["selection"]["ttl_ms"]);
    mixed_call(
        &runtime,
        json!({"method":"factory_runs.start","params":{"scope_id":scope["id"]},"expected_revision":scope["revision"],"idempotency_key":"rework-start-once"}),
    );
    // Independent positive case's initially reviewed60s wall-clock window is
    // distinct from its unchanged30s cumulative execution budget. Existing
    // normal/1s observers and the original30s expiry failure stay unchanged.
    let until = Instant::now() + Duration::from_secs(55);
    let mut reads = ReworkReadReconciliation {
        scope: scope["id"].as_str().unwrap().into(),
        child: None,
        goal: None,
        deadline: until,
        origin: scenario_started,
        phase: "wait_rework_awaiting_owner",
        scope_expires_at_ms: scope["expires_at_ms"].as_i64().unwrap(),
        calls: 0,
        method_calls: [0; 3],
        method_elapsed_ms: [0; 3],
        retry_budget: 2,
        errors: vec![],
        evidence: evidence.clone(),
    };
    eprintln!(
        "REWORK_PHASE stage=factory_start_returned scalars={}",
        reads.observation(
            "factory_runs.start",
            "owner_write_returned",
            Instant::now(),
            None
        )
    );
    let before = loop {
        // Discover immutable addresses once, not authority. Every later read
        // still goes through the same authenticated server query/whitelist.
        if reads.child.is_none() {
            let factory = reads.call(
                &runtime,
                "factory_runs.get",
                json!({"scope_id":scope["id"]}),
            );
            assert_eq!(factory["run"]["scope_id"], scope["id"]);
            if let Some(child) = factory["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| !t["parent_task_id"].is_null())
            {
                assert_eq!(child["parent_task_id"], scope["root_task_id"]);
                assert_eq!(child["factory_scope_id"], scope["id"]);
                reads.child = Some(child["task_id"].as_str().unwrap().to_owned());
            }
        }
        if let Some(child) = reads.child.clone() {
            let list = reads.call(
                &runtime,
                "factory_child_goals.list",
                json!({"scope_id":scope["id"],"task_id":child,"goal_run_id":null}),
            );
            if let Some(summary) = list.as_array().unwrap().first() {
                assert_eq!(summary["factory_scope_id"], scope["id"]);
                assert_eq!(summary["task_id"], child);
                assert_eq!(summary["parent_task_id"], scope["root_task_id"]);
                assert_eq!(summary["factory_root_task_id"], scope["root_task_id"]);
                assert_eq!(summary["plan"]["scope_id"], scope["id"]);
                assert_eq!(summary["plan"]["run_id"], summary["factory_run_id"]);
                assert_eq!(summary["plan"]["task_id"], child);
                assert_eq!(summary["goal"]["run"]["task_id"], child);
                assert_eq!(
                    summary["plan"]["revision"],
                    summary["goal"]["run"]["plan_revision"]
                );
                assert_eq!(
                    summary["plan"]["plan_digest"],
                    summary["goal"]["run"]["plan_digest"]
                );
                let goal = summary["goal"]["run"]["id"].as_str().unwrap().to_owned();
                if let Some(old) = &reads.goal {
                    assert_eq!(old, &goal);
                } else {
                    reads.goal = Some(goal);
                }
                assert_ne!(
                    summary["goal"]["run"]["state"], "outcome_unknown",
                    "unknown must never become an automatic rework: {summary}"
                );
                // A summary is only a readiness gate. Reconstruct the full
                // validated history after revision2 reaches the Owner stage;
                // it never substitutes for the original full-view assertions.
                if summary["plan"]["revision"] == 2
                    && summary["goal"]["run"]["state"] == "awaiting_acceptance"
                {
                    let view=reads.call(&runtime,"factory_child_goals.get",json!({"scope_id":scope["id"],"task_id":child,"goal_run_id":summary["goal"]["run"]["id"]}));
                    assert_eq!(view["details"]["run"]["id"], summary["goal"]["run"]["id"]);
                    assert_eq!(view["details"]["run"]["task_id"], child);
                    assert_eq!(view["summary"]["plan"], summary["plan"]);
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
        }
        assert!(
            Instant::now() < until,
            "failed Validator did not produce the same-Graph joined rework; child_bound={} goal_bound={} stderr={:?}",
            reads.child.is_some(), reads.goal.is_some(),
            fs::read_to_string(workspace_root.join("reports/fixture-stderr.txt"))
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    eprintln!(
        "REWORK_PHASE stage=rework_awaiting_owner scalars={}",
        reads.observation("factory_child_goals.get", "milestone", Instant::now(), None)
    );
    reads.phase = "owner_exact_second_review";
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
    fs::write(evidence.join("owner-write-checkpoint.json"),serde_json::to_vec_pretty(&json!({"method":"approvals.finalize","owner_reply_received":false,"single_write_call_planned":true,"actual_send_confirmed":false,"write_retried":false})).unwrap()).unwrap();
    let decision = finalize(&runtime, &p, "rework-exact-second-owner-consent");
    fs::write(evidence.join("owner-write-checkpoint.json"),serde_json::to_vec_pretty(&json!({"method":"approvals.finalize","owner_reply_received":true,"reply_sha256":sha256(&serde_json::to_vec(&decision).unwrap()),"task_accepted":decision["task_accepted"],"write_attempts":1,"write_retried":false})).unwrap()).unwrap();
    assert_eq!(decision["task_accepted"], true);
    eprintln!(
        "REWORK_PHASE stage=owner_decision_returned scalars={}",
        reads.observation(
            "approvals.finalize",
            "owner_write_returned",
            Instant::now(),
            None
        )
    );
    reads.phase = "wait_parent_resume_completed";
    let final_factory = loop {
        let value = reads.call(
            &runtime,
            "factory_runs.get",
            json!({"scope_id":scope["id"]}),
        );
        if value["run"]["state"] == "completed" {
            break value;
        }
        assert!(
            Instant::now() < until,
            "parent did not finish within the original observation window: {value}"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
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
    assert_eq!(
        db.query_row(
            "SELECT document FROM execution_grants WHERE id=?1",
            [&original_base_grant.id],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        unused_grant_raw,
        "the original unused children0 Grant is not enlarged or renewed"
    );
    let mut reservations = db
        .prepare("SELECT document FROM factory_budget_reservations")
        .unwrap();
    for raw in reservations
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
    {
        let actual: Value = serde_json::from_str(&raw.unwrap()).unwrap();
        let cap = if actual["task_id"] == scope["root_task_id"] {
            30000
        } else {
            20000
        };
        assert!(actual["reservation"]["runtime_ms"].as_u64().unwrap()<=cap,"the larger reviewed wall-clock window must not enlarge effective execution reservations");
    }
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
    assert_eq!(
        recovery["saved"]["proposal"]["nodes"][0]["budget"]["runtime_ms"],
        20000
    );
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
    // This real chain, unlike the single-reservation library fixture, supplies
    // two legal reservations in each of two revisions of the SAME derived run.
    // Compare every durable fee/stop/binding to the authenticated public Node
    // history, without fabricating another run or exposing a test wire method.
    let mut q = db.prepare("SELECT attempt_id,node_id,document FROM goal_run_reservations WHERE run_id=?1 ORDER BY json_extract(document,'$.plan_revision'),node_id").unwrap();
    let rows = q
        .query_map([goal_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .unwrap();
    let mut revisions = std::collections::BTreeMap::<u64, usize>::new();
    let mut positive_history = Vec::new();
    for row in rows {
        let (attempt, node_id, raw) = row.unwrap();
        let receipt: Value = serde_json::from_str(&raw).unwrap();
        let revision = receipt["plan_revision"].as_u64().unwrap();
        let node = details["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["attempt_id"] == attempt && n["plan_revision"] == revision)
            .expect("every real reservation must retain its exact authenticated historical Node");
        assert_eq!(receipt["run_id"], goal_id);
        assert_eq!(receipt["node_id"], node_id);
        assert_eq!(node["node"]["id"], node_id);
        assert_eq!(receipt["reservation"], node["node"]["reservation"]);
        assert_eq!(receipt["settled"], true);
        assert_eq!(node["stop_confirmed"], true);
        for field in [
            "actual_runtime_ms",
            "actual_output_bytes",
            "tokens",
            "cost_microunits",
        ] {
            assert_eq!(receipt[field], node[field], "{revision}/{node_id}/{field}");
        }
        assert!(receipt["actual_runtime_ms"].is_u64());
        assert!(receipt["actual_output_bytes"].is_u64());
        *revisions.entry(revision).or_default() += 1;
        positive_history.push(json!({"attempt_id":attempt,"node_id":node_id,"receipt":receipt}));
    }
    assert_eq!(
        revisions,
        std::collections::BTreeMap::from([(1, 2), (2, 2)])
    );
    assert_eq!(positive_history.len(), 4);
    // All eight actually settled attempts share this project/root-Grant. Their
    // exact persisted actual fees must match both common ledgers once, without
    // charging the derived projection a second time or resetting old costs.
    let mut fees = db.prepare("SELECT document FROM goal_run_reservations UNION ALL SELECT document FROM factory_budget_reservations").unwrap();
    let mut count = 0;
    let (mut runtime, mut output) = (0u64, 0u64);
    for raw in fees.query_map([], |row| row.get::<_, String>(0)).unwrap() {
        let receipt: Value = serde_json::from_str(&raw.unwrap()).unwrap();
        assert_eq!(receipt["settled"], true);
        runtime = runtime
            .checked_add(receipt["actual_runtime_ms"].as_u64().unwrap())
            .unwrap();
        output = output
            .checked_add(receipt["actual_output_bytes"].as_u64().unwrap())
            .unwrap();
        count += 1;
    }
    assert_eq!(count, 8);
    for key in [
        format!("project:{}", scope["project_id"].as_str().unwrap()),
        format!("root-grant:{}", grant.root_grant_id),
    ] {
        let raw: String = db
            .query_row(
                "SELECT document FROM goal_ledgers WHERE scope=?1",
                [&key],
                |row| row.get(0),
            )
            .unwrap();
        let ledger: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(ledger["runtime_ms"], runtime, "{key}");
        assert_eq!(ledger["output_bytes"], output, "{key}");
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
    let body = json!({"dimension":"real generated ACP/private scoped MCP7/Actor/owned Graph and sessionless fixed Command/joined bounded rework/private Owner/parent resume; no model or OS dialog","positive_budget_history":{"run_id":goal_id,"revisions_each_two":revisions,"receipts":positive_history,"shared_actual_runtime_ms":runtime,"shared_actual_output_bytes":output,"settled_attempt_count":count,"cross_run_reuse":"not claimed"},"factory_scope_approval_route":"original retained Store::propose_factory_scope(selection,private0500-single-link-companion)+trusted fixture finalize_proposal; not factory_scopes.propose IPC or OS dialog","readonly_transport_errors":reads.errors,"readonly_reconciliation_scope":"only bound get/list within original55s; recorded transport failures are not a product latency PASS; Owner writes never retried","test_source_sha256":sha256(include_bytes!("factory_child_goal_rework.rs")),"provider_source_sha256":sha256(include_bytes!("factory_child_goal_rework_acp.py")),"companion_sha256":companion_sha,"fixed_command_sha256":executable_sha,"initial_scope":scope,"definition":definition,"recovery":recovery,"before_owner":before,"owner_proposal":p,"owner_decision":decision,"final_factory":final_factory,"journal":journal});
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
