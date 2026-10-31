//! FIFO custody and the original real Owner dispatcher. These are source tests;
//! they do not claim socket latency, CI performance or a diagnosed queue cause.
use super::*;

fn request(token: &str, id: &str, method: &str) -> Value {
    json!({"api_version":1,"request_id":id,"token":token,"method":method,
        "params":{"scope_id":"generated-missing-scope"},
        "expected_revision":null,"idempotency_key":null})
}
fn job(raw: Value) -> (ReadyRequest, Receiver<Value>) {
    let (send, receive) = mpsc::sync_channel(1);
    (
        ReadyRequest {
            raw: raw.to_string(),
            reply: ReadyReply::Wire(send),
            deadline: Instant::now() + Duration::from_secs(60),
            factory_dispatch: None,
        },
        receive,
    )
}
fn id(job: &ReadyRequest) -> String {
    serde_json::from_str::<Value>(&job.raw).unwrap()["request_id"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn send(sender: &SyncSender<ReadyRequest>, job: ReadyRequest) {
    assert!(sender.try_send(job).is_ok());
}
fn runtime() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("ab-owner-read-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in("/tmp")
        .unwrap()
}

#[test]
fn only_closed_owner_read_envelopes_can_enter_before_ticks() {
    let valid = request("owner", "read", "factory_runs.get");
    let (read, _) = job(valid.clone());
    assert!(queued_owner_factory_run_read(&read, "owner"));
    for (key, value) in [
        ("api_version", json!(2)),
        ("token", json!("other-owner")),
        ("request_id", json!("")),
        ("request_id", json!("x".repeat(129))),
        ("method", json!("factory_runs.cancel")),
        ("method", json!("factory_child_goals.list")),
        ("expected_revision", json!(0)),
        ("idempotency_key", json!("")),
        ("params", json!({})),
        ("params", json!({"scope_id":3})),
        ("params", json!({"scope_id":"scope","extra":true})),
        ("params", json!({"scope_id":"scope","handle_id":"handle"})),
        ("scoped_token", json!("scope-token")),
        ("handle_id", json!("handle")),
        ("extra", json!(true)),
    ] {
        let mut raw = valid.clone();
        raw[key] = value;
        let (candidate, _) = job(raw);
        assert!(!queued_owner_factory_run_read(&candidate, "owner"), "{key}");
    }
    for raw in [
        "not-json",
        r#"{"api_version":1,"request_id":"r","token":"owner","method":"factory_runs.get","params":{"scope_id":"one","scope_id":"two"}}"#,
        r#"{"api_version":1,"request_id":"r","token":"owner","method":"factory_runs.get","method":"factory_runs.cancel","params":{"scope_id":"one"}}"#,
    ] {
        let (mut candidate, _) = job(valid.clone());
        candidate.raw = raw.into();
        assert!(!queued_owner_factory_run_read(&candidate, "owner"));
    }
}

#[test]
fn held_write_blocks_later_read_and_keeps_original_fifo_and_reply_ownership() {
    let (send_queue, ready) = mpsc::sync_channel(16);
    let (write, write_reply) = job(request("owner", "write", "factory_runs.cancel"));
    let (read, read_reply) = job(request("owner", "read", "factory_runs.get"));
    send(&send_queue, write);
    send(&send_queue, read);
    let (mut turn, early) = ReadyTurn::queue_head(&ready, "owner");
    assert!(early.is_none(), "must never scan past a queued write");
    assert!(write_reply.try_recv().is_err());
    assert!(read_reply.try_recv().is_err());
    let first = turn.next(&ready).unwrap();
    assert_eq!(id(&first), "write");
    first.reply.try_send(json!({"owned":"write"})).unwrap();
    assert_eq!(write_reply.try_recv().unwrap()["owned"], "write");
    let second = turn.next(&ready).unwrap();
    assert_eq!(id(&second), "read");
    second.reply.try_send(json!({"owned":"read"})).unwrap();
    assert_eq!(read_reply.try_recv().unwrap()["owned"], "read");
    assert!(turn.next(&ready).is_none());
    assert!(write_reply.try_recv().is_err());
    assert!(read_reply.try_recv().is_err());
}

#[test]
fn early_head_and_later_drain_share_sixteen_and_leave_the_seventeenth_queued() {
    let (send_queue, ready) = mpsc::sync_channel(16);
    for index in 0..16 {
        let method = if index == 0 {
            "factory_runs.get"
        } else {
            "factory_runs.cancel"
        };
        send(
            &send_queue,
            job(request("owner", &index.to_string(), method)).0,
        );
    }
    let (mut turn, early) = ReadyTurn::queue_head(&ready, "owner");
    assert_eq!(id(&early.unwrap()), "0");
    // Custody frees one existing channel slot; it does not enlarge the channel.
    send(
        &send_queue,
        job(request("owner", "16", "factory_runs.get")).0,
    );
    let mut observed = Vec::new();
    while let Some(next) = turn.next(&ready) {
        observed.push(id(&next));
    }
    assert_eq!(observed, (1..16).map(|i| i.to_string()).collect::<Vec<_>>());
    assert_eq!(id(&ready.try_recv().unwrap()), "16");
    assert!(ready.try_recv().is_err());
}

#[test]
fn held_head_still_counts_in_the_original_sixteen_admissions() {
    let (send_queue, ready) = mpsc::sync_channel(16);
    for index in 0..16 {
        send(
            &send_queue,
            job(request("owner", &index.to_string(), "factory_runs.cancel")).0,
        );
    }
    let (mut turn, early) = ReadyTurn::queue_head(&ready, "owner");
    assert!(early.is_none());
    send(
        &send_queue,
        job(request("owner", "16", "factory_runs.get")).0,
    );
    let mut observed = Vec::new();
    while let Some(next) = turn.next(&ready) {
        observed.push(id(&next));
    }
    assert_eq!(observed, (0..16).map(|i| i.to_string()).collect::<Vec<_>>());
    assert_eq!(id(&ready.try_recv().unwrap()), "16");
}

#[test]
fn a_read_queued_after_the_single_head_observation_waits_for_the_normal_drain() {
    let (send_queue, ready) = mpsc::sync_channel(16);
    let (mut turn, early) = ReadyTurn::queue_head(&ready, "owner");
    assert!(early.is_none());
    send(
        &send_queue,
        job(request("owner", "later", "factory_runs.get")).0,
    );
    assert_eq!(id(&turn.next(&ready).unwrap()), "later");
    assert!(turn.next(&ready).is_none());
}

#[test]
fn original_owner_handler_keeps_complete_wire_errors_and_read_only_sequence() {
    let runtime = runtime();
    let mut daemon = Daemon::bind(&runtime.path().canonicalize().unwrap()).unwrap();
    let raw = request(&daemon.token, "original-wire", "factory_runs.get");
    let sequence = daemon.store.sequence().unwrap();
    let expected = daemon.handle(&raw.to_string());
    assert!(
        expected.get("error").is_some(),
        "missing Scope cannot become a result"
    );
    let (candidate, response) = job(raw);
    assert!(queued_owner_factory_run_read(&candidate, &daemon.token));
    daemon.handle_ready_request(candidate);
    daemon.service_after_early_owner_read().unwrap();
    assert_eq!(response.try_recv().unwrap(), expected);
    assert_eq!(daemon.store.sequence().unwrap(), sequence);
}

#[test]
fn eligible_read_keeps_its_original_expired_deadline_and_never_dispatches() {
    let runtime = runtime();
    let mut daemon = Daemon::bind(&runtime.path().canonicalize().unwrap()).unwrap();
    let sequence = daemon.store.sequence().unwrap();
    let (mut candidate, response) = job(request(&daemon.token, "expired", "factory_runs.get"));
    candidate.deadline = Instant::now() - Duration::from_secs(1);
    assert!(queued_owner_factory_run_read(&candidate, &daemon.token));
    daemon.handle_ready_request(candidate);
    daemon.service_after_early_owner_read().unwrap();
    let actual = response.try_recv().unwrap();
    assert_eq!(actual["request_id"], Value::Null);
    assert_eq!(actual["error"]["code"], "RESOURCE_LIMIT");
    assert_eq!(actual["error"]["side_effect_state"], "not_started");
    assert_eq!(actual["error"]["retryable"], false);
    assert_eq!(actual["revision"], 0);
    assert_eq!(actual["warnings"], json!([]));
    assert!(actual.get("result").is_none());
    assert_eq!(daemon.store.sequence().unwrap(), sequence);
}

fn retained_failure() -> CoreError {
    CoreError {
        code: "RESOURCE_LIMIT".into(),
        message: "generated retained admission failure".into(),
        retryable: false,
        side_effect_state: "unknown".into(),
        details_ref: Some("generated-original-evidence".into()),
    }
}

#[test]
fn original_fatal_before_head_observation_exits_and_cleans_the_socket() {
    let runtime = runtime();
    let mut daemon = Daemon::bind(&runtime.path().canonicalize().unwrap()).unwrap();
    let socket = daemon.socket.clone();
    let expected = serde_json::to_value(retained_failure()).unwrap();
    daemon.factory_fatal_error = Some(retained_failure());
    let error = daemon.run(Arc::new(AtomicBool::new(false))).unwrap_err();
    assert_eq!(serde_json::to_value(error).unwrap(), expected);
    assert!(!socket.exists(), "original Drop cleanup must still run");
}

#[test]
fn early_post_request_service_propagates_retained_fatal_with_original_fields() {
    let runtime = runtime();
    let mut daemon = Daemon::bind(&runtime.path().canonicalize().unwrap()).unwrap();
    let sequence = daemon.store.sequence().unwrap();
    let (candidate, response) = job(request(
        &daemon.token,
        "before-retained-fatal",
        "factory_runs.get",
    ));
    daemon.handle_ready_request(candidate);
    let original_reply = response.try_recv().unwrap();
    let expected = serde_json::to_value(retained_failure()).unwrap();
    // A retained error at the post-request boundary must survive the unchanged
    // worker service and be taken before the caller can enter more ticks.
    daemon.factory_fatal_error = Some(retained_failure());
    let error = daemon.service_after_early_owner_read().unwrap_err();
    assert_eq!(serde_json::to_value(error).unwrap(), expected);
    assert!(daemon.factory_fatal_error.is_none());
    assert!(original_reply.get("error").is_some());
    assert!(original_reply.get("result").is_none());
    assert!(
        response.try_recv().is_err(),
        "no duplicate reply or request"
    );
    assert_eq!(daemon.store.sequence().unwrap(), sequence);
}

#[test]
fn rejected_owner_identity_stays_in_normal_fifo_and_original_auth_denial() {
    let runtime = runtime();
    let mut daemon = Daemon::bind(&runtime.path().canonicalize().unwrap()).unwrap();
    let sequence = daemon.store.sequence().unwrap();
    let (send_queue, ready) = mpsc::sync_channel(16);
    let (invalid_owner, response) = job(request(
        "another-generated-owner",
        "denied",
        "factory_runs.get",
    ));
    let raw = invalid_owner.raw.clone();
    let original = daemon.handle(&raw);
    send(&send_queue, invalid_owner);
    let (mut turn, early) = ReadyTurn::queue_head(&ready, &daemon.token);
    assert!(early.is_none());
    daemon.handle_ready_request(turn.next(&ready).unwrap());
    daemon.service_after_ready_request().unwrap();
    let actual = response.try_recv().unwrap();
    assert_eq!(actual, original);
    assert_eq!(actual["error"]["code"], "AUTH_REQUIRED");
    assert_eq!(actual["error"]["side_effect_state"], "not_started");
    assert_eq!(actual["revision"], 0);
    assert!(turn.next(&ready).is_none());
    assert_eq!(daemon.store.sequence().unwrap(), sequence);
}
