//! Private, historical all-stop proof for Factory rework. No filesystem read,
//! session state label, producer join or public JSON can manufacture this proof.
use super::*;

#[derive(Serialize)]
pub(in crate::goal_store::runtime) struct StoppedGraphFailure {
    pub(in crate::goal_store::runtime) run_id: String,
    pub(in crate::goal_store::runtime) plan_revision: u64,
    pub(in crate::goal_store::runtime) plan_digest: String,
    pub(in crate::goal_store::runtime) failure_refs: Vec<String>,
    pub(in crate::goal_store::runtime) nodes_digest: String,
    pub(in crate::goal_store::runtime) jobs_digest: String,
    pub(in crate::goal_store::runtime) receipts_digest: String,
    pub(in crate::goal_store::runtime) receipt_cursor: u64,
}

/// `None` means still running, unknown, cancelled, unmeasured or no actual
/// failure. Corrupt source/columns/order is an error, never a retry permission.
pub(in crate::goal_store::runtime) fn stopped_failure(
    db: &Connection,
    r: &GoalRun,
) -> Result<Option<StoppedGraphFailure>> {
    if r.cancel_requested || r.task_accepted || !matches!(r.state.as_str(), "blocked" | "failed") {
        return Ok(None);
    }
    Ok(stopped_graph(db, r)?.filter(|p| !p.failure_refs.is_empty()))
}
pub(in crate::goal_store::runtime) fn stopped_graph(
    db: &Connection,
    r: &GoalRun,
) -> Result<Option<StoppedGraphFailure>> {
    checked_capability(db, r)?;
    // Check the aggregate in SQL before any revision's Node is materialized.
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(run_id)+octet_length(node_id)+octet_length(state)+COALESCE(octet_length(session_id),0)+COALESCE(octet_length(attempt_id),0)),0),COALESCE(max(octet_length(document)),0) FROM goal_run_nodes WHERE run_id=?1",[&r.id],65536,MAX_GOAL_PLAN_BYTES as i64)?;
    let plans = bounded_plan_history(db, &r.scope_id)?;
    let mut nodes = Vec::new();
    for plan in &plans {
        let exists: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM goal_run_nodes WHERE run_id=?1 AND plan_revision=?2)",
                params![r.id, sql_num(plan.proposal.revision)?],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if exists {
            nodes.extend(node_records_at(
                db,
                r,
                plan.proposal.revision,
                &plan.digest,
                plan,
            )?);
        }
    }
    if nodes.is_empty() {
        return Err(changed());
    }
    for n in &nodes {
        if matches!(
            n.state.as_str(),
            "reserved" | "running" | "stopping" | "outcome_unknown"
        ) || n.attempt_id.is_some()
            && (!n.stop_confirmed
                || n.actual_runtime_ms.is_none()
                || n.actual_output_bytes.is_none())
        {
            return Ok(None);
        }
    }
    // Every attempted node must have exactly its original settled reservation;
    // never turn an unknown measurement into zero or release a held reservation.
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(attempt_id)+octet_length(run_id)+octet_length(node_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_run_reservations WHERE run_id=?1",[&r.id],65536,MAX_GOAL_PLAN_BYTES as i64)?;
    let mut q=db.prepare("SELECT attempt_id,node_id,document FROM goal_run_reservations WHERE run_id=?1 ORDER BY attempt_id").map_err(db_error)?;
    let rows = q
        .query_map([&r.id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(db_error)?;
    let mut count = 0usize;
    for row in rows {
        let (attempt, node, raw) = row.map_err(db_error)?;
        let receipt: ReservationRecord = decode(&raw)?;
        let n = nodes
            .iter()
            .find(|n| {
                n.node.id == node
                    && n.plan_revision == receipt.plan_revision
                    && n.attempt_id.as_deref() == Some(attempt.as_str())
            })
            .ok_or_else(changed)?;
        if receipt.run_id != r.id
            || receipt.node_id != node
            || receipt.reservation != n.node.reservation
            || receipt.shared_root_grant_id.is_some()
            || receipt.settled != n.stop_confirmed
            || receipt.actual_runtime_ms != n.actual_runtime_ms
            || receipt.actual_output_bytes != n.actual_output_bytes
            || receipt.tokens != n.tokens
            || receipt.cost_microunits != n.cost_microunits
        {
            return Err(changed());
        }
        if !receipt.settled {
            return Ok(None);
        }
        count = count.checked_add(1).ok_or_else(limited)?;
    }
    drop(q);
    if count != nodes.iter().filter(|n| n.attempt_id.is_some()).count() {
        return Err(changed());
    }
    let candidates = graph_candidates(db, r)?;
    let validations = graph_validations(db, r)?;
    let owners = graph_owner_decisions(db, r)?;
    if validations.iter().any(|v| {
        !v.stop_confirmed
            || !matches!(v.io_completion.as_str(), "worker_joined" | "not_started")
            || v.state == "outcome_unknown"
    }) {
        return Ok(None);
    }
    let Some(jobs) = terminal_jobs(db, r, &nodes, &candidates, &validations)? else {
        return Ok(None);
    };
    let mut failures = nodes
        .iter()
        .filter(|n| n.plan_revision == r.plan_revision && n.state == "failed")
        .map(|n| format!("node:{}:{}", n.plan_revision, n.node.id))
        .collect::<Vec<_>>();
    failures.extend(
        validations
            .iter()
            .filter(|v| v.plan_revision == r.plan_revision && v.state == "failed" && !v.passed)
            .map(|v| format!("validation:{}", v.id)),
    );
    failures.extend(
        owners
            .iter()
            .filter(|o| o.plan_revision == r.plan_revision && o.decision == "rejected")
            .map(|o| format!("owner:{}", o.id)),
    );
    failures.extend(
        jobs.iter()
            .filter(|j| j.1 == r.plan_revision && j.2 == "failed")
            .map(|j| format!("capture-or-validator:{}", j.0)),
    );
    failures.sort();
    failures.dedup();
    Ok(Some(StoppedGraphFailure {
        run_id: r.id.clone(),
        plan_revision: r.plan_revision,
        plan_digest: r.plan_digest.clone(),
        failure_refs: failures,
        nodes_digest: sha(&nodes)?,
        jobs_digest: sha(&jobs)?,
        receipts_digest: sha(&(candidates, validations, owners))?,
        receipt_cursor: failure_receipt_cursor(db, r)?,
    }))
}

type TerminalJob = (String, u64, String, String, i64);
fn terminal_jobs(
    db: &Connection,
    r: &GoalRun,
    nodes: &[RuntimeNode],
    candidates: &[GraphCandidateReceipt],
    validations: &[GraphValidationReceipt],
) -> Result<Option<Vec<TerminalJob>>> {
    raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)+octet_length(run_id)+octet_length(node_id)+octet_length(state)+COALESCE(octet_length(attempt_id),0)+octet_length(workspace_id)),0),COALESCE(max(octet_length(document)),0) FROM goal_graph_jobs WHERE run_id=?1",[&r.id],65536,MAX_GOAL_PLAN_BYTES as i64)?;
    let mut q=db.prepare("SELECT id,node_id,plan_revision,state,attempt_id,workspace_id,document FROM goal_graph_jobs WHERE run_id=?1 ORDER BY id").map_err(db_error)?;
    let rows = q
        .query_map([&r.id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(db_error)?;
    let cap_digest = sha(&checked_capability(db, r)?.snapshot)?;
    let mut result = Vec::new();
    for row in rows {
        let (id, node, rev, state, attempt, workspace, raw) = row.map_err(db_error)?;
        if matches!(
            state.as_str(),
            "prepared" | "running" | "cancel_requested" | "outcome_unknown"
        ) {
            return Ok(None);
        }
        if !matches!(state.as_str(), "completed" | "failed" | "cancelled") {
            return Err(changed());
        }
        let revision = u64::try_from(rev).map_err(|_| changed())?;
        let n = nodes
            .iter()
            .find(|n| n.node.id == node && n.plan_revision == revision)
            .ok_or_else(changed)?;
        if n.attempt_id != attempt || n.node.workspace_id != workspace || !n.stop_confirmed {
            return Err(changed());
        }
        let value: Value = decode(&raw)?;
        if value.get("job_kind").is_some() {
            if value["job_kind"] != "validator" {
                return Err(changed());
            }
            // graph_validations above already verifies actual origin, durable
            // intent stop and ordered worker-start/stop/settled receipt chains.
            let matching = validations
                .iter()
                .filter(|v| v.details_ref == format!("goal-graph-job:{id}"))
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err(changed());
            }
            let v = matching[0];
            if v.node_id != node
                || v.plan_revision != revision
                || v.attempt_id != attempt.as_deref().ok_or_else(changed)?
                || (state == "completed") != v.passed
                || (state != "completed" && state != v.state)
            {
                return Err(changed());
            }
            let seq:i64=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.validation_settled' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.payload.validation_id')=?2 AND json_extract(document,'$.payload.receipt_digest')=?3 ORDER BY sequence LIMIT 1",params![r.id,v.id,sha(v)?],|row|row.get(0)).map_err(db_error)?;
            result.push((id, revision, state, v.io_completion.clone(), seq));
            continue;
        }
        let origin: CaptureOrigin = serde_json::from_value(value).map_err(|_| changed())?;
        if origin.id != id
            || origin.run_id != r.id
            || origin.node_id != node
            || origin.plan_revision != revision
            || origin.attempt_id != attempt.as_deref().ok_or_else(changed)?
            || origin.workspace_id != workspace
            || origin.capability_digest != cap_digest
            || origin.session_id != n.session_id.as_deref().ok_or_else(changed)?
        {
            return Err(changed());
        }
        let session: crate::sessions::ManagedSession =
            super::super::raw(db, "sessions", &origin.session_id)?;
        if session.task_id != r.task_id
            || session.attempt_id != origin.attempt_id
            || trusted_session_stop(db, &r.id, &session)? != sql_num(origin.stop_receipt_sequence)?
        {
            return Err(changed());
        }
        let prepared:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.capture_prepared' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.payload.job_id')=?2 AND json_extract(document,'$.payload.origin_digest')=?3 AND json_extract(document,'$.payload.stop_receipt_sequence')=?4 ORDER BY sequence LIMIT 1",params![r.id,id,sha(&origin)?,sql_num(origin.stop_receipt_sequence)?],|row|row.get(0)).optional().map_err(db_error)?;
        raw_collection_budget(db,"SELECT count(*),COALESCE(sum(octet_length(document)),0),COALESCE(max(octet_length(document)),0) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.capture_settled' AND json_extract(document,'$.payload.job_id')=?2",params![r.id,id],1,MAX_STORED_BYTES)?;
        let (seq,raw):(i64,String)=db.query_row("SELECT sequence,document FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.capture_settled' AND json_extract(document,'$.payload.job_id')=?2",params![r.id,id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(db_error)?;
        let e: Event = decode(&raw)?;
        if e.source != "goal_core"
            || e.aggregate_id != r.id
            || e.kind != "goal.capture_settled"
            || e.payload["job_id"] != id
            || e.payload["state"] != state
            || e.payload["producer_stop_confirmed"] != true
            || prepared.is_none_or(|p| {
                p <= sql_num(origin.stop_receipt_sequence).unwrap_or(i64::MAX) || p >= seq
            })
        {
            return Err(denied());
        }
        let started:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.capture_worker_started' AND json_extract(document,'$.source')='goal_core' AND json_extract(document,'$.payload.job_id')=?2 AND json_extract(document,'$.payload.origin_digest')=?3 AND json_extract(document,'$.payload.owned_thread_started')=1 ORDER BY sequence LIMIT 1",params![r.id,id,sha(&origin)?],|row|row.get(0)).optional().map_err(db_error)?;
        let completion = e.payload["io_completion"].as_str().ok_or_else(changed)?;
        match completion {
            "worker_joined"
                if e.payload["capture_worker_joined"] == true
                    && matches!((prepared,started),(Some(p),Some(s)) if p<s&&s<seq) => {}
            "not_started"
                if e.payload["capture_worker_joined"] == false
                    && started.is_none()
                    && state == "failed" => {}
            _ => return Err(denied()),
        }
        if state == "completed" && candidates.iter().filter(|c| c.capture_job_id == id).count() != 1
        {
            return Err(changed());
        }
        result.push((id, revision, state, completion.into(), seq));
    }
    Ok(Some(result))
}

fn failure_receipt_cursor(db: &Connection, r: &GoalRun) -> Result<u64> {
    let seq:i64=db.query_row("SELECT COALESCE(max(sequence),0) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source') IN('goal_core','local_owner_confirmation','local_owner_rejection') AND json_extract(document,'$.kind') IN('goal.node_settled','goal.capture_settled','goal.validation_settled','goal.owner_decided','goal.owner_node_settled')",[&r.id],|row|row.get(0)).map_err(db_error)?;
    u64::try_from(seq).map_err(|_| changed())
}
