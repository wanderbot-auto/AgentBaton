//! One bounded domain worker. The Actor retains the exact original request;
//! neither serialized proof nor Renderer-supplied completion enters this path.
use super::{IntegrationReadPlan, IntegrationReadProof};
use crate::{CoreError, Result};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Instant,
};
struct ReadWorker {
    request: crate::ipc::ReadyRequest,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<Result<IntegrationReadProof>>,
}
#[derive(Default)]
pub(crate) struct IntegrationWorkers {
    reader: Option<ReadWorker>,
    armed_after: Option<String>,
    stage: Option<super::stage_workers::OwnedStage>,
    retained: Option<(
        super::runtime::IntegrationStageTicket,
        super::stage_workers::JoinedStage,
    )>,
}
impl IntegrationWorkers {
    pub(crate) fn has_workers(&self) -> bool {
        self.reader.is_some() || self.stage.is_some() || self.retained.is_some()
    }
    pub(crate) fn defer_read(
        &mut self,
        plan: IntegrationReadPlan,
        request: crate::ipc::ReadyRequest,
    ) -> std::result::Result<(), Box<(CoreError, crate::ipc::ReadyRequest)>> {
        if self.has_workers() {
            return Err(Box::new((
                CoreError::new(
                    "WORKSPACE_BUSY",
                    "The bounded integration I/O worker is already owned",
                ),
                request,
            )));
        }
        if Instant::now() >= request.deadline {
            return Err(Box::new((
                CoreError::new(
                    "DEADLINE_EXCEEDED",
                    "Integration request expired before worker scheduling",
                ),
                request,
            )));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let deadline = request.deadline;
        let thread = match std::thread::Builder::new()
            .name("integration-read".into())
            .spawn(move || plan.execute(flag, deadline))
        {
            Ok(thread) => thread,
            Err(_) => {
                return Err(Box::new((
                    CoreError::new(
                        "RESOURCE_LIMIT",
                        "Cannot start the owned integration read worker",
                    ),
                    request,
                )))
            }
        };
        self.reader = Some(ReadWorker {
            request,
            cancel,
            thread,
        });
        Ok(())
    }
    pub(crate) fn poll_read(
        &mut self,
    ) -> Option<(crate::ipc::ReadyRequest, Result<IntegrationReadProof>)> {
        let worker = self.reader.as_ref()?;
        if Instant::now() >= worker.request.deadline {
            worker.cancel.store(true, Ordering::Release);
        }
        if !worker.thread.is_finished() {
            return None;
        }
        let worker = self.reader.take()?;
        // A finished label alone never passes authority; actually join before
        // returning any FD-backed proof to the Actor.
        let result = worker.thread.join().unwrap_or_else(|_| {
            Err(CoreError::new(
                "INTERNAL_ERROR",
                "Owned integration read worker panicked",
            ))
        });
        Some((worker.request, result))
    }
    pub(crate) fn stop_reads(&mut self) {
        if let Some(worker) = &self.reader {
            worker.cancel.store(true, Ordering::Release);
        }
    }
    pub(crate) fn join_reads(
        &mut self,
    ) -> Option<(crate::ipc::ReadyRequest, Result<IntegrationReadProof>)> {
        let worker = self.reader.take()?;
        worker.cancel.store(true, Ordering::Release);
        let result = worker.thread.join().unwrap_or_else(|_| {
            Err(CoreError::new(
                "INTERNAL_ERROR",
                "Owned integration read worker panicked during shutdown",
            ))
        });
        Some((worker.request, result))
    }
}

impl Drop for IntegrationWorkers {
    fn drop(&mut self) {
        self.stop_reads();
        self.stop_stages();
        drop(self.join_reads());
        self.join_stages();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};
    fn retain_until_join(unwind: bool) {
        let (cancelled_tx, cancelled_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let owner = std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                let cancel = Arc::new(AtomicBool::new(false));
                let flag = cancel.clone();
                let thread = std::thread::spawn(move || {
                    let deadline = Instant::now() + Duration::from_secs(3);
                    while !flag.load(Ordering::Acquire) {
                        assert!(
                            Instant::now() < deadline,
                            "owned cancellation was not delivered"
                        );
                        std::thread::yield_now();
                    }
                    cancelled_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
                    Err(CoreError::new(
                        "CANCEL_REQUESTED",
                        "bounded unit-fixture observation stopped",
                    ))
                });
                let (reply, _) = mpsc::sync_channel(1);
                let _manager = IntegrationWorkers {
                    reader: Some(ReadWorker {
                        request: crate::ipc::ReadyRequest {
                            raw: "{}".into(),
                            reply,
                            deadline: Instant::now() + Duration::from_secs(60),
                            factory_dispatch: None,
                        },
                        cancel,
                        thread,
                    }),
                    stage: None,
                    retained: None,
                    armed_after: None,
                };
                if unwind {
                    panic!("unit-fixture original first cause");
                }
                Err::<(), _>(CoreError::new(
                    "INPUT_CHANGED",
                    "unit-fixture original first cause",
                ))
            });
            assert_eq!(result.is_err(), unwind);
            done_tx.send(()).unwrap();
        });
        cancelled_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(
            matches!(done_rx.try_recv(), Err(mpsc::TryRecvError::Empty)),
            "owner returned before its blocked original worker joined"
        );
        release_tx.send(()).unwrap();
        done_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        owner.join().unwrap();
    }
    #[test]
    fn read_error_drop_retains_ownership_until_actual_join() {
        retain_until_join(false);
    }
    #[test]
    fn read_unwind_drop_retains_ownership_until_actual_join() {
        retain_until_join(true);
    }
}

