//! Durable acceptance watcher and private stage admissions. Nothing here turns
//! a Goal acceptance or a serialized pass flag into branch permission.
use super::{
    authority::*,
    budget::{self, IntegrationAttemptRecord},
    records, *,
};
use crate::{
    execution::{DispatchIntent, ExecutionBinding, IntentState},
    registry::ManagedWorkspace,
    storage::{db_error, decode, encode, now, Store},
    Result,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RefFence {
    pub(super) repository_digest: String,
    pub(super) target_ref: String,
    pub(super) integration_id: String,
    pub(super) revision: u64,
    pub(super) state: String,
    pub(super) common_dir: PathBuf,
    pub(super) common_device: String,
    pub(super) common_inode: String,
    pub(super) source_workspace_id: String,
    pub(super) active_attempt_id: Option<String>,
    pub(super) receipt_sequence: Option<u64>,
}
#[derive(Clone)]
pub(crate) struct IntegrationStageTicket {
    pub(super) policy: IntegrationPolicy,
    pub(super) run: IntegrationRun,
    pub(super) attempt: IntegrationAttemptRecord,
    pub(super) acceptance: crate::goal_store::GraphTaskAcceptance,
    pub(super) control: ManagedWorkspace,
    pub(super) candidate: ManagedWorkspace,
    pub(super) managed_root: PathBuf,
    pub(super) queued_at: Instant,
    pub(super) deadline: Instant,
}
/// Created only from an already admitted private stage ticket. It is never a
/// wire input, deserializable capability, or a replacement for live DB checks.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct IntegrationStageReadRef {
    pub(super) run_id: String,
    pub(super) attempt_id: String,
    pub(super) policy_id: String,
}
impl IntegrationStageTicket {
    pub(super) fn read_reference(&self) -> IntegrationStageReadRef {
        IntegrationStageReadRef {
            run_id: self.run.id.clone(),
            attempt_id: self.attempt.view.attempt_id.clone(),
            policy_id: self.policy.id.clone(),
        }
    }
}
pub(crate) struct IntegrationQueueResult {
    pub(crate) view: IntegrationRunView,
    pub(crate) ticket: Option<IntegrationStageTicket>,
}
fn terminal(state: IntegrationState) -> bool {
    matches!(
        state,
        IntegrationState::ConfirmedLocalRef
            | IntegrationState::Blocked
            | IntegrationState::StaleTarget
            | IntegrationState::Failed
            | IntegrationState::Cancelled
    )
}
fn run_ids(db: &Connection) -> Result<Vec<String>> {
    let(count,bytes):(i64,i64)=db.query_row("SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)),0) FROM integrations",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    if !(0..=100000).contains(&count) || !(0..=4194304).contains(&bytes) {
        return Err(limited());
    }
    let mut statement = db
        .prepare("SELECT id FROM integrations ORDER BY id")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    Ok(rows)
}
fn attempts(db: &Connection, run: &str) -> Result<Vec<IntegrationAttemptRecord>> {
    let(count,bytes):(i64,i64)=db.query_row("SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(attempt_id)),0) FROM integration_attempts WHERE integration_id=?1",[run],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    if !(0..=528).contains(&count) || !(0..=1048576).contains(&bytes) {
        return Err(limited());
    }
    let mut statement=db.prepare("SELECT attempt_id FROM integration_attempts WHERE integration_id=?1 ORDER BY round,rowid").map_err(db_error)?;
    let ids = statement
        .query_map([run], |r| r.get::<_, String>(0))
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    ids.iter().map(|id| budget::attempt(db, id)).collect()
}
pub(super) fn fence(db: &Connection, run: &IntegrationRun) -> Result<RefFence> {
    let p = records::policy(db, &run.policy_id)?;
    let row:Option<(String,i64,String,Option<String>)>=db.query_row("SELECT integration_id,revision,state,CASE WHEN octet_length(document)<=1048576 THEN document END FROM integration_ref_fences WHERE repository_digest=?1 AND target_ref=?2",params![p.bindings.repository_digest,run.target_ref],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(db_error)?;
    let Some((id, revision, state, text)) = row else {
        if terminal(run.state) {
            return historical_fence(db, run, &p);
        }
        return Err(changed());
    };
    if id != run.id {
        if terminal(run.state) {
            return historical_fence(db, run, &p);
        }
        return Err(changed());
    }
    let f: RefFence = decode(&text.ok_or_else(limited)?)?;
    let seq = f.receipt_sequence.ok_or_else(changed)?;
    let mut image = f.clone();
    image.receipt_sequence = None;
    let receipt:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE sequence=?1 AND octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?2 AND json_extract(document,'$.source')='integration_core' AND json_extract(document,'$.kind') IN('integration.fence_prepared','integration.fence_updated','integration.fence_released') AND json_extract(document,'$.payload.fence_digest')=?3)",params![sql_num(seq)?,run.id,sha(&image)?],|r|r.get(0)).map_err(db_error)?;
    if !receipt
        || f.integration_id != run.id
        || f.repository_digest != p.bindings.repository_digest
        || f.target_ref != run.target_ref
        || f.revision != u64::try_from(revision).map_err(|_| changed())?
        || f.state != state
        || f.common_dir != p.bindings.source.common_dir
        || f.common_device != p.bindings.source.common_device
        || f.common_inode != p.bindings.source.common_inode
        || f.source_workspace_id != p.bindings.workspace_id
    {
        return Err(changed());
    }
    Ok(f)
}
fn historical_fence(
    db: &Connection,
    run: &IntegrationRun,
    p: &IntegrationPolicy,
) -> Result<RefFence> {
    let row:Option<(i64,Option<String>)>=db.query_row("SELECT sequence,CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source')='integration_core' AND json_extract(document,'$.kind')='integration.fence_released' ORDER BY sequence DESC LIMIT 1",[&run.id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
    let (sequence, text) = row.ok_or_else(changed)?;
    let event: crate::storage::Event = decode(&text.ok_or_else(limited)?)?;
    let mut original: RefFence =
        serde_json::from_value(event.payload["fence"].clone()).map_err(|_| changed())?;
    if original.receipt_sequence.is_some()
        || original.integration_id != run.id
        || original.repository_digest != p.bindings.repository_digest
        || original.target_ref != run.target_ref
        || original.revision != event.revision
        || original.state != "released"
        || original.active_attempt_id.is_some()
        || original.common_dir != p.bindings.source.common_dir
        || original.common_device != p.bindings.source.common_device
        || original.common_inode != p.bindings.source.common_inode
        || original.source_workspace_id != p.bindings.workspace_id
        || event.payload["fence_digest"] != sha(&original)?
    {
        return Err(changed());
    }
    original.receipt_sequence = Some(u64::try_from(sequence).map_err(|_| changed())?);
    Ok(original)
}
pub(super) fn write_fence(db: &Connection, f: &mut RefFence, kind: &str) -> Result<()> {
    // Validate old rows before an UPSERT can hide corrupted mirror state.
    let _ = super::fences::checked_fences(db)?;
    f.receipt_sequence = None;
    let seq = records::event(
        db,
        &f.integration_id,
        f.revision,
        kind,
        &json!({"fence_digest":sha(f)?,"state":f.state,"active_attempt_id":f.active_attempt_id,"fence":f}),
    )?;
    f.receipt_sequence = Some(seq);
    db.execute("INSERT INTO integration_ref_fences(repository_digest,target_ref,integration_id,revision,state,document) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(repository_digest,target_ref) DO UPDATE SET integration_id=excluded.integration_id,revision=excluded.revision,state=excluded.state,document=excluded.document",params![f.repository_digest,f.target_ref,f.integration_id,sql_num(f.revision)?,f.state,encode(f)?]).map_err(db_error)?;
    Ok(())
}
fn live_policy(store: &Store, p: &IntegrationPolicy) -> Result<IntegrationScopeMaterial> {
    if p.revoked || p.expires_at_ms <= ms()? {
        return Err(denied());
    }
    let material =
        store.integration_scope_material(&p.selection.goal_scope_id, &p.selection.task_id)?;
    if material.task.spec_revision != p.selection.expected_spec_revision
        || material.task.goal != p.bindings.task_goal
        || material.scope_digest != p.bindings.goal_scope_digest
        || material.scope_revision != p.bindings.goal_scope_revision
        || material.project.id != p.bindings.project_id
        || material.project.revision != p.bindings.project_revision
        || material.workspace.id != p.bindings.workspace_id
        || material.workspace.revision != p.bindings.workspace_revision
        || material.grant.root_grant_id != p.bindings.root_grant_id
    {
        return Err(changed());
    }
    Ok(material)
}
fn source_idle(db: &Connection, p: &IntegrationPolicy) -> Result<()> {
    let active:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM dispatch_intents d JOIN attempts a ON a.id=d.attempt_id WHERE d.state!='stopped' AND (json_extract(a.binding,'$.workspace_id')=?1 OR a.root_task_id=?2)) OR EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id=?1) OR EXISTS(SELECT 1 FROM goal_graph_jobs WHERE workspace_id=?1 AND state IN('prepared','running','cancel_requested','outcome_unknown')) OR EXISTS(SELECT 1 FROM workspace_leases WHERE resource=?1 AND holder!='')",params![p.bindings.workspace_id,p.bindings.root_task_id],|r|r.get(0)).map_err(db_error)?;
    if active {
        return Err(CoreError::new("WORKSPACE_BUSY","Integration source still has an owned writer, unresolved intent or read fence; lease expiry does not confirm stop"));
    }
    Ok(())
}
fn derive_binding(p: &IntegrationPolicy, workspace: &str, head: &str) -> Result<ExecutionBinding> {
    let grant = p.bindings.grant_lineage.first().ok_or_else(denied)?;
    let (id, connection) = grant.connection_bindings.iter().next().ok_or_else(denied)?;
    Ok(ExecutionBinding {
        connection_id: id.clone(),
        environment_id: connection.environment_id.clone(),
        identity_ref: connection.identity_ref.clone(),
        host_id: connection.host_id.clone(),
        workspace_id: workspace.into(),
        config_revision: connection.config_revision.clone(),
        input_revision: head.into(),
        capability_snapshot_id: connection.capability_snapshot_id.clone(),
        policy_revision: format!("{}:{}", grant.id, grant.revision),
    })
}
fn reserve_counters(db: &Connection, p: &IntegrationPolicy) -> Result<()> {
    crate::policy::validate_ancestor_grant_budgets_from(db, &p.bindings.root_grant_id)?;
    for key in [
        format!("project:{}", p.bindings.project_id),
        format!("root:{}", p.bindings.root_task_id),
    ] {
        if db.execute("UPDATE budgets SET consumed=consumed+1,active=active+1 WHERE scope=?1 AND consumed<total_limit AND active<parallel_limit",[key]).map_err(db_error)?!=1{return Err(limited());}
    }
    Ok(())
}
fn reserve_ancestors(
    db: &Connection,
    p: &IntegrationPolicy,
    reservation: &crate::goal_plan::AttemptReservation,
) -> Result<()> {
    let Some(scope_id) = &p.bindings.factory_scope_id else {
        return Ok(());
    };
    let scope = crate::factory::checked_child_goal_scope(db, scope_id)?;
    let all =
        crate::factory::accounting_records(db, &p.bindings.root_grant_id, &p.bindings.project_id)?;
    for (ancestor, budget) in
        crate::factory::child_goal_ancestor_budgets(db, &scope, &p.selection.task_id)?
    {
        let relevant = all
            .iter()
            .filter(|r| {
                r.scope_id.as_deref() == Some(scope_id.as_str()) && r.ancestors.contains(&ancestor)
            })
            .collect::<Vec<_>>();
        if relevant.len() >= budget.attempts as usize
            || relevant.iter().filter(|r| !r.settled).count() >= budget.parallel as usize
        {
            return Err(limited());
        }
        let mut runtime = reservation.runtime_ms;
        let mut output = reservation.output_bytes;
        for old in relevant {
            runtime = runtime
                .checked_add(if old.settled {
                    old.actual_runtime_ms.ok_or_else(|| {
                        CoreError::new(
                            "BUDGET_UNKNOWN",
                            "An integration Factory ancestor has unknown runtime",
                        )
                    })?
                } else {
                    old.reservation.runtime_ms
                })
                .ok_or_else(limited)?;
            output = output
                .checked_add(if old.settled {
                    old.actual_output_bytes.ok_or_else(|| {
                        CoreError::new(
                            "BUDGET_UNKNOWN",
                            "An integration Factory ancestor has unknown selected output",
                        )
                    })?
                } else {
                    old.reservation.output_bytes
                })
                .ok_or_else(limited)?;
        }
        if runtime > budget.runtime_ms || output > budget.output_bytes {
            return Err(limited());
        }
    }
    Ok(())
}
impl Store {
    pub fn get_integration(&self, id: &str) -> Result<IntegrationRunView> {
        let run = records::run(&self.conn, id)?;
        let p = records::policy(&self.conn, &run.policy_id)?;
        let attempts = attempts(&self.conn, id)?
            .into_iter()
            .map(|r| r.view)
            .collect::<Vec<_>>();
        let current = fence(&self.conn, &run)?;
        let view = IntegrationRunView {
            run,
            attempts,
            shared_usage: self.integration_historical_usage(&p)?,
            fence: IntegrationFenceView {
                repository_digest: current.repository_digest,
                target_ref: current.target_ref,
                integration_id: current.integration_id,
                revision: current.revision,
                state: current.state,
            },
        };
        if encode(&view)?.len() > MAX_INTEGRATION_BYTES {
            return Err(limited());
        }
        Ok(view)
    }
    pub fn cancel_integration(
        &mut self,
        id: &str,
        revision: u64,
        key_id: &str,
    ) -> Result<IntegrationRunView> {
        let payload = encode(&json!({"id":id,"expected_revision":revision}))?;
        let scope = format!("integration:{id}:cancel");
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) = records::replay(&tx, &scope, key_id, &payload)? {
            return Ok(old);
        }
        let mut run = records::run(&tx, id)?;
        if run.revision != revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Integration changed before cancellation",
            ));
        }
        if !run.cancel_requested {
            run.cancel_requested = true;
            if !terminal(run.state) && run.state != IntegrationState::OutcomeUnknown {
                run.state = IntegrationState::CancelRequested;
            }
            records::save_run(
                &tx,
                &mut run,
                "integration.cancel_requested",
                json!({"owned_workers_require_join":true,"new_dispatch":false}),
            )?;
        }
        let view = self.get_integration(id)?;
        records::remember(&tx, &scope, key_id, &payload, &view)?;
        records::check_metadata_bounds(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(view)
    }
    pub fn reconcile_integration(
        &mut self,
        id: &str,
        revision: u64,
        key_id: &str,
    ) -> Result<IntegrationReconciliation> {
        let payload = encode(&json!({"id":id,"expected_revision":revision}))?;
        let scope = format!("integration:{id}:reconcile");
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) = records::replay(&tx, &scope, key_id, &payload)? {
            return Ok(old);
        }
        let integration = self.get_integration(id)?;
        if integration.run.revision != revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Integration changed before reconciliation",
            ));
        }
        let historical = historical_cas_confirmed(&tx, &integration.run)?;
        let result = IntegrationReconciliation {
            integration,
            observed_target_head: None,
            observation_scope: "recorded_private_intent_and_owned_receipts_only_no_ref_replay"
                .into(),
            historical_cas_confirmed: historical,
        };
        records::remember(&tx, &scope, key_id, &payload, &result)?;
        records::check_metadata_bounds(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
    pub(crate) fn queue_integration_start(
        &mut self,
        policy_id: &str,
        revision: u64,
        key_id: &str,
        managed_root: &Path,
        queued_at: Instant,
    ) -> Result<IntegrationQueueResult> {
        let payload = encode(&json!({"policy_id":policy_id,"expected_revision":revision}))?;
        let scope = format!("integration:{policy_id}:start");
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(view) = records::replay(&tx, &scope, key_id, &payload)? {
            return Ok(IntegrationQueueResult { view, ticket: None });
        }
        let p = records::policy(&tx, policy_id)?;
        if p.revision != revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Integration policy changed before start",
            ));
        }
        let existing:Option<String>=tx.query_row("SELECT id FROM integrations WHERE policy_id=?1 AND task_id=?2 AND spec_revision=?3",params![p.id,p.selection.task_id,sql_num(p.selection.expected_spec_revision)?],|r|r.get(0)).optional().map_err(db_error)?;
        if let Some(id) = existing {
            let view = self.get_integration(&id)?;
            records::remember(&tx, &scope, key_id, &payload, &view)?;
            records::check_metadata_bounds(&tx)?;
            tx.commit().map_err(db_error)?;
            return Ok(IntegrationQueueResult { view, ticket: None });
        }
        let material = live_policy(self, &p)?;
        let acceptance=self.integration_verified_acceptance(&p.selection.goal_scope_id,&p.selection.task_id)?.ok_or_else(||CoreError::new("AUTH_REQUIRED","Integration remains armed until the exact Task has an authentic complete acceptance"))?;
        if acceptance.spec_revision != p.selection.expected_spec_revision
            || acceptance.workspace_id != p.bindings.workspace_id
            || acceptance.input_revision != p.bindings.source.head
        {
            return Err(changed());
        }
        source_idle(&tx, &p)?;
        super::fences::check_workspace_fences(
            &tx,
            material.workspace.identity.as_ref().ok_or_else(changed)?,
            None,
        )?;
        let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM integration_ref_fences WHERE repository_digest=?1 AND state IN('held','unknown'))",params![p.bindings.repository_digest],|r|r.get(0)).map_err(db_error)?;
        if occupied {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "The integration repository/ref remains fenced by an owned or unknown prior intent",
            ));
        }
        let run_id = Uuid::new_v4().to_string();
        let attempt_id = Uuid::new_v4().to_string();
        let intent_id = Uuid::new_v4().to_string();
        let stamp = now()?;
        let run = IntegrationRun {
            id: run_id.clone(),
            policy_id: p.id.clone(),
            policy_revision: p.revision,
            revision: 1,
            task_id: p.selection.task_id.clone(),
            spec_revision: p.selection.expected_spec_revision,
            goal_scope_id: p.selection.goal_scope_id.clone(),
            scope_digest: p.bindings.goal_scope_digest.clone(),
            acceptance_ref: acceptance.id.clone(),
            acceptance_digest: sha(&acceptance)?,
            source_commit: None,
            source_tree: None,
            target_ref: p.selection.target_ref.clone(),
            expected_target_head: p.selection.expected_target_head.clone(),
            integration_commit: None,
            integration_tree: None,
            round: 1,
            state: IntegrationState::Queued,
            cancel_requested: false,
            active_stage: Some(IntegrationStageKind::Preparation),
            io_completion: None,
            validation_receipts: vec![],
            last_error: None,
            created_at: stamp.clone(),
            updated_at: stamp.clone(),
        };
        let reservation = p.selection.stage_reservations.preparation.clone();
        reserve_ancestors(&tx, &p, &reservation)?;
        reserve_counters(&tx, &p)?;
        let binding = derive_binding(&p, &p.bindings.workspace_id, &p.bindings.source.head)?;
        let dispatch = DispatchIntent {
            id: intent_id.clone(),
            attempt_id: attempt_id.clone(),
            task_id: run.task_id.clone(),
            root_task_id: p.bindings.root_task_id.clone(),
            state: IntentState::Prepared,
            executor_instance: None,
            binding: binding.clone(),
        };
        tx.execute("INSERT INTO integrations(id,policy_id,task_id,spec_revision,state,revision,document) VALUES(?1,?2,?3,?4,'queued',1,?5)",params![run.id,p.id,run.task_id,sql_num(run.spec_revision)?,encode(&run)?]).map_err(db_error)?;
        records::event(
            &tx,
            &run.id,
            1,
            "integration.queued",
            &json!({"acceptance_ref":run.acceptance_ref,"acceptance_digest":run.acceptance_digest,"policy_id":p.id,"source_capture_started":false}),
        )?;
        tx.execute("INSERT INTO attempts(id,task_id,root_task_id,binding,idempotency_key) VALUES(?1,?2,?3,?4,?5)",params![attempt_id,run.task_id,p.bindings.root_task_id,encode(&binding)?,format!("integration:{}:1:preparation",run.id)]).map_err(db_error)?;
        tx.execute("INSERT INTO dispatch_intents(id,attempt_id,state,executor_instance,document) VALUES(?1,?2,'prepared',NULL,?3)",params![intent_id,attempt_id,encode(&dispatch)?]).map_err(db_error)?;
        let attempt = IntegrationAttemptRecord {
            view: IntegrationAttemptView {
                attempt_id: attempt_id.clone(),
                round: 1,
                kind: IntegrationStageKind::Preparation,
                validator: None,
                state: "prepared".into(),
                reservation: reservation.clone(),
                settled: false,
                actual_runtime_ms: None,
                actual_output_bytes: None,
                io_completion: None,
                last_error: None,
                receipt_sequence: None,
            },
            integration_id: run.id.clone(),
            policy_id: p.id.clone(),
            policy_snapshot_digest: p.snapshot_digest.clone(),
            root_task_id: p.bindings.root_task_id.clone(),
            root_grant_id: p.bindings.root_grant_id.clone(),
            project_id: p.bindings.project_id.clone(),
            task_id: run.task_id.clone(),
            factory_scope_id: p.bindings.factory_scope_id.clone(),
            ancestors: p.bindings.ancestors.clone(),
            intent_id,
            binding,
            prepared_at_ms: ms()?,
            queued_ms: u64::try_from(queued_at.elapsed().as_millis()).map_err(|_| limited())?,
        };
        tx.execute("INSERT INTO integration_attempts(attempt_id,integration_id,round,kind,authority_kind,root_task_id,root_grant_id,project_id,state,document) VALUES(?1,?2,1,'preparation','integration',?3,?4,?5,'prepared',?6)",params![attempt_id,run.id,p.bindings.root_task_id,p.bindings.root_grant_id,p.bindings.project_id,encode(&attempt)?]).map_err(db_error)?;
        records::event(
            &tx,
            &run.id,
            1,
            "integration.attempt_prepared",
            &json!({"attempt_id":attempt_id,"origin_digest":sha(&budget::original(attempt.clone()))?}),
        )?;
        let checked =
            budget::accounting_records(&tx, &p.bindings.root_grant_id, &p.bindings.project_id)?
                .into_iter()
                .find(|r| r.attempt_id == attempt_id)
                .ok_or_else(changed)?;
        Store::reserve_integration_execution_meter(
            &tx,
            &checked,
            &p.bindings.root_budget,
            &p.bindings.project_budget,
        )?;
        let mut ref_fence = RefFence {
            repository_digest: p.bindings.repository_digest.clone(),
            target_ref: p.selection.target_ref.clone(),
            integration_id: run.id.clone(),
            revision: 1,
            state: "held".into(),
            common_dir: p.bindings.source.common_dir.clone(),
            common_device: p.bindings.source.common_device.clone(),
            common_inode: p.bindings.source.common_inode.clone(),
            source_workspace_id: p.bindings.workspace_id.clone(),
            active_attempt_id: Some(attempt_id),
            receipt_sequence: None,
        };
        write_fence(&tx, &mut ref_fence, "integration.fence_prepared")?;
        let control = ManagedWorkspace {
            id: Uuid::new_v4().to_string(),
            project_id: p.bindings.project_id.clone(),
            identity: None,
            root: managed_root.join(format!("integration-{}-r1-control", run.id)),
            base_revision: p.bindings.source.head.clone(),
            created_at: stamp.clone(),
            state: "creating".into(),
            revision: 1,
        };
        let candidate = ManagedWorkspace {
            id: Uuid::new_v4().to_string(),
            project_id: p.bindings.project_id.clone(),
            identity: None,
            root: managed_root.join(format!("integration-{}-r1-candidate", run.id)),
            base_revision: p.selection.expected_target_head.clone(),
            created_at: stamp,
            state: "creating".into(),
            revision: 1,
        };
        // Keep the original monotonic clock, including request queuing and all
        // subsequent read/hash/capture/merge/cleanup work in this same attempt.
        let deadline = queued_at + Duration::from_millis(reservation.runtime_ms);
        let expiry = p.expires_at_ms.min(material.expires_at_ms).min(
            material
                .grant_lineage
                .iter()
                .map(|g| g.expires_at_ms)
                .min()
                .ok_or_else(denied)?,
        );
        let until_expiry = Duration::from_millis(
            u64::try_from(expiry.checked_sub(ms()?).ok_or_else(denied)?).map_err(|_| denied())?,
        );
        let deadline = deadline.min(Instant::now() + until_expiry);
        if Instant::now() >= deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Original integration preparation reservation expired before its durable admission",
            ));
        }
        let ticket = IntegrationStageTicket {
            policy: p.clone(),
            run: run.clone(),
            attempt: attempt.clone(),
            acceptance,
            control: control.clone(),
            candidate: candidate.clone(),
            managed_root: managed_root.into(),
            queued_at,
            deadline,
        };
        let permit = super::fences::for_stage(&tx, &ticket)?;
        for workspace in [&control, &candidate] {
            crate::registry::reserve_workspace_row_for_integration(
                &tx,
                workspace,
                &format!("integration:{}:{}", run.id, workspace.id),
                &encode(
                    &json!({"integration_id":run.id,"policy_id":p.id,"role":if workspace.id==control.id {"control"}else{"candidate"}}),
                )?,
                &permit,
            )?;
        }
        let view = self.get_integration(&run.id)?;
        records::remember(&tx, &scope, key_id, &payload, &view)?;
        records::check_metadata_bounds(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(IntegrationQueueResult {
            view,
            ticket: Some(ticket),
        })
    }
    /// Called before generic dispatch recovery: integration uncertainty never
    /// rewrites the already accepted Task into a Provider dispatch outcome.
    pub(crate) fn recover_integrations(&mut self) -> Result<()> {
        // Validate every origin even if it has no active run. No orphan policy,
        // reservation or fence is treated as absent/zero merely by filtering.
        let (count,bytes):(i64,i64)=self.conn.query_row("SELECT count(*),COALESCE(sum(octet_length(document)+octet_length(id)),0) FROM integration_policies",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        if !(0..=100000).contains(&count) || !(0..=4194304).contains(&bytes) {
            return Err(limited());
        }
        let policies = {
            let mut q = self
                .conn
                .prepare("SELECT id FROM integration_policies ORDER BY id")
                .map_err(db_error)?;
            let rows = q
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(db_error)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_error)?;
            rows
        };
        for id in policies {
            records::policy(&self.conn, &id)?;
        }
        let _ = budget::accounting_records(&self.conn, "", "")?;
        let (count,bytes):(i64,i64)=self.conn.query_row("SELECT count(*),COALESCE(sum(octet_length(document)),0) FROM integration_ref_fences",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        if !(0..=100000).contains(&count) || !(0..=4194304).contains(&bytes) {
            return Err(limited());
        }
        let fenced = {
            let mut q=self.conn.prepare("SELECT integration_id FROM integration_ref_fences ORDER BY repository_digest,target_ref").map_err(db_error)?;
            let rows = q
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(db_error)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_error)?;
            rows
        };
        for id in fenced {
            let run = records::run(&self.conn, &id)?;
            fence(&self.conn, &run)?;
        }
        let ids = run_ids(&self.conn)?;
        for id in ids {
            let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let mut run = records::run(&tx, &id)?;
            let mut rows = attempts(&tx, &id)?;
            if !terminal(run.state)
                && !(run.state == IntegrationState::OutcomeUnknown
                    && run.io_completion == Some(IntegrationIoCompletion::InterruptedNoJoinReceipt))
            {
                let mut held = fence(&tx, &run)?;
                for row in rows.iter_mut().filter(|r| !r.view.settled) {
                    let mut dispatch: DispatchIntent = decode(
                        &tx.query_row(
                            "SELECT document FROM dispatch_intents WHERE id=?1",
                            [&row.intent_id],
                            |r| r.get::<_, String>(0),
                        )
                        .map_err(db_error)?,
                    )?;
                    dispatch.state = IntentState::OutcomeUnknown;
                    tx.execute("UPDATE dispatch_intents SET state='outcome_unknown',document=?1 WHERE id=?2",params![encode(&dispatch)?,dispatch.id]).map_err(db_error)?;
                    row.view.state = "outcome_unknown".into();
                    row.view.io_completion =
                        Some(IntegrationIoCompletion::InterruptedNoJoinReceipt);
                    tx.execute("UPDATE integration_attempts SET state='outcome_unknown',document=?1 WHERE attempt_id=?2",params![encode(row)?,row.view.attempt_id]).map_err(db_error)?;
                }
                held.state = "unknown".into();
                held.revision = held.revision.checked_add(1).ok_or_else(limited)?;
                write_fence(&tx, &mut held, "integration.fence_updated")?;
                run.state = IntegrationState::OutcomeUnknown;
                run.io_completion = Some(IntegrationIoCompletion::InterruptedNoJoinReceipt);
                let mut error=CoreError::new("OUTCOME_UNKNOWN","Daemon restart has no actual worker join/CAS receipt; retain the original fence and do not replay");
                error.side_effect_state = "unknown".into();
                run.last_error = Some(error);
                records::save_run(
                    &tx,
                    &mut run,
                    "integration.interrupted",
                    json!({"replayed":false,"task_acceptance_unchanged":true,"resources_released":false}),
                )?;
            }
            records::check_metadata_bounds(&tx)?;
            tx.commit().map_err(db_error)?;
        }
        Ok(())
    }
}

