//! Integration's own checked origin in the existing shared ledger. No Factory
//! reservation/session row, alternate quota or deserialized permission is made.
use super::{authority::*, records, *};
use crate::{
    execution::ExecutionBinding,
    storage::{db_error, decode, encode},
    Result,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IntegrationAttemptRecord {
    pub(super) view: IntegrationAttemptView,
    pub(super) integration_id: String,
    pub(super) policy_id: String,
    pub(super) policy_snapshot_digest: String,
    pub(super) root_task_id: String,
    pub(super) root_grant_id: String,
    pub(super) project_id: String,
    pub(super) task_id: String,
    pub(super) factory_scope_id: Option<String>,
    pub(super) ancestors: Vec<String>,
    pub(super) intent_id: String,
    pub(super) binding: ExecutionBinding,
    pub(super) prepared_at_ms: i64,
    pub(super) queued_ms: u64,
}
/// This checked projection cannot be deserialized or directly constructed by a
/// sibling module; its origin has already passed the stored Native/event chain.
pub(crate) struct IntegrationAccounting {
    pub(crate) attempt_id: String,
    pub(crate) integration_id: String,
    pub(crate) scope_id: Option<String>,
    pub(crate) root_task_id: String,
    pub(crate) root_grant_id: String,
    pub(crate) project_id: String,
    pub(crate) task_id: String,
    pub(crate) ancestors: Vec<String>,
    pub(crate) reservation: crate::goal_plan::AttemptReservation,
    pub(crate) settled: bool,
    pub(crate) actual_runtime_ms: Option<u64>,
    pub(crate) actual_output_bytes: Option<u64>,
    _checked: (),
}
fn unknown() -> CoreError {
    CoreError::new("BUDGET_UNKNOWN","Integration accounting origin, journal or settlement is incomplete; history was not discarded")
}
pub(super) fn original(mut r: IntegrationAttemptRecord) -> IntegrationAttemptRecord {
    r.view.state = "prepared".into();
    r.view.settled = false;
    r.view.actual_runtime_ms = None;
    r.view.actual_output_bytes = None;
    r.view.io_completion = None;
    r.view.last_error = None;
    r.view.receipt_sequence = None;
    r
}
pub(super) fn attempt(db: &Connection, id: &str) -> Result<IntegrationAttemptRecord> {
    let actual: IntegrationAttemptRecord = raw(db, "integration_attempts", id)?;
    let run = records::run(db, &actual.integration_id)?;
    let policy = records::policy(db, &actual.policy_id)?;
    let mirror:bool=db.query_row("SELECT integration_id=?2 AND round=?3 AND kind=?4 AND authority_kind='integration' AND root_task_id=?5 AND root_grant_id=?6 AND project_id=?7 AND state=?8 FROM integration_attempts WHERE attempt_id=?1",params![id,actual.integration_id,i64::from(actual.view.round),encode(&actual.view.kind)?.trim_matches('"'),actual.root_task_id,actual.root_grant_id,actual.project_id,actual.view.state],|r|r.get(0)).map_err(db_error)?;
    let native = policy.bindings.grant_lineage.first().ok_or_else(unknown)?;
    let expected_reservation = match actual.view.kind {
        IntegrationStageKind::Preparation => &policy.selection.stage_reservations.preparation,
        IntegrationStageKind::Cas => &policy.selection.stage_reservations.cas,
        IntegrationStageKind::Validator => {
            &policy
                .selection
                .stage_reservations
                .validators
                .iter()
                .find(|r| Some(&r.validator) == actual.view.validator.as_ref())
                .ok_or_else(unknown)?
                .reservation
        }
    };
    let original_digest = sha(&original(actual.clone()))?;
    let prepared:Option<i64>=db.query_row("SELECT sequence FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source')='integration_core' AND json_extract(document,'$.kind')='integration.attempt_prepared' AND json_extract(document,'$.payload.attempt_id')=?2 AND json_extract(document,'$.payload.origin_digest')=?3 ORDER BY sequence LIMIT 1",params![actual.integration_id,id,original_digest],|r|r.get(0)).optional().map_err(db_error)?;
    let db_binding: Option<(String, String, String)> = db
        .query_row(
            "SELECT task_id,root_task_id,binding FROM attempts WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(db_error)?;
    let intent: Option<(String, String, String)> = db
        .query_row(
            "SELECT attempt_id,state,document FROM dispatch_intents WHERE id=?1",
            [&actual.intent_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(db_error)?;
    let intent = intent.ok_or_else(unknown)?;
    let dispatch: crate::execution::DispatchIntent = decode(&intent.2)?;
    if !mirror
        || actual.view.attempt_id != id
        || actual.policy_id != run.policy_id
        || actual.task_id != run.task_id
        || actual.root_task_id != policy.bindings.root_task_id
        || actual.root_grant_id != policy.bindings.root_grant_id
        || actual.project_id != policy.bindings.project_id
        || actual.factory_scope_id != policy.bindings.factory_scope_id
        || actual.ancestors != policy.bindings.ancestors
        || actual.policy_snapshot_digest != policy.snapshot_digest
        || actual.view.round == 0
        || actual.view.round > policy.selection.max_rounds
        || actual.view.reservation != *expected_reservation
        || prepared.is_none()
        || actual.binding.policy_revision != format!("{}:{}", native.id, native.revision)
        || db_binding
            != Some((
                actual.task_id.clone(),
                actual.root_task_id.clone(),
                encode(&actual.binding)?,
            ))
        || dispatch.id != actual.intent_id
        || dispatch.attempt_id != id
        || dispatch.task_id != actual.task_id
        || dispatch.root_task_id != actual.root_task_id
        || dispatch.binding != actual.binding
        || intent.0 != id
        || encode(&dispatch.state)?.trim_matches('"') != intent.1
    {
        return Err(unknown());
    }
    if actual.view.settled {
        if actual.view.state != "finished"
            || !matches!(
                actual.view.io_completion,
                Some(IntegrationIoCompletion::WorkerJoined | IntegrationIoCompletion::NotScheduled)
            )
            || intent.1 != "stopped"
        {
            return Err(unknown());
        }
        if actual.view.io_completion == Some(IntegrationIoCompletion::NotScheduled)
            && (actual.view.actual_output_bytes != Some(0) || dispatch.executor_instance.is_some())
        {
            return Err(unknown());
        }
        let sequence = actual.view.receipt_sequence.ok_or_else(unknown)?;
        let mut settled = actual.clone();
        settled.view.receipt_sequence = None;
        let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE sequence=?1 AND sequence>?2 AND octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?3 AND json_extract(document,'$.source')='integration_core' AND json_extract(document,'$.kind')=?6 AND json_extract(document,'$.payload.attempt_id')=?4 AND json_extract(document,'$.payload.receipt_digest')=?5)",params![sql_num(sequence)?,prepared,actual.integration_id,id,sha(&settled)?,if actual.view.io_completion==Some(IntegrationIoCompletion::NotScheduled) {"integration.attempt_not_scheduled"}else{"integration.attempt_joined"}],|r|r.get(0)).map_err(db_error)?;
        if !valid {
            return Err(unknown());
        }
    } else if actual.view.state == "finished"
        || matches!(
            actual.view.io_completion,
            Some(IntegrationIoCompletion::WorkerJoined | IntegrationIoCompletion::NotScheduled)
        )
        || actual.view.receipt_sequence.is_some()
        || actual.view.actual_runtime_ms.is_some()
        || actual.view.actual_output_bytes.is_some()
        || intent.1 == "stopped"
    {
        return Err(unknown());
    }
    Ok(actual)
}
pub(crate) fn accounting_records(
    db: &Connection,
    root: &str,
    project: &str,
) -> Result<Vec<IntegrationAccounting>> {
    let version: u32 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(db_error)?;
    if version < 17 {
        let count:i64=db.query_row("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name IN('integration_policies','integrations','integration_attempts','integration_ref_fences')",[],|r|r.get(0)).map_err(db_error)?;
        if count == 0 {
            return Ok(vec![]);
        }
        return Err(unknown());
    }
    if version != 17 {
        return Err(unknown());
    }
    let(count,bytes,max):(i64,i64,i64)=db.query_row("SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(attempt_id)),0),COALESCE(max(octet_length(document)),0) FROM integration_attempts",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(db_error)?;
    if !(0..=100000).contains(&count)
        || !(0..=4194304).contains(&bytes)
        || !(0..=1048576).contains(&max)
    {
        return Err(limited());
    }
    let mut statement = db
        .prepare("SELECT attempt_id FROM integration_attempts ORDER BY attempt_id")
        .map_err(db_error)?;
    let ids = statement
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    let mut result = vec![];
    for id in ids {
        let r = attempt(db, &id)?;
        if r.project_id == project || r.root_grant_id == root {
            result.push(IntegrationAccounting {
                attempt_id: id,
                integration_id: r.integration_id,
                scope_id: r.factory_scope_id,
                root_task_id: r.root_task_id,
                root_grant_id: r.root_grant_id,
                project_id: r.project_id,
                task_id: r.task_id,
                ancestors: r.ancestors,
                reservation: r.view.reservation,
                settled: r.view.settled,
                actual_runtime_ms: r.view.actual_runtime_ms,
                actual_output_bytes: r.view.actual_output_bytes,
                _checked: (),
            });
        }
    }
    Ok(result)
}
