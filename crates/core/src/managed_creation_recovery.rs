//! Owner recovery is historical reconciliation, never a retry or execution grant.
use super::*;
use crate::client_recovery_journal::{
    validate_binding, validate_marker, ClientRecoveryMarker, ManagedCreationBinding,
    ManagedCreationKind,
};
const TOMBSTONE: &str = "managed_creation:abandoned:v1";
#[derive(Clone, Debug)]
pub struct NoNewExecution {
    _private: (),
}
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryConnection {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub protocol_route: ProtocolRoute,
    pub config_revision: String,
    pub revision: u64,
    pub probe_state: String,
    pub runtime_plan_digest: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryOutcome {
    pub marker: ClientRecoveryMarker,
    pub state: String,
    pub connection: Option<RecoveryConnection>,
    #[serde(skip)]
    proof: NoNewExecution,
}
impl RecoveryOutcome {
    pub fn no_new_execution(&self) -> &NoNewExecution {
        &self.proof
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BindingRecoveryOutcome {
    pub binding: ManagedCreationBinding,
    pub state: String,
    pub connection: Option<RecoveryConnection>,
    #[serde(skip)]
    proof: NoNewExecution,
}
impl BindingRecoveryOutcome {
    pub fn no_new_execution(&self) -> &NoNewExecution {
        &self.proof
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tombstone {
    binding: ManagedCreationBinding,
    event_id: String,
}
fn conflict() -> CoreError {
    CoreError::new(
        "CONFIG_CONFLICT",
        "Recovery marker does not match the original managed creation",
    )
}
fn kind_name(kind: ManagedCreationKind) -> &'static str {
    match kind {
        ManagedCreationKind::ManagedMcp => "managed_mcp",
        ManagedCreationKind::ManagedSkill => "managed_skill",
    }
}
fn binding(m: &ClientRecoveryMarker) -> ManagedCreationBinding {
    ManagedCreationBinding {
        kind: m.kind,
        request_key: m.request_key.clone(),
        preview_digest: m.preview_digest.clone(),
        base_connection_id: m.base_connection_id.clone(),
        base_connection_revision: m.base_connection_revision,
        operation: m.operation,
    }
}
fn tombstone_scope(kind: &str) -> String {
    format!("{TOMBSTONE}:{kind}")
}
fn tombstone(db: &Connection, kind: &str, key: &str) -> Result<Option<ManagedCreationBinding>> {
    let row:Option<(Option<String>,Option<String>)>=db.query_row("SELECT CASE WHEN octet_length(payload)<=4096 THEN payload END,CASE WHEN octet_length(result)<=8192 THEN result END FROM idempotency WHERE scope=?1 AND key=?2",params![tombstone_scope(kind),key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
    let Some((payload, result)) = row else {
        return Ok(None);
    };
    let payload = payload.ok_or_else(conflict)?;
    let t: Tombstone = decode(&result.ok_or_else(conflict)?)?;
    if kind_name(t.binding.kind) != kind
        || t.binding.request_key != key
        || encode(&t.binding)? != payload
    {
        return Err(conflict());
    }
    let event:Option<String>=db.query_row("SELECT CASE WHEN octet_length(document)<=16384 THEN document END FROM events WHERE json_extract(document,'$.event_id')=?1",[&t.event_id],|r|r.get(0)).optional().map_err(db_error)?;
    let e: Event = decode(&event.ok_or_else(conflict)?)?;
    if e.kind != "managed_creation.abandoned"
        || e.source != "local_owner_recovery"
        || e.revision != 1
        || e.aggregate_id != format!("managed_creation:{kind}:{key}")
        || e.payload != json!(t.binding)
    {
        return Err(conflict());
    }
    Ok(Some(t.binding))
}
pub(super) fn ensure_open(db: &Connection, kind: &str, key: &str) -> Result<()> {
    if matches!(kind, "managed_mcp" | "managed_skill") && tombstone(db, kind, key)?.is_some() {
        return Err(CoreError::new("PERMISSION_DENIED","This managed creation key was permanently abandoned; it cannot be proposed or finalized"));
    }
    Ok(())
}
/// Linearize historical proposal replays against abandonment, before any source read.
pub(super) fn proposal_replay(
    db: &mut Connection,
    kind: &str,
    key: &str,
    intent: &Value,
) -> Result<Option<ApprovalProposal>> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    ensure_open(&tx, kind, key)?;
    let raw:Option<String>=tx.query_row("SELECT CASE WHEN octet_length(result)<=1048576 THEN result END FROM idempotency WHERE scope=?1 AND key=?2",params![format!("local:approvals.propose:{kind}"),key],|r|r.get(0)).optional().map_err(db_error)?;
    let result = raw
        .map(|raw| {
            let p: ApprovalProposal = decode(&raw)?;
            if p.kind != kind || p.snapshot["intent"] != *intent || p.snapshot["request_key"] != key
            {
                return Err(conflict());
            }
            Ok(p)
        })
        .transpose()?;
    tx.commit().map_err(db_error)?;
    Ok(result)
}
fn verify_binding(p: &ApprovalProposal, m: &ManagedCreationBinding) -> Result<()> {
    let i = &p.snapshot["intent"];
    let r = &i["request"];
    if p.kind != kind_name(m.kind)
        || p.snapshot["request_key"] != m.request_key
        || i["expected_preview_digest"] != m.preview_digest
        || r["base_connection_id"] != m.base_connection_id
        || r["expected_base_revision"] != m.base_connection_revision
        || r["operation"] != json!(m.operation)
    {
        return Err(conflict());
    }
    Ok(())
}
fn original(
    db: &Connection,
    m: &ManagedCreationBinding,
) -> Result<Option<(ApprovalProposal, Option<String>)>> {
    let kind = kind_name(m.kind);
    let raw:Option<String>=db.query_row("SELECT CASE WHEN octet_length(result)<=1048576 THEN result END FROM idempotency WHERE scope=?1 AND key=?2",params![format!("local:approvals.propose:{kind}"),m.request_key],|r|r.get(0)).optional().map_err(db_error)?;
    let Some(raw) = raw else {
        let orphan:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM approval_proposals WHERE json_extract(document,'$.kind')=?1 AND json_extract(document,'$.snapshot.request_key')=?2)",params![kind,m.request_key],|r|r.get(0)).map_err(db_error)?;
        if orphan {
            return Err(conflict());
        }
        return Ok(None);
    };
    let cached: ApprovalProposal = decode(&raw)?;
    verify_binding(&cached, m)?;
    let (p, result) = match m.kind {
        ManagedCreationKind::ManagedMcp => super::managed_mcps::proposal_row(db, &cached.id)?,
        ManagedCreationKind::ManagedSkill => super::managed_skills::proposal_row(db, &cached.id)?,
    };
    verify_binding(&p, m)?;
    if p.snapshot != cached.snapshot || p.id != cached.id || p.consumed != result.is_some() {
        return Err(conflict());
    }
    Ok(Some((p, result)))
}
fn binding_outcome(db: &Connection, m: &ManagedCreationBinding) -> Result<BindingRecoveryOutcome> {
    validate_binding(m)?;
    if let Some((p, Some(raw))) = original(db, m)? {
        let (c, plan) = match m.kind {
            ManagedCreationKind::ManagedMcp => {
                let c = super::managed_mcps::committed(db, &p, &raw)?;
                (c.connection, c.profile.runtime_plan_digest)
            }
            ManagedCreationKind::ManagedSkill => {
                let c = super::managed_skills::committed(db, &p, &raw)?;
                (c.connection, c.profile.runtime_plan_digest)
            }
        };
        return Ok(BindingRecoveryOutcome {
            binding: m.clone(),
            state: "confirmed".into(),
            connection: Some(RecoveryConnection {
                id: c.id,
                name: c.name,
                provider: c.provider,
                protocol_route: c.protocol_route,
                config_revision: c.config_revision,
                revision: c.revision,
                probe_state: c.probe_state,
                runtime_plan_digest: plan,
            }),
            proof: NoNewExecution { _private: () },
        });
    }
    let abandoned = if let Some(t) = tombstone(db, kind_name(m.kind), &m.request_key)? {
        if t != *m {
            return Err(conflict());
        }
        true
    } else {
        false
    };
    Ok(BindingRecoveryOutcome {
        binding: m.clone(),
        state: if abandoned { "abandoned" } else { "unresolved" }.into(),
        connection: None,
        proof: NoNewExecution { _private: () },
    })
}
fn outcome(db: &Connection, m: &ClientRecoveryMarker) -> Result<RecoveryOutcome> {
    validate_marker(m)?;
    let result = binding_outcome(db, &binding(m))?;
    Ok(RecoveryOutcome {
        marker: m.clone(),
        state: result.state,
        connection: result.connection,
        proof: result.proof,
    })
}
impl Store {
    /// Owner-authenticated historical lookup only; a filter is not cancellation authority.
    pub fn managed_creation_binding_outcome(
        &self,
        b: &ManagedCreationBinding,
    ) -> Result<BindingRecoveryOutcome> {
        let tx = self.conn.unchecked_transaction().map_err(db_error)?;
        let result = binding_outcome(&tx, b)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
    pub fn managed_creation_outcome(&self, m: &ClientRecoveryMarker) -> Result<RecoveryOutcome> {
        let tx = self.conn.unchecked_transaction().map_err(db_error)?;
        let result = outcome(&tx, m)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
    pub fn abandon_managed_creation(
        &mut self,
        m: &ClientRecoveryMarker,
    ) -> Result<RecoveryOutcome> {
        validate_marker(m)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let result = outcome(&tx, m)?;
        if result.state != "unresolved" {
            tx.commit().map_err(db_error)?;
            return Ok(result);
        }
        let binding = binding(m);
        let event_id = Uuid::new_v4().to_string();
        let t = Tombstone {
            binding: binding.clone(),
            event_id: event_id.clone(),
        };
        tx.execute(
            "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
            params![
                tombstone_scope(kind_name(m.kind)),
                m.request_key,
                encode(&binding)?,
                encode(&t)?
            ],
        )
        .map_err(db_error)?;
        let e = Event {
            sequence: 0,
            event_id,
            aggregate_id: format!("managed_creation:{}:{}", kind_name(m.kind), m.request_key),
            revision: 1,
            kind: "managed_creation.abandoned".into(),
            occurred_at: now()?,
            payload: json!(binding),
            source: "local_owner_recovery".into(),
        };
        tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&e)?])
            .map_err(db_error)?;
        let result = outcome(&tx, m)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "managed_creation_recovery_tests.rs"]
mod tests;
