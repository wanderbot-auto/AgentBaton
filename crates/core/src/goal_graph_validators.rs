//! Intrinsic graph attempts share the original atomic Goal/session ledgers.
//! A job is not a ProviderSession and its view cannot mint a validator receipt.
use super::*;
use crate::goal_validator_io::{
    GoalValidatorDefinition, GoalValidatorObservation, PreparedGoalValidator,
};
use agentbaton_workspaces::validation::{ValidationSubject, VersionBinding};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ValidatorOrigin {
    job_kind: String,
    id: String,
    run_id: String,
    node_id: String,
    plan_revision: u64,
    plan_digest: String,
    attempt_id: String,
    intent_id: String,
    workspace_id: String,
    root: agentbaton_workspaces::WorkspaceIdentity,
    writer_epoch: u64,
    capability_digest: String,
    rule_id: String,
    definition_digest: String,
    definition_source: GoalValidatorRef,
    candidate_refs: Vec<GraphArtifactReference>,
    input_bindings: Vec<GraphInputBinding>,
    reservation: AttemptReservation,
}
pub(crate) struct PreparedGraphValidation {
    origin: ValidatorOrigin,
    rule: PreparedGoalValidator,
    queued_at: Instant,
}
struct Worker {
    origin: ValidatorOrigin,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<Result<GoalValidatorObservation>>,
    queued_at: Instant,
    deadline: Instant,
    unknown_recorded: bool,
}
#[derive(Default)]
pub(super) struct ValidatorWorkers {
    workers: BTreeMap<String, Worker>,
    retained: BTreeMap<String, (ValidatorOrigin, GoalValidatorObservation)>,
}
fn live_definition(
    db: &Connection,
    record: &ScopeRecord,
    rule: &str,
    p: &GraphAuthority,
) -> Result<ResolvedValidator> {
    let def = record
        .refs
        .validators
        .iter()
        .find(|d| d.rule_id == rule)
        .ok_or_else(denied)?
        .clone();
    let approved = p.snapshot["definition_reuse"]
        .as_array()
        .ok_or_else(changed)?
        .iter()
        .find(|d| d["rule_id"] == rule)
        .ok_or_else(denied)?;
    let action = match def.reference.kind {
        GoalValidatorKind::Integrity => "artifact_integrity_only",
        GoalValidatorKind::Data => "data_contract_only",
        GoalValidatorKind::Command => "approved_deterministic_command_only",
    };
    if approved["definition_digest"] != def.digest
        || approved["definition"] != def.definition
        || approved["source"] != json!(def.reference)
        || approved["goal_acceptance"] != false
        || approved["allowed_actions"] != json!([action])
    {
        return Err(denied());
    }
    let table = match def.reference.kind {
        GoalValidatorKind::Integrity => "validation_policies",
        GoalValidatorKind::Data => "data_validation_policies",
        GoalValidatorKind::Command => "command_validation_policies",
    };
    let current: Value = raw(db, table, &def.reference.policy_id)?;
    if current["revoked"] != false
        || current["revision"] != def.reference.revision
        || sha(&current)? != def.digest
    {
        return Err(changed());
    }
    Ok(def)
}
fn normalized_io(
    record: &ScopeRecord,
    producer: &GraphCandidateReceipt,
    refs: &[GraphArtifactReference],
    def: &ResolvedValidator,
    p: &GraphAuthority,
    reservation: &AttemptReservation,
) -> Result<PreparedGoalValidator> {
    if refs.is_empty()
        || refs.len() > 64
        || refs.iter().any(|a| {
            a.producer_attempt_id != producer.attempt_id || !producer.artifacts.contains(a)
        })
    {
        return Err(changed());
    }
    let contract = if def.reference.kind == GoalValidatorKind::Integrity {
        &def.definition["contract"]
    } else if def.reference.kind == GoalValidatorKind::Data {
        &def.definition["file_contract"]
    } else {
        &def.definition["contract"]["file_contract"]
    };
    let template = contract["outputs"].as_array().ok_or_else(changed)?;
    let expected = template
        .iter()
        .map(|o| o["requirement"]["artifact_id"].as_str().ok_or_else(changed))
        .collect::<Result<BTreeSet<_>>>()?;
    if refs
        .iter()
        .map(|a| a.artifact_id.as_str())
        .collect::<BTreeSet<_>>()
        != expected
    {
        return Err(err("CAPABILITY_UNSUPPORTED","Validator requires all exact approved producer outputs; a partial or cross-attempt subject is not authorized"));
    }
    let outputs = refs
        .iter()
        .map(|a| ArtifactRequirement {
            artifact_id: a.artifact_id.clone(),
            relative_path: a.relative_path.clone(),
            max_bytes: a.bytes.max(1),
            expected_sha256: Some(a.sha256.clone()),
        })
        .collect::<Vec<_>>();
    let candidate_pins = outputs
        .iter()
        .cloned()
        .zip(refs.iter().map(|a| a.file_pin.clone()))
        .collect();
    let program_id = def.definition["contract"]["program"]["requirement"]["artifact_id"].as_str();
    let script_id = def.definition["contract"]["script"]["requirement"]["artifact_id"].as_str();
    let selected = contract["inputs"].as_array().ok_or_else(changed)?;
    let inputs = selected
        .iter()
        .filter(|i| {
            i["requirement"]["artifact_id"].as_str() != program_id
                && i["requirement"]["artifact_id"].as_str() != script_id
        })
        .map(|i| {
            let template: ArtifactRequirement =
                serde_json::from_value(i["requirement"].clone()).map_err(|_| changed())?;
            let source = record
                .refs
                .inputs
                .iter()
                .find(|s| {
                    s.artifact.artifact_id == template.artifact_id
                        && s.artifact.relative_path == template.relative_path
                })
                .ok_or_else(changed)?;
            if template
                .expected_sha256
                .as_ref()
                .is_some_and(|sha| sha != &source.sha256)
            {
                return Err(changed());
            }
            let mut a = source.artifact.clone();
            a.expected_sha256 = Some(source.sha256.clone());
            Ok(a)
        })
        .collect::<Result<Vec<_>>>()?;
    let subject = ValidationSubject {
        task_id: producer.task_id.clone(),
        attempt_id: producer.attempt_id.clone(),
        spec_revision: producer.spec_revision,
        input_revision: producer.input_revision.clone(),
        version: if def.reference.kind == GoalValidatorKind::Data {
            VersionBinding::Data {
                dataset_revision: producer.input_revision.clone(),
            }
        } else {
            VersionBinding::WorkspaceArtifacts {
                workspace_id: producer.workspace_id.clone(),
                base_revision: producer.input_revision.clone(),
            }
        },
    };
    let definition = match def.reference.kind {
        GoalValidatorKind::Integrity => GoalValidatorDefinition::Integrity {
            analysis: serde_json::from_value(contract["analysis"].clone())
                .map_err(|_| changed())?,
        },
        GoalValidatorKind::Data => GoalValidatorDefinition::Data {
            table: serde_json::from_value(def.definition["table"].clone())
                .map_err(|_| changed())?,
        },
        GoalValidatorKind::Command => {
            let c = &def.definition["contract"];
            let program = &c["program"];
            let (settings, script) =
                crate::command_validation::graph_definition_settings(&def.definition)?;
            if u64::from(settings.stdout_limit)
                .checked_add(u64::from(settings.stderr_limit))
                .is_none_or(|n| n > reservation.output_bytes)
            {
                return Err(err("RESOURCE_LIMIT","This command's approved retained-output ceilings exceed its Goal reservation; declare adequate existing-scope budget before execution"));
            }
            if script.is_some() {
                let pin = record
                    .refs
                    .workspace
                    .identity
                    .as_ref()
                    .ok_or_else(changed)?;
                if c["script"]["canonical_root"] != json!(record.refs.workspace.root)
                    || c["script"]["root_device"] != pin.device
                    || c["script"]["root_inode"] != pin.inode
                {
                    return Err(changed());
                }
            }
            GoalValidatorDefinition::Command {
                program_root: serde_json::from_value(program["canonical_root"].clone())
                    .map_err(|_| changed())?,
                program_device: program["root_device"].as_u64().ok_or_else(changed)?,
                program_inode: program["root_inode"].as_u64().ok_or_else(changed)?,
                program: serde_json::from_value(program["requirement"].clone())
                    .map_err(|_| changed())?,
                settings,
                script,
            }
        }
    };
    Ok(PreparedGoalValidator {
        approval_id: p.approval_id().into(),
        definition_revision: def.digest.clone(),
        subject,
        workspace: record.refs.workspace.identity.clone().ok_or_else(changed)?,
        inputs,
        outputs,
        candidate_pins,
        definition,
    })
}
impl Store {
    pub(crate) fn prepare_graph_validation(
        &mut self,
        p: &GoalRunDispatch,
        revision: u64,
    ) -> Result<Option<PreparedGraphValidation>> {
        let queued_at = Instant::now();
        let r = run(&self.conn, &p.run_id)?;
        if r.revision != revision || sequence(&self.conn)? != p.expected_ledger_revision {
            return Err(err(
                "STALE_REVISION",
                "Validator admission requires the exact run and shared ledger",
            ));
        }
        if r.cancel_requested || !matches!(r.state.as_str(), "queued" | "running") {
            return Ok(None);
        }
        if local_goal_slot_is_occupied(&self.conn)? {
            return Ok(None);
        }
        let (record, scope) = checked_runtime_scope(&self.conn, &r, self.data_input.as_ref())?;
        let cap = checked_capability(&self.conn, &r)?;
        let candidates = graph_candidates(&self.conn, &r)?;
        let nodes = node_records(&self.conn, &r.id)?;
        let Some(node) = graph_ready_node(&self.conn, &r, &nodes, &candidates)? else {
            return Ok(None);
        };
        let Executor::Validator { rule_id } = &node.node.executor else {
            return Ok(None);
        };
        if node.node.kind != NodeKind::Validate
            || !node.node.outputs.is_empty()
            || !node.node.write_set.is_empty()
            || node.node.side_effect != SideEffect::AdvisoryReadOnly
        {
            return Err(denied());
        }
        let refs = graph_input_refs(&node.node, &candidates, r.plan_revision)?;
        require_read(&self.conn, &r, &refs, self.graph_input.as_ref(), None)?;
        let def = live_definition(&self.conn, &record, rule_id, &cap)?;
        let producer = candidates
            .iter()
            .find(|c| refs.first().is_some_and(|a| c.artifacts.contains(a)))
            .ok_or_else(changed)?;
        let rule = normalized_io(&record, producer, &refs, &def, &cap, &node.node.reservation)?;
        let mut consumed = input_bindings(&node.node, &record, &candidates, r.plan_revision)?;
        for input in &rule.inputs {
            let item = GraphInputBinding {
                artifact_id: input.artifact_id.clone(),
                contract_id: format!("input:{}", input.artifact_id),
                sha256: input.expected_sha256.clone().ok_or_else(changed)?,
                candidate: None,
            };
            if !consumed.contains(&item) {
                consumed.push(item);
            }
        }
        let binding = scope_view(&self.conn, &record, &scope)?.binding;
        let task = self.get(&r.task_id)?;
        let lease = self
            .workspace_lease(&record.refs.workspace.id)?
            .ok_or_else(changed)?;
        if !lease.holder.is_empty() {
            return Err(err(
                "WORKSPACE_BUSY",
                "Validator requires a stopped writer and exclusive read fence",
            ));
        }
        let intent = crate::execution::DispatchIntent {
            id: Uuid::new_v4().to_string(),
            attempt_id: Uuid::new_v4().to_string(),
            task_id: r.task_id.clone(),
            root_task_id: root_task(&record).into(),
            state: crate::execution::IntentState::Prepared,
            executor_instance: None,
            binding: binding.clone(),
        };
        let origin = ValidatorOrigin {
            job_kind: "validator".into(),
            id: Uuid::new_v4().to_string(),
            run_id: r.id.clone(),
            node_id: node.node.id.clone(),
            plan_revision: r.plan_revision,
            plan_digest: r.plan_digest.clone(),
            attempt_id: intent.attempt_id.clone(),
            intent_id: intent.id.clone(),
            workspace_id: record.refs.workspace.id.clone(),
            root: record.refs.workspace.identity.clone().ok_or_else(changed)?,
            writer_epoch: lease.fencing_token,
            capability_digest: sha(&cap.snapshot)?,
            rule_id: rule_id.clone(),
            definition_digest: def.digest,
            definition_source: def.reference,
            candidate_refs: refs,
            input_bindings: consumed,
            reservation: node.node.reservation.clone(),
        };
        let context = GoalDispatchContext {
            run: r.clone(),
            node: node.node.clone(),
            ledger_revision: p.expected_ledger_revision,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        for key in [
            format!("project:{}", r.project_id),
            format!("root:{}", root_task(&record)),
        ] {
            if tx.execute("UPDATE budgets SET consumed=consumed+1,active=active+1 WHERE scope=?1 AND consumed<total_limit AND active<parallel_limit",[key]).map_err(db_error)?!=1{return Err(err("RESOURCE_LIMIT","Shared project/root attempt budget is exhausted"));}
        }
        tx.execute(
            "INSERT INTO attempts VALUES(?1,?2,?3,?4,?5)",
            params![
                intent.attempt_id,
                r.task_id,
                root_task(&record),
                encode(&binding)?,
                format!(
                    "goal-validator:{}:{}:{}",
                    r.id, r.plan_revision, node.node.id
                )
            ],
        )
        .map_err(db_error)?;
        tx.execute(
            "INSERT INTO dispatch_intents VALUES(?1,?2,'prepared',NULL,?3)",
            params![intent.id, intent.attempt_id, encode(&intent)?],
        )
        .map_err(db_error)?;
        reserve_attempt(
            &tx,
            &context,
            GoalAttemptTarget {
                attempt_id: &intent.attempt_id,
                task_id: &r.task_id,
                grant_id: &r.root_grant_id,
                session: None,
            },
            &binding,
            self.data_input.as_ref(),
            self.graph_input.as_ref(),
        )?;
        tx.execute(
            "INSERT INTO goal_graph_jobs VALUES(?1,?2,?3,?4,'prepared',?5,?6,?7)",
            params![
                origin.id,
                r.id,
                origin.node_id,
                sql_num(origin.plan_revision)?,
                origin.attempt_id,
                origin.workspace_id,
                encode(&origin)?
            ],
        )
        .map_err(db_error)?;
        let mut updated = task;
        updated.state = TaskState::Running;
        updated.revision = updated.revision.checked_add(1).ok_or_else(limited)?;
        updated.updated_at = now()?;
        tx.execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![encode(&updated)?, updated.id],
        )
        .map_err(db_error)?;
        crate::storage::append_event(
            &tx,
            &updated,
            "attempt.prepared",
            json!({"attempt_id":intent.attempt_id,"intent_id":intent.id,"binding":binding,"executor_kind":"validator"}),
        )?;
        event(
            &tx,
            &r.id,
            r.revision,
            "goal.validator_prepared",
            json!({"job_id":origin.id,"origin_digest":sha(&origin)?,"node_id":origin.node_id,"attempt_id":origin.attempt_id,"intent_id":origin.intent_id,"plan_revision":r.plan_revision,"plan_digest":r.plan_digest,"definition_digest":origin.definition_digest,"reservation":origin.reservation,"executor_started":false,"task_accepted":false}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(Some(PreparedGraphValidation {
            origin,
            rule,
            queued_at,
        }))
    }
}

impl ValidatorWorkers {
    pub(super) fn has_workers(&self) -> bool {
        !self.workers.is_empty() || !self.retained.is_empty()
    }
    pub(super) fn start(
        &mut self,
        store: &mut Store,
        prepared: PreparedGraphValidation,
    ) -> Result<()> {
        if self.has_workers() {
            return Err(err(
                "WORKSPACE_BUSY",
                "The owned validator slot remains occupied",
            ));
        }
        let PreparedGraphValidation {
            origin,
            rule,
            queued_at,
        } = prepared;
        let deadline = queued_at
            .checked_add(Duration::from_millis(origin.reservation.runtime_ms))
            .ok_or_else(limited)?;
        if Instant::now() >= deadline {
            return store.finish_validator_without_observation(
                &origin,
                false,
                true,
                "RESOURCE_LIMIT",
                queued_at.elapsed(),
            );
        }
        // The irreversible boundary is durable before any thread or child can
        // execute. A failed thread constructor is a distinct not-started proof.
        store.mark_dispatch_started(&origin.intent_id, &origin.id)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        match std::thread::Builder::new()
            .name("baton-goal-validator".into())
            .spawn(move || rule.execute(&flag, deadline))
        {
            Ok(thread) => {
                let id = origin.id.clone();
                self.workers.insert(
                    id.clone(),
                    Worker {
                        origin,
                        cancel,
                        thread,
                        queued_at,
                        deadline,
                        unknown_recorded: false,
                    },
                );
                store.mark_validator_worker_started(
                    &self.workers.get(&id).ok_or_else(changed)?.origin,
                )?;
                Ok(())
            }
            Err(_) => store.finish_validator_without_observation(
                &origin,
                false,
                true,
                "RESOURCE_LIMIT",
                queued_at.elapsed(),
            ),
        }
    }
    pub(super) fn tick(&mut self, store: &mut Store) -> Result<()> {
        for w in self.workers.values_mut() {
            let r = run(&store.conn, &w.origin.run_id)?;
            let record: ScopeRecord = raw(&store.conn, "goal_scopes", &r.scope_id)?;
            let cap = checked_capability(&store.conn, &r)?;
            let task = store.get(&r.task_id)?;
            let project = store.get_project(&r.project_id)?;
            if project.archived
                || project.revision != record.refs.project_revision
                || task.spec_revision != record.refs.spec_revision
                || task.goal != record.refs.task_goal
                || matches!(
                    task.state,
                    TaskState::CancelRequested
                        | TaskState::Cancelled
                        | TaskState::OutcomeUnknown
                        | TaskState::Accepted
                )
                || r.cancel_requested
                || record.revoked
                || crate::policy::checked_lineage(&store.conn, &r.root_grant_id).is_err()
                || live_definition(&store.conn, &record, &w.origin.rule_id, &cap).is_err()
            {
                w.cancel.store(true, Ordering::Relaxed);
            }
            if Instant::now() >= w.deadline && !w.thread.is_finished() && !w.unknown_recorded {
                w.cancel.store(true, Ordering::Relaxed);
                store.mark_validator_unknown(&w.origin, "DEADLINE_EXCEEDED")?;
                w.unknown_recorded = true;
            }
        }
        let done = self
            .workers
            .iter()
            .filter(|(_, w)| w.thread.is_finished())
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in done {
            let worker = self.workers.remove(&id).ok_or_else(changed)?;
            match worker.thread.join() {
                Ok(Ok(observation)) => {
                    // Retain an owned command before any Store/CAS/revocation
                    // validation can fail. Fatal Drop still owns its Child.
                    self.retained
                        .insert(id.clone(), (worker.origin, observation));
                    let (origin, observation) = self.retained.get(&id).ok_or_else(changed)?;
                    store.finish_graph_validation(
                        origin,
                        observation,
                        worker.unknown_recorded,
                        worker.cancel.load(Ordering::Relaxed),
                        worker.queued_at.elapsed(),
                    )?;
                }
                Ok(Err(error)) => store.finish_validator_without_observation(
                    &worker.origin,
                    true,
                    false,
                    &error.code,
                    worker.queued_at.elapsed(),
                )?,
                Err(_) => store.mark_validator_unknown(&worker.origin, "OUTCOME_UNKNOWN")?,
            }
        }
        let ids = self.retained.keys().cloned().collect::<Vec<_>>();
        for id in ids {
            let (origin, observation) = self.retained.get_mut(&id).ok_or_else(changed)?;
            let pending = observation
                .command
                .as_ref()
                .is_some_and(crate::command_validation::has_unreaped_child);
            if pending {
                if observation
                    .command
                    .as_mut()
                    .is_some_and(|run| run.try_reap_owned_leader().unwrap_or(false))
                {
                    // A late reap is reconciliation of the held original child,
                    // never validation success or an automatic graph restart.
                    store.finish_graph_validation(
                        origin,
                        observation,
                        true,
                        true,
                        observation.elapsed,
                    )?;
                    self.retained.remove(&id);
                }
            } else {
                self.retained.remove(&id);
            }
        }
        Ok(())
    }
    pub(super) fn request_all_stops(&self) {
        for w in self.workers.values() {
            w.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub(super) fn reap_owned_workers(&mut self) {
        self.request_all_stops();
        for (id, w) in std::mem::take(&mut self.workers) {
            if let Ok(Ok(obs)) = w.thread.join() {
                self.retained.insert(id, (w.origin, obs));
            }
        }
        while !self.retained.is_empty() {
            self.retained.retain(|_, (_, obs)| {
                obs.command.as_mut().is_some_and(|run| {
                    crate::command_validation::has_unreaped_child(run)
                        && !run.try_reap_owned_leader().unwrap_or(false)
                })
            });
            if !self.retained.is_empty() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
impl Drop for ValidatorWorkers {
    fn drop(&mut self) {
        self.reap_owned_workers();
    }
}

fn origin_record(db: &Connection, id: &str) -> Result<(ValidatorOrigin, String)> {
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(run_id)+octet_length(node_id)+octet_length(state)+octet_length(attempt_id)+octet_length(workspace_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_jobs WHERE id=?1",[id],1,MAX_STORED_BYTES)?;
    let row:Option<(String,String,String,i64,String,String,String)>=db.query_row("SELECT run_id,node_id,state,plan_revision,attempt_id,workspace_id,document FROM goal_graph_jobs WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional().map_err(db_error)?;
    let (run_id, node, state, rev, attempt, workspace, doc) = row.ok_or_else(changed)?;
    let o: ValidatorOrigin = decode(&doc)?;
    if o.id != id
        || o.job_kind != "validator"
        || o.run_id != run_id
        || o.node_id != node
        || o.plan_revision != u64::try_from(rev).map_err(|_| changed())?
        || o.attempt_id != attempt
        || o.workspace_id != workspace
    {
        return Err(changed());
    }
    let prepared:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='goal.validator_prepared' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.job_id')=?2 AND json_extract(document,'$.payload.origin_digest')=?3 AND json_extract(document,'$.payload.attempt_id')=?4 AND json_extract(document,'$.payload.intent_id')=?5 AND json_extract(document,'$.payload.plan_digest')=?6 ORDER BY sequence LIMIT 1",params![o.run_id,id,sha(&o)?,o.attempt_id,o.intent_id,o.plan_digest],|r|r.get(0)).optional().map_err(db_error)?;
    let reserved:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='goal.node_reserved' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.node_id')=?2 AND json_extract(document,'$.payload.attempt_id')=?3 AND json_extract(document,'$.payload.session_id') IS NULL AND json_extract(document,'$.payload.plan_revision')=?4 AND json_extract(document,'$.payload.plan_digest')=?5 ORDER BY sequence LIMIT 1",params![o.run_id,o.node_id,o.attempt_id,sql_num(o.plan_revision)?,o.plan_digest],|r|r.get(0)).optional().map_err(db_error)?;
    if !matches!((reserved,prepared),(Some(a),Some(b)) if a>0&&b>a) {
        return Err(denied());
    }
    Ok((o, state))
}
pub(super) fn verify_intrinsic(db: &Connection, r: &GoalRun, n: &RuntimeNode) -> Result<()> {
    if n.node.kind != NodeKind::Validate || n.session_id.is_some() {
        return Err(changed());
    }
    let attempt = n.attempt_id.as_deref().ok_or_else(changed)?;
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(run_id)+octet_length(node_id)+octet_length(state)+octet_length(workspace_id)+COALESCE(octet_length(attempt_id),0)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_jobs WHERE attempt_id=?1",[attempt],1,MAX_STORED_BYTES)?;
    let id: Option<String> = db
        .query_row(
            "SELECT id FROM goal_graph_jobs WHERE attempt_id=?1",
            [attempt],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let (o, state) = origin_record(db, &id.ok_or_else(changed)?)?;
    if o.run_id != r.id
        || o.node_id != n.node.id
        || o.plan_revision != n.plan_revision
        || o.reservation != n.node.reservation
        || o.workspace_id != n.node.workspace_id
    {
        return Err(changed());
    }
    let cap = checked_capability(db, r)?;
    if sha(&cap.snapshot)? != o.capability_digest {
        return Err(changed());
    }
    let reservation:Option<String>=db.query_row("SELECT document FROM goal_run_reservations WHERE attempt_id=?1 AND run_id=?2 AND node_id=?3 AND octet_length(document)<=262144",params![attempt,r.id,n.node.id],|row|row.get(0)).optional().map_err(db_error)?;
    let charged: ReservationRecord = decode(&reservation.ok_or_else(changed)?)?;
    if charged.run_id != r.id
        || charged.node_id != n.node.id
        || charged.plan_revision != n.plan_revision
        || charged.reservation != n.node.reservation
    {
        return Err(changed());
    }
    if !matches!(
        n.state.as_str(),
        "reserved"
            | "running"
            | "stopping"
            | "validated"
            | "failed"
            | "cancelled"
            | "outcome_unknown"
    ) || !matches!(
        state.as_str(),
        "prepared" | "running" | "completed" | "failed" | "cancelled" | "outcome_unknown"
    ) {
        return Err(changed());
    }
    if n.stop_confirmed {
        let receipt = validation_receipts(db, r)?
            .into_iter()
            .rev()
            .find(|v| {
                v.attempt_id == attempt
                    && v.node_id == n.node.id
                    && v.plan_revision == n.plan_revision
                    && v.stop_confirmed
            })
            .ok_or_else(changed)?;
        if receipt.actual_runtime_ms != n.actual_runtime_ms
            || receipt.state == "passed" && n.state != "validated"
        {
            return Err(changed());
        }
    } else if n.state == "validated" {
        return Err(changed());
    }
    Ok(())
}
fn validation_receipts(db: &Connection, r: &GoalRun) -> Result<Vec<GraphValidationReceipt>> {
    super::graph_records(
        db,
        r,
        "goal_graph_validations",
        "goal.validation_settled",
        "validation_id",
        "goal_core",
    )
}
impl Store {
    fn checked_validator_origin(&self, origin: &ValidatorOrigin) -> Result<(GoalRun, ScopeRecord)> {
        let (stored, _) = origin_record(&self.conn, &origin.id)?;
        if encode(&stored)? != encode(origin)? {
            return Err(changed());
        }
        let r = run(&self.conn, &origin.run_id)?;
        let cap = checked_capability(&self.conn, &r)?;
        if r.plan_revision != origin.plan_revision
            || r.plan_digest != origin.plan_digest
            || sha(&cap.snapshot)? != origin.capability_digest
        {
            return Err(changed());
        }
        let record: ScopeRecord = raw(&self.conn, "goal_scopes", &r.scope_id)?;
        Ok((r, record))
    }
    fn mark_validator_worker_started(&mut self, origin: &ValidatorOrigin) -> Result<()> {
        let (r, _) = self.checked_validator_origin(origin)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if tx
            .execute(
                "UPDATE goal_graph_jobs SET state='running' WHERE id=?1 AND state='prepared'",
                [&origin.id],
            )
            .map_err(db_error)?
            != 1
        {
            return Err(changed());
        }
        let doc:String=tx.query_row("SELECT document FROM goal_run_nodes WHERE run_id=?1 AND node_id=?2 AND plan_revision=?3 AND octet_length(document)<=262144",params![r.id,origin.node_id,sql_num(origin.plan_revision)?],|row|row.get(0)).map_err(db_error)?;
        let mut n: RuntimeNode = decode(&doc)?;
        n.state = "running".into();
        save_node(&tx, &r.id, &n)?;
        event(
            &tx,
            &r.id,
            r.revision,
            "goal.validator_worker_started",
            json!({"job_id":origin.id,"origin_digest":sha(origin)?,"attempt_id":origin.attempt_id,"intent_id":origin.intent_id,"owned_thread_started":true,"worker_joined":false,"task_accepted":false}),
        )?;
        tx.commit().map_err(db_error)
    }
    fn mark_validator_unknown(&mut self, origin: &ValidatorOrigin, code: &str) -> Result<()> {
        let (mut r, _) = self.checked_validator_origin(origin)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute(
            "UPDATE goal_graph_jobs SET state='outcome_unknown' WHERE id=?1",
            [&origin.id],
        )
        .map_err(db_error)?;
        let raw:String=tx.query_row("SELECT document FROM goal_run_nodes WHERE run_id=?1 AND node_id=?2 AND plan_revision=?3 AND octet_length(document)<=262144",params![r.id,origin.node_id,sql_num(origin.plan_revision)?],|row|row.get(0)).map_err(db_error)?;
        let mut n: RuntimeNode = decode(&raw)?;
        n.state = "outcome_unknown".into();
        n.actual_runtime_ms = None;
        n.actual_output_bytes = None;
        n.tokens = None;
        n.cost_microunits = None;
        save_node(&tx, &r.id, &n)?;
        for key in [
            format!("project:{}", r.project_id),
            format!("root-grant:{}", r.root_grant_id),
        ] {
            let raw:String=tx.query_row("SELECT document FROM goal_ledgers WHERE scope=?1 AND octet_length(document)<=1048576",[&key],|row|row.get(0)).map_err(db_error)?;
            let mut meter: MeterLedger = decode(&raw)?;
            meter.runtime_ms = None;
            meter.output_bytes = None;
            meter.tokens = None;
            meter.cost_microunits = None;
            tx.execute(
                "UPDATE goal_ledgers SET document=?1 WHERE scope=?2",
                params![encode(&meter)?, key],
            )
            .map_err(db_error)?;
        }
        r.state = "outcome_unknown".into();
        save_run(
            &tx,
            &mut r,
            "goal.validation_unknown",
            json!({"job_id":origin.id,"attempt_id":origin.attempt_id,"reason_code":code,"worker_joined":false,"reservation_refunded":false,"workspace_fence_retained":true,"task_accepted":false}),
        )?;
        tx.commit().map_err(db_error)
    }
    fn finish_graph_validation(
        &mut self,
        origin: &ValidatorOrigin,
        observed: &GoalValidatorObservation,
        was_unknown: bool,
        cancelled: bool,
        elapsed: Duration,
    ) -> Result<()> {
        let stopped = observed
            .command
            .as_ref()
            .is_none_or(|c| !crate::command_validation::has_unreaped_child(c));
        let mut view = observed.view.clone();
        let (stdout, stderr) = observed
            .command
            .as_ref()
            .map(|c| {
                (
                    c.captured_stdout().len() as u64,
                    c.captured_stderr().len() as u64,
                )
            })
            .unwrap_or((0, 0));
        let generated = stdout.checked_add(stderr).ok_or_else(limited)?;
        view["goal_io"]["ledger_output_domain"] =
            json!("new_declared_artifacts_plus_retained_stream_bytes");
        view["goal_io"]["new_declared_artifact_bytes"] = json!(0);
        view["goal_io"]["retained_stdout_bytes"] = json!(stdout);
        view["goal_io"]["retained_stderr_bytes"] = json!(stderr);
        view["goal_io"]["checked_candidate_bytes"] = json!(origin
            .candidate_refs
            .iter()
            .try_fold(0u64, |a, c| a.checked_add(c.bytes))
            .ok_or_else(limited)?);
        view["goal_io"]["total_filesystem_write_bytes"] = Value::Null;
        view["goal_io"]["total_filesystem_write_enforcement"] = json!("not_enforced");
        self.settle_graph_validator(
            origin,
            view,
            observed.passed,
            observed.scope,
            observed.correctness,
            stopped,
            true,
            was_unknown,
            cancelled,
            elapsed,
            generated,
        )
    }
    fn finish_validator_without_observation(
        &mut self,
        origin: &ValidatorOrigin,
        joined: bool,
        never_started: bool,
        code: &str,
        elapsed: Duration,
    ) -> Result<()> {
        let (scope, correctness) = match origin.definition_source.kind {
            GoalValidatorKind::Integrity => ("artifact_integrity_only", "not_established"),
            GoalValidatorKind::Data => {
                ("data_contract_only", "approved_schema_ranges_metadata_only")
            }
            GoalValidatorKind::Command => (
                "approved_deterministic_command_only",
                "approved_fixed_command_observations_only",
            ),
        };
        self.settle_graph_validator(origin,json!({"goal_io":{"state":"not_started_or_preflight_rejected","error_code":code},"task_acceptance":"not_established"}),false,scope,correctness,never_started||joined,joined,false,false,elapsed,0)
    }
}

impl Store {
    #[allow(clippy::too_many_arguments)]
    fn settle_graph_validator(
        &mut self,
        origin: &ValidatorOrigin,
        observation: Value,
        observed_passed: bool,
        scope: &str,
        correctness: &str,
        stopped: bool,
        joined: bool,
        was_unknown: bool,
        cancelled: bool,
        elapsed: Duration,
        generated_output_bytes: u64,
    ) -> Result<()> {
        let (mut r, record) = self.checked_validator_origin(origin)?;
        let cap = checked_capability(&self.conn, &r)?;
        let task: Task = raw(&self.conn, "tasks", &r.task_id)?;
        let project: Project = raw(&self.conn, "projects", &r.project_id)?;
        let task_cancelled = matches!(
            task.state,
            TaskState::CancelRequested | TaskState::Cancelled
        );
        if task_cancelled {
            r.cancel_requested = true;
        }
        let allowed = !project.archived
            && project.revision == record.refs.project_revision
            && task.spec_revision == record.refs.spec_revision
            && task.goal == record.refs.task_goal
            && !matches!(
                task.state,
                TaskState::Accepted
                    | TaskState::CancelRequested
                    | TaskState::Cancelled
                    | TaskState::OutcomeUnknown
            )
            && !record.revoked
            && !r.cancel_requested
            && crate::policy::checked_lineage(&self.conn, &r.root_grant_id).is_ok()
            && live_definition(&self.conn, &record, &origin.rule_id, &cap).is_ok();
        let elapsed_ms = u64::try_from(elapsed.as_millis()).map_err(|_| limited())?;
        let unknown = was_unknown || !stopped;
        let passed = observed_passed
            && joined
            && stopped
            && !unknown
            && !cancelled
            && allowed
            && elapsed_ms <= origin.reservation.runtime_ms;
        let state = if unknown {
            "outcome_unknown"
        } else if cancelled || r.cancel_requested {
            "cancelled"
        } else if passed {
            "passed"
        } else {
            "failed"
        };
        let io = if joined {
            "worker_joined"
        } else {
            "not_started"
        };
        let metric = if unknown { None } else { Some(elapsed_ms) };
        // Intrinsic validators declare no artifact writes; this is the same
        // selected-output domain as Goal Work, not an OS filesystem quota.
        let output = if unknown {
            None
        } else {
            Some(generated_output_bytes)
        };
        let receipt = GraphValidationReceipt {
            id: Uuid::new_v4().to_string(),
            graph_contract_version: 1,
            run_id: r.id.clone(),
            plan_revision: origin.plan_revision,
            plan_digest: origin.plan_digest.clone(),
            node_id: origin.node_id.clone(),
            attempt_id: origin.attempt_id.clone(),
            rule_id: origin.rule_id.clone(),
            definition_digest: origin.definition_digest.clone(),
            candidate_refs: origin.candidate_refs.clone(),
            input_bindings: origin.input_bindings.clone(),
            state: state.into(),
            passed,
            scope: scope.into(),
            correctness: correctness.into(),
            task_acceptance: "not_established".into(),
            result_digest: sha(&observation)?,
            observation,
            details_ref: format!("goal-graph-job:{}", origin.id),
            stop_confirmed: stopped,
            io_completion: io.into(),
            actual_runtime_ms: metric,
            created_at: now()?,
            source: "baton_goal_validator".into(),
        };
        let receipt_doc = encode(&receipt)?;
        if receipt_doc.len() > MAX_STORED_BYTES as usize {
            return Err(limited());
        }
        if stopped {
            let intent = self.intent(&origin.intent_id)?;
            let evidence = if intent.state == crate::execution::IntentState::Prepared {
                crate::execution::StopEvidence::NeverDispatched
            } else if joined {
                crate::execution::StopEvidence::ConfirmedStopped {
                    executor_instance: origin.id.clone(),
                    receipt_ref: format!("owned-goal-validator-stop:{}", origin.id),
                }
            } else {
                crate::execution::StopEvidence::ConfirmedNotStarted {
                    executor_instance: origin.id.clone(),
                    receipt_ref: format!("owned-goal-validator-no-start:{}", origin.id),
                }
            };
            self.confirm_stop(&origin.intent_id, evidence)?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if !stopped {
            let mut intent:crate::execution::DispatchIntent=decode(&tx.query_row("SELECT document FROM dispatch_intents WHERE id=?1 AND octet_length(document)<=1048576",[&origin.intent_id],|row|row.get::<_,String>(0)).map_err(db_error)?)?;
            intent.state = crate::execution::IntentState::OutcomeUnknown;
            tx.execute(
                "UPDATE dispatch_intents SET state='outcome_unknown',document=?1 WHERE id=?2",
                params![encode(&intent)?, origin.intent_id],
            )
            .map_err(db_error)?;
        }
        let doc:String=tx.query_row("SELECT document FROM goal_run_nodes WHERE run_id=?1 AND node_id=?2 AND plan_revision=?3 AND octet_length(document)<=262144",params![r.id,origin.node_id,sql_num(origin.plan_revision)?],|row|row.get(0)).map_err(db_error)?;
        let mut n: RuntimeNode = decode(&doc)?;
        let raw:String=tx.query_row("SELECT document FROM goal_run_reservations WHERE attempt_id=?1 AND octet_length(document)<=262144",[&origin.attempt_id],|row|row.get(0)).map_err(db_error)?;
        let mut reservation: ReservationRecord = decode(&raw)?;
        let first = !reservation.settled;
        reservation.settled = stopped;
        reservation.actual_runtime_ms = metric;
        reservation.actual_output_bytes = output;
        reservation.tokens = None;
        reservation.cost_microunits = None;
        tx.execute(
            "UPDATE goal_run_reservations SET document=?1 WHERE attempt_id=?2",
            params![encode(&reservation)?, origin.attempt_id],
        )
        .map_err(db_error)?;
        for (key, limits) in [
            (
                format!("project:{}", r.project_id),
                &record.selection.project_budget,
            ),
            (
                format!("root-grant:{}", r.root_grant_id),
                &record.selection.root_budget,
            ),
        ] {
            let raw:String=tx.query_row("SELECT document FROM goal_ledgers WHERE scope=?1 AND octet_length(document)<=1048576",[&key],|row|row.get(0)).map_err(db_error)?;
            let mut meter: MeterLedger = decode(&raw)?;
            if first {
                meter.runtime_ms = meter
                    .runtime_ms
                    .and_then(|a| metric.and_then(|b| a.checked_add(b)));
                meter.output_bytes = meter
                    .output_bytes
                    .and_then(|a| output.and_then(|b| a.checked_add(b)));
            }
            if unknown {
                meter.runtime_ms = None;
                meter.output_bytes = None;
            }
            meter.tokens = None;
            meter.cost_microunits = None;
            if meter.runtime_ms.is_none_or(|v| v > limits.runtime_ms)
                || meter.output_bytes.is_none_or(|v| v > limits.output_bytes)
            {
                r.state = "blocked".into();
            }
            tx.execute(
                "UPDATE goal_ledgers SET document=?1 WHERE scope=?2",
                params![encode(&meter)?, key],
            )
            .map_err(db_error)?;
        }
        n.state = if passed {
            "validated"
        } else if unknown {
            "outcome_unknown"
        } else if state == "cancelled" {
            "cancelled"
        } else {
            "failed"
        }
        .into();
        n.stop_confirmed = stopped;
        n.actual_runtime_ms = metric;
        n.actual_output_bytes = output;
        n.tokens = None;
        n.cost_microunits = None;
        let overrun = metric
            .is_some_and(|v| v > origin.reservation.runtime_ms || v > n.node.budget.runtime_ms);
        if unknown {
            r.state = "outcome_unknown".into();
        } else if r.cancel_requested {
            r.state = "cancelled".into();
        } else if !passed || overrun || r.state == "blocked" {
            r.state = "blocked".into();
        } else {
            r.state = "queued".into();
        }
        tx.execute(
            "INSERT INTO goal_graph_validations VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                receipt.id,
                r.id,
                n.node.id,
                sql_num(origin.plan_revision)?,
                origin.attempt_id,
                receipt_doc
            ],
        )
        .map_err(db_error)?;
        tx.execute(
            "UPDATE goal_graph_jobs SET state=?1 WHERE id=?2",
            params![
                if unknown {
                    "outcome_unknown"
                } else if state == "passed" {
                    "completed"
                } else {
                    state
                },
                origin.id
            ],
        )
        .map_err(db_error)?;
        save_node(&tx, &r.id, &n)?;
        save_run(
            &tx,
            &mut r,
            "goal.validation_settled",
            json!({"validation_id":receipt.id,"receipt_digest":sha(&receipt)?,"result_digest":receipt.result_digest,"job_id":origin.id,"attempt_id":origin.attempt_id,"intent_id":origin.intent_id,"scope":scope,"correctness":correctness,"passed":passed,"stop_confirmed":stopped,"io_completion":io,"actual_runtime_ms":metric,"actual_output_bytes":output,"observed_runtime_lower_bound_ms":elapsed_ms,"budget_overrun":overrun,"task_accepted":false}),
        )?;
        tx.commit().map_err(db_error)
    }
}

pub(super) fn verify_receipt(
    db: &Connection,
    r: &GoalRun,
    v: &GraphValidationReceipt,
) -> Result<()> {
    let job = v
        .details_ref
        .strip_prefix("goal-graph-job:")
        .ok_or_else(changed)?;
    let (o, _) = origin_record(db, job)?;
    let (scope, correctness) = match o.definition_source.kind {
        GoalValidatorKind::Integrity => ("artifact_integrity_only", "not_established"),
        GoalValidatorKind::Data => ("data_contract_only", "approved_schema_ranges_metadata_only"),
        GoalValidatorKind::Command => (
            "approved_deterministic_command_only",
            "approved_fixed_command_observations_only",
        ),
    };
    if v.source != "baton_goal_validator"
        || v.run_id != r.id
        || v.node_id != o.node_id
        || v.attempt_id != o.attempt_id
        || v.plan_revision != o.plan_revision
        || v.plan_digest != o.plan_digest
        || v.rule_id != o.rule_id
        || v.definition_digest != o.definition_digest
        || v.candidate_refs != o.candidate_refs
        || v.input_bindings != o.input_bindings
        || v.result_digest != sha(&v.observation)?
        || v.scope != scope
        || v.correctness != correctness
        || v.task_acceptance != "not_established"
        || v.passed
            && (!v.stop_confirmed || v.io_completion != "worker_joined" || v.state != "passed")
        || !matches!(
            v.state.as_str(),
            "passed" | "failed" | "cancelled" | "outcome_unknown"
        )
    {
        return Err(changed());
    }
    let candidates = graph_candidates(db, r)?;
    if v.candidate_refs
        .iter()
        .any(|a| !candidates.iter().any(|c| c.artifacts.contains(a)))
    {
        return Err(changed());
    }
    let validation_seq:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='goal.validation_settled' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.validation_id')=?2 AND json_extract(document,'$.payload.receipt_digest')=?3 AND json_extract(document,'$.payload.job_id')=?4 ORDER BY sequence LIMIT 1",params![r.id,v.id,sha(v)?,o.id],|row|row.get(0)).optional().map_err(db_error)?;
    let seq = validation_seq.ok_or_else(denied)?;
    let started:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='goal.validator_worker_started' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.job_id')=?2 AND json_extract(document,'$.payload.origin_digest')=?3 AND json_extract(document,'$.payload.owned_thread_started')=1 ORDER BY sequence LIMIT 1",params![r.id,o.id,sha(&o)?],|row|row.get(0)).optional().map_err(db_error)?;
    if v.io_completion == "worker_joined" && started.is_none_or(|s| s <= 0 || s >= seq) {
        return Err(denied());
    }
    if v.stop_confirmed {
        let intent:crate::execution::DispatchIntent=decode(&db.query_row("SELECT document FROM dispatch_intents WHERE id=?1 AND state='stopped' AND octet_length(document)<=1048576",[&o.intent_id],|row|row.get::<_,String>(0)).map_err(db_error)?)?;
        if intent.attempt_id != o.attempt_id
            || intent.task_id != r.task_id
            || intent.id != o.intent_id
            || intent.state != crate::execution::IntentState::Stopped
        {
            return Err(changed());
        }
        raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)),0),COALESCE(max(octet_length(document)),0) FROM events WHERE json_extract(document,'$.kind')='attempt.stopped' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.intent_id')=?2",params![r.task_id,o.intent_id],1,MAX_STORED_BYTES)?;
        let (stop_seq,raw):(i64,String)=db.query_row("SELECT sequence,document FROM events WHERE json_extract(document,'$.kind')='attempt.stopped' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.intent_id')=?2",params![r.task_id,o.intent_id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(db_error)?;
        let e: Event = decode(&raw)?;
        let proof = &e.payload["stop_evidence"];
        let valid = match proof["kind"].as_str() {
            Some("never_dispatched") => {
                intent.executor_instance.is_none() && v.io_completion == "not_started"
            }
            Some("executor_not_started") => {
                intent.executor_instance.as_deref() == Some(&o.id)
                    && proof["executor_instance"] == o.id
                    && proof["receipt_ref"] == format!("owned-goal-validator-no-start:{}", o.id)
                    && v.io_completion == "not_started"
            }
            Some("executor_stop") => {
                intent.executor_instance.as_deref() == Some(&o.id)
                    && proof["executor_instance"] == o.id
                    && proof["receipt_ref"] == format!("owned-goal-validator-stop:{}", o.id)
                    && v.io_completion == "worker_joined"
            }
            _ => false,
        };
        if !valid
            || e.source != "user_action"
            || stop_seq <= 0
            || stop_seq >= seq
            || v.io_completion == "worker_joined" && started.is_none_or(|s| s >= stop_seq)
        {
            return Err(denied());
        }
    }
    Ok(())
}
pub(super) fn node_job_id(db: &Connection, r: &GoalRun, n: &RuntimeNode) -> Result<Option<String>> {
    if n.node.kind != NodeKind::Validate {
        return Ok(None);
    }
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(id)+octet_length(document)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_jobs WHERE run_id=?1 AND node_id=?2 AND plan_revision=?3",params![r.id,n.node.id,sql_num(n.plan_revision)?],1,MAX_STORED_BYTES)?;
    db.query_row(
        "SELECT id FROM goal_graph_jobs WHERE run_id=?1 AND node_id=?2 AND plan_revision=?3",
        params![r.id, n.node.id, sql_num(n.plan_revision)?],
        |row| row.get(0),
    )
    .optional()
    .map_err(db_error)
}