impl IntegrationWorkers {
    pub(crate) fn start(
        &mut self,
        store: &mut crate::storage::Store,
        policy_id: &str,
        revision: u64,
        key: &str,
        managed_root: &std::path::Path,
        queued_at: Instant,
    ) -> Result<super::IntegrationRunView> {
        if let Some(view) = store.integration_start_replay(policy_id, revision, key)? {
            return Ok(view);
        }
        if self.has_workers() && !store.integration_existing_run(policy_id)? {
            return Err(CoreError::new("WORKSPACE_BUSY","The bounded integration worker is already retained by original work; no new intent was admitted"));
        }
        let queued =
            store.queue_integration_start(policy_id, revision, key, managed_root, queued_at)?;
        if let Some(ticket) = queued.ticket {
            self.schedule(store, ticket, None)?;
        }
        // The exact original start response is the durable queued view, never a
        // fabricated completed integration. Fixed get returns later real state.
        Ok(queued.view)
    }
    fn schedule(
        &mut self,
        store: &mut crate::storage::Store,
        ticket: super::runtime::IntegrationStageTicket,
        context: Option<super::io::IntegrationPreparedContext>,
    ) -> Result<()> {
        let (plan, raw) = match store.prepare_integration_stage_read(&ticket) {
            Ok(value) => value,
            Err(error) => {
                self.retained = Some((
                    ticket.clone(),
                    super::stage_workers::JoinedStage::not_scheduled(&ticket, error, context),
                ));
                return Ok(());
            }
        };
        match super::stage_workers::OwnedStage::spawn(ticket.clone(), context, plan, raw) {
            Ok(worker) => self.stage = Some(worker),
            Err(failure) => {
                self.retained = Some((
                    ticket.clone(),
                    super::stage_workers::JoinedStage::not_scheduled(
                        &ticket,
                        failure.error,
                        failure.context,
                    ),
                ))
            }
        }
        Ok(())
    }
    pub(crate) fn tick_stages(&mut self, store: &mut crate::storage::Store) -> Result<()> {
        if let Some(worker) = &self.stage {
            let stop = store.integration_stage_stop_reason(&worker.ticket);
            let reason = match stop {
                Ok(reason) => reason,
                Err(ref error)
                    if matches!(
                        error.code.as_str(),
                        "PERMISSION_DENIED" | "AUTH_REQUIRED" | "INPUT_CHANGED" | "BUDGET_UNKNOWN"
                    ) =>
                {
                    Some(error.code.as_str())
                }
                Err(error) => {
                    worker.cancel();
                    return Err(error);
                }
            };
            if let Some(reason) = reason {
                worker.cancel();
                store.request_integration_supervisor_stop(&worker.ticket.run.id, reason)?;
            }
            if let Some(request) = worker.take_read_request()? {
                let result = store.admit_integration_stage_effect(
                    &worker.ticket,
                    request.proof,
                    &request.raw,
                );
                worker.admit(result);
            }
        }
        if self.stage.as_ref().is_some_and(|worker| worker.finished()) {
            let worker = self
                .stage
                .take()
                .expect("checked original integration worker");
            self.retained = Some(worker.join());
        }
        let Some((ticket, joined)) = self.retained.as_ref() else {
            return Ok(());
        };
        // Unknown outcomes remain owned and readable, not replayed by tick.
        if store.get_integration(&ticket.run.id)?.run.state
            == super::IntegrationState::OutcomeUnknown
        {
            return Ok(());
        }
        let settled = store.settle_integration_stage(ticket, joined)?;
        if settled.unknown {
            return Ok(());
        }
        let (ticket, mut joined) = self.retained.take().expect("settled original completion");
        if settled.terminal {
            return Ok(());
        }
        let (kind, validator) = settled.next.ok_or_else(|| {
            CoreError::new(
                "INTERNAL_ERROR",
                "Joined integration continuation has no exact next stage",
            )
        })?;
        let context = joined.context.take().ok_or_else(|| {
            CoreError::new(
                "OUTCOME_UNKNOWN",
                "Joined integration candidate handles were lost; no reconstruction or replay",
            )
        })?;
        match store.queue_integration_next_stage(&ticket, &context, kind, validator) {
            Ok(next) => self.schedule(store, next, Some(context))?,
            Err(error) => store.block_integration_continuation(&ticket, &error)?,
        }
        Ok(())
    }
    pub(crate) fn tick_armed(
        &mut self,
        store: &mut crate::storage::Store,
        managed_root: &std::path::Path,
    ) -> Result<()> {
        if self.has_workers() {
            return Ok(());
        }
        let (policies, next) = store.armed_integration_policies(self.armed_after.as_deref())?;
        self.armed_after = next;
        for policy in policies {
            let key = format!("automatic-integration:{}", policy.id);
            match self.start(
                store,
                &policy.id,
                policy.revision,
                &key,
                managed_root,
                Instant::now(),
            ) {
                Ok(_) => return Ok(()),
                Err(error)
                    if matches!(
                        error.code.as_str(),
                        "AUTH_REQUIRED"
                            | "PERMISSION_DENIED"
                            | "STALE_REVISION"
                            | "INPUT_CHANGED"
                            | "WORKSPACE_BUSY"
                            | "RESOURCE_LIMIT"
                            | "BUDGET_UNKNOWN"
                            | "DEADLINE_EXCEEDED"
                    ) =>
                {
                    ()
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
    pub(crate) fn stop_stages(&self) {
        if let Some(worker) = &self.stage {
            worker.cancel();
        }
    }
    pub(crate) fn join_stages(&mut self) {
        if let Some(worker) = self.stage.take() {
            worker.cancel();
            let (_, mut joined) = worker.join();
            super::stage_workers::dispose_context(joined.context.take());
        }
        // Actual retained CommandRun ownership is also consumed before the
        // daemon's socket/writer lock is released. Unjournalled effects stay
        // unknown on restart; this destructor does not mint joined receipts.
        if let Some((_, mut joined)) = self.retained.take() {
            super::stage_workers::dispose_context(joined.context.take());
        }
    }
    pub(crate) fn shutdown_stages(&mut self, store: &mut crate::storage::Store) -> Result<()> {
        self.stop_stages();
        if let Some(worker) = self.stage.take() {
            let stopped = store
                .request_integration_supervisor_stop(&worker.ticket.run.id, "CANCEL_REQUESTED");
            self.retained = Some(worker.join());
            stopped?;
            let (ticket, joined) = self.retained.as_ref().expect("joined shutdown stage");
            if store.get_integration(&ticket.run.id)?.run.state
                != super::IntegrationState::OutcomeUnknown
            {
                let _ = store.settle_integration_stage(ticket, joined)?;
            }
        }
        self.join_stages();
        Ok(())
    }
}
