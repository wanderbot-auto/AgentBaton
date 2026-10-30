// Actual Core-approved history and owned readonly threads; no Actor/Provider/OS
// acceptance is implied by these private service tests.
use super::*;
use crate::factory::{FactoryHistoryQuery, FactoryHistoryWorkers};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
fn q(s: &FactoryScope, g: &FactoryChildGoalSummary) -> FactoryChildGoalQuery {
    FactoryChildGoalQuery {
        scope_id: s.id.clone(),
        task_id: s.root_task_id.clone(),
        goal_run_id: Some(g.goal.run.id.clone()),
    }
}
fn job(deadline: Instant) -> (crate::ipc::ReadyRequest, mpsc::Receiver<serde_json::Value>) {
    let (reply, receive) = mpsc::sync_channel(1);
    (
        crate::ipc::ReadyRequest {
            raw: "private-test-read".into(),
            reply: crate::ipc::ReadyReply::Wire(reply),
            deadline,
            factory_dispatch: None,
        },
        receive,
    )
}
fn start(
    workers: &mut FactoryHistoryWorkers,
    store: &crate::storage::Store,
    query: FactoryHistoryQuery,
) -> mpsc::Receiver<serde_json::Value> {
    let (job, receive) = job(Instant::now() + Duration::from_secs(10));
    if let Err(failure) = workers.start(store, query, "private-read".into(), job) {
        panic!("unexpected service refusal: {}", failure.0.code);
    }
    receive
}
fn finish(
    workers: &mut FactoryHistoryWorkers,
    store: &crate::storage::Store,
    receive: mpsc::Receiver<serde_json::Value>,
) -> serde_json::Value {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        workers.poll(store);
        match receive.try_recv() {
            Ok(value) => return value,
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => panic!("owned readonly reply lost"),
        }
        assert!(Instant::now() < until, "owned readonly reply deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn real_history_thread_returns_exact_readonly_complete_dto_and_list_without_writes() {
    let (f, s, g) = queued();
    let counts = unchanged_execution(&f);
    let seq = f.store.sequence().unwrap();
    let expected = f.store.get_factory_child_goal(&q(&s, &g)).unwrap();
    let mut workers = FactoryHistoryWorkers::default();
    workers.readonly_next_for_test();
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::Get(q(&s, &g)));
    let got = finish(&mut workers, &f.store, receive);
    assert_eq!(
        serde_json::to_vec(&got["result"]).unwrap(),
        serde_json::to_vec(&json!(expected)).unwrap()
    );
    assert_eq!(got["revision"], seq);
    assert!(got.get("error").is_none());
    let mut list = q(&s, &g);
    list.goal_run_id = None;
    let expected = f.store.list_factory_child_goals(&list).unwrap();
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::List(list));
    let got = finish(&mut workers, &f.store, receive);
    assert_eq!(got["result"], json!(expected));
    assert_eq!(got["revision"], seq);
    assert_eq!(unchanged_execution(&f), counts);
    assert_eq!(f.store.sequence().unwrap(), seq);
    assert_eq!(workers.database_owners_for_test(), 1);
    assert!(workers.database_current_for_test());
    workers.release_database(&f.store);
    assert_eq!(workers.database_owners_for_test(), 0);
}
#[test]
fn real_wal_snapshot_keeps_old_dto_header_while_original_cancel_and_new_read_progress() {
    let (mut f, s, g) = queued();
    let expected = f.store.get_factory_child_goal(&q(&s, &g)).unwrap();
    let seq = f.store.sequence().unwrap();
    let mut workers = FactoryHistoryWorkers::default();
    let (started, release) = workers.hold_next_for_test();
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::Get(q(&s, &g)));
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    let cancel = FactoryChildGoalCancel {
        scope_id: s.id.clone(),
        task_id: s.root_task_id.clone(),
        goal_run_id: g.goal.run.id.clone(),
        expected_goal_run_revision: g.goal.run.revision,
    };
    let (cancelled, stops) = f
        .store
        .cancel_factory_child_goal_with_stops(&cancel, s.revision, "history-concurrent-real-cancel")
        .unwrap();
    assert_eq!(cancelled.goal.run.state, "cancelled");
    assert!(stops.is_empty(), "all nodes truly never started");
    assert!(f.store.sequence().unwrap() > seq);
    release.send(()).unwrap();
    let got = finish(&mut workers, &f.store, receive);
    assert_eq!(got["result"], json!(expected));
    assert_eq!(got["revision"], seq);
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::Get(q(&s, &g)));
    let fresh = finish(&mut workers, &f.store, receive);
    assert_eq!(
        fresh["result"]["summary"]["goal"]["run"]["state"],
        "cancelled"
    );
    assert_eq!(fresh["revision"], f.store.sequence().unwrap());
    assert!(!fresh["result"]["summary"]["goal"]["task_accepted"]
        .as_bool()
        .unwrap());
}
#[test]
fn empty_history_list_snapshot_never_uses_later_actor_sequence_and_busy_is_retryable() {
    let (mut f, s, g) = queued();
    let seq = f.store.sequence().unwrap();
    let mut empty = q(&s, &g);
    empty.task_id = "no-history-for-this-target".into();
    empty.goal_run_id = None;
    assert!(f.store.list_factory_child_goals(&empty).unwrap().is_empty());
    let mut workers = FactoryHistoryWorkers::default();
    let (started, release) = workers.hold_next_for_test();
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::List(empty));
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    let (second, unused) = job(Instant::now() + Duration::from_secs(10));
    let failure = match workers.start(
        &f.store,
        FactoryHistoryQuery::Get(q(&s, &g)),
        "second".into(),
        second,
    ) {
        Err(error) => error,
        Ok(()) => panic!("second reader admitted"),
    };
    assert_eq!(failure.0.code, "WORKSPACE_BUSY");
    assert!(failure.0.retryable);
    assert_eq!(failure.0.side_effect_state, "not_started");
    drop(failure);
    assert!(unused.try_recv().is_err());
    let task = f.store.get(&s.root_task_id).unwrap();
    f.store
        .create_registered(
            &task.project_id,
            "New actual task while history reads",
            "history-concurrent-task",
        )
        .unwrap();
    assert!(f.store.sequence().unwrap() > seq);
    release.send(()).unwrap();
    let got = finish(&mut workers, &f.store, receive);
    assert_eq!(got["result"], json!([]));
    assert_eq!(got["revision"], seq);
}
#[test]
fn new_owned_read_rechecks_corrupt_provenance_with_original_refusal() {
    let (f, s, g) = queued();
    let mut workers = FactoryHistoryWorkers::default();
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::Get(q(&s, &g)));
    assert!(finish(&mut workers, &f.store, receive)
        .get("error")
        .is_none());
    f.store.conn.execute("UPDATE factory_child_goal_runs SET record_digest=(CASE substr(record_digest,1,1) WHEN '0' THEN '1' ELSE '0' END)||substr(record_digest,2) WHERE goal_run_id=?1",[&g.goal.run.id]).unwrap();
    let expected = f.store.get_factory_child_goal(&q(&s, &g)).unwrap_err();
    let seq = f.store.sequence().unwrap();
    let counts = unchanged_execution(&f);
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::Get(q(&s, &g)));
    let got = finish(&mut workers, &f.store, receive);
    assert_eq!(got["error"], json!(expected));
    assert!(got.get("result").is_none());
    assert_eq!(got["revision"], seq);
    assert_eq!(f.store.sequence().unwrap(), seq);
    assert_eq!(unchanged_execution(&f), counts);
}
#[test]
fn original_expired_queue_request_never_starts_thread_or_acquires_database_pin() {
    let (f, s, g) = queued();
    let seq = f.store.sequence().unwrap();
    let mut workers = FactoryHistoryWorkers::default();
    let (expired, unused) = job(Instant::now() - Duration::from_millis(1));
    let error = match workers.start(
        &f.store,
        FactoryHistoryQuery::Get(q(&s, &g)),
        "expired".into(),
        expired,
    ) {
        Err(error) => error,
        Ok(()) => panic!("expired request admitted"),
    };
    assert_eq!(error.0.code, "RESOURCE_LIMIT");
    assert_eq!(error.0.side_effect_state, "not_started");
    assert_eq!(workers.database_owners_for_test(), 0);
    drop(error);
    assert!(unused.try_recv().is_err());
    assert_eq!(f.store.sequence().unwrap(), seq);
}
#[test]
fn own_sql_interrupt_does_not_interrupt_actor_transaction_or_release_raw_pin() {
    let (mut f, s, g) = queued();
    let mut workers = FactoryHistoryWorkers::default();
    let (started, interrupted) = workers.long_sql_next_for_test();
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::Get(q(&s, &g)));
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    let project = f.store.get(&s.root_task_id).unwrap().project_id;
    let added = f
        .store
        .create_registered(
            &project,
            "Actual concurrent Actor writer",
            "history-actor-write",
        )
        .unwrap();
    let tx = f.store.conn.unchecked_transaction().unwrap();
    workers.cancel_owned();
    let value: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1)",
            [&added.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(value);
    workers.release_database(&f.store);
    assert!(workers.database_owners_for_test() >= 1);
    tx.commit().unwrap();
    let got = finish(&mut workers, &f.store, receive);
    assert_eq!(got["error"]["code"], "CANCEL_REQUESTED");
    assert!(got.get("result").is_none());
    assert!(
        interrupted.recv_timeout(Duration::from_secs(1)).unwrap(),
        "real reader SQL must observe SQLITE_INTERRUPT"
    );
    assert_eq!(workers.database_owners_for_test(), 1);
    assert!(workers.database_current_for_test());
    let tx = f.store.conn.unchecked_transaction().unwrap();
    workers.release_database(&f.store);
    assert_eq!(workers.database_owners_for_test(), 1);
    tx.rollback().unwrap();
    workers.release_database(&f.store);
    assert_eq!(workers.database_owners_for_test(), 0);
    assert!(f.store.conn.is_autocommit());
}
#[test]
fn long_sql_uses_original_deadline_and_cannot_reply_success_after_it() {
    let (f, s, g) = queued();
    let mut workers = FactoryHistoryWorkers::default();
    let (started, interrupted) = workers.long_sql_next_for_test();
    let deadline = Instant::now() + Duration::from_secs(2);
    let (pending, receive) = job(deadline);
    if let Err(failure) = workers.start(
        &f.store,
        FactoryHistoryQuery::Get(q(&s, &g)),
        "deadline".into(),
        pending,
    ) {
        panic!("unexpected start refusal: {}", failure.0.code)
    }
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    let got = finish(&mut workers, &f.store, receive);
    assert!(Instant::now() >= deadline);
    assert_eq!(got["error"]["code"], "RESOURCE_LIMIT");
    assert!(got.get("result").is_none());
    assert!(
        interrupted.recv_timeout(Duration::from_secs(1)).unwrap(),
        "deadline really interrupts this SQL connection"
    );
    assert!(workers.database_current_for_test());
}
#[test]
fn panic_and_shutdown_really_join_while_actor_retains_database_identity() {
    let (f, s, g) = queued();
    let mut workers = FactoryHistoryWorkers::default();
    workers.panic_next_for_test();
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::Get(q(&s, &g)));
    let got = finish(&mut workers, &f.store, receive);
    assert_eq!(got["error"]["code"], "OUTCOME_UNKNOWN");
    assert_eq!(workers.database_owners_for_test(), 1);
    assert!(workers.database_current_for_test());
    let (started, _release) = workers.hold_next_for_test();
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::Get(q(&s, &g)));
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    workers.reap_owned_workers();
    assert_eq!(
        receive.recv_timeout(Duration::from_secs(1)).unwrap()["error"]["code"],
        "CANCEL_REQUESTED"
    );
    assert_eq!(workers.database_owners_for_test(), 1);
    assert!(workers.database_current_for_test());
    workers.release_database(&f.store);
    assert_eq!(workers.database_owners_for_test(), 0);
}
#[test]
fn socket_history_selector_preserves_exact_owner_read_envelope_and_domain() {
    let raw = json!({"api_version":1,"request_id":"history-route","token":"owner-token","method":"factory_child_goals.list","params":{"scope_id":"scope","task_id":"task","goal_run_id":null}});
    assert!(
        crate::ipc::factory_history_query(&raw.to_string(), "owner-token")
            .unwrap()
            .is_some()
    );
    for (field, value) in [
        ("token", json!("foreign")),
        ("api_version", json!(2)),
        ("request_id", json!("")),
        ("method", json!("factory_child_goals.cancel")),
    ] {
        let mut wrong = raw.clone();
        wrong[field] = value;
        assert!(
            crate::ipc::factory_history_query(&wrong.to_string(), "owner-token")
                .unwrap()
                .is_none()
        );
    }
    for (field, value) in [
        ("expected_revision", json!(1)),
        ("idempotency_key", json!("key")),
        (
            "params",
            json!({"scope_id":"scope","task_id":"task","authority":true}),
        ),
    ] {
        let mut wrong = raw.clone();
        wrong[field] = value;
        assert!(crate::ipc::factory_history_query(&wrong.to_string(), "owner-token").is_err());
    }
    let mut nulls = raw.clone();
    nulls["expected_revision"] = json!(null);
    nulls["idempotency_key"] = json!(null);
    assert!(
        crate::ipc::factory_history_query(&nulls.to_string(), "owner-token")
            .unwrap()
            .is_some()
    );
    let mut extra = raw;
    extra["unknown"] = json!(true);
    assert!(
        crate::ipc::factory_history_query(&extra.to_string(), "owner-token")
            .unwrap()
            .is_none()
    );
}

