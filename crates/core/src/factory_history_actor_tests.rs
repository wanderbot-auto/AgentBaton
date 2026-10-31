// Real Unix framing and generated ACP under manually driven private Actor
// services. Not Daemon.run, macOS, or a complete derived Factory Goal flow.
use super::*;
#[derive(Default)]
struct WireRead {
    client: Option<std::thread::JoinHandle<crate::Result<Value>>>,
    socket: Option<std::thread::JoinHandle<()>>,
}
impl WireRead {
    fn start(&mut self, fixture: &mut Fixture, command: Value) {
        let runtime = fixture.daemon.socket.parent().unwrap().to_owned();
        self.client = Some(std::thread::spawn(move || {
            crate::ipc::call(&runtime, command)
        }));
        let until = Instant::now() + Duration::from_secs(2);
        let accepted = loop {
            match fixture.daemon.listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => panic!("owned test socket failed"),
            }
            assert!(
                Instant::now() < until,
                "real client must reach owned Unix listener"
            );
            std::thread::sleep(Duration::from_millis(1));
        };
        let (send, receive) = mpsc::sync_channel(16);
        self.socket = Some(std::thread::spawn(move || socket_worker(accepted, send)));
        let job = receive.recv_timeout(Duration::from_secs(2)).unwrap();
        fixture.daemon.handle_ready_request(job);
    }
    fn pending(&self) -> bool {
        !self.client.as_ref().unwrap().is_finished()
    }
    fn finish(&mut self) -> Value {
        self.socket.take().unwrap().join().unwrap();
        self.client.take().unwrap().join().unwrap().unwrap()
    }
}
impl Drop for WireRead {
    fn drop(&mut self) {
        // Tests declare these guards before Fixture: its Daemon cancels/joins
        // and replies before these client threads are joined on panic.
        if let Some(socket) = self.socket.take() {
            let _ = socket.join();
        }
        if let Some(client) = self.client.take() {
            let _ = client.join();
        }
    }
}
fn missing_query(f: &Fixture) -> crate::factory::FactoryChildGoalQuery {
    crate::factory::FactoryChildGoalQuery {
        scope_id: "no-factory-scope-in-native-goal-fixture".into(),
        task_id: f.task.id.clone(),
        goal_run_id: None,
    }
}
fn finish_history(f: &mut Fixture, wire: &mut WireRead, release: mpsc::SyncSender<()>) -> Value {
    release.send(()).unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    while wire.pending() {
        f.daemon.history_workers.poll(&f.daemon.store);
        assert!(Instant::now() < until, "real read reply after release");
        std::thread::sleep(Duration::from_millis(1));
    }
    wire.finish()
}
#[test]
fn actual_unix_held_history_allows_committed_owned_turn_stop_and_real_reap() {
    let mut wire = WireRead::default();
    let mut f = Fixture::new(false);
    let (run, session) = f.goal_session();
    let q = missing_query(&f);
    let original = f.daemon.store.get_factory_child_goal(&q).unwrap_err();
    let (started, release) = f.daemon.history_workers.hold_next_for_test();
    wire.start(
        &mut f,
        json!({"method":"factory_child_goals.get","params":q}),
    );
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(wire.pending());
    assert!(!f.daemon.data_workers.has_workers());
    let attempts: i64 = f
        .daemon
        .store
        .conn
        .query_row("SELECT count(*) FROM attempts", [], |r| r.get(0))
        .unwrap();
    let before_stop = f.daemon.store.sequence().unwrap();
    f.wait_turn(&session.id);
    f.daemon.service_hinted_goal_stops().unwrap();
    assert_eq!(f.stop_requests(&session.id), 1);
    f.wait_stopped(&session.id);
    assert!(
        wire.pending(),
        "owned stop/reap completed while readonly worker still held"
    );
    f.no_extra_execution_or_task_acceptance(attempts, before_stop);
    assert!(!f.daemon.store.get_goal_run(&run.id).unwrap().task_accepted);
    let response = finish_history(&mut f, &mut wire, release);
    assert_eq!(response["error"], json!(original));
    assert!(response.get("result").is_none());
}
#[test]
fn actual_unix_cancel_remains_serviceable_while_owned_history_read_is_held() {
    let mut history = WireRead::default();
    let mut cancel = WireRead::default();
    let mut f = Fixture::new(true);
    let (run, session) = f.goal_session();
    let turn_before = f.pending_turn_before(&session.id);
    let until = Instant::now() + Duration::from_secs(5);
    while f.daemon.store.conn.query_row("SELECT count(*) FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='session.update'",[&session.id],|r|r.get::<_,i64>(0)).unwrap()==0 {
        f.daemon.supervisor.tick(&mut f.daemon.store).unwrap();assert!(Instant::now()<until,"actual held Provider prompt");std::thread::sleep(Duration::from_millis(5));
    }
    f.assert_pending_turn_unchanged(&session.id, &turn_before);
    assert!(!f.daemon.supervisor.take_goal_stop_hint());
    let q = missing_query(&f);
    let original = f.daemon.store.get_factory_child_goal(&q).unwrap_err();
    let (started, release) = f.daemon.history_workers.hold_next_for_test();
    history.start(
        &mut f,
        json!({"method":"factory_child_goals.get","params":q}),
    );
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(history.pending());
    let attempts: i64 = f
        .daemon
        .store
        .conn
        .query_row("SELECT count(*) FROM attempts", [], |r| r.get(0))
        .unwrap();
    let before_cancel = f.daemon.store.sequence().unwrap();
    let current = f.daemon.store.get_goal_run(&run.id).unwrap();
    cancel.start(&mut f,json!({"method":"goal_runs.cancel","params":{"run_id":run.id},"expected_revision":current.revision,"idempotency_key":"history-concurrent-owned-cancel"}));
    let response = cancel.finish();
    assert!(response.get("error").is_none());
    assert_eq!(response["result"]["cancel_requested"], true);
    f.wait_stopped(&session.id);
    assert!(history.pending());
    assert!(!f.daemon.data_workers.has_workers());
    f.no_extra_execution_or_task_acceptance(attempts, before_cancel);
    let run = f.daemon.store.get_goal_run(&run.id).unwrap();
    assert!(run.cancel_requested);
    assert!(!run.task_accepted);
    let response = finish_history(&mut f, &mut history, release);
    assert_eq!(response["error"], json!(original));
    assert!(response.get("result").is_none());
}
