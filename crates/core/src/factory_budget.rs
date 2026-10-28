//! Shared accounting only. A nullable legacy reservation is never Factory authority.
use super::store::*;
use crate::goal_plan::{AttemptReservation, BudgetLimits};
use crate::{
    storage::{db_error, decode, encode, Store},
    CoreError, Result,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FactoryReservation {
    pub authority_kind: String,
    pub scope_id: Option<String>,
    pub run_id: Option<String>,
    pub root_task_id: String,
    pub root_grant_id: String,
    pub project_id: String,
    pub task_id: String,
    pub session_id: String,
    pub attempt_id: String,
    pub ancestors: Vec<String>,
    #[serde(default)]
    pub queued_ms: u64,
    pub reservation: AttemptReservation,
    pub settled: bool,
    pub actual_runtime_ms: Option<u64>,
    pub actual_output_bytes: Option<u64>,
    pub measurement_scope: String,
    pub filesystem_total_bytes: Option<u64>,
    pub captured_provider_message_bytes: Option<u64>,
    pub provider_message_capture_complete: bool,
}
pub(crate) fn budget_records(
    db: &Connection,
    root: &str,
    project: &str,
) -> Result<Vec<FactoryReservation>> {
    let(c,bytes):(i64,i64)=db.query_row("SELECT count(*),COALESCE(sum(octet_length(document)),0) FROM factory_budget_reservations WHERE project_id=?1 OR json_extract(document,'$.root_grant_id')=?2",params![project,root],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    if c > 100_000 || bytes > 1024 * 1024 {
        return Err(limited());
    }
    let mut q=db.prepare("SELECT attempt_id,scope_id,root_task_id,project_id,document FROM factory_budget_reservations WHERE project_id=?1 OR json_extract(document,'$.root_grant_id')=?2").map_err(db_error)?;
    let docs = q
        .query_map(params![project, root], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    let mut out = Vec::new();
    for (id, scope_id, root_task, project_id, raw) in docs {
        let r: FactoryReservation = decode(&raw)?;
        let authority_ok = match (
            r.authority_kind.as_str(),
            scope_id.as_ref(),
            r.scope_id.as_ref(),
        ) {
            ("factory_task", Some(s), Some(d)) if s == d => {
                let origin = scope(db, s)?;
                let owned:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM factory_attempts WHERE session_id=?1 AND attempt_id=?2 AND scope_id=?3 AND root_task_id=?4)",params![r.session_id,r.attempt_id,s,r.root_task_id],|row|row.get(0)).map_err(db_error)?;
                owned
                    && origin.root_task_id == r.root_task_id
                    && origin.root_grant_id == r.root_grant_id
                    && origin.project_id == r.project_id
                    && r.run_id.is_some()
            }
            ("legacy_shared_session", None, None) => {
                r.run_id.is_none()
                    && !super::runtime::is_factory_task(db, &r.task_id)?
                    && !super::runtime::owns_session(db, &r.session_id)?
                    && r.measurement_scope == "empty_selected_artifacts_no_validator_streams"
                    && r.reservation.output_bytes == 0
            }
            _ => false,
        };
        if !authority_ok
            || r.attempt_id != id
            || r.root_task_id != root_task
            || r.project_id != project_id
            || r.filesystem_total_bytes.is_some()
            || r.provider_message_capture_complete
        {
            return Err(CoreError::new("BUDGET_UNKNOWN","Shared reservation origin or measurement scope is inconsistent; no missing history was treated as zero"));
        }
        let actual:Option<(String,String,String,String)>=db.query_row("SELECT s.id,s.task_id,s.root_grant_id,a.root_task_id FROM sessions s JOIN attempts a ON a.id=json_extract(s.document,'$.attempt_id') WHERE a.id=?1",[&id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional().map_err(db_error)?;
        if actual
            != Some((
                r.session_id.clone(),
                r.task_id.clone(),
                r.root_grant_id.clone(),
                r.root_task_id.clone(),
            ))
        {
            return Err(CoreError::new(
                "BUDGET_UNKNOWN",
                "Shared reservation is not bound to the actual session/attempt/root",
            ));
        }
        out.push(r);
    }
    Ok(out)
}
impl Store {
    pub(crate) fn shared_session_duration(&self, id: &str) -> Result<Option<u64>> {
        let raw:Option<Option<String>>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM factory_budget_reservations WHERE attempt_id=(SELECT json_extract(document,'$.attempt_id') FROM sessions WHERE id=?1)",[id],|r|r.get(0)).optional().map_err(db_error)?;
        raw.map(|r| {
            decode::<FactoryReservation>(&r.ok_or_else(limited)?).map(|r| r.reservation.runtime_ms)
        })
        .transpose()
    }
    pub(crate) fn shared_session_queue(&self, id: &str) -> Result<Option<u64>> {
        let row:Option<Option<String>>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM factory_budget_reservations WHERE json_extract(document,'$.session_id')=?1",[id],|r|r.get(0)).optional().map_err(db_error)?;
        row.map(|r| decode::<FactoryReservation>(&r.ok_or_else(limited)?).map(|r| r.queued_ms))
            .transpose()
    }
    pub(crate) fn reserve_ordinary_shared_budget(
        db: &Connection,
        session: &crate::sessions::ManagedSession,
        grant: &crate::policy::ExecutionGrant,
        root_task_id: &str,
    ) -> Result<()> {
        let row:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_ledgers WHERE scope=?1",[format!("project:{}",session.project_id)],|r|r.get(0)).optional().map_err(db_error)?;
        let Some(row) = row else { return Ok(()) };
        let ledger: serde_json::Value = decode(&row.ok_or_else(limited)?)?;
        let project_limits: BudgetLimits =
            serde_json::from_value(ledger["limits"].clone()).map_err(|_| changed())?;
        let root_key = format!("root-grant:{}", grant.root_grant_id);
        let root_row:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_ledgers WHERE scope=?1",[root_key],|r|r.get(0)).optional().map_err(db_error)?;
        let root_limits = if let Some(raw) = root_row {
            let v: serde_json::Value = decode(&raw.ok_or_else(limited)?)?;
            serde_json::from_value::<BudgetLimits>(v["limits"].clone()).map_err(|_| changed())?
        } else {
            BudgetLimits {
                attempts: grant.scope.max_attempts.min(project_limits.attempts),
                parallel: grant.scope.max_concurrency.min(project_limits.parallel),
                runtime_ms: grant
                    .scope
                    .max_duration_ms
                    .saturating_mul(u64::from(grant.scope.max_attempts))
                    .min(project_limits.runtime_ms),
                output_bytes: project_limits.output_bytes,
                children: project_limits.children,
                reworks: 0,
                tokens: None,
                cost_microunits: None,
            }
        };
        if project_limits.tokens.is_some()
            || project_limits.cost_microunits.is_some()
            || root_limits.tokens.is_some()
            || root_limits.cost_microunits.is_some()
        {
            return Err(CoreError::new("BUDGET_UNKNOWN","This Provider has no reliable token/cost meter; preserve the existing declared hard limit instead of silently bypassing it"));
        }
        let duration = grant
            .scope
            .max_duration_ms
            .min(root_limits.runtime_ms)
            .min(project_limits.runtime_ms)
            .min(
                Store::remaining_shared_runtime(db, &grant.root_grant_id, &session.project_id)?
                    .unwrap_or(u64::MAX),
            );
        if duration == 0 {
            return Err(limited());
        }
        let r = FactoryReservation {
            authority_kind: "legacy_shared_session".into(),
            scope_id: None,
            run_id: None,
            root_task_id: root_task_id.into(),
            root_grant_id: grant.root_grant_id.clone(),
            project_id: session.project_id.clone(),
            task_id: session.task_id.clone(),
            session_id: session.id.clone(),
            attempt_id: session.attempt_id.clone(),
            ancestors: vec![session.task_id.clone()],
            queued_ms: 0,
            reservation: AttemptReservation {
                runtime_ms: duration,
                output_bytes: 0,
                tokens: None,
                cost_microunits: None,
            },
            settled: false,
            actual_runtime_ms: None,
            actual_output_bytes: None,
            measurement_scope: "empty_selected_artifacts_no_validator_streams".into(),
            filesystem_total_bytes: None,
            captured_provider_message_bytes: None,
            provider_message_capture_complete: false,
        };
        Store::reserve_shared_execution_meter(db, &r, &root_limits, &project_limits)?;
        db.execute("INSERT INTO factory_budget_reservations(attempt_id,scope_id,root_task_id,project_id,document) VALUES(?1,NULL,?2,?3,?4)",params![r.attempt_id,r.root_task_id,r.project_id,encode(&r)?]).map_err(db_error)?;
        event(
            db,
            &session.id,
            session.revision,
            "shared_budget.session_reserved",
            serde_json::json!({"attempt_id":r.attempt_id,"root_task_id":r.root_task_id,"measurement_scope":r.measurement_scope,"filesystem_total_bytes":null,"reservation":r.reservation,"factory_authority":false}),
            "budget_core",
        )?;
        Ok(())
    }
}

impl Store {
    pub(crate) fn settle_shared_session(
        &mut self,
        id: &str,
        elapsed_ms: u64,
        stopped: bool,
        never_started: bool,
        capture: Option<&super::FactoryCapture>,
    ) -> Result<()> {
        let row:Option<Option<String>>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM factory_budget_reservations WHERE json_extract(document,'$.session_id')=?1",[id],|r|r.get(0)).optional().map_err(db_error)?;
        let Some(row) = row else { return Ok(()) };
        let mut record: FactoryReservation = decode(&row.ok_or_else(limited)?)?;
        if record.settled {
            return Ok(());
        }
        record.actual_runtime_ms = if stopped { Some(elapsed_ms) } else { None };
        record.actual_output_bytes = if never_started || record.scope_id.is_none() {
            Some(0)
        } else {
            capture
                .filter(|c| c.recheck().is_ok() && c.recheck_data(&self.conn).is_ok())
                .and_then(|c| c.observed_bytes)
        };
        record.captured_provider_message_bytes=Some(u64::try_from(self.conn.query_row("SELECT COALESCE(sum(octet_length(json_extract(document,'$.payload.update'))),0) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='session.update'",[id],|r|r.get::<_,i64>(0)).map_err(db_error)?).map_err(|_|changed())?);
        record.settled = stopped;
        let tx = self.conn.transaction().map_err(db_error)?;
        Store::settle_shared_execution_meter(&tx, &record)?;
        tx.execute(
            "UPDATE factory_budget_reservations SET document=?1 WHERE attempt_id=?2",
            params![encode(&record)?, record.attempt_id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            id,
            1,
            "shared_budget.session_settled",
            serde_json::json!({"attempt_id":record.attempt_id,"stopped":stopped,"never_started":never_started,"runtime_ms":record.actual_runtime_ms,"selected_artifact_bytes":record.actual_output_bytes,"measurement_scope":record.measurement_scope,"filesystem_total_bytes":null,"task_accepted":false}),
            "budget_core",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(())
    }
}

impl Store {
    pub(crate) fn shared_observation_within_reservation(&self, id: &str) -> Result<bool> {
        let row:Option<Option<String>>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM factory_budget_reservations WHERE json_extract(document,'$.session_id')=?1",[id],|r|r.get(0)).optional().map_err(db_error)?;
        let record: FactoryReservation = decode(&row.flatten().ok_or_else(changed)?)?;
        Ok(record.settled
            && record
                .actual_runtime_ms
                .is_some_and(|n| n <= record.reservation.runtime_ms)
            && record
                .actual_output_bytes
                .is_some_and(|n| n <= record.reservation.output_bytes))
    }
}
