//! A single original owned thread for one already-reserved stage. Read results
//! return to the Actor before effects. Only its private one-shot proof handoff
//! can release that thread, and actual JoinHandle consumption produces completion.
use super::{
    authority::*, io::IntegrationPreparedContext, reads::*, runtime::IntegrationStageTicket, *,
};
use crate::{CoreError, Result};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub(super) enum StageFact {
    Prepared,
    TargetAdvanced(ObservedTargetAdvance),
    Validator {
        reference: crate::goal_store::GoalValidatorRef,
        view: serde_json::Value,
        passed: bool,
        stopped: bool,
        target_probe: Option<Result<Box<super::io::ValidatorTargetDriftProof>>>,
    },
    Cas(agentbaton_workspaces::adoption::ControlledIntegrationResult),
}
struct StageReturn {
    context: Option<IntegrationPreparedContext>,
    result: Result<StageFact>,
    output_bytes: Option<u64>,
    effects_admitted: bool,
    preparation_io: Option<agentbaton_workspaces::adoption::ControlledPreparationIo>,
}
pub(super) struct StageReadRequest {
    pub(super) proof: IntegrationReadProof,
    pub(super) raw: String,
}
/// Cannot be deserialized or returned before actual owned-thread join.
pub(super) struct JoinedStage {
    pub(super) _database_guard: Option<super::reads::DatabasePin>,
    pub(super) context: Option<IntegrationPreparedContext>,
    pub(super) result: Result<StageFact>,
    pub(super) output_bytes: Option<u64>,
    pub(super) elapsed_ms: u64,
    pub(super) effects_admitted: bool,
    pub(super) worker_joined: bool,
    pub(super) never_scheduled: bool,
    pub(super) preparation_io: Option<agentbaton_workspaces::adoption::ControlledPreparationIo>,
}
pub(super) struct OwnedStage {
    database_guard: Option<super::reads::DatabasePin>,
    pub(super) ticket: IntegrationStageTicket,
    cancel: Arc<AtomicBool>,
    requests: Receiver<StageReadRequest>,
    admission: SyncSender<Result<IntegrationReadProof>>,
    thread: Option<JoinHandle<StageReturn>>,
}
pub(super) struct StageSpawnFailure {
    pub(super) error: CoreError,
    pub(super) context: Option<IntegrationPreparedContext>,
}
impl OwnedStage {
    pub(super) fn spawn(
        ticket: IntegrationStageTicket,
        context: Option<IntegrationPreparedContext>,
        plan: IntegrationReadPlan,
        raw: String,
    ) -> std::result::Result<Self, Box<StageSpawnFailure>> {
        let database_guard = Some(plan.database_guard());
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (request_tx, requests) = mpsc::sync_channel(1);
        let (admission, permit_rx) = mpsc::sync_channel::<Result<IntegrationReadProof>>(1);
        // A failed spawn must return the original candidate/CommandRun handles;
        // dropping a closure is not a successful stop receipt.
        let input = Arc::new(Mutex::new(Some((ticket.clone(), context, plan, raw))));
        let packet = input.clone();
        let thread = std::thread::Builder::new()
            .name("integration-stage".into())
            .spawn(move || {
                let (ticket, context, plan, raw) = packet
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take()
                    .expect("single owned integration stage packet");
                let read = plan
                    .execute(flag.clone(), ticket.deadline)
                    .and_then(|proof| {
                        request_tx
                            .send(StageReadRequest {
                                proof,
                                raw: raw.clone(),
                            })
                            .map_err(|_| {
                                CoreError::new(
                                    "CANCEL_REQUESTED",
                                    "Integration Actor stopped before stage observation admission",
                                )
                            })?;
                        loop {
                            if flag.load(Ordering::Acquire) {
                                return Err(CoreError::new(
                                    "CANCEL_REQUESTED",
                                    "Owned integration stage cancellation observed before effects",
                                ));
                            }
                            let remaining =
                                ticket.deadline.saturating_duration_since(Instant::now());
                            if remaining.is_zero() {
                                return Err(CoreError::new(
                                    "DEADLINE_EXCEEDED",
                                    "Original integration stage expired while awaiting admission",
                                ));
                            }
                            match permit_rx.recv_timeout(remaining.min(Duration::from_millis(10))) {
                                Ok(result) => return result,
                                Err(mpsc::RecvTimeoutError::Timeout) => (),
                                Err(mpsc::RecvTimeoutError::Disconnected) => {
                                    return Err(CoreError::new(
                                        "CANCEL_REQUESTED",
                                        "Integration Actor stopped before private effect admission",
                                    ))
                                }
                            }
                        }
                    });
                let mut proof = match read {
                    Ok(proof) => proof,
                    Err(error) => {
                        return StageReturn {
                            context,
                            result: Err(error),
                            output_bytes: Some(0),
                            effects_admitted: false,
                            preparation_io: None,
                        }
                    }
                };
                if let Err(error) = proof.verify_on_worker(&raw, flag.clone(), ticket.deadline) {
                    return StageReturn {
                        context,
                        result: Err(error),
                        output_bytes: Some(0),
                        effects_admitted: false,
                        preparation_io: None,
                    };
                }
                if let Some(advance) = proof.take_target_advance() {
                    return StageReturn {
                        context,
                        result: Ok(StageFact::TargetAdvanced(advance)),
                        output_bytes: Some(0),
                        effects_admitted: false,
                        preparation_io: None,
                    };
                }
                execute(&ticket, context, flag, &proof, &raw)
            });
        match thread {
            Ok(thread) => Ok(Self {
                database_guard,
                ticket,
                cancel,
                requests,
                admission,
                thread: Some(thread),
            }),
            Err(_) => {
                let (_, context, _, _) = input
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take()
                    .expect("unstarted stage retains its packet");
                Err(Box::new(StageSpawnFailure {
                    error: CoreError::new(
                        "RESOURCE_LIMIT",
                        "Cannot start the reserved integration stage worker",
                    ),
                    context,
                }))
            }
        }
    }
    pub(super) fn take_read_request(&self) -> Result<Option<StageReadRequest>> {
        match self.requests.try_recv() {
            Ok(request) => Ok(Some(request)),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(None),
        }
    }
    pub(super) fn admit(&self, result: Result<IntegrationReadProof>) {
        // A failed send is a stopped/expired original worker; never dispatch a
        // replacement. Its real join and result remain owned by this object.
        let _ = self.admission.try_send(result);
    }
    pub(super) fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub(super) fn finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub(super) fn join(mut self) -> (IntegrationStageTicket, JoinedStage) {
        let elapsed_ms =
            u64::try_from(self.ticket.queued_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        let result = self
            .thread
            .take()
            .expect("original owned integration stage")
            .join();
        // Include time actually spent joining; the clock began before admission.
        let elapsed_ms = elapsed_ms
            .max(u64::try_from(self.ticket.queued_at.elapsed().as_millis()).unwrap_or(u64::MAX));
        let database_guard = self.database_guard.take();
        let joined = match result {
            Ok(result) => JoinedStage {
                _database_guard: database_guard,
                context: result.context,
                result: result.result,
                output_bytes: result.output_bytes,
                elapsed_ms,
                effects_admitted: result.effects_admitted,
                worker_joined: true,
                never_scheduled: false,
                preparation_io: result.preparation_io,
            },
            Err(_) => JoinedStage {
                _database_guard: database_guard,
                context: None,
                result: Err(CoreError::new(
                    "OUTCOME_UNKNOWN",
                    "Owned integration stage panicked; effects and output have no complete receipt",
                )),
                output_bytes: None,
                elapsed_ms,
                effects_admitted: true,
                worker_joined: true,
                never_scheduled: false,
                preparation_io: None,
            },
        };
        (self.ticket.clone(), joined)
    }
}
fn join_owned_stage_on_drop(thread: &mut Option<JoinHandle<StageReturn>>, cancel: &AtomicBool) {
    // A consumed join has no active thread to stop. Do not manufacture an
    // Owner cancellation on the proof's original epoch, or reset a real one.
    if let Some(thread) = thread.take() {
        cancel.store(true, Ordering::Release);
        if let Ok(returned) = thread.join() {
            dispose_context(returned.context);
        }
    }
}
impl Drop for OwnedStage {
    fn drop(&mut self) {
        // Error/unwind consumes the real thread and retained Command handles
        // before the original raw DB guard and daemon writer lock are released.
        join_owned_stage_on_drop(&mut self.thread, &self.cancel);
    }
}

fn execute(
    ticket: &IntegrationStageTicket,
    mut context: Option<IntegrationPreparedContext>,
    cancel: Arc<AtomicBool>,
    read_proof: &IntegrationReadProof,
    raw: &str,
) -> StageReturn {
    match ticket.attempt.view.kind {
        IntegrationStageKind::Preparation => {
            let observed = super::io::prepare(ticket, cancel);
            match observed.result {
                Ok(prepared) => {
                    let bytes = if observed.io.io_complete() {
                        prepared
                            .outputs
                            .iter()
                            .try_fold(0u64, |total, (_, pin)| total.checked_add(pin.bytes))
                    } else {
                        None
                    };
                    StageReturn {
                        context: Some(prepared),
                        result: Ok(StageFact::Prepared),
                        output_bytes: bytes,
                        effects_admitted: true,
                        preparation_io: Some(observed.io.clone()),
                    }
                }
                Err(error) => StageReturn {
                    context,
                    result: Err(error),
                    output_bytes: if !observed.io.effects_may_have_started()
                        && observed.io.io_complete()
                    {
                        Some(0)
                    } else {
                        None
                    },
                    effects_admitted: true,
                    preparation_io: Some(observed.io.clone()),
                },
            }
        }
        IntegrationStageKind::Validator => {
            let result = (|| {
                let reference = ticket
                    .attempt
                    .view
                    .validator
                    .as_ref()
                    .ok_or_else(denied)?
                    .clone();
                // Reserve before any command may start; place its returned real
                // handle under the context's unwind guard before further work.
                context
                    .as_mut()
                    .ok_or_else(denied)?
                    .validations
                    .try_reserve(1)
                    .map_err(|_| limited())?;
                let stored_reference = reference.clone();
                let prepared = context
                    .as_ref()
                    .ok_or_else(denied)?
                    .validation(&reference, &ticket.attempt.view.attempt_id)?;
                let observed = prepared.execute(&cancel, ticket.deadline)?;
                let retained = context.as_mut().ok_or_else(denied)?;
                retained.validations.push((stored_reference, observed));
                let observed = &retained.validations.last().ok_or_else(denied)?.1;
                let bytes = observed
                    .command
                    .as_ref()
                    .map(|run| {
                        u64::try_from(run.captured_stdout().len())
                            .ok()?
                            .checked_add(u64::try_from(run.captured_stderr().len()).ok()?)
                    })
                    .unwrap_or(Some(0));
                // The actual opaque command is already under the context's
                // unwind guard before any fallible readonly post observation.
                let target_probe =
                    retained.probe_validator_target_drift(ticket, read_proof, raw, cancel.clone());
                let observed = &retained.validations.last().ok_or_else(denied)?.1;
                let fact = StageFact::Validator {
                    reference: reference.clone(),
                    view: observed.view.clone(),
                    passed: observed.passed,
                    stopped: IntegrationPreparedContext::validation_is_stopped(observed),
                    target_probe,
                };
                Ok((fact, bytes))
            })();
            match result {
                Ok((fact, output_bytes)) => StageReturn {
                    context,
                    result: Ok(fact),
                    output_bytes,
                    effects_admitted: true,
                    preparation_io: None,
                },
                Err(error) => StageReturn {
                    context,
                    result: Err(error),
                    output_bytes: None,
                    effects_admitted: true,
                    preparation_io: None,
                },
            }
        }
        IntegrationStageKind::Cas => {
            let result = context.as_ref().ok_or_else(denied).and_then(|prepared| {
                prepared.cas(cancel, ticket.deadline, &ticket.attempt.view.attempt_id)
            });
            let output_bytes = result.as_ref().ok().and_then(|receipt| {
                receipt.io().io_complete().then_some(())?;
                receipt
                    .io()
                    .stdout_retained_bytes()?
                    .checked_add(receipt.io().stderr_retained_bytes()?)
            });
            StageReturn {
                context,
                result: result.map(StageFact::Cas),
                output_bytes,
                effects_admitted: true,
                preparation_io: None,
            }
        }
    }
}

impl JoinedStage {
    pub(super) fn not_scheduled(
        ticket: &IntegrationStageTicket,
        error: CoreError,
        context: Option<IntegrationPreparedContext>,
    ) -> Self {
        Self {
            _database_guard: None,
            context,
            result: Err(error),
            output_bytes: Some(0),
            elapsed_ms: u64::try_from(ticket.queued_at.elapsed().as_millis()).unwrap_or(u64::MAX),
            effects_admitted: false,
            worker_joined: false,
            never_scheduled: true,
            preparation_io: None,
        }
    }
}

pub(super) fn dispose_context(context: Option<IntegrationPreparedContext>) {
    let Some(context) = context else {
        return;
    };
    let packet = Arc::new(Mutex::new(Some(context)));
    let input = packet.clone();
    match std::thread::Builder::new()
        .name("integration-owned-cleanup".into())
        .spawn(move || {
            drop(input.lock().unwrap_or_else(|p| p.into_inner()).take());
        }) {
        Ok(cleanup) => {
            let _ = cleanup.join();
        }
        Err(_) => {
            // Only the final ownership/shutdown fallback, never a live Owner
            // request. Dropping the guarded context actually waits before the
            // daemon writer lock may be released, even if no thread can start.
            drop(packet.lock().unwrap_or_else(|p| p.into_inner()).take());
        }
    }
}

#[cfg(test)]
mod target_drift_stage_ownership_tests {
    use super::*;
    fn returned() -> StageReturn {
        StageReturn {
            context: None,
            result: Err(CoreError::new("CANCEL_REQUESTED", "owned test completion")),
            output_bytes: Some(0),
            effects_admitted: false,
            preparation_io: None,
        }
    }
    #[test]
    fn consumed_real_join_does_not_manufacture_cancel() {
        let cancel = AtomicBool::new(false);
        let mut thread = Some(std::thread::spawn(returned));
        drop(thread.take().unwrap().join().unwrap());
        join_owned_stage_on_drop(&mut thread, &cancel);
        assert!(!cancel.load(Ordering::Acquire));
        assert!(thread.is_none());
    }
    #[test]
    fn consumed_real_join_never_resets_owner_cancel() {
        let cancel = AtomicBool::new(true);
        let mut thread = Some(std::thread::spawn(returned));
        drop(thread.take().unwrap().join().unwrap());
        join_owned_stage_on_drop(&mut thread, &cancel);
        assert!(cancel.load(Ordering::Acquire));
        assert!(thread.is_none());
    }
    #[test]
    fn active_drop_requests_cancel_and_consumes_actual_join() {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let joined = Arc::new(AtomicBool::new(false));
        let observed = joined.clone();
        let (ready, receiving) = mpsc::sync_channel(1);
        let mut thread = Some(std::thread::spawn(move || {
            ready.send(()).unwrap();
            while !flag.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            observed.store(true, Ordering::Release);
            returned()
        }));
        let readiness = receiving.recv_timeout(Duration::from_secs(1));
        // Consume the owner even if readiness fails; no test panic can detach
        // a live flag-waiting thread.
        join_owned_stage_on_drop(&mut thread, &cancel);
        assert!(readiness.is_ok());
        assert!(cancel.load(Ordering::Acquire));
        assert!(joined.load(Ordering::Acquire));
        assert!(thread.is_none());
    }
}
