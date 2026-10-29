//! Append-only adoption of an exact new Factory saved plan. Initial issuer and
//! old Node/candidate/Owner histories are immutable; an adoption is not a Grant.
use super::*;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Adoption {
    run_id: String,
    scope_id: String,
    factory_scope_id: String,
    previous: FactoryChildPlanRef,
    plan: FactoryChildPlanRef,
    expected_run_revision: u64,
    expected_factory_run_revision: u64,
    scope_revision: u64,
    namespace: String,
    key: String,
    request_sha256: String,
    caller_task_id: String,
    caller_session_id: Option<String>,
    caller_attempt_id: Option<String>,
    caller_fencing_token: Option<u64>,
    rework_receipt: Option<Value>,
}

pub(super) fn adopted_plan(
    db: &Connection,
    r: &GoalRun,
    source: &DerivedRecord,
) -> Result<FactoryChildPlanRef> {
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)),0),COALESCE(max(octet_length(document)),0) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.factory_plan_adopted'",[&r.id],255,MAX_GOAL_PLAN_BYTES as i64)?;
    let mut q=db.prepare("SELECT sequence,document FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.factory_plan_adopted' ORDER BY sequence").map_err(db_error)?;
    let rows = q
        .query_map([&r.id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(db_error)?;
    let mut current = source.origin.initial_plan.clone();
    let mut prior_sequence = sql_num(source.receipt_sequence)?;
    let mut prior_revision = source.initial_run.revision;
    for row in rows {
        let (seq, raw) = row.map_err(db_error)?;
        let e: Event = decode(&raw)?;
        let a: Adoption =
            serde_json::from_value(e.payload["adoption"].clone()).map_err(|_| changed())?;
        let complete = crate::factory::checked_child_goal_plan(db, &a.plan)?;
        let previous = crate::factory::checked_child_goal_plan(db, &a.previous)?;
        if e.aggregate_id != r.id
            || e.kind != "goal.factory_plan_adopted"
            || e.source != "goal_core"
            || e.payload["adoption_digest"] != sha(&a)?
            || a.run_id != r.id
            || a.scope_id != r.scope_id
            || a.factory_scope_id != source.origin.factory_scope_id
            || a.previous != current
            || previous.reference != current
            || a.plan.scope_id != a.factory_scope_id
            || a.plan.run_id != source.origin.factory_run_id
            || a.plan.task_id != r.task_id
            || a.plan.revision <= current.revision
            || a.scope_revision != source.origin.approval_revision
            || a.expected_run_revision < prior_revision
            || e.revision != a.expected_run_revision.checked_add(1).ok_or_else(limited)?
            || seq <= prior_sequence
            || seq <= sql_num(a.plan.receipt_sequence)?
            || (a.caller_session_id.is_some() != a.caller_attempt_id.is_some())
            || (a.caller_session_id.is_some() != a.caller_fencing_token.is_some())
        {
            return Err(changed());
        }
        let namespace = if a.caller_session_id.is_some() {
            format!(
                "factory:{}:scoped:{}:{}:child-goal:advance",
                a.factory_scope_id, a.caller_task_id, r.task_id
            )
        } else {
            format!(
                "factory:{}:owner:{}:child-goal:advance",
                a.factory_scope_id, r.task_id
            )
        };
        if a.namespace != namespace {
            return Err(denied());
        }
        let (request, stored) = idempotency_entry(db, &a.namespace, &a.key)?.ok_or_else(denied)?;
        let receipt: FactoryChildGoalSummary = decode(&stored)?;
        if agentbaton_workspaces::validation::sha256(request.as_bytes()) != a.request_sha256
            || receipt.goal.run.id != r.id
            || receipt.goal.run.scope_id != r.scope_id
            || receipt.goal.run.revision != e.revision
            || receipt.plan != a.plan
            || receipt.goal.ledger_revision != u64::try_from(seq).map_err(|_| changed())?
            || receipt.factory_scope_id != a.factory_scope_id
            || receipt.task_id != r.task_id
        {
            return Err(changed());
        }
        let original = crate::factory::checked_child_goal_scope(db, &a.factory_scope_id)?;
        crate::factory::verify_child_goal_caller(
            db,
            &original,
            &source.origin.factory_run_id,
            &r.task_id,
            (
                &a.caller_task_id,
                a.caller_session_id.as_deref(),
                a.caller_attempt_id.as_deref(),
                a.caller_fencing_token,
            ),
            u64::try_from(seq).map_err(|_| changed())?,
        )?;
        verify_imported_plan(db, &r.scope_id, &complete)?;
        if let Some(rework) = &a.rework_receipt {
            verify_historical_rework(db, r, &current, rework, prior_sequence, seq)?;
        }
        current = a.plan;
        prior_sequence = seq;
        prior_revision = e.revision;
    }
    if current.revision != r.plan_revision || current.plan_digest != r.plan_digest {
        return Err(denied());
    }
    Ok(current)
}
fn verify_imported_plan(
    db: &Connection,
    scope_id: &str,
    p: &crate::factory::FactoryChildPlanView,
) -> Result<()> {
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)),0),COALESCE(max(octet_length(document)),0) FROM goal_plan_proposals WHERE scope_id=?1 AND revision=?2",params![scope_id,sql_num(p.reference.revision)?],1,MAX_STORED_BYTES)?;
    let raw: String = db
        .query_row(
            "SELECT document FROM goal_plan_proposals WHERE scope_id=?1 AND revision=?2",
            params![scope_id, sql_num(p.reference.revision)?],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    let imported: GoalPlanRawSubmission = decode(&raw)?;
    let stored:GoalPlanProposal=decode(&db.query_row("SELECT document FROM goal_plan_revisions WHERE scope_id=?1 AND revision=?2 AND digest=?3 AND octet_length(document)<=262144",params![scope_id,sql_num(p.reference.revision)?,p.reference.plan_digest],|row|row.get::<_,String>(0)).map_err(db_error)?)?;
    if imported.scope_id != scope_id || imported.plan_json != p.plan_json || stored != p.proposal {
        return Err(changed());
    }
    Ok(())
}
fn verify_historical_rework(
    db: &Connection,
    r: &GoalRun,
    previous: &FactoryChildPlanRef,
    context: &Value,
    prior_seq: i64,
    adopt_seq: i64,
) -> Result<()> {
    let seq = context["rework_receipt_sequence"]
        .as_i64()
        .ok_or_else(changed)?;
    let raw:Option<String>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE sequence=?1",[seq],|row|row.get(0)).map_err(db_error)?;
    let e: Event = decode(&raw.ok_or_else(limited)?)?;
    let failure = context["failure_receipt_sequence"]
        .as_i64()
        .ok_or_else(changed)?;
    let raw:Option<String>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE sequence=?1",[failure],|row|row.get(0)).map_err(db_error)?;
    let f: Event = decode(&raw.ok_or_else(limited)?)?;
    if seq <= failure
        || failure <= prior_seq
        || seq >= adopt_seq
        || e.aggregate_id != r.id
        || e.source != "goal_core"
        || e.kind != "goal.factory_rework_reserved"
        || sha(&e)? != context["rework_receipt_digest"]
        || e.payload["plan_revision"] != previous.revision
        || e.payload["plan_digest"] != previous.plan_digest
        || e.payload["failure_receipt_sequence"] != failure
        || e.payload["failure_proof_digest"] != context["failure_proof_digest"]
        || f.aggregate_id != r.id
        || f.source != "goal_core"
        || f.kind != "goal.factory_failure_confirmed"
        || f.payload["plan_revision"] != previous.revision
        || f.payload["plan_digest"] != previous.plan_digest
        || f.payload["proof_digest"] != context["failure_proof_digest"]
        || f.payload["proof_digest"] != sha(&f.payload["proof"])?
    {
        return Err(denied());
    }
    Ok(())
}