impl Store {
    pub(crate) fn admit_integration_stage_effect(
        &mut self,
        ticket: &IntegrationStageTicket,
        mut proof: super::reads::IntegrationReadProof,
        raw: &str,
    ) -> Result<super::reads::IntegrationReadProof> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if !matches!(proof.intent(),super::reads::IntegrationReadIntent::Stage(reference) if reference==&ticket.read_reference())
        {
            return Err(denied());
        }
        proof.verify(self, raw, ticket.deadline)?;
        let policy = records::policy(&tx, &ticket.policy.id)?;
        let _ = live_policy(self, &policy)?;
        let mut run = records::run(&tx, &ticket.run.id)?;
        let mut record = budget::attempt(&tx, &ticket.attempt.view.attempt_id)?;
        let current = fence(&tx, &run)?;
        if run.cancel_requested
            || run.state == IntegrationState::OutcomeUnknown
            || record.view.state != "prepared"
            || record.view.settled
            || current.state != "held"
            || current.active_attempt_id.as_deref() != Some(record.view.attempt_id.as_str())
            || run.active_stage != Some(record.view.kind)
        {
            return Err(changed());
        }
        let mut intent: DispatchIntent = raw_dispatch(&tx, &record.intent_id)?;
        if intent.state != IntentState::Prepared {
            return Err(changed());
        }
        if proof.observed_target_advance().is_some() {
            // The old reservation pays for its actual readonly preflight. No
            // stage effects or replacement work are released by this receipt.
            proof.seal_observation()?;
            tx.commit().map_err(db_error)?;
            return Ok(proof);
        }
        record.view.state = "running".into();
        if tx.execute("UPDATE integration_attempts SET state='running',document=?1 WHERE attempt_id=?2 AND state='prepared'",params![encode(&record)?,record.view.attempt_id]).map_err(db_error)? != 1 { return Err(changed()); }
        intent.state = IntentState::Sent;
        intent.executor_instance = Some(format!("integration:{}", record.view.attempt_id));
        if tx.execute("UPDATE dispatch_intents SET state='sent',executor_instance=?1,document=?2 WHERE id=?3 AND state='prepared'",params![intent.executor_instance,encode(&intent)?,record.intent_id]).map_err(db_error)?!=1 { return Err(changed()); }
        run.state = match record.view.kind {
            IntegrationStageKind::Preparation => IntegrationState::Capturing,
            IntegrationStageKind::Validator => IntegrationState::Validating,
            IntegrationStageKind::Cas => IntegrationState::CasPrepared,
        };
        records::save_run(
            &tx,
            &mut run,
            "integration.stage_dispatched",
            json!({"attempt_id":record.view.attempt_id,"kind":record.view.kind,"new_provider_dispatch":false,"original_deadline_retained":true}),
        )?;
        proof.reseal_stage_after_own_transition(self, ticket, raw)?;
        records::check_metadata_bounds(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(proof)
    }
}
fn raw_dispatch(db: &Connection, id: &str) -> Result<DispatchIntent> {
    raw(db, "dispatch_intents", id)
}

