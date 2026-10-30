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

impl Drop for IntegrationWorkers {
    fn drop(&mut self) {
        self.stop_reads();
        drop(self.join_reads());
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
