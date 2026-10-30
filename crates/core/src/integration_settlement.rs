//! Settlement consumes only an actually joined private stage outcome. It never
//! retries effects, refunds consumed attempts, or derives a CAS author from HEAD.
use super::{
    authority::*,
    budget, records,
    runtime::*,
    stage_workers::{JoinedStage, StageFact},
    *,
};
use crate::{
    execution::{DispatchIntent, IntentState},
    registry::ManagedWorkspace,
    storage::{db_error, encode, Store},
    Result,
};
use rusqlite::{params, Transaction, TransactionBehavior};
use serde_json::json;

pub(super) struct StageSettlement {
    pub(super) terminal: bool,
    pub(super) unknown: bool,
    pub(super) next: Option<(
        IntegrationStageKind,
        Option<crate::goal_store::GoalValidatorRef>,
    )>,
}
fn fail_unknown(code: &str) -> CoreError {
    let mut error=CoreError::new(code,"Integration retains its original reservation and repository fence until owned IO and journal facts can be reconciled");
    error.side_effect_state = "unknown".into();
    error
}
fn update_attempt(
    db: &rusqlite::Connection,
    record: &budget::IntegrationAttemptRecord,
) -> Result<()> {
    if db
        .execute(
            "UPDATE integration_attempts SET state=?1,document=?2 WHERE attempt_id=?3",
            params![record.view.state, encode(record)?, record.view.attempt_id],
        )
        .map_err(db_error)?
        != 1
    {
        return Err(changed());
    }
    Ok(())
}
fn persist_workspaces(
    tx: &Transaction<'_>,
    ticket: &IntegrationStageTicket,
    context: &super::io::IntegrationPreparedContext,
) -> Result<()> {
    for (expected, identity) in [
        (&ticket.control, &context.control),
        (&ticket.candidate, context.candidate.workspace()),
    ] {
        let mut workspace: ManagedWorkspace = raw(tx, "managed_workspaces", &expected.id)?;
        if workspace.id != expected.id
            || workspace.project_id != ticket.policy.bindings.project_id
            || workspace.root != expected.root
            || workspace.state != "creating"
            || identity.canonical_root != workspace.root
        {
            return Err(changed());
        }
        let agentbaton_workspaces::WorkspaceKind::Git { head, .. } = &identity.kind else {
            return Err(changed());
        };
        workspace.state = "ready".into();
        workspace.identity = Some(identity.clone());
        workspace.base_revision = head.clone();
        workspace.revision = workspace.revision.checked_add(1).ok_or_else(limited)?;
        crate::registry::finalize_workspace_row(tx, &workspace)?;
    }
    Ok(())
}
impl Store {
    pub(crate) fn integration_stage_stop_reason(
        &self,
        ticket: &IntegrationStageTicket,
    ) -> Result<Option<&'static str>> {
        let run = records::run(&self.conn, &ticket.run.id)?;
        let policy = records::policy(&self.conn, &ticket.policy.id)?;
        if run.cancel_requested {
            return Ok(Some("CANCEL_REQUESTED"));
        }
        if policy.revoked || policy.expires_at_ms <= ms()? {
            return Ok(Some("PERMISSION_DENIED"));
        }
        if std::time::Instant::now() >= ticket.deadline {
            return Ok(Some("DEADLINE_EXCEEDED"));
        }
        let _ = super::fences::for_stage(&self.conn, ticket)?;
        Ok(None)
    }
    pub(super) fn settle_integration_stage(
        &mut self,
        ticket: &IntegrationStageTicket,
        joined: &JoinedStage,
    ) -> Result<StageSettlement> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut run = records::run(&tx, &ticket.run.id)?;
        let policy = records::policy(&tx, &ticket.policy.id)?;
        let mut attempt = budget::attempt(&tx, &ticket.attempt.view.attempt_id)?;
        let mut fence = fence(&tx, &run)?;
        if attempt.view.settled
            || attempt.integration_id != run.id
            || run.active_stage != Some(attempt.view.kind)
            || fence.active_attempt_id.as_deref() != Some(attempt.view.attempt_id.as_str())
            || !matches!(
                attempt.view.state.as_str(),
                "prepared" | "running" | "cancel_requested"
            )
        {
            return Err(changed());
        }
        let mut intent: DispatchIntent = raw(&tx, "dispatch_intents", &attempt.intent_id)?;
        let clock_ms = u64::try_from(ticket.queued_at.elapsed().as_millis())
            .map_err(|_| limited())?
            .max(joined.elapsed_ms);
        let mut output = joined.output_bytes;
        let mut unknown = (!joined.worker_joined && !joined.never_scheduled) || output.is_none();
        if joined.never_scheduled
            && (joined.worker_joined
                || joined.effects_admitted
                || joined.result.is_ok()
                || output != Some(0))
        {
            return Err(changed());
        }
        let mut historical_cas = false;
        if let Ok(StageFact::Cas(receipt)) = &joined.result {
            let observed = receipt.record();
            if observed.grant_ref != policy.approval_id
                || observed.target_ref != run.target_ref
                || observed.expected_target_head != run.expected_target_head
                || run.integration_commit.as_deref() != Some(observed.candidate_commit.as_str())
            {
                return Err(changed());
            }
            historical_cas = observed.state
                == agentbaton_workspaces::adoption::IntegrationState::ConfirmedLocalRef;
            unknown |= !receipt.io().io_complete()
                || observed.state
                    == agentbaton_workspaces::adoption::IntegrationState::OutcomeUnknown;
            records::event(
                &tx,
                &run.id,
                run.revision,
                "integration.cas_observed",
                &json!({"attempt_id":attempt.view.attempt_id,"round":run.round,"historical_cas_confirmed":historical_cas,"record":observed,"io":receipt.io(),"not_a_current_ref_authorship_inference":true}),
            )?;
        }
        if let Ok(StageFact::Validator { stopped, .. }) = &joined.result {
            unknown |= !*stopped;
        }
        if let Err(error) = &joined.result {
            unknown |= matches!(
                error.code.as_str(),
                "OUTCOME_UNKNOWN" | "INTERNAL_ERROR" | "BUDGET_UNKNOWN"
            );
        }
        if unknown {
            attempt.view.state = "outcome_unknown".into();
            attempt.view.io_completion = Some(IntegrationIoCompletion::InterruptedNoJoinReceipt);
            attempt.view.last_error = Some(fail_unknown("OUTCOME_UNKNOWN"));
            // Retain reservation and counters. Lower bounds are diagnostics,
            // never settled totals or a refund for an unobserved effect.
            update_attempt(&tx, &attempt)?;
            intent.state = IntentState::OutcomeUnknown;
            tx.execute(
                "UPDATE dispatch_intents SET state='outcome_unknown',document=?1 WHERE id=?2",
                params![encode(&intent)?, intent.id],
            )
            .map_err(db_error)?;
            run.state = IntegrationState::OutcomeUnknown;
            run.io_completion = Some(IntegrationIoCompletion::InterruptedNoJoinReceipt);
            run.last_error = Some(fail_unknown("OUTCOME_UNKNOWN"));
            records::save_run(
                &tx,
                &mut run,
                "integration.stage_unknown",
                json!({"attempt_id":attempt.view.attempt_id,"observed_runtime_lower_bound_ms":clock_ms,"historical_cas_confirmed":historical_cas,"worker_thread_joined":joined.worker_joined,"reservation_refunded":false,"effects_admitted":joined.effects_admitted}),
            )?;
            fence.state = "unknown".into();
            fence.revision = fence.revision.checked_add(1).ok_or_else(limited)?;
            write_fence(&tx, &mut fence, "integration.fence_updated")?;
            records::check_metadata_bounds(&tx)?;
            tx.commit().map_err(db_error)?;
            return Ok(StageSettlement {
                terminal: true,
                unknown: true,
                next: None,
            });
        }
        let output = output.take().ok_or_else(changed)?;
        let overrun = clock_ms > attempt.view.reservation.runtime_ms
            || output > attempt.view.reservation.output_bytes;
        let expired = policy.revoked
            || policy.expires_at_ms <= ms()?
            || policy.bindings.goal_scope_expires_at_ms <= ms()?;
        let cancelled = run.cancel_requested;
        if let Ok(StageFact::Validator { .. } | StageFact::Cas(_)) = &joined.result {
            let context = joined.context.as_ref().ok_or_else(changed)?;
            let agentbaton_workspaces::validation::VersionBinding::Git {
                integration_candidate,
                tree,
                ..
            } = context.candidate.version_binding()
            else {
                return Err(changed());
            };
            if context.policy.id != policy.id
                || context.policy.snapshot_digest != policy.snapshot_digest
                || context.run_id != run.id
                || run.source_commit.as_deref() != Some(context.captured.commit())
                || run.integration_commit.as_deref() != Some(integration_candidate.as_str())
                || run.integration_tree.as_deref() != Some(tree.as_str())
            {
                return Err(changed());
            }
        }
        let mut next = None;
        let mut terminal = true;
        match &joined.result {
            Ok(StageFact::Prepared) => {
                let context = joined.context.as_ref().ok_or_else(changed)?;
                if context.policy.id != policy.id
                    || context.policy.snapshot_digest != policy.snapshot_digest
                    || context.run_id != run.id
                {
                    return Err(changed());
                }
                let agentbaton_workspaces::validation::VersionBinding::Git {
                    integration_candidate,
                    tree,
                    target_ref,
                    expected_target_head,
                    source_candidate,
                } = context.candidate.version_binding()
                else {
                    return Err(changed());
                };
                if target_ref != run.target_ref
                    || expected_target_head != run.expected_target_head
                    || source_candidate != context.captured.commit()
                {
                    return Err(changed());
                }
                persist_workspaces(&tx, ticket, context)?;
                run.source_commit = Some(context.captured.commit().into());
                run.source_tree = Some(context.captured.tree().into());
                run.integration_commit = Some(integration_candidate);
                run.integration_tree = Some(tree);
                run.state = IntegrationState::Preparing;
                let validator = policy
                    .selection
                    .stage_reservations
                    .validators
                    .first()
                    .ok_or_else(denied)?
                    .validator
                    .clone();
                next = Some((IntegrationStageKind::Validator, Some(validator)));
                terminal = false;
            }
            Ok(StageFact::Validator {
                reference,
                view,
                passed,
                stopped,
            }) => {
                if attempt.view.validator.as_ref() != Some(reference) {
                    return Err(changed());
                }
                let definition = policy
                    .bindings
                    .validators
                    .iter()
                    .find(|v| v.reference == *reference)
                    .ok_or_else(changed)?;
                let digest = sha(view)?;
                let sequence = records::event(
                    &tx,
                    &run.id,
                    run.revision,
                    "integration.validator_observed",
                    &json!({"attempt_id":attempt.view.attempt_id,"round":run.round,"validator":reference,"definition_digest":definition.definition_digest,"integration_commit":run.integration_commit,"integration_tree":run.integration_tree,"result_digest":digest,"view":view,"passed":passed,"stop_confirmed":stopped,"worker_joined":joined.worker_joined}),
                )?;
                run.validation_receipts.push(IntegrationValidationReceipt {
                    attempt_id: attempt.view.attempt_id.clone(),
                    round: run.round,
                    validator: reference.clone(),
                    definition_digest: definition.definition_digest.clone(),
                    integration_commit: run.integration_commit.clone().ok_or_else(changed)?,
                    integration_tree: run.integration_tree.clone().ok_or_else(changed)?,
                    result_digest: digest,
                    passed: *passed,
                    stop_confirmed: *stopped,
                    worker_joined: joined.worker_joined,
                    actual_runtime_ms: Some(clock_ms),
                    actual_output_bytes: Some(output),
                    details_ref: format!("integration-event:{sequence}"),
                    sequence,
                });
                if *passed && *stopped {
                    let position = policy
                        .selection
                        .stage_reservations
                        .validators
                        .iter()
                        .position(|v| v.validator == *reference)
                        .ok_or_else(changed)?;
                    next = Some(
                        if let Some(next) = policy
                            .selection
                            .stage_reservations
                            .validators
                            .get(position + 1)
                        {
                            (
                                IntegrationStageKind::Validator,
                                Some(next.validator.clone()),
                            )
                        } else {
                            (IntegrationStageKind::Cas, None)
                        },
                    );
                    terminal = false;
                    run.state = IntegrationState::Validating;
                } else {
                    run.state = IntegrationState::Failed;
                    run.last_error = Some(CoreError::new(
                        "VALIDATION_FAILED",
                        "Fresh merged-candidate correctness validation did not pass",
                    ));
                }
            }
            Ok(StageFact::Cas(receipt)) => {
                if historical_cas {
                    run.state = IntegrationState::ConfirmedLocalRef;
                } else {
                    run.state = IntegrationState::StaleTarget;
                    run.last_error=Some(CoreError::new("INPUT_CHANGED","Exact target compare-and-swap was rejected; no replacement dispatch is authorized"));
                }
                let _ = receipt;
            }
            Err(error) => {
                run.state = IntegrationState::Failed;
                run.last_error = Some(error.clone());
            }
        }
        // A proven historical CAS is never erased by a late cancel/clock race.
        if !historical_cas {
            if overrun {
                run.state = IntegrationState::Blocked;
                run.last_error = Some(CoreError::new(
                    "RESOURCE_LIMIT",
                    "Actual owned integration stage exceeded its unchanged reservation",
                ));
                terminal = true;
                next = None;
            } else if cancelled || expired {
                run.state = IntegrationState::Cancelled;
                run.last_error = Some(CoreError::new(
                    if cancelled {
                        "CANCEL_REQUESTED"
                    } else {
                        "PERMISSION_DENIED"
                    },
                    "Original integration cancellation or expiry prevents any continuation",
                ));
                terminal = true;
                next = None;
            }
        }
        attempt.view.state = "finished".into();
        attempt.view.settled = true;
        attempt.view.actual_runtime_ms = Some(clock_ms);
        attempt.view.actual_output_bytes = Some(output);
        let completion = if joined.never_scheduled {
            IntegrationIoCompletion::NotScheduled
        } else {
            IntegrationIoCompletion::WorkerJoined
        };
        attempt.view.io_completion = Some(completion);
        attempt.view.last_error = run.last_error.clone();
        attempt.view.receipt_sequence = None;
        let receipt = records::event(
            &tx,
            &run.id,
            run.revision,
            if joined.never_scheduled {
                "integration.attempt_not_scheduled"
            } else {
                "integration.attempt_joined"
            },
            &json!({"attempt_id":attempt.view.attempt_id,"receipt_digest":sha(&attempt)?,"observed_runtime_ms":clock_ms,"selected_artifact_and_retained_stream_bytes":output,"total_filesystem_output":null,"budget_overrun":overrun,"historical_cas_confirmed":historical_cas,"reservation_refunded":false}),
        )?;
        attempt.view.receipt_sequence = Some(receipt);
        intent.state = IntentState::Stopped;
        tx.execute(
            "UPDATE dispatch_intents SET state='stopped',document=?1 WHERE id=?2",
            params![encode(&intent)?, intent.id],
        )
        .map_err(db_error)?;
        update_attempt(&tx, &attempt)?;
        let checked = budget::accounting_records(
            &tx,
            &policy.bindings.root_grant_id,
            &policy.bindings.project_id,
        )?
        .into_iter()
        .find(|r| r.attempt_id == attempt.view.attempt_id)
        .ok_or_else(changed)?;
        Store::settle_integration_execution_meter(&tx, &checked)?;
        for scope in [
            format!("project:{}", policy.bindings.project_id),
            format!("root:{}", policy.bindings.root_task_id),
        ] {
            if tx
                .execute(
                    "UPDATE budgets SET active=active-1 WHERE scope=?1 AND active>0",
                    [scope],
                )
                .map_err(db_error)?
                != 1
            {
                return Err(changed());
            }
        }
        run.active_stage = None;
        run.io_completion = Some(completion);
        records::save_run(
            &tx,
            &mut run,
            if terminal {
                "integration.finished"
            } else {
                "integration.stage_joined"
            },
            json!({"attempt_id":attempt.view.attempt_id,"worker_joined":joined.worker_joined,"not_scheduled":joined.never_scheduled,"budget_overrun":overrun,"historical_cas_confirmed":historical_cas,"new_provider_dispatch":false,"task_accepted":false}),
        )?;
        fence.active_attempt_id = None;
        fence.revision = fence.revision.checked_add(1).ok_or_else(limited)?;
        if terminal {
            fence.state = "released".into();
        }
        write_fence(
            &tx,
            &mut fence,
            if terminal {
                "integration.fence_released"
            } else {
                "integration.fence_updated"
            },
        )?;
        records::check_metadata_bounds(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(StageSettlement {
            terminal,
            unknown: false,
            next,
        })
    }
}