fn historical_cas_confirmed(db: &Connection, run: &IntegrationRun) -> Result<bool> {
    let policy = records::policy(db, &run.policy_id)?;
    let mut statement=db.prepare("SELECT sequence,CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source')='integration_core' AND json_extract(document,'$.kind')='integration.cas_observed' AND json_type(document,'$.payload.historical_cas_confirmed')='true' ORDER BY sequence LIMIT 2").map_err(db_error)?;
    let events = statement
        .query_map([&run.id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
        })
        .map_err(db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db_error)?;
    if events.len() > 1 {
        return Err(changed());
    }
    let Some((sequence, text)) = events.into_iter().next() else {
        if run.state == IntegrationState::ConfirmedLocalRef {
            return Err(changed());
        }
        return Ok(false);
    };
    let event: crate::storage::Event = crate::storage::decode(&text.ok_or_else(limited)?)?;
    let attempt_id = event.payload["attempt_id"].as_str().ok_or_else(changed)?;
    let attempt = budget::attempt(db, attempt_id)?;
    let record: &serde_json::Value = &event.payload["record"];
    if attempt.integration_id != run.id
        || attempt.view.kind != IntegrationStageKind::Cas
        || attempt.view.round != run.round
        || event.revision > run.revision
        || event.payload["round"] != run.round
        || event.payload["historical_cas_confirmed"] != true
        || record["state"] != "confirmed_local_ref"
        || record["grant_ref"] != policy.approval_id
        || record["target_ref"] != run.target_ref
        || record["expected_target_head"] != run.expected_target_head
        || record["candidate_commit"].as_str() != run.integration_commit.as_deref()
        || sequence <= 0
    {
        return Err(changed());
    }
    Ok(true)
}

