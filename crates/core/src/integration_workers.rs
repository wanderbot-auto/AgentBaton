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
}
impl IntegrationWorkers {
    pub(crate) fn has_workers(&self) -> bool {
        self.reader.is_some()
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
