//! One owned readonly history request, never an admission/authority worker.
use super::{reads::DatabasePin, FactoryChildGoalQuery};
use crate::{ipc::ReadyRequest, storage::Store, CoreError, Result};
use rusqlite::InterruptHandle;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

// Parsed only after the Actor's existing Owner envelope checks. No Deserialize,
// arbitrary method, path, SQL, dispatch ticket or request token crosses here.
pub(crate) enum FactoryHistoryQuery {
    Get(FactoryChildGoalQuery),
    List(FactoryChildGoalQuery),
}
struct HistoryReply {
    result: Result<Value>,
    revision: u64,
}
struct Worker {
    request_id: String,
    job: ReadyRequest,
    cancel: Arc<AtomicBool>,
    interrupt: Option<InterruptHandle>,
    interrupts: Receiver<InterruptHandle>,
    thread: JoinHandle<Result<HistoryReply>>,
}
#[derive(Default)]
pub(crate) struct FactoryHistoryWorkers {
    // This strong owner stays on the Actor across requests. Worker panic/drop
    // can never close the last raw DB descriptor while the Actor is writing.
    database: Option<Arc<DatabasePin>>,
    worker: Option<Worker>,
    #[cfg(test)]
    fixture: Option<FixtureAction>,
}
#[cfg(test)]
enum FixtureAction {
    Hold(mpsc::SyncSender<()>, Receiver<()>),
    LongSql(mpsc::SyncSender<()>, mpsc::SyncSender<bool>),
    Panic,
    Readonly,
}
#[cfg(test)]
fn fixture_action(
    reader: &Store,
    action: Option<FixtureAction>,
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<()> {
    match action {
        Some(FixtureAction::Hold(started, release)) => {
            started.try_send(()).map_err(|_| cancelled())?;
            loop {
                check(cancel, deadline)?;
                match release.recv_timeout(Duration::from_millis(1)) {
                    Ok(()) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Err(cancelled()),
                }
            }
        }
        Some(FixtureAction::LongSql(started, observed_interrupt)) => {
            started.try_send(()).map_err(|_| cancelled())?;
            let result: rusqlite::Result<i64> = reader.conn.query_row("WITH RECURSIVE count(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM count WHERE x<100000000) SELECT sum(x) FROM count", [], |row| row.get(0));
            let interrupted = matches!(&result, Err(rusqlite::Error::SqliteFailure(code, _)) if code.code == rusqlite::ErrorCode::OperationInterrupted);
            let _ = observed_interrupt.try_send(interrupted);
            result.map_err(crate::storage::db_error)?;
        }
        Some(FixtureAction::Panic) => panic!("owned readonly fixture panic"),
        Some(FixtureAction::Readonly) => {
            assert_eq!(
                reader
                    .conn
                    .pragma_query_value(None, "query_only", |r| r.get::<_, i64>(0))
                    .unwrap(),
                1
            );
            let error = reader
                .conn
                .execute("INSERT INTO events(document) VALUES('{}')", [])
                .unwrap_err();
            assert!(
                matches!(error, rusqlite::Error::SqliteFailure(code, _) if code.code == rusqlite::ErrorCode::ReadOnly)
            );
        }
        None => {}
    }
    Ok(())
}
fn busy() -> CoreError {
    CoreError {
        retryable: true,
        side_effect_state: "not_started".into(),
        ..CoreError::new("WORKSPACE_BUSY", "One readonly Factory history request is already running; retry this read after it finishes")
    }
}
fn expired() -> CoreError {
    CoreError::new(
        "RESOURCE_LIMIT",
        "The original readonly request deadline expired; no action was started",
    )
}
fn cancelled() -> CoreError {
    CoreError::new(
        "CANCEL_REQUESTED",
        "Readonly history was cancelled before reply; no action was started",
    )
}
fn check(cancel: &AtomicBool, deadline: Instant) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(cancelled());
    }
    if Instant::now() >= deadline {
        return Err(expired());
    }
    Ok(())
}
fn execute(
    database: &DatabasePin,
    query: FactoryHistoryQuery,
    cancel: &AtomicBool,
    deadline: Instant,
    interrupt: mpsc::SyncSender<InterruptHandle>,
    #[cfg(test)] fixture: Option<FixtureAction>,
) -> Result<HistoryReply> {
    check(cancel, deadline)?;
    let connection = database.open_readonly(deadline)?;
    // The handle interrupts only this request's connection, never the Actor's.
    interrupt
        .try_send(connection.get_interrupt_handle())
        .map_err(|_| cancelled())?;
    check(cancel, deadline)?;
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(crate::storage::db_error)?;
    if version != 15 {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Readonly history database schema changed",
        ));
    }
    let reader = Store {
        conn: connection,
        data_input: None,
        graph_input: None,
        factory_read_control: None,
        graph_capture_clocks: Default::default(),
        factory_recovery_queue_epochs: Vec::new(),
    };
    // open_readonly already owns BEGIN. Existing get/list intentionally uses
    // its full strict existing-transaction path. No historical predicate is
    // omitted; empty-list and DTO/header revisions share this same snapshot.
    let revision = reader.sequence()?;
    #[cfg(test)]
    fixture_action(&reader, fixture, cancel, deadline)?;
    let result = match query {
        FactoryHistoryQuery::Get(query) => reader.get_factory_child_goal(&query).map(|v| json!(v)),
        FactoryHistoryQuery::List(query) => {
            reader.list_factory_child_goals(&query).map(|v| json!(v))
        }
    };
    check(cancel, deadline)?;
    database.recheck()?;
    reader
        .conn
        .execute_batch("COMMIT")
        .map_err(crate::storage::db_error)?;
    check(cancel, deadline)?;
    Ok(HistoryReply { result, revision })
}
impl Worker {
    fn interrupt_if_stopped(&mut self) {
        if self.interrupt.is_none() {
            self.interrupt = self.interrupts.try_recv().ok();
        }
        if Instant::now() >= self.job.deadline {
            self.cancel.store(true, Ordering::Relaxed);
        }
        if self.cancel.load(Ordering::Relaxed) {
            if let Some(interrupt) = &self.interrupt {
                interrupt.interrupt();
            }
        }
    }
    fn stop_and_join(mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        // Do not detach on unwind. Before the worker sends its interrupt
        // handle it performs only the identity-pinned readonly open. Kernel
        // stalls are not forcibly interruptible, just as in existing I/O.
        while !self.thread.is_finished() {
            self.interrupt_if_stopped();
            std::thread::sleep(Duration::from_millis(1));
        }
        let _ = self.thread.join();
        let _ = self.job.reply.try_send(
            json!({"request_id":self.request_id,"error":cancelled(),"revision":0,"warnings":[]}),
        );
    }
}
impl FactoryHistoryWorkers {
    pub(crate) fn start(
        &mut self,
        store: &Store,
        query: FactoryHistoryQuery,
        request_id: String,
        job: ReadyRequest,
    ) -> std::result::Result<(), Box<(CoreError, ReadyRequest)>> {
        if self.worker.is_some() || !store.conn.is_autocommit() {
            return Err(Box::new((busy(), job)));
        }
        if Instant::now() >= job.deadline {
            return Err(Box::new((expired(), job)));
        }
        if self.database.is_none() {
            let database = match DatabasePin::capture(&store.conn) {
                Ok(database) => database,
                Err(error) => return Err(Box::new((error, job))),
            };
            self.database = Some(Arc::new(database));
        }
        let database = Arc::clone(self.database.as_ref().unwrap());
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let deadline = job.deadline;
        let (send_interrupt, interrupts) = mpsc::sync_channel(1);
        #[cfg(test)]
        let fixture = self.fixture.take();
        let thread = match std::thread::Builder::new()
            .name("baton-factory-history".into())
            .spawn(move || {
                execute(
                    &database,
                    query,
                    &flag,
                    deadline,
                    send_interrupt,
                    #[cfg(test)]
                    fixture,
                )
            }) {
            Ok(thread) => thread,
            Err(_) => {
                return Err(Box::new((
                    CoreError {
                        retryable: true,
                        ..CoreError::new(
                            "RESOURCE_LIMIT",
                            "Readonly history worker could not start",
                        )
                    },
                    job,
                )))
            }
        };
        self.worker = Some(Worker {
            request_id,
            job,
            cancel,
            interrupt: None,
            interrupts,
            thread,
        });
        Ok(())
    }
    pub(crate) fn poll(&mut self, store: &Store) {
        let Some(worker) = self.worker.as_mut() else {
            return;
        };
        worker.interrupt_if_stopped();
        if !worker.thread.is_finished() || !store.conn.is_autocommit() {
            return;
        }
        let worker = self.worker.take().unwrap();
        let stopped = worker.cancel.load(Ordering::Relaxed);
        let deadline = worker.job.deadline;
        let result = worker.thread.join().unwrap_or_else(|_| {
            Err(CoreError::new(
                "OUTCOME_UNKNOWN",
                "Readonly history worker panicked; no action was started",
            ))
        });
        let (result, revision) = if Instant::now() >= deadline {
            (Err(expired()), 0)
        } else if stopped {
            (Err(cancelled()), 0)
        } else {
            match result {
                Ok(reply) => (reply.result, reply.revision),
                Err(error) => (Err(error), 0),
            }
        };
        let response = match result {
            Ok(value) => {
                json!({"request_id":worker.request_id,"result":value,"revision":revision,"warnings":[]})
            }
            Err(error) => {
                json!({"request_id":worker.request_id,"error":error,"revision":revision,"warnings":[]})
            }
        };
        let _ = worker.job.reply.try_send(response);
    }
    pub(crate) fn reap_owned_workers(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.stop_and_join();
        }
    }
    pub(crate) fn cancel_owned(&mut self) {
        if let Some(worker) = self.worker.as_mut() {
            worker.cancel.store(true, Ordering::Relaxed);
            worker.interrupt_if_stopped();
        }
    }
    pub(crate) fn release_database(&mut self, store: &Store) {
        // Called only by the Actor after join and outside its write transaction.
        // On an exceptional raw BEGIN left open, preserve the pin until Store's
        // connection closes: Daemon declares Store before this owned service.
        if self.worker.is_none() && store.conn.is_autocommit() {
            self.database = None;
        }
    }
    #[cfg(test)]
    pub(crate) fn hold_next_for_test(&mut self) -> (Receiver<()>, mpsc::SyncSender<()>) {
        let (started, receive) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::sync_channel(1);
        self.fixture = Some(FixtureAction::Hold(started, wait));
        (receive, release)
    }
    #[cfg(test)]
    pub(crate) fn long_sql_next_for_test(&mut self) -> (Receiver<()>, Receiver<bool>) {
        let (started, receive) = mpsc::sync_channel(1);
        let (observed, interrupted) = mpsc::sync_channel(1);
        self.fixture = Some(FixtureAction::LongSql(started, observed));
        (receive, interrupted)
    }
    #[cfg(test)]
    pub(crate) fn panic_next_for_test(&mut self) {
        self.fixture = Some(FixtureAction::Panic);
    }
    #[cfg(test)]
    pub(crate) fn readonly_next_for_test(&mut self) {
        self.fixture = Some(FixtureAction::Readonly);
    }
    #[cfg(test)]
    pub(crate) fn database_owners_for_test(&self) -> usize {
        self.database.as_ref().map_or(0, Arc::strong_count)
    }
    #[cfg(test)]
    pub(crate) fn database_current_for_test(&self) -> bool {
        self.database
            .as_ref()
            .is_some_and(|pin| pin.recheck().is_ok())
    }
}
impl Drop for FactoryHistoryWorkers {
    fn drop(&mut self) {
        self.reap_owned_workers();
    }
}
