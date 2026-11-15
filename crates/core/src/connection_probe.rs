//! One newly confirmed, exact-digest version probe. Observations never grant
//! session, model, installation update or account authority.
use super::*;
use std::os::unix::fs::PermissionsExt;
const ORIGIN: &str = "connection-probe-origin-v1";
const DISPATCH: &str = "connection-probe-dispatch-v1";
const TERMINAL: &str = "connection-probe-terminal-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionInspectRequest {
    pub connection_id: String,
    pub expected_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionProbeId {
    pub attempt_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionProbeFind {
    pub connection_id: String,
    pub request_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionInspection {
    pub connection_id: String,
    pub connection_revision: u64,
    pub observed_at_unix_ms: u64,
    pub executable_status: String,
    pub executable_is_executable: bool,
    pub approved_executable_sha256: String,
    pub current_executable_sha256: Option<String>,
    pub probe_state: String,
    pub version_evidence: Option<VersionEvidence>,
    pub retry_eligible: bool,
    pub retry_basis: Option<String>,
    pub retry_blocked_reason: Option<String>,
    pub latest_probe_attempt: Option<ConnectionProbeAttempt>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionProbeAttempt {
    pub id: String,
    pub connection_id: String,
    pub connection_revision: u64,
    pub request_key: String,
    pub proposal_id: String,
    pub executable_sha256: String,
    pub config_revision: String,
    pub state: String,
    pub process_reaped: bool,
    pub not_started: bool,
    pub version_evidence: Option<VersionEvidence>,
    pub created_at: String,
    pub finished_at: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionProbeConfirmation {
    pub connection: ProviderConnection,
    pub probe_attempt: ConnectionProbeAttempt,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Origin {
    connection: ProviderConnection,
    attempt: ConnectionProbeAttempt,
}
fn digest<T: Serialize>(v: &T) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(encode(v)?.as_bytes())))
}
fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Connection or exact probe evidence changed; review the existing record",
    )
}
fn row(db: &Connection, scope: &str, id: &str) -> Result<Option<(String, String)>> {
    let sizes:Option<(i64,i64)>=db.query_row("SELECT octet_length(payload),octet_length(result) FROM idempotency WHERE scope=?1 AND key=?2",params![scope,id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
    let Some((a, b)) = sizes else { return Ok(None) };
    if a < 0 || b < 0 || a + b > 1048576 {
        return Err(CoreError::new(
            "RESOURCE_LIMIT",
            "Probe history exceeds bounded record",
        ));
    }
    db.query_row(
        "SELECT payload,result FROM idempotency WHERE scope=?1 AND key=?2",
        params![scope, id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(db_error)
}
fn connection(db: &Connection, id: &str) -> Result<ProviderConnection> {
    let raw:String=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM connections WHERE id=?1",[id],|r|r.get(0)).map_err(db_error)?;
    let c: ProviderConnection = decode(&raw)?;
    if c.id != id {
        return Err(changed());
    }
    Ok(c)
}
fn origin(db: &Connection, id: &str) -> Result<Origin> {
    let (payload, raw) = row(db, ORIGIN, id)?
        .ok_or_else(|| CoreError::new("NOT_FOUND", "Version probe attempt unavailable"))?;
    let o: Origin = decode(&raw)?;
    if o.attempt.id != id
        || o.attempt.connection_id != o.connection.id
        || o.attempt.connection_revision != o.connection.revision
        || o.attempt.state != "prepared"
        || o.attempt.executable_sha256 != o.connection.executable_sha256
        || o.attempt.config_revision != o.connection.config_revision
        || payload != digest(&o)?
    {
        return Err(changed());
    }
    let approved:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM approval_proposals WHERE id=?1 AND octet_length(document)<=1048576 AND json_extract(document,'$.kind')='connection_probe' AND json_extract(document,'$.consumed')=1 AND json_extract(result,'$.probe_attempt.id')=?2)",params![o.attempt.proposal_id,id],|r|r.get(0)).map_err(db_error)?;
    let committed:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='connection.probe_retry_authorized' AND json_extract(document,'$.source')='local_owner_confirmation' AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.payload.attempt_id')=?2 AND json_extract(document,'$.payload.origin_digest')=?3)",params![o.connection.id,id,payload],|r|r.get(0)).map_err(db_error)?;
    if !approved || !committed {
        return Err(changed());
    }
    Ok(o)
}
fn attempt(db: &Connection, id: &str) -> Result<ConnectionProbeAttempt> {
    let o = origin(db, id)?;
    if let Some((payload, raw)) = row(db, TERMINAL, id)? {
        let a: ConnectionProbeAttempt = decode(&raw)?;
        if payload != digest(&o)?
            || a.id != id
            || a.connection_id != o.attempt.connection_id
            || a.connection_revision != o.attempt.connection_revision
            || a.proposal_id != o.attempt.proposal_id
            || a.request_key != o.attempt.request_key
            || a.executable_sha256 != o.attempt.executable_sha256
            || a.config_revision != o.attempt.config_revision
        {
            return Err(changed());
        }
        let committed:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='connection.probe_retry_finished' AND json_extract(document,'$.source')='provider_runtime' AND json_extract(document,'$.payload.attempt_id')=?1 AND json_extract(document,'$.payload.result_digest')=?2)",params![id,digest(&a)?],|r|r.get(0)).map_err(db_error)?;
        if !committed {
            return Err(changed());
        }
        return Ok(a);
    }
    let mut a = o.attempt;
    if let Some((payload, _)) = row(db, DISPATCH, id)? {
        if payload != digest(&o.connection)? {
            return Err(changed());
        }
        a.state = "running".into();
    }
    Ok(a)
}
pub(super) fn latest(db: &Connection, id: &str) -> Result<Option<ConnectionProbeAttempt>> {
    // The native confirmation remains a durable backlink if an attempt row is
    // missing. A missing/oversized attempt must never revive the legacy probe path.
    let key:Option<String>=db.query_row("SELECT CASE WHEN octet_length(result)<=1048576 THEN json_extract(result,'$.probe_attempt.id') END FROM approval_proposals WHERE json_extract(document,'$.kind')='connection_probe' AND json_extract(document,'$.snapshot.intent.connection_id')=?1 AND result IS NOT NULL ORDER BY rowid DESC LIMIT 1",[id],|r|r.get(0)).optional().map_err(db_error)?;
    if key.is_none() {
        let marked:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM idempotency WHERE scope=?1 AND json_extract(result,'$.connection.id')=?2) OR EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?2 AND json_extract(document,'$.kind')='connection.probe_retry_authorized')",params![ORIGIN,id],|r|r.get(0)).map_err(db_error)?;
        if marked {
            return Err(changed());
        }
    }
    key.map(|key| {
        key_valid(&key)?;
        attempt(db, &key)
    })
    .transpose()
}
fn active(db: &Connection, id: &str) -> Result<bool> {
    db.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE connection_id=?1 AND state NOT IN('stopped','failed')) OR EXISTS(SELECT 1 FROM conversations c JOIN conversation_dispatches d ON d.conversation_id=c.id WHERE c.connection_id=?1 AND (c.state NOT IN('stopped','failed') OR d.state!='stopped')) OR EXISTS(SELECT 1 FROM attempts a JOIN dispatch_intents d ON d.attempt_id=a.id WHERE json_extract(a.binding,'$.connection_id')=?1 AND d.state!='stopped') OR EXISTS(SELECT 1 FROM goal_graph_jobs WHERE state IN('prepared','running','cancel_requested','outcome_unknown'))",[id],|r|r.get(0)).map_err(db_error)
}
fn eligible(db: &Connection, c: &ProviderConnection) -> Result<()> {
    if !std::fs::symlink_metadata(&c.executable).is_ok_and(|m| {
        m.is_file() && !m.file_type().is_symlink() && m.permissions().mode() & 0o111 != 0
    }) {
        return Err(CoreError::new(
            "PERMISSION_DENIED",
            "Selected program is not currently an executable regular file",
        ));
    }
    if c.probe_state != "failed" {
        return Err(CoreError::new(
            "PERMISSION_DENIED",
            "Only failed connections can receive a newly approved retry",
        ));
    }
    if active(db, &c.id)? {
        return Err(CoreError::new(
            "WORKSPACE_BUSY",
            "Connection has an active or unreconciled execution binding",
        ));
    }
    let stopped = if let Some(a) = latest(db, &c.id)? {
        a.connection_revision.checked_add(1) == Some(c.revision)
            && (a.not_started
                || (a.process_reaped
                    && a.version_evidence
                        .as_ref()
                        .is_some_and(|e| e.process_reaped)))
            && matches!(a.state.as_str(), "failed" | "not_started")
    } else {
        let raw:Option<String>=db.query_row("SELECT document FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.revision')=?2 AND json_extract(document,'$.kind') IN('connection.probe_finished','connection.initial_probe_not_started') ORDER BY sequence DESC LIMIT 1",params![c.id,i64::try_from(c.revision).map_err(|_|changed())?],|r|r.get(0)).optional().map_err(db_error)?;
        let e = raw.map(|raw| decode::<Event>(&raw)).transpose()?;
        e.is_some_and(|e| match e.kind.as_str() {
            "connection.probe_finished" => {
                e.source == "provider_runtime"
                    && e.payload["version_evidence"] == json!(c.version_evidence)
                    && c.version_evidence.as_ref().is_some_and(|v| {
                        v.process_reaped
                            && v.executable_sha256.as_str() == c.executable_sha256
                            && v.state != VersionProbeState::Succeeded
                    })
            }
            "connection.initial_probe_not_started" => {
                e.source == "owned_supervisor"
                    && e.payload["not_started"] == true
                    && e.payload["executable_sha256"] == c.executable_sha256
                    && c.version_evidence.is_none()
            }
            _ => false,
        })
    };
    if !stopped {
        return Err(CoreError::new("OUTCOME_UNKNOWN","No complete stop or never-started proof exists; this connection cannot execute another probe"));
    }
    Ok(())
}
fn stop_proof(db: &Connection, c: &ProviderConnection) -> Result<Value> {
    eligible(db, c)?;
    if let Some(e) = &c.version_evidence {
        return Ok(json!({"kind":"process_reaped","version_evidence":e}));
    }
    let sequence:i64=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.revision')=?2 AND json_extract(document,'$.kind') IN('connection.initial_probe_not_started','connection.probe_retry_finished') ORDER BY sequence DESC LIMIT 1",params![c.id,i64::try_from(c.revision).map_err(|_|changed())?],|r|r.get(0)).map_err(db_error)?;
    if sequence <= 0 {
        return Err(changed());
    }
    Ok(json!({"kind":"not_started","event_sequence":sequence}))
}
impl Store {
    pub fn find_connection_probe(
        &self,
        r: &ConnectionProbeFind,
    ) -> Result<Option<ConnectionProbeConfirmation>> {
        key_valid(&r.connection_id)?;
        key_valid(&r.request_key)?;
        let mut q=self.conn.prepare("SELECT CASE WHEN octet_length(result)<=1048576 THEN json_extract(result,'$.probe_attempt.id') END FROM approval_proposals WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='connection_probe' AND json_extract(document,'$.snapshot.intent.connection_id')=?1 AND json_extract(document,'$.snapshot.request_key')=?2 AND result IS NOT NULL LIMIT 2").map_err(db_error)?;
        let ids = q
            .query_map(params![r.connection_id, r.request_key], |row| {
                row.get::<_, String>(0)
            })
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        if ids.len() > 1 {
            return Err(changed());
        }
        ids.first()
            .map(|id| self.get_connection_probe(id))
            .transpose()
    }
    pub(crate) fn record_initial_probe_not_started(&mut self, id: &str) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut c = connection(&tx, id)?;
        if c.revision != 1
            || c.probe_state != "pending"
            || c.version_evidence.is_some()
            || latest(&tx, id)?.is_some()
        {
            return Err(changed());
        }
        let bound:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.revision')=1 AND json_extract(document,'$.kind')='connection.probe_authorized' AND json_extract(document,'$.source')='user_action' AND json_extract(document,'$.payload.approval_id')=?2 AND json_extract(document,'$.payload.executable_sha256')=?3)",params![id,c.approval_id,c.executable_sha256],|r|r.get(0)).map_err(db_error)?;
        if !bound {
            return Err(changed());
        }
        c.revision = 2;
        c.probe_state = "failed".into();
        tx.execute(
            "UPDATE connections SET document=?1 WHERE id=?2",
            params![encode(&c)?, id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            id,
            c.revision,
            "connection.initial_probe_not_started",
            json!({"executable_sha256":c.executable_sha256,"not_started":true}),
            "owned_supervisor",
        )?;
        tx.commit().map_err(db_error)
    }
    pub fn inspect_connection(&self, r: &ConnectionInspectRequest) -> Result<ConnectionInspection> {
        key_valid(&r.connection_id)?;
        let c = connection(&self.conn, &r.connection_id)?;
        if c.revision != r.expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Connection changed; refresh before inspecting",
            ));
        }
        let executable = std::fs::symlink_metadata(&c.executable).is_ok_and(|m| {
            m.is_file() && !m.file_type().is_symlink() && m.permissions().mode() & 0o111 != 0
        });
        let current = executable_digest(&c.executable)
            .ok()
            .map(|h| h.as_str().to_owned());
        let status = match &current {
            Some(h) if h == &c.executable_sha256 => "matched",
            Some(_) => "changed",
            None => "unavailable",
        };
        let eligibility = eligible(&self.conn, &c);
        let basis = eligibility.as_ref().ok().map(|_| {
            if c.version_evidence.is_some() {
                "reaped_failure".into()
            } else {
                "not_started".into()
            }
        });
        let blocked = if !executable {
            Some("PERMISSION_DENIED".into())
        } else if status != "matched" {
            Some("INPUT_CHANGED".into())
        } else {
            eligibility.err().map(|e| e.code)
        };
        Ok(ConnectionInspection {
            connection_id: c.id.clone(),
            connection_revision: c.revision,
            observed_at_unix_ms: u64::try_from(ms()?).map_err(|_| changed())?,
            executable_status: status.into(),
            executable_is_executable: executable,
            approved_executable_sha256: c.executable_sha256,
            current_executable_sha256: current,
            probe_state: c.probe_state,
            version_evidence: c.version_evidence,
            retry_eligible: blocked.is_none(),
            retry_basis: basis,
            retry_blocked_reason: blocked,
            latest_probe_attempt: latest(&self.conn, &c.id)?,
        })
    }
    pub fn propose_connection_probe(
        &mut self,
        r: &ConnectionInspectRequest,
        key: &str,
    ) -> Result<ApprovalProposal> {
        key_valid(key)?;
        let scope = "local:approvals.propose:connection_probe";
        if let Some((_, raw)) = row(&self.conn, scope, key)? {
            let p: ApprovalProposal = decode(&raw)?;
            if p.snapshot["intent"] != json!(r) {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Probe key belongs to another connection selection",
                ));
            }
            return Ok(p);
        }
        let c = connection(&self.conn, &r.connection_id)?;
        if c.revision != r.expected_revision {
            return Err(CoreError::new("STALE_REVISION", "Connection changed"));
        }
        eligible(&self.conn, &c)?;
        verify_executable(
            &c.executable,
            &BinaryDigest::parse(&c.executable_sha256).map_err(|_| changed())?,
        )
        .map_err(|_| changed())?;
        let snapshot = json!({"intent":r,"connection":c,"connection_digest":digest(&c)?,"scope":"version_probe_only","arguments":["--version"],"timeout_ms":3000,"stdout_limit_bytes":4096,"request_key":key,"previous_stop_proof":stop_proof(&self.conn,&c)?,"identity_status":"unknown","warning":"Executes the approved binary once; no account check, model call, update or sandbox guarantee"});
        self.persist_proposal("connection_probe", snapshot, key)
    }
    pub fn finalize_connection_probe(&mut self, id: &str, revision: u64) -> Result<Value> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let(raw,result):(String,Option<String>)=tx.query_row("SELECT document,result FROM approval_proposals WHERE id=?1 AND octet_length(document)<=1048576 AND COALESCE(octet_length(result),0)<=1048576",[id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        let mut p: ApprovalProposal = decode(&raw)?;
        if p.kind != "connection_probe" {
            return Err(changed());
        }
        if let Some(raw) = result {
            let prior: ConnectionProbeConfirmation = decode(&raw)?;
            let result = (|| -> Result<Value> {
                let a = attempt(&tx, &prior.probe_attempt.id)?;
                Ok(json!(ConnectionProbeConfirmation {
                    connection: connection(&tx, &a.connection_id)?,
                    probe_attempt: a
                }))
            })();
            return result.map_err(|mut e| {
                e.side_effect_state = "confirmed".into();
                e
            });
        }
        if p.revision != revision {
            return Err(CoreError::new("STALE_REVISION", "Owner proposal changed"));
        }
        if p.expires_at_ms <= ms()? {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Owner probe approval expired",
            ));
        }
        let r: ConnectionInspectRequest =
            serde_json::from_value(p.snapshot["intent"].clone()).map_err(|_| changed())?;
        let mut c = connection(&tx, &r.connection_id)?;
        if c.revision != r.expected_revision
            || p.snapshot["connection_digest"] != digest(&c)?
            || p.snapshot["connection"] != json!(c)
        {
            return Err(changed());
        }
        eligible(&tx, &c)?;
        if p.snapshot["previous_stop_proof"] != stop_proof(&tx, &c)? {
            return Err(changed());
        }
        verify_executable(
            &c.executable,
            &BinaryDigest::parse(&c.executable_sha256).map_err(|_| changed())?,
        )
        .map_err(|_| changed())?;
        c.revision = c.revision.checked_add(1).ok_or_else(changed)?;
        c.probe_state = "pending".into();
        c.version_evidence = None;
        let a = ConnectionProbeAttempt {
            id: Uuid::new_v4().to_string(),
            connection_id: c.id.clone(),
            connection_revision: c.revision,
            request_key: p.snapshot["request_key"]
                .as_str()
                .ok_or_else(changed)?
                .into(),
            proposal_id: id.into(),
            executable_sha256: c.executable_sha256.clone(),
            config_revision: c.config_revision.clone(),
            state: "prepared".into(),
            process_reaped: false,
            not_started: false,
            version_evidence: None,
            created_at: now()?,
            finished_at: None,
        };
        let o = Origin {
            connection: c.clone(),
            attempt: a.clone(),
        };
        let wrapper = ConnectionProbeConfirmation {
            connection: c.clone(),
            probe_attempt: a,
        };
        p.consumed = true;
        p.revision += 1;
        tx.execute(
            "UPDATE connections SET document=?1 WHERE id=?2",
            params![encode(&c)?, c.id],
        )
        .map_err(db_error)?;
        tx.execute(
            "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
            params![ORIGIN, o.attempt.id, digest(&o)?, encode(&o)?],
        )
        .map_err(db_error)?;
        tx.execute(
            "UPDATE approval_proposals SET document=?1,result=?2 WHERE id=?3 AND result IS NULL",
            params![encode(&p)?, encode(&wrapper)?, id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &c.id,
            c.revision,
            "connection.probe_retry_authorized",
            json!({"attempt_id":o.attempt.id,"origin_digest":digest(&o)?}),
            "local_owner_confirmation",
        )?;
        event(
            &tx,
            id,
            p.revision,
            "approval.confirmed",
            json!({"kind":"connection_probe","proposal_id":id,"consumed":true}),
            "local_owner_confirmation",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(json!(wrapper))
    }
    pub fn get_connection_probe(&self, id: &str) -> Result<ConnectionProbeConfirmation> {
        key_valid(id)?;
        let a = attempt(&self.conn, id)?;
        Ok(ConnectionProbeConfirmation {
            connection: connection(&self.conn, &a.connection_id)?,
            probe_attempt: a,
        })
    }
    pub(super) fn finish_connection_probe(
        &mut self,
        id: &str,
        result: std::result::Result<VersionEvidence, String>,
        not_started: bool,
    ) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let o = origin(&tx, id)?;
        if row(&tx, TERMINAL, id)?.is_some() {
            attempt(&tx, id)?;
            return Ok(());
        }
        let mut c = connection(&tx, &o.connection.id)?;
        if encode(&c)? != encode(&o.connection)? {
            return Err(changed());
        }
        let mut a = o.attempt.clone();
        let evidence = result.ok();
        if evidence
            .as_ref()
            .is_some_and(|e| e.executable_sha256.as_str() != a.executable_sha256)
        {
            return Err(changed());
        }
        a.process_reaped = evidence.as_ref().is_some_and(|e| e.process_reaped);
        a.not_started = not_started;
        a.state = if not_started {
            "not_started"
        } else if a.process_reaped {
            if evidence
                .as_ref()
                .is_some_and(|e| e.state == VersionProbeState::Succeeded)
            {
                "succeeded"
            } else {
                "failed"
            }
        } else {
            "outcome_unknown"
        }
        .into();
        a.version_evidence = evidence;
        a.finished_at = Some(now()?);
        c.revision = c.revision.checked_add(1).ok_or_else(changed)?;
        c.probe_state = if a.state == "succeeded" {
            "ready"
        } else {
            "failed"
        }
        .into();
        c.version_evidence = a.version_evidence.clone();
        tx.execute(
            "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
            params![TERMINAL, id, digest(&o)?, encode(&a)?],
        )
        .map_err(db_error)?;
        tx.execute(
            "UPDATE connections SET document=?1 WHERE id=?2",
            params![encode(&c)?, c.id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &c.id,
            c.revision,
            "connection.probe_retry_finished",
            json!({"attempt_id":id,"result_digest":digest(&a)?}),
            "provider_runtime",
        )?;
        tx.commit().map_err(db_error)
    }
}
impl Supervisor {
    pub fn probe_retry(&mut self, store: &mut Store, id: &str) -> Result<()> {
        let a = attempt(&store.conn, id)?;
        if a.state != "prepared"
            || self.retry_probes.contains_key(id)
            || self.retry_probe_receipts.contains_key(id)
        {
            return Ok(());
        }
        let prepare = (|| -> Result<VersionProbeRequest> {
            if self.probes.len() + self.retry_probes.len() >= 4 {
                return Err(CoreError::new(
                    "RESOURCE_LIMIT",
                    "Version probe concurrency limit reached",
                ));
            }
            let o = origin(&store.conn, id)?;
            let c = connection(&store.conn, &a.connection_id)?;
            if encode(&c)? != encode(&o.connection)? {
                return Err(changed());
            }
            if active(&store.conn, &c.id)? {
                return Err(CoreError::new(
                    "WORKSPACE_BUSY",
                    "Execution binding is active",
                ));
            }
            verify_executable(
                &c.executable,
                &BinaryDigest::parse(&c.executable_sha256).map_err(|_| changed())?,
            )
            .map_err(|_| changed())?;
            Ok(VersionProbeRequest {
                executable: c.executable.clone(),
                expected_sha256: BinaryDigest::parse(&c.executable_sha256)
                    .map_err(|_| changed())?,
                working_directory: self.runtime.clone(),
                approval_reference: a.proposal_id.clone(),
                environment: self.environment(&c)?,
                timeout: Duration::from_secs(3),
            })
        })();
        let request = match prepare {
            Ok(r) => r,
            Err(mut e) => {
                store.finish_connection_probe(id, Err("pre_spawn_failed".into()), true)?;
                e.side_effect_state = "confirmed".into();
                return Err(e);
            }
        };
        let o = origin(&store.conn, id)?;
        store
            .conn
            .execute(
                "INSERT INTO idempotency VALUES(?1,?2,?3,'dispatched')",
                params![DISPATCH, id, digest(&o.connection)?],
            )
            .map_err(db_error)?;
        match std::thread::Builder::new()
            .name("baton-version-retry".into())
            .spawn(move || probe_version(request).map_err(|_| "version_probe_failed".into()))
        {
            Ok(worker) => {
                self.retry_probes.insert(id.into(), worker);
                Ok(())
            }
            Err(_) => {
                store.finish_connection_probe(id, Err("thread_not_started".into()), true)?;
                let mut e = CoreError::new("RESOURCE_LIMIT", "Version probe thread did not start");
                e.side_effect_state = "confirmed".into();
                Err(e)
            }
        }
    }
    pub(super) fn tick_retry_probes(&mut self, store: &mut Store) -> Result<()> {
        let done: Vec<String> = self
            .retry_probes
            .iter()
            .filter(|(_, w)| w.is_finished())
            .map(|(id, _)| id.clone())
            .collect();
        for id in done {
            let w = self.retry_probes.remove(&id).expect("owned worker");
            self.retry_probe_receipts.insert(
                id,
                w.join()
                    .unwrap_or_else(|_| Err("probe_worker_failed".into())),
            );
        }
        for id in self
            .retry_probe_receipts
            .keys()
            .cloned()
            .collect::<Vec<_>>()
        {
            store.finish_connection_probe(&id, self.retry_probe_receipts[&id].clone(), false)?;
            self.retry_probe_receipts.remove(&id);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "connection_probe_tests.rs"]
mod tests;