#[test]
fn original_revoked_history_remains_readonly_and_does_not_supply_effect_authority() {
    let (mut f, s, g) = queued();
    let revoked = f
        .store
        .revoke_factory_scope(&s.id, s.revision, "history-real-revoke")
        .unwrap();
    assert!(revoked.revoked);
    let request = FactoryChildGoalStart {
        scope_id: s.id.clone(),
        request: FactoryChildGoalSubmission {
            task_id: s.root_task_id.clone(),
            plan: g.plan.clone(),
            expected_factory_run_revision: f.store.get_factory_run(&s.id).unwrap().run.revision,
        },
    };
    let before = f
        .store
        .start_factory_child_goal(&request, revoked.revision, "history-never-new-execution")
        .unwrap_err();
    let expected = f.store.get_factory_child_goal(&q(&s, &g)).unwrap();
    let counts = unchanged_execution(&f);
    let seq = f.store.sequence().unwrap();
    let mut workers = FactoryHistoryWorkers::default();
    let receive = start(&mut workers, &f.store, FactoryHistoryQuery::Get(q(&s, &g)));
    let got = finish(&mut workers, &f.store, receive);
    assert_eq!(got["result"], json!(expected));
    assert_eq!(got["revision"], seq);
    assert!(f.store.data_input.is_none());
    assert!(f.store.graph_input.is_none());
    let after = f
        .store
        .start_factory_child_goal(&request, revoked.revision, "history-never-new-execution")
        .unwrap_err();
    assert_eq!(json!(before), json!(after));
    assert_eq!(unchanged_execution(&f), counts);
    assert_eq!(f.store.sequence().unwrap(), seq);
}