impl Store {
    /// Only the manager retaining the actual merged-candidate handles calls
    /// this after consuming the preceding real join/settlement. No wire input
    /// can reconstruct that handle or turn a stored pass into continuation.
    pub(super) fn queue_integration_next_stage(
        &mut self,
        previous: &IntegrationStageTicket,
        context: Option<&super::io::IntegrationPreparedContext>,
        kind: IntegrationStageKind,
        validator: Option<crate::goal_store::GoalValidatorRef>,
    ) -> Result<IntegrationStageTicket> {
        let queued_at = Instant::now();
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let p = records::policy(&tx, &previous.policy.id)?;
        let material = live_policy(self, &p)?;
        let mut run = records::run(&tx, &previous.run.id)?;
        let prior = budget::attempt(&tx, &previous.attempt.view.attempt_id)?;
        let mut ref_fence = fence(&tx, &run)?;
        let _ = super::fences::checked_fences(&tx)?;
        if !prior.view.settled
            || prior.view.io_completion != Some(IntegrationIoCompletion::WorkerJoined)
            || prior.view.actual_runtime_ms.is_none()
            || prior.view.actual_output_bytes.is_none()
            || run.cancel_requested
            || terminal(run.state)
            || run.state == IntegrationState::OutcomeUnknown
            || run.active_stage.is_some()
            || ref_fence.state != "held"
            || ref_fence.active_attempt_id.is_some()
        {
            return Err(changed());
        }
        if let Some(context) = context {
            if context.run_id != run.id
                || context.policy.id != p.id
                || context.policy.snapshot_digest != p.snapshot_digest
                || run.integration_commit.as_deref() != Some(context.candidate.candidate_commit())
                || run.source_commit.as_deref() != Some(context.captured.commit())
            {
                return Err(changed());
            }
        } else if kind != IntegrationStageKind::Preparation
            || run.source_commit.is_some()
            || run.integration_commit.is_some()
        {
            return Err(changed());
        }
        source_idle(&tx, &p)?;
        let reservation = match kind {
            IntegrationStageKind::Preparation => {
                if validator.is_some()
                    || p.selection.on_target_advance
                        != IntegrationTargetAdvance::BoundedDescendantReprepare
                    || run.round != prior.view.round.checked_add(1).ok_or_else(limited)?
                    || run.round > p.selection.max_rounds
                {
                    return Err(denied());
                }
                let mut statement=tx.prepare("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source')='integration_core' AND json_extract(document,'$.kind')='integration.descendant_target_observed' AND json_extract(document,'$.payload.attempt_id')=?2 ORDER BY sequence LIMIT 2").map_err(db_error)?;
                let rows = statement
                    .query_map(params![run.id, prior.view.attempt_id], |r| {
                        r.get::<_, Option<String>>(0)
                    })
                    .map_err(db_error)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(db_error)?;
                if rows.len() != 1 {
                    return Err(changed());
                }
                let event: crate::storage::Event = decode(rows[0].as_deref().ok_or_else(limited)?)?;
                if event.payload["from_head"] != previous.run.expected_target_head
                    || event.payload["to_head"] != run.expected_target_head
                    || event.payload["from_round"] != prior.view.round
                    || event.payload["next_round"] != run.round
                    || event.payload["read_worker_joined"] != true
                {
                    return Err(changed());
                }
                p.selection.stage_reservations.preparation.clone()
            }
            IntegrationStageKind::Validator => {
                let context = context.ok_or_else(denied)?;
                let reference = validator.as_ref().ok_or_else(denied)?;
                let index = p
                    .selection
                    .stage_reservations
                    .validators
                    .iter()
                    .position(|v| &v.validator == reference)
                    .ok_or_else(denied)?;
                if (index == 0 && prior.view.kind != IntegrationStageKind::Preparation)
                    || (index > 0
                        && (prior.view.kind != IntegrationStageKind::Validator
                            || prior.view.validator.as_ref()
                                != Some(
                                    &p.selection.stage_reservations.validators[index - 1].validator,
                                )))
                    || context.validations.len() != index
                    || context.validations.iter().any(|(_, observed)| {
                        !observed.passed
                            || !super::io::IntegrationPreparedContext::validation_is_stopped(
                                observed,
                            )
                    })
                {
                    return Err(denied());
                }
                p.selection.stage_reservations.validators[index]
                    .reservation
                    .clone()
            }
            IntegrationStageKind::Cas => {
                let context = context.ok_or_else(denied)?;
                if validator.is_some()
                    || prior.view.kind != IntegrationStageKind::Validator
                    || context.validations.len() != p.bindings.validators.len()
                    || context.validations.iter().any(|(_, observed)| {
                        !observed.passed
                            || !super::io::IntegrationPreparedContext::validation_is_stopped(
                                observed,
                            )
                    })
                {
                    return Err(denied());
                }
                p.selection.stage_reservations.cas.clone()
            }
        };
        reserve_ancestors(&tx, &p, &reservation)?;
        reserve_counters(&tx, &p)?;
        let (candidate, control, binding) = if kind == IntegrationStageKind::Preparation {
            let stamp = now()?;
            let control = ManagedWorkspace {
                id: Uuid::new_v4().to_string(),
                project_id: p.bindings.project_id.clone(),
                identity: None,
                root: previous
                    .managed_root
                    .join(format!("integration-{}-r{}-control", run.id, run.round)),
                base_revision: p.bindings.source.head.clone(),
                created_at: stamp.clone(),
                state: "creating".into(),
                revision: 1,
            };
            let candidate = ManagedWorkspace {
                id: Uuid::new_v4().to_string(),
                project_id: p.bindings.project_id.clone(),
                identity: None,
                root: previous
                    .managed_root
                    .join(format!("integration-{}-r{}-candidate", run.id, run.round)),
                base_revision: run.expected_target_head.clone(),
                created_at: stamp,
                state: "creating".into(),
                revision: 1,
            };
            let binding = derive_binding(&p, &p.bindings.workspace_id, &p.bindings.source.head)?;
            (candidate, control, binding)
        } else {
            let context = context.ok_or_else(denied)?;
            if prior.view.round != run.round {
                return Err(changed());
            }
            let candidate: ManagedWorkspace =
                raw(&tx, "managed_workspaces", &previous.candidate.id)?;
            let control: ManagedWorkspace = raw(&tx, "managed_workspaces", &previous.control.id)?;
            if candidate.state != "ready"
                || control.state != "ready"
                || candidate.root != context.candidate.workspace().canonical_root
                || control.root != context.control.canonical_root
                || candidate.identity.as_ref() != Some(context.candidate.workspace())
                || control.identity.as_ref() != Some(&context.control)
            {
                return Err(changed());
            }
            let binding = derive_binding(&p, &candidate.id, context.candidate.candidate_commit())?;
            (candidate, control, binding)
        };
        let attempt_id = Uuid::new_v4().to_string();
        let intent_id = Uuid::new_v4().to_string();
        let dispatch = DispatchIntent {
            id: intent_id.clone(),
            attempt_id: attempt_id.clone(),
            task_id: run.task_id.clone(),
            root_task_id: p.bindings.root_task_id.clone(),
            state: IntentState::Prepared,
            executor_instance: None,
            binding: binding.clone(),
        };
        tx.execute("INSERT INTO attempts(id,task_id,root_task_id,binding,idempotency_key) VALUES(?1,?2,?3,?4,?5)",params![attempt_id,run.task_id,p.bindings.root_task_id,encode(&binding)?,format!("integration:{}:{}:{:?}:{}",run.id,run.round,kind,validator.as_ref().map(|v|encode(v)).transpose()?.unwrap_or_default())]).map_err(db_error)?;
        tx.execute("INSERT INTO dispatch_intents(id,attempt_id,state,executor_instance,document) VALUES(?1,?2,'prepared',NULL,?3)",params![intent_id,attempt_id,encode(&dispatch)?]).map_err(db_error)?;
        let attempt = IntegrationAttemptRecord {
            view: IntegrationAttemptView {
                attempt_id: attempt_id.clone(),
                round: run.round,
                kind,
                validator,
                state: "prepared".into(),
                reservation: reservation.clone(),
                settled: false,
                actual_runtime_ms: None,
                actual_output_bytes: None,
                io_completion: None,
                last_error: None,
                receipt_sequence: None,
            },
            integration_id: run.id.clone(),
            policy_id: p.id.clone(),
            policy_snapshot_digest: p.snapshot_digest.clone(),
            root_task_id: p.bindings.root_task_id.clone(),
            root_grant_id: p.bindings.root_grant_id.clone(),
            project_id: p.bindings.project_id.clone(),
            task_id: run.task_id.clone(),
            factory_scope_id: p.bindings.factory_scope_id.clone(),
            ancestors: p.bindings.ancestors.clone(),
            intent_id,
            binding,
            prepared_at_ms: ms()?,
            queued_ms: u64::try_from(queued_at.elapsed().as_millis()).map_err(|_| limited())?,
        };
        tx.execute("INSERT INTO integration_attempts(attempt_id,integration_id,round,kind,authority_kind,root_task_id,root_grant_id,project_id,state,document) VALUES(?1,?2,?3,?4,'integration',?5,?6,?7,'prepared',?8)",params![attempt_id,run.id,i64::from(run.round),encode(&kind)?.trim_matches('"'),p.bindings.root_task_id,p.bindings.root_grant_id,p.bindings.project_id,encode(&attempt)?]).map_err(db_error)?;
        records::event(
            &tx,
            &run.id,
            run.revision,
            "integration.attempt_prepared",
            &json!({"attempt_id":attempt_id,"origin_digest":sha(&budget::original(attempt.clone()))?}),
        )?;
        let checked =
            budget::accounting_records(&tx, &p.bindings.root_grant_id, &p.bindings.project_id)?
                .into_iter()
                .find(|r| r.attempt_id == attempt_id)
                .ok_or_else(changed)?;
        Store::reserve_integration_execution_meter(
            &tx,
            &checked,
            &p.bindings.root_budget,
            &p.bindings.project_budget,
        )?;
        run.active_stage = Some(kind);
        run.io_completion = None;
        records::save_run(
            &tx,
            &mut run,
            "integration.stage_queued",
            json!({"attempt_id":attempt_id,"kind":kind,"new_provider_dispatch":false,"original_shared_meter":true}),
        )?;
        ref_fence.active_attempt_id = Some(attempt_id);
        ref_fence.revision = ref_fence.revision.checked_add(1).ok_or_else(limited)?;
        write_fence(&tx, &mut ref_fence, "integration.fence_updated")?;
        let remaining = u64::try_from(
            p.expires_at_ms
                .min(material.expires_at_ms)
                .checked_sub(ms()?)
                .ok_or_else(denied)?,
        )
        .map_err(|_| denied())?;
        let deadline = (queued_at + Duration::from_millis(reservation.runtime_ms))
            .min(Instant::now() + Duration::from_millis(remaining));
        if Instant::now() >= deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Original next-stage queue time exhausted its unchanged reservation",
            ));
        }
        let acceptance = self
            .integration_verified_acceptance(&p.selection.goal_scope_id, &p.selection.task_id)?
            .ok_or_else(denied)?;
        if acceptance.id != run.acceptance_ref || sha(&acceptance)? != run.acceptance_digest {
            return Err(changed());
        }
        let ticket = IntegrationStageTicket {
            policy: p,
            run,
            attempt,
            acceptance,
            control,
            candidate,
            managed_root: previous.managed_root.clone(),
            queued_at,
            deadline,
        };
        if kind == IntegrationStageKind::Preparation {
            let permit = super::fences::for_stage(&tx, &ticket)?;
            for workspace in [&ticket.control, &ticket.candidate] {
                crate::registry::reserve_workspace_row_for_integration(
                    &tx,
                    workspace,
                    &format!("integration:{}:{}", ticket.run.id, workspace.id),
                    &encode(
                        &json!({"integration_id":ticket.run.id,"policy_id":ticket.policy.id,"round":ticket.run.round,"role":if workspace.id==ticket.control.id {"control"}else{"candidate"}}),
                    )?,
                    &permit,
                )?;
            }
        }
        records::check_metadata_bounds(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(ticket)
    }
}

