//! Immutable native linkage and bounded Owner history. Views confer no authority.
use super::{authority::*, *};
use crate::{
    sessions::ApprovalProposal,
    storage::{db_error, decode, encode, now, Event, Store},
    Result,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub(super) fn sequence(db: &Connection) -> Result<u64> {
    db.query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| {
        r.get(0)
    })
    .map_err(db_error)
}
pub(super) fn event<T: Serialize>(
    db: &Connection,
    id: &str,
    revision: u64,
    kind: &str,
    payload: &T,
) -> Result<u64> {
    let e = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: id.into(),
        revision,
        kind: kind.into(),
        occurred_at: now()?,
        payload: serde_json::to_value(payload).map_err(|_| invalid())?,
        source: "integration_core".into(),
    };
    let text = encode(&e)?;
    if text.len() > MAX_INTEGRATION_BYTES {
        return Err(limited());
    }
    db.execute("INSERT INTO events(document) VALUES(?1)", [text])
        .map_err(db_error)?;
    sequence(db)
}
pub(super) fn replay<T: for<'a> Deserialize<'a>>(
    db: &Connection,
    scope: &str,
    key: &str,
    payload: &str,
) -> Result<Option<T>> {
    self::key(key)?;
    let row:Option<(i64,i64,Option<String>,Option<String>)>=db.query_row("SELECT octet_length(payload),octet_length(result),CASE WHEN octet_length(payload)<=1048576 THEN payload END,CASE WHEN octet_length(result)<=1048576 THEN result END FROM idempotency WHERE scope=?1 AND key=?2",params![scope,key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(db_error)?;
    let Some((p, r, old, result)) = row else {
        return Ok(None);
    };
    if !(1..=MAX_INTEGRATION_BYTES as i64).contains(&p)
        || !(1..=MAX_INTEGRATION_BYTES as i64).contains(&r)
    {
        return Err(limited());
    }
    if old.as_deref() != Some(payload) {
        return Err(CoreError::new(
            "CONFIG_CONFLICT",
            "Integration key was already committed for different exact parameters",
        ));
    }
    Ok(Some(decode(&result.ok_or_else(limited)?)?))
}
pub(super) fn remember<T: Serialize>(
    db: &Connection,
    scope: &str,
    key: &str,
    payload: &str,
    result: &T,
) -> Result<()> {
    let result = encode(result)?;
    if payload.len() > MAX_INTEGRATION_BYTES || result.len() > MAX_INTEGRATION_BYTES {
        return Err(limited());
    }
    db.execute(
        "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
        params![scope, key, payload, result],
    )
    .map_err(db_error)?;
    Ok(())
}
pub(super) fn policy(db: &Connection, id: &str) -> Result<IntegrationPolicy> {
    let actual: IntegrationPolicy = raw(db, "integration_policies", id)?;
    let mirror:bool=db.query_row("SELECT task_id=?2 AND spec_revision=?3 AND goal_scope_id=?4 AND approval_id=?5 AND revision=?6 FROM integration_policies WHERE id=?1",params![id,actual.selection.task_id,sql_num(actual.selection.expected_spec_revision)?,actual.selection.goal_scope_id,actual.approval_id,sql_num(actual.revision)?],|r|r.get(0)).map_err(db_error)?;
    let proposal: ApprovalProposal = raw(db, "approval_proposals", &actual.approval_id)?;
    let snapshot: IntegrationPolicySnapshot =
        serde_json::from_value(proposal.snapshot.clone()).map_err(|_| changed())?;
    let result:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(result)<=1048576 THEN result END FROM approval_proposals WHERE id=?1",[&actual.approval_id],|r|r.get(0)).optional().map_err(db_error)?;
    let original: IntegrationPolicy = decode(&result.flatten().ok_or_else(denied)?)?;
    let mut immutable = actual.clone();
    immutable.revision = 1;
    immutable.revoked = false;
    if !mirror
        || actual.id != id
        || actual.id != actual.approval_id
        || !proposal.consumed
        || proposal.kind != "integration"
        || proposal.id != id
        || snapshot.authority_kind != "authorized_local_integration"
        || snapshot.intent != actual.selection
        || sha(&snapshot.bindings)? != sha(&actual.bindings)?
        || actual.snapshot_digest != sha(&proposal.snapshot)?
        || encode(&immutable)? != encode(&original)?
    {
        return Err(denied());
    }
    let confirmed:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='approval.confirmed' AND json_extract(document,'$.source')='local_owner_confirmation' AND json_extract(document,'$.payload.kind')='integration' ORDER BY sequence LIMIT 1",[id],|r|r.get(0)).optional().map_err(db_error)?;
    let prepared:Option<String>=db.query_row("SELECT document FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='integration.policy_approved' AND json_extract(document,'$.source')='integration_core' AND json_extract(document,'$.payload.snapshot_digest')=?2 AND sequence>?3 ORDER BY sequence LIMIT 1",params![id,actual.snapshot_digest,confirmed.ok_or_else(denied)?],|r|r.get(0)).optional().map_err(db_error)?;
    if prepared.is_none() {
        return Err(denied());
    }
    if actual.revoked {
        if actual.revision != 2 {
            return Err(changed());
        }
        let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='integration.policy_revoked' AND json_extract(document,'$.source')='integration_core' AND json_extract(document,'$.revision')=2)",[id],|r|r.get(0)).map_err(db_error)?;
        if !valid {
            return Err(denied());
        }
    } else if actual.revision != 1 {
        return Err(changed());
    }
    Ok(actual)
}
pub(super) fn run(db: &Connection, id: &str) -> Result<IntegrationRun> {
    let r: IntegrationRun = raw(db, "integrations", id)?;
    let mirror:bool=db.query_row("SELECT policy_id=?2 AND task_id=?3 AND spec_revision=?4 AND state=?5 AND revision=?6 FROM integrations WHERE id=?1",params![id,r.policy_id,r.task_id,sql_num(r.spec_revision)?,encode(&r.state)?.trim_matches('"'),sql_num(r.revision)?],|row|row.get(0)).map_err(db_error)?;
    let p = policy(db, &r.policy_id)?;
    if !mirror
        || r.id != id
        || r.task_id != p.selection.task_id
        || r.spec_revision != p.selection.expected_spec_revision
        || r.goal_scope_id != p.selection.goal_scope_id
        || r.scope_digest != p.bindings.goal_scope_digest
        || r.target_ref != p.selection.target_ref
        || r.round == 0
        || r.round > p.selection.max_rounds
        || r.policy_revision != 1
    {
        return Err(changed());
    }
    let queued:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='integration.queued' AND json_extract(document,'$.source')='integration_core' AND json_extract(document,'$.payload.acceptance_ref')=?2 AND json_extract(document,'$.payload.acceptance_digest')=?3)",params![id,r.acceptance_ref,r.acceptance_digest],|row|row.get(0)).map_err(db_error)?;
    if !queued {
        return Err(denied());
    }
    Ok(r)
}
pub(super) fn save_run(
    db: &Connection,
    r: &mut IntegrationRun,
    kind: &str,
    payload: Value,
) -> Result<()> {
    let before = r.revision;
    r.revision = r.revision.checked_add(1).ok_or_else(limited)?;
    r.updated_at = now()?;
    let raw = encode(r)?;
    if raw.len() > MAX_INTEGRATION_BYTES {
        return Err(limited());
    }
    if db
        .execute(
            "UPDATE integrations SET revision=?1,state=?2,document=?3 WHERE id=?4 AND revision=?5",
            params![
                sql_num(r.revision)?,
                encode(&r.state)?.trim_matches('"'),
                raw,
                r.id,
                sql_num(before)?
            ],
        )
        .map_err(db_error)?
        != 1
    {
        return Err(changed());
    }
    event(db, &r.id, r.revision, kind, &payload)?;
    Ok(())
}
fn ids(db: &Connection, table: &str, q: &IntegrationListQuery) -> Result<Vec<String>> {
    if !id(&q.task_id) || q.after_id.as_deref().is_some_and(|x| !id(x)) {
        return Err(invalid());
    }
    let limit = q.limit.unwrap_or(20);
    if limit == 0 || limit > MAX_INTEGRATION_PAGE {
        return Err(invalid());
    }
    let sql=match table {"integration_policies"=>"SELECT id,octet_length(document) FROM integration_policies WHERE task_id=?1 AND id>?2 ORDER BY id LIMIT ?3","integrations"=>"SELECT id,octet_length(document) FROM integrations WHERE task_id=?1 AND id>?2 ORDER BY id LIMIT ?3",_=>return Err(invalid())};
    let mut stmt = db.prepare(sql).map_err(db_error)?;
    let mut rows = stmt
        .query(params![
            q.task_id,
            q.after_id.as_deref().unwrap_or(""),
            limit
        ])
        .map_err(db_error)?;
    let mut out = Vec::new();
    let mut bytes = 0i64;
    while let Some(r) = rows.next().map_err(db_error)? {
        let size: i64 = r.get(1).map_err(db_error)?;
        bytes = bytes.checked_add(size).ok_or_else(limited)?;
        if size < 0 || bytes > MAX_INTEGRATION_BYTES as i64 {
            return Err(limited());
        }
        out.push(r.get(0).map_err(db_error)?);
    }
    Ok(out)
}
impl Store {
    pub fn get_integration_policy(&self, id: &str) -> Result<IntegrationPolicy> {
        policy(&self.conn, id)
    }
    pub fn list_integration_policies(
        &self,
        q: &IntegrationListQuery,
    ) -> Result<Vec<IntegrationPolicy>> {
        ids(&self.conn, "integration_policies", q)?
            .iter()
            .map(|id| policy(&self.conn, id))
            .collect()
    }
    pub fn list_integrations(&self, q: &IntegrationListQuery) -> Result<Vec<IntegrationRun>> {
        ids(&self.conn, "integrations", q)?
            .iter()
            .map(|id| run(&self.conn, id))
            .collect()
    }
    pub fn revoke_integration_policy(
        &mut self,
        id: &str,
        revision: u64,
        key: &str,
    ) -> Result<IntegrationPolicy> {
        let payload = encode(&json!({"policy_id":id,"expected_revision":revision}))?;
        let scope = format!("integration-policy:{id}:revoke");
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(r) = replay(&tx, &scope, key, &payload)? {
            return Ok(r);
        }
        let mut p = policy(&tx, id)?;
        if p.revision != revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Integration policy revision changed",
            ));
        }
        if !p.revoked {
            p.revoked = true;
            p.revision = 2;
            tx.execute(
                "UPDATE integration_policies SET revision=2,document=?1 WHERE id=?2 AND revision=1",
                params![encode(&p)?, id],
            )
            .map_err(db_error)?;
            event(
                &tx,
                id,
                2,
                "integration.policy_revoked",
                &json!({"policy_id":id,"new_dispatch":false,"owned_workers_require_join":true}),
            )?;
        }
        remember(&tx, &scope, key, &payload, &p)?;
        tx.commit().map_err(db_error)?;
        Ok(p)
    }
}