impl Store {
    /// Internal supervisor stop intent. No witness/expensive hash is needed to
    /// request stopping already-owned work; failure cannot create a stop proof.
    pub(crate) fn request_integration_supervisor_stop(
        &mut self,
        id: &str,
        reason: &str,
    ) -> Result<()> {
        if !matches!(
            reason,
            "CANCEL_REQUESTED"
                | "DEADLINE_EXCEEDED"
                | "PERMISSION_DENIED"
                | "AUTH_REQUIRED"
                | "INPUT_CHANGED"
                | "BUDGET_UNKNOWN"
        ) {
            return Err(invalid());
        }
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut run = records::run(&tx, id)?;
        if !run.cancel_requested
            && !matches!(
                run.state,
                IntegrationState::ConfirmedLocalRef
                    | IntegrationState::Failed
                    | IntegrationState::Blocked
                    | IntegrationState::StaleTarget
                    | IntegrationState::Cancelled
            )
        {
            run.cancel_requested = true;
            if run.state != IntegrationState::OutcomeUnknown {
                run.state = IntegrationState::CancelRequested;
            }
            records::save_run(
                &tx,
                &mut run,
                "integration.supervisor_stop_requested",
                json!({"reason_code":reason,"stop_not_confirmed":true,"reservation_refunded":false}),
            )?;
        }
        records::check_metadata_bounds(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(())
    }
    pub(super) fn block_integration_continuation(
        &mut self,
        ticket: &IntegrationStageTicket,
        error: &CoreError,
    ) -> Result<()> {
        if !matches!(
            error.code.as_str(),
            "RESOURCE_LIMIT"
                | "BUDGET_UNKNOWN"
                | "PERMISSION_DENIED"
                | "AUTH_REQUIRED"
                | "INPUT_CHANGED"
                | "CANCEL_REQUESTED"
                | "DEADLINE_EXCEEDED"
                | "WORKSPACE_BUSY"
                | "CAPABILITY_UNSUPPORTED"
                | "CONFIG_CONFLICT"
        ) {
            return Err(error.clone());
        }
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut run = records::run(&tx, &ticket.run.id)?;
        let attempt = budget::attempt(&tx, &ticket.attempt.view.attempt_id)?;
        let mut fence = fence(&tx, &run)?;
        if !attempt.view.settled
            || attempt.view.io_completion != Some(IntegrationIoCompletion::WorkerJoined)
            || attempt.view.actual_runtime_ms.is_none()
            || attempt.view.actual_output_bytes.is_none()
            || run.active_stage.is_some()
            || fence.active_attempt_id.is_some()
            || fence.state != "held"
        {
            return Err(changed());
        }
        let unknown = error.code == "BUDGET_UNKNOWN";
        run.state = if unknown {
            IntegrationState::OutcomeUnknown
        } else if run.cancel_requested {
            IntegrationState::Cancelled
        } else {
            IntegrationState::Blocked
        };
        run.last_error = Some(error.clone());
        records::save_run(
            &tx,
            &mut run,
            "integration.continuation_blocked",
            json!({"reason_code":error.code,"preceding_attempt_id":attempt.view.attempt_id,"new_attempt_started":false,"reservation_refunded":false}),
        )?;
        fence.state = if unknown { "unknown" } else { "released" }.into();
        fence.revision = fence.revision.checked_add(1).ok_or_else(limited)?;
        write_fence(
            &tx,
            &mut fence,
            if unknown {
                "integration.fence_updated"
            } else {
                "integration.fence_released"
            },
        )?;
        records::check_metadata_bounds(&tx)?;
        tx.commit().map_err(db_error)?;
        Ok(())
    }
}