impl Store {
    pub(crate) fn integration_start_replay(
        &self,
        policy_id: &str,
        revision: u64,
        key_id: &str,
    ) -> Result<Option<IntegrationRunView>> {
        let payload = encode(&json!({"policy_id":policy_id,"expected_revision":revision}))?;
        records::replay(
            &self.conn,
            &format!("integration:{policy_id}:start"),
            key_id,
            &payload,
        )
    }
    pub(crate) fn integration_existing_run(&self, policy_id: &str) -> Result<bool> {
        let policy = records::policy(&self.conn, policy_id)?;
        self.conn.query_row("SELECT EXISTS(SELECT 1 FROM integrations WHERE policy_id=?1 AND task_id=?2 AND spec_revision=?3)",params![policy.id,policy.selection.task_id,sql_num(policy.selection.expected_spec_revision)?],|r|r.get(0)).map_err(db_error)
    }
    pub(crate) fn armed_integration_policies(
        &self,
        after: Option<&str>,
    ) -> Result<(Vec<IntegrationPolicy>, Option<String>)> {
        records::check_metadata_bounds(&self.conn)?;
        let mut statement=self.conn.prepare("SELECT p.id FROM integration_policies p WHERE (?1 IS NULL OR p.id>?1) AND NOT EXISTS(SELECT 1 FROM integrations r WHERE r.policy_id=p.id AND r.task_id=p.task_id AND r.spec_revision=p.spec_revision) ORDER BY p.id LIMIT 100").map_err(db_error)?;
        let ids = statement
            .query_map([after], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        let next = if ids.len() == 100 {
            ids.last().cloned()
        } else {
            None
        };
        let mut result = Vec::new();
        for id in ids {
            let policy = records::policy(&self.conn, &id)?;
            if policy.revoked || policy.expires_at_ms <= ms()? {
                continue;
            }
            let task: crate::storage::Task = raw(&self.conn, "tasks", &policy.selection.task_id)?;
            if task.id != policy.selection.task_id {
                return Err(changed());
            }
            if task.spec_revision != policy.selection.expected_spec_revision {
                continue;
            }
            if task.state == crate::storage::TaskState::Accepted {
                result.push(policy);
            }
        }
        Ok((result, next))
    }
}