impl Store {
    pub fn advance_factory_child_goal(
        &mut self,
        p: &crate::factory::FactoryChildGoalAdvance,
        scope_revision: u64,
        key_id: &str,
    ) -> Result<FactoryChildGoalSummary> {
        self.advance_factory_child_goal_as(p, scope_revision, key_id, None)
    }
    pub(crate) fn advance_factory_child_goal_as(
        &mut self,
        p: &crate::factory::FactoryChildGoalAdvance,
        scope_revision: u64,
        key_id: &str,
        caller: Option<(String, String, String, u64)>,
    ) -> Result<FactoryChildGoalSummary> {
        key(key_id)?;
        let original = checked_record(&self.conn, &p.goal_run_id)?;
        if original.origin.factory_scope_id != p.scope_id
            || original.initial_run.task_id != p.request.task_id
        {
            return Err(denied());
        }
        let caller_task = caller
            .as_ref()
            .map_or(p.request.task_id.clone(), |c| c.0.clone());
        let namespace = if caller.is_some() {
            format!(
                "factory:{}:scoped:{}:{}:child-goal:advance",
                p.scope_id, caller_task, p.request.task_id
            )
        } else {
            format!(
                "factory:{}:owner:{}:child-goal:advance",
                p.scope_id, p.request.task_id
            )
        };
        let request = encode(&(scope_revision, p))?;
        if let Some(old) =
            replay::<FactoryChildGoalSummary>(&self.conn, &namespace, key_id, &request)?
        {
            capability(&self.conn, &run(&self.conn, &p.goal_run_id)?)?;
            return Ok(old);
        }
        let source = self.prepare_factory_goal_adoption_source(
            &p.scope_id,
            &p.request,
            scope_revision,
            &p.goal_run_id,
        )?;
        let mut r = run(&self.conn, &p.goal_run_id)?;
        capability(&self.conn, &r)?;
        if r.revision != p.expected_goal_run_revision {
            return Err(err(
                "STALE_REVISION",
                "The child Goal changed before explicit plan adoption",
            ));
        }
        if r.cancel_requested
            || r.task_accepted
            || matches!(
                r.state.as_str(),
                "accepted" | "cancelled" | "cancel_requested" | "outcome_unknown"
            )
        {
            return Err(denied());
        }
        let previous = adopted_plan(&self.conn, &r, &original)?;
        if p.request.plan.revision <= previous.revision {
            return Err(err(
                "STALE_REVISION",
                "Adopt only a newer exact Factory saved head",
            ));
        }
        let nodes = node_records(&self.conn, &r.id)?;
        let rework = if nodes.iter().any(|n| n.attempt_id.is_some()) {
            Some(verify_confirmed_failure(&self.conn, &r)?)
        } else {
            if nodes.iter().any(|n| n.state != "not_started") {
                return Err(denied());
            }
            None
        };
        let scope: ScopeRecord = super::super::raw(&self.conn, "goal_scopes", &r.scope_id)?;
        let approved = approved_scope(&self.conn, &source.task, &scope)?;
        let selected = source.plans.last().ok_or_else(changed)?;
        ValidatedPlan::from_verified_factory_saved(&approved, selected.proposal.clone())
            .map_err(plan_error)?;
        require_complete_plan(&scope, &selected.proposal)?;
        require_output_mappings(&scope.selection, &selected.proposal)?;
        require_metered_capabilities(&selected.proposal)?;
        let before = self.get_factory_child_goal(&crate::factory::FactoryChildGoalQuery {
            scope_id: p.scope_id.clone(),
            task_id: p.request.task_id.clone(),
            goal_run_id: Some(r.id.clone()),
        })?;
        let new_nodes = selected
            .proposal
            .nodes
            .iter()
            .map(|node| GraphNodeView {
                node: node.clone(),
                plan_revision: selected.reference.revision,
                state: "not_started".into(),
                session_id: None,
                attempt_id: None,
                job_id: None,
                execution_kind: match node.kind {
                    NodeKind::Work | NodeKind::Summary => "connection",
                    NodeKind::Validate => "validator",
                    NodeKind::OwnerApproval => "owner",
                }
                .into(),
                acceptance_state: "not_established".into(),
                stop_confirmed: false,
                actual_runtime_ms: None,
                actual_output_bytes: None,
                tokens: None,
                cost_microunits: None,
                candidate_ids: vec![],
                validation_ids: vec![],
                owner_decision_ids: vec![],
                blocked_reason: None,
            })
            .collect::<Vec<_>>();
        // Conservative complete future get-envelope bound before any write:
        // old lossless history + every new Node + explicit metadata headroom.
        if encode(&json!({"request_id":"\0".repeat(128),"result":{"history":before,"new_nodes":new_nodes},"revision":u64::MAX,"warnings":[]}))?.len().checked_add(65536).is_none_or(|n|n>MAX_STORED_BYTES as usize){return Err(limited());}
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        crate::factory::factory_goal_source_is_current(&tx, &source)?;
        let actual = run(&tx, &r.id)?;
        if actual.revision != r.revision || actual.plan_digest != r.plan_digest {
            return Err(changed());
        }
        refuse_retained_overrun(&tx, &r.root_grant_id, &r.project_id)?;
        let rework_receipt = if rework.is_some() {
            Some(reserve_rework(&tx, &r.id)?)
        } else {
            None
        };
        for saved in source
            .plans
            .iter()
            .filter(|s| s.reference.revision > previous.revision)
        {
            let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM goal_plan_revisions WHERE scope_id=?1 AND revision=?2)",params![r.scope_id,sql_num(saved.reference.revision)?],|row|row.get(0)).map_err(db_error)?;
            if exists {
                verify_imported_plan(&tx, &r.scope_id, saved)?;
                continue;
            }
            let previous_digest = source
                .plans
                .iter()
                .find(|s| s.reference.revision.checked_add(1) == Some(saved.reference.revision))
                .map(|s| s.reference.plan_digest.clone())
                .ok_or_else(changed)?;
            let submission = GoalPlanRawSubmission {
                scope_id: r.scope_id.clone(),
                plan_json: saved.plan_json.clone(),
            };
            tx.execute("INSERT INTO goal_plan_proposals(id,scope_id,plan_id,revision,document,digest) VALUES(?1,?2,?3,?4,?5,?6)",params![Uuid::new_v4().to_string(),r.scope_id,saved.proposal.plan_id,sql_num(saved.reference.revision)?,encode(&submission)?,saved.reference.plan_digest]).map_err(db_error)?;
            tx.execute("INSERT INTO goal_plan_revisions(scope_id,plan_id,revision,previous_digest,digest,document,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![r.scope_id,saved.proposal.plan_id,sql_num(saved.reference.revision)?,previous_digest,saved.reference.plan_digest,encode(&saved.proposal)?,saved.created_at]).map_err(db_error)?;
            event(
                &tx,
                &saved.proposal.plan_id,
                saved.reference.revision,
                "goal.plan_validated",
                json!({"scope_id":r.scope_id,"plan_id":saved.proposal.plan_id,"plan_revision":saved.reference.revision,"digest":saved.reference.plan_digest,"previous_digest":previous_digest,"dispatched":false,"reservation_made":false,"factory_saved_plan":saved.reference}),
            )?;
        }
        let a = Adoption {
            run_id: r.id.clone(),
            scope_id: r.scope_id.clone(),
            factory_scope_id: p.scope_id.clone(),
            previous,
            plan: selected.reference.clone(),
            expected_run_revision: r.revision,
            expected_factory_run_revision: source.factory_run.revision,
            scope_revision,
            namespace: namespace.clone(),
            key: key_id.into(),
            request_sha256: agentbaton_workspaces::validation::sha256(request.as_bytes()),
            caller_task_id: caller_task,
            caller_session_id: caller.as_ref().map(|c| c.1.clone()),
            caller_attempt_id: caller.as_ref().map(|c| c.2.clone()),
            caller_fencing_token: caller.as_ref().map(|c| c.3),
            rework_receipt,
        };
        for (ordinal, node) in selected.proposal.nodes.iter().enumerate() {
            let n = RuntimeNode {
                node: node.clone(),
                plan_revision: selected.reference.revision,
                state: "not_started".into(),
                session_id: None,
                attempt_id: None,
                started_at_ms: None,
                stop_confirmed: false,
                actual_runtime_ms: None,
                actual_output_bytes: None,
                tokens: None,
                cost_microunits: None,
            };
            tx.execute(
                "INSERT INTO goal_run_nodes VALUES(?1,?2,?3,?4,'not_started',NULL,NULL,?5)",
                params![
                    r.id,
                    sql_num(n.plan_revision)?,
                    node.id,
                    ordinal as i64,
                    encode(&n)?
                ],
            )
            .map_err(db_error)?;
        }
        tx.execute(
            "UPDATE goal_plan_heads SET plan_id=?1,revision=?2,digest=?3 WHERE scope_id=?4",
            params![
                selected.proposal.plan_id,
                sql_num(selected.reference.revision)?,
                selected.reference.plan_digest,
                r.scope_id
            ],
        )
        .map_err(db_error)?;
        r.plan_revision = selected.reference.revision;
        r.plan_digest = selected.reference.plan_digest.clone();
        r.state = "queued".into();
        save_run(
            &tx,
            &mut r,
            "goal.factory_plan_adopted",
            json!({"adoption":a,"adoption_digest":sha(&a)?,"new_permissions":false,"cumulative_consumption_reset":false,"previous_owner_acceptance_reused":false}),
        )?;
        let receipt = FactoryChildGoalSummary {
            authority_kind: "factory_child_goal_execution_v1".into(),
            factory_scope_id: p.scope_id.clone(),
            factory_run_id: original.origin.factory_run_id,
            factory_root_task_id: original.origin.root_task_id,
            parent_task_id: original.origin.parent_task_id,
            task_id: r.task_id.clone(),
            plan: selected.reference.clone(),
            execution_policy_digest: original.origin.execution_policy_digest,
            goal: GraphRunSummary {
                graph_contract_version: 1,
                run: r,
                ledger_revision: sequence(&tx)?,
                task_accepted: false,
            },
        };
        if encode(&json!({"request_id":"\0".repeat(128),"result":receipt,"revision":u64::MAX,"warnings":[]}))?.len()>MAX_STORED_BYTES as usize{return Err(limited());}
        remember(&tx, &namespace, key_id, &request, &receipt)?;
        tx.commit().map_err(db_error)?;
        Ok(receipt)
    }
}
