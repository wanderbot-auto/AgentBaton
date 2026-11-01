//! One original Goal Work turn may stop only its already owned process.
//! These private witnesses cannot be deserialized or issued by a provider.
use super::*;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct LaunchBinding {
    session: String,
    attempt: String,
    intent: String,
    run: String,
    plan: u64,
    digest: String,
    node: String,
}

pub(super) struct TurnTicket {
    origin: Arc<LaunchBinding>,
    native: String,
    turn: String,
    key: String,
    payload: String,
    document: String,
}

pub(super) struct CompletedTurnProof(TurnTicket);

fn changed() -> CoreError {
    CoreError::new("INPUT_CHANGED", "Original owned Goal turn binding changed")
}

// The original Goal launcher already checked the complete RuntimeNode. Read
// its exact bounded identity here; this is stop eligibility, not fresh authority.
pub(super) fn launch_binding(
    store: &Store,
    session: &ManagedSession,
) -> Result<Option<Arc<LaunchBinding>>> {
    if !crate::goal_store::owns_session(&store.conn, &session.id)?
        || crate::factory::owns_session(&store.conn, &session.id)?
    {
        return Ok(None);
    }
    let (run_id, node_raw, run_raw): (String, Option<String>, Option<String>) = store
        .conn
        .query_row(
            "SELECT n.run_id,CASE WHEN octet_length(n.document)<=262144 THEN n.document END,CASE WHEN octet_length(r.document)<=1048576 THEN r.document END FROM goal_run_nodes n JOIN goal_runs r ON r.id=n.run_id WHERE n.session_id=?1",
            [&session.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(db_error)?;
    let node: Value = decode(&node_raw.ok_or_else(changed)?)?;
    let plan_node: crate::goal_plan::PlanNode =
        serde_json::from_value(node["node"].clone()).map_err(|_| changed())?;
    if plan_node.kind != crate::goal_plan::NodeKind::Work {
        return Ok(None);
    }
    let run: crate::goal_store::GoalRun = decode(&run_raw.ok_or_else(changed)?)?;
    if run.id != run_id
        || node["session_id"].as_str() != Some(session.id.as_str())
        || node["attempt_id"].as_str() != Some(session.attempt_id.as_str())
        || node["plan_revision"].as_u64() != Some(run.plan_revision)
        || !matches!(
            node["state"].as_str(),
            Some("reserved" | "running" | "stopping")
        )
    {
        return Err(changed());
    }
    Ok(Some(Arc::new(LaunchBinding {
        session: session.id.clone(),
        attempt: session.attempt_id.clone(),
        intent: session.intent_id.clone(),
        run: run_id,
        plan: run.plan_revision,
        digest: run.plan_digest,
        node: plan_node.id,
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TurnDocument {
    turn_id: String,
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TurnPayload {
    text: String,
    expected_revision: u64,
}

pub(super) fn committed_ticket(
    store: &Store,
    origin: &Arc<LaunchBinding>,
    text: &str,
) -> Result<TurnTicket> {
    let session = store.get_session(&origin.session)?;
    let current = launch_binding(store, &session)?.ok_or_else(changed)?;
    if current.as_ref() != origin.as_ref()
        || session.state != "running"
        || session.turn_state.as_deref() != Some("running")
    {
        return Err(changed());
    }
    let mut q = store.conn.prepare("SELECT id,idempotency_key,CASE WHEN octet_length(payload)<=1048576 THEN payload END,CASE WHEN octet_length(document)<=1048576 THEN document END FROM session_turns WHERE session_id=?1 AND state='pending' LIMIT 2").map_err(db_error)?;
    let rows = q
        .query_map([&origin.session], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    let [(turn, key, payload, document)] = rows.as_slice() else {
        return Err(changed());
    };
    let payload = payload.as_ref().ok_or_else(changed)?;
    let document = document.as_ref().ok_or_else(changed)?;
    let body: TurnDocument = decode(document)?;
    let request: TurnPayload = decode(payload)?;
    if body.turn_id != *turn
        || body.text != text
        || request.text != text
        || request.expected_revision.checked_add(1) != Some(session.revision)
    {
        return Err(changed());
    }
    Ok(TurnTicket {
        origin: origin.clone(),
        native: session.native_session_id.ok_or_else(changed)?,
        turn: turn.clone(),
        key: key.clone(),
        payload: payload.clone(),
        document: document.clone(),
    })
}

impl TurnTicket {
    pub(super) fn matches(
        &self,
        original: Option<&Arc<LaunchBinding>>,
        session: &str,
        native: &str,
    ) -> bool {
        original.is_some_and(|o| Arc::ptr_eq(o, &self.origin))
            && self.origin.session == session
            && self.native == native
    }
    pub(super) fn completed(self) -> CompletedTurnProof {
        CompletedTurnProof(self)
    }
}

impl CompletedTurnProof {
    // The actual owned-thread result, not the log channel, carries this proof.
    pub(super) fn persist(&self, store: &mut Store, id: &str) -> Result<()> {
        self.0.persist_terminal(store, id, "completed")
    }
}
impl TurnTicket {
    pub(super) fn persist_terminal(&self, store: &mut Store, id: &str, state: &str) -> Result<()> {
        if !matches!(
            state,
            "completed" | "cancelled" | "interrupted" | "outcome_unknown"
        ) {
            return Err(changed());
        }
        let t = self;
        let mut session = store.get_session(id)?;
        if id != t.origin.session
            || session.attempt_id != t.origin.attempt
            || session.intent_id != t.origin.intent
            || session.native_session_id.as_deref() != Some(t.native.as_str())
        {
            return Err(changed());
        }
        let current = launch_binding(store, &session)?.ok_or_else(changed)?;
        if current.as_ref() != t.origin.as_ref() {
            return Err(changed());
        }
        let tx = store.conn.transaction().map_err(db_error)?;
        let row: (String, String, String, String) = tx.query_row(
            "SELECT state,idempotency_key,payload,document FROM session_turns WHERE id=?1 AND session_id=?2 AND octet_length(payload)<=1048576 AND octet_length(document)<=1048576",
            params![t.turn,id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
        ).map_err(db_error)?;
        if row.1 != t.key || row.2 != t.payload || row.3 != t.document {
            return Err(changed());
        }
        let pending: u32 = tx
            .query_row(
                "SELECT count(*) FROM session_turns WHERE session_id=?1 AND state='pending'",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if row.0 == "ended" {
            // Exact already-committed proof only. Never overwrite a later turn.
            if pending == 0 && session.turn_state.as_deref() == Some(state) {
                tx.commit().map_err(db_error)?;
                return Ok(());
            }
            return Err(changed());
        }
        if row.0 != "pending"
            || pending != 1
            || session.turn_state.as_deref() != Some("running")
            || !matches!(session.state.as_str(), "running" | "cancel_requested")
        {
            return Err(changed());
        }
        if session.state == "running" {
            session.state = "ready".into();
        }
        session.turn_state = Some(state.into());
        if tx.execute("UPDATE session_turns SET state='ended' WHERE id=?1 AND session_id=?2 AND state='pending'",params![t.turn,id]).map_err(db_error)? != 1 {
            return Err(changed());
        }
        save_session(
            &tx,
            &mut session,
            "session.turn_ended",
            json!({"turn_state":state,"task_accepted":false}),
            "provider_runtime",
        )?;
        tx.commit().map_err(db_error)
    }
}

/// Fixed progress through the existing receipt pipeline, not durable stop truth.
/// In particular a successfully released lease must never be released twice.
#[derive(Default)]
pub(super) struct SettlementProgress {
    pub(super) terminal: bool,
    pub(super) task: bool,
    pub(super) stop: bool,
    pub(super) lease: bool,
    pub(super) session: bool,
    pub(super) goal: bool,
    pub(super) shared: bool,
}
pub(super) struct RetainedReceipt {
    pub(super) receipt: WorkerReceipt,
    pub(super) progress: SettlementProgress,
}
impl RetainedReceipt {
    pub(super) fn new(receipt: WorkerReceipt) -> Self {
        Self {
            receipt,
            progress: SettlementProgress::default(),
        }
    }
}

#[cfg(test)]
#[path = "sessions_owned_stop_tests.rs"]
mod tests;
