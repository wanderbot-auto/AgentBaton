use super::*;
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
};

const ORIGINAL: &str = "14d2e12e-bb40-4baf-86b1-95ffe94e4f88";
const OTHER: &str = "735f6fa9-7067-476d-b605-c6a85cd768b8";
const REQUEST_ID: &str = "c92a9c20-fb94-48dc-8e34-7038bfdb2cbd";
const HOST_JOURNAL: &str = "host-registration-recovery.json";

struct PrivateRuntime(PathBuf);

impl PrivateRuntime {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = Path::new("/tmp").canonicalize().unwrap();
        let path = base.join(format!(
            "ab-host-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        Self(path)
    }

    fn call(&self, operation: Operation, args: Value) -> Value {
        bridge(operation, args, &self.0, |command| {
            ipc::call(&self.0, command)
        })
    }

    fn pending(&self) -> HostRegistrationIntent {
        serde_json::from_value(self.call(Operation::Pending, json!({}))["result"]["intent"].clone())
            .unwrap()
    }

    fn journal_bytes(&self) -> Vec<u8> {
        fs::read(self.0.join(HOST_JOURNAL)).unwrap()
    }
}

impl Drop for PrivateRuntime {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

struct RunningDaemon {
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<CoreResult<()>>>,
}

impl RunningDaemon {
    fn start(runtime: &PrivateRuntime) -> Self {
        let daemon = ipc::Daemon::bind(&runtime.0).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let quit = stop.clone();
        let worker = thread::spawn(move || daemon.run(quit));
        Self {
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for RunningDaemon {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let finished = self.worker.take().unwrap().join();
        if !thread::panicking() {
            finished.unwrap().unwrap();
        }
    }
}

fn registration() -> Value {
    json!({
        "schema_version":1,
        "name":"Synthetic Host",
        "declared_platform":"linux",
        "endpoint":{
            "expected_host_identity":"synthetic-host",
            "address":"example.invalid",
            "user":"baton",
            "port":22,
            "host_key_alias":"synthetic-alias"
        },
        "known_hosts":{"reference":"known-hosts:synthetic","file_sha256":"a".repeat(64)},
        "transport_identity_ref":"identity:synthetic",
        "configuration":{"reference":"config:synthetic","declared_ownership":"external_read_only"},
        "directory_mappings":[{"id":"source","remote_root":"/synthetic/source"}]
    })
}

fn bridge(
    operation: Operation,
    args: Value,
    runtime: &Path,
    core: impl FnMut(Value) -> CoreResult<Value>,
) -> Value {
    checked_arguments(operation, &args)
        .map_err(Failure::from)
        .and_then(|args| execute(operation, args, runtime, core))
        .unwrap_or_else(Failure::into_response)
}

fn success(reply: &Value) -> &Value {
    assert!(reply.get("error").is_none(), "{reply}");
    &reply["result"]
}

fn lookup_frame() -> Value {
    json!({"method":"hosts.find_registration","params":{"key":ORIGINAL}})
}

fn register_frame(key: &str) -> Value {
    json!({"method":"hosts.register","params":registration(),"idempotency_key":key})
}

fn register_args() -> Value {
    json!({"registration":registration(),"idempotencyKey":ORIGINAL})
}

fn ack_args(entry_id: &str, receipt: &Value) -> Value {
    json!({
        "entryId":entry_id,
        "hostId":receipt["host_id"],
        "createdSequence":receipt["created_sequence"],
    })
}

fn reply_with(result: Value) -> Value {
    json!({"request_id":REQUEST_ID,"revision":77,"warnings":[],"result":result})
}

fn establish_pending(runtime: &PrivateRuntime) -> HostRegistrationIntent {
    let lost = bridge(
        Operation::Register,
        register_args(),
        &runtime.0,
        |command| {
            assert_eq!(command, register_frame(ORIGINAL));
            Err(CoreError::transport())
        },
    );
    assert_eq!(lost["error"]["code"], "OUTCOME_UNKNOWN");
    runtime.pending()
}

#[test]
fn authenticated_socket_register_restart_reconcile_read_and_acknowledge() {
    let runtime = PrivateRuntime::new();
    let first_daemon = RunningDaemon::start(&runtime);
    let created = runtime.call(Operation::Register, register_args());
    let submitted = success(&created).clone();
    let intent = runtime.pending();
    assert_eq!(submitted["submission_version"], 1);
    assert_eq!(submitted["entry_id"], intent.entry_id);
    assert_eq!(intent.request_key, ORIGINAL);
    let receipt = &submitted["receipt"];
    assert_eq!(receipt["revision"], 1);
    assert_eq!(receipt["registration"], registration());

    let second = ipc::call(&runtime.0, register_frame(OTHER)).unwrap();
    assert_ne!(success(&second)["host_id"], receipt["host_id"]);
    let pending_bytes = runtime.journal_bytes();
    drop(first_daemon);
    assert!(!runtime.0.join("batond.sock").exists());
    assert_eq!(runtime.pending(), intent);
    assert_eq!(runtime.journal_bytes(), pending_bytes);

    let _daemon = RunningDaemon::start(&runtime);
    let reconciled = runtime.call(Operation::Reconcile, json!({"entryId":intent.entry_id}));
    assert_eq!(
        success(&reconciled),
        &json!({"recovery_version":1,"entry_id":intent.entry_id,
                "lookup":{"lookup_version":1,"key":ORIGINAL,"receipt":receipt}}),
    );
    assert_ne!(reconciled["revision"], receipt["created_sequence"]);
    let fetched = runtime.call(Operation::Get, json!({"hostId":receipt["host_id"]}));
    assert_eq!(success(&fetched)["record"]["id"], receipt["host_id"]);
    assert_eq!(
        success(&fetched)["record"]["created_sequence"],
        receipt["created_sequence"]
    );
    let listed = runtime.call(Operation::List, json!({"request":{"limit":10}}));
    assert_eq!(success(&listed)["hosts"].as_array().unwrap().len(), 2);

    let wrong_sequence = runtime.call(
        Operation::Acknowledge,
        json!({"entryId":intent.entry_id,"hostId":receipt["host_id"],
               "createdSequence":reconciled["revision"]}),
    );
    assert_eq!(wrong_sequence["error"]["code"], "CONFIG_CONFLICT");
    assert_eq!(runtime.journal_bytes(), pending_bytes);
    let args = ack_args(&intent.entry_id, receipt);
    let acknowledged = runtime.call(Operation::Acknowledge, args.clone());
    assert_eq!(
        success(&acknowledged),
        &json!({"acknowledgement_version":1,"entry_id":intent.entry_id,"acknowledged":true})
    );
    assert_eq!(
        success(&runtime.call(Operation::Pending, json!({}))),
        &json!({"pending_version":1,"intent":null})
    );
    let repeated = bridge(Operation::Acknowledge, args, &runtime.0, |_| {
        panic!("an already-cleared entry cannot query Core")
    });
    assert_eq!(success(&repeated)["acknowledged"], false);
    assert_eq!(
        success(&runtime.call(Operation::List, json!({"request":{"limit":10}})))["hosts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn a_lost_successful_write_response_recovers_the_original_receipt_without_resending() {
    let runtime = PrivateRuntime::new();
    let _daemon = RunningDaemon::start(&runtime);
    let mut committed = None;
    let lost = bridge(
        Operation::Register,
        register_args(),
        &runtime.0,
        |command| {
            assert_eq!(command, register_frame(ORIGINAL));
            assert_eq!(runtime.pending().request_key, ORIGINAL);
            let result = ipc::call(&runtime.0, command)?;
            committed = Some(success(&result).clone());
            // The shared client can classify an incomplete write-reply frame as
            // RESOURCE_LIMIT even though the daemon has already committed.
            Err(CoreError::new("RESOURCE_LIMIT", "Incomplete reply frame"))
        },
    );
    assert_eq!(lost["error"]["code"], "OUTCOME_UNKNOWN");
    assert_eq!(lost["error"]["side_effect_state"], "unknown");
    let intent = runtime.pending();
    let raw = runtime.journal_bytes();
    let duplicate = bridge(
        Operation::Register,
        json!({"registration":registration(),"idempotencyKey":OTHER}),
        &runtime.0,
        |_| panic!("a new key cannot displace the pending request"),
    );
    assert_eq!(duplicate["error"]["code"], "CLIENT_RECOVERY_REQUIRED");
    let mut calls = Vec::new();
    let recovered = bridge(
        Operation::Retry,
        json!({"entryId":intent.entry_id}),
        &runtime.0,
        |command| {
            calls.push(command.clone());
            ipc::call(&runtime.0, command)
        },
    );
    assert_eq!(calls, [lookup_frame()]);
    assert_eq!(success(&recovered)["receipt"], committed.unwrap());
    assert_eq!(success(&recovered)["entry_id"], intent.entry_id);
    assert_eq!(runtime.journal_bytes(), raw);
}

#[test]
fn an_explicit_retry_after_a_successful_lookup_miss_uses_the_exact_original_key_and_body() {
    let runtime = PrivateRuntime::new();
    let intent = establish_pending(&runtime);
    assert!(!runtime.0.join("batond.sock").exists());
    let _daemon = RunningDaemon::start(&runtime);
    let reconciled = bridge(
        Operation::Reconcile,
        json!({"entryId":intent.entry_id}),
        &runtime.0,
        |command| {
            assert_eq!(command, lookup_frame());
            ipc::call(&runtime.0, command)
        },
    );
    assert_eq!(
        success(&reconciled)["lookup"],
        json!({"lookup_version":1,"key":ORIGINAL,"receipt":null})
    );
    let mut calls = Vec::new();
    let recovered = bridge(
        Operation::Retry,
        json!({"entryId":intent.entry_id}),
        &runtime.0,
        |command| {
            calls.push(command.clone());
            ipc::call(&runtime.0, command)
        },
    );
    assert_eq!(calls, [lookup_frame(), register_frame(ORIGINAL)]);
    assert_eq!(
        success(&recovered)["receipt"]["registration"],
        registration()
    );
    assert_eq!(runtime.pending(), intent);
}

#[test]
fn lookup_errors_and_ambiguous_results_never_write_or_acknowledge() {
    let runtime = PrivateRuntime::new();
    let intent = establish_pending(&runtime);
    let pending = runtime.journal_bytes();
    let error = json!({"request_id":REQUEST_ID,"revision":77,"warnings":[],
        "error":CoreError::new("AUTH_REQUIRED","Local authentication failed")});
    let invalid = [
        (Value::Null, "INVALID_CORE_RESPONSE"),
        (
            reply_with(json!({"lookup_version":1,"key":ORIGINAL})),
            "INVALID_CORE_RESPONSE",
        ),
        (
            reply_with(json!({"lookup_version":1,"key":ORIGINAL,"receipt":{} })),
            "INVALID_CORE_RESPONSE",
        ),
        (
            reply_with(json!({"lookup_version":1,"key":ORIGINAL,"receipt":null,"extra":true})),
            "INVALID_CORE_RESPONSE",
        ),
        (
            reply_with(json!({"lookup_version":2,"key":ORIGINAL,"receipt":null})),
            "VERSION_UNSUPPORTED",
        ),
        (
            reply_with(json!({"lookup_version":1,"key":OTHER,"receipt":null})),
            "CONFIG_CONFLICT",
        ),
        (error.clone(), "AUTH_REQUIRED"),
    ];
    for (reply, code) in invalid {
        for operation in [
            Operation::Retry,
            Operation::Reconcile,
            Operation::Acknowledge,
        ] {
            let args = if matches!(operation, Operation::Acknowledge) {
                json!({"entryId":intent.entry_id,"hostId":OTHER,"createdSequence":1})
            } else {
                json!({"entryId":intent.entry_id})
            };
            let mut calls = 0;
            let result = bridge(operation, args, &runtime.0, |command| {
                calls += 1;
                assert_eq!(command, lookup_frame());
                Ok(reply.clone())
            });
            assert_eq!(calls, 1);
            assert_eq!(result["error"]["code"], code, "{result}");
            if code == "AUTH_REQUIRED" {
                assert_eq!(result, error);
            }
            assert_eq!(runtime.journal_bytes(), pending);
        }
    }
    let disconnected = bridge(
        Operation::Retry,
        json!({"entryId":intent.entry_id}),
        &runtime.0,
        |command| {
            assert_eq!(command, lookup_frame());
            Err(CoreError::transport())
        },
    );
    assert_eq!(disconnected["error"]["code"], "PROVIDER_DISCONNECTED");
    assert_eq!(runtime.journal_bytes(), pending);
}

#[test]
fn malformed_write_envelopes_preserve_uncertainty_and_the_intent() {
    for malformed in [
        reply_with(json!({"receipt_version":1})),
        json!({"request_id":null,"revision":67,"warnings":[],"result":{}}),
        json!({"request_id":REQUEST_ID,"revision":67,"warnings":[],"result":{},
               "error":CoreError::new("AUTH_REQUIRED","local authentication failed")}),
        json!({"request_id":REQUEST_ID,"revision":67,"warnings":[],
               "error":{"code":"INVALID_REQUEST","message":"failure","retryable":false,
               "side_effect_state":"not_started"}}),
        reply_with(json!({"unbounded":"x".repeat(MAX_CORE_RESPONSE)})),
    ] {
        let runtime = PrivateRuntime::new();
        let reply = bridge(
            Operation::Register,
            register_args(),
            &runtime.0,
            |command| {
                assert_eq!(command, register_frame(ORIGINAL));
                Ok(malformed.clone())
            },
        );
        assert_eq!(reply["error"]["code"], "OUTCOME_UNKNOWN");
        assert_eq!(reply["error"]["side_effect_state"], "unknown");
        assert_eq!(runtime.pending().request_key, ORIGINAL);
    }

    let runtime = PrivateRuntime::new();
    let denied = json!({"request_id":REQUEST_ID,"revision":99,"warnings":[],
        "error":CoreError::new("VERSION_UNSUPPORTED","Unsupported local API version")});
    let reply = bridge(Operation::Register, register_args(), &runtime.0, |_| {
        Ok(denied.clone())
    });
    assert_eq!(reply, denied);
    assert_eq!(runtime.pending().request_key, ORIGINAL);
}

#[test]
fn a_racing_ack_and_replacement_survive_the_original_acknowledgement() {
    let runtime = PrivateRuntime::new();
    let _daemon = RunningDaemon::start(&runtime);
    let created = runtime.call(Operation::Register, register_args());
    let intent = runtime.pending();
    let args = ack_args(&intent.entry_id, &success(&created)["receipt"]);
    let result = bridge(
        Operation::Acknowledge,
        args.clone(),
        &runtime.0,
        |command| {
            assert_eq!(command, lookup_frame());
            let lookup = ipc::call(&runtime.0, command)?;
            thread::scope(|scope| {
                scope
                    .spawn(|| {
                        assert_eq!(
                            success(&runtime.call(Operation::Acknowledge, args.clone()))
                                ["acknowledged"],
                            true
                        );
                        let journal = ClientRecoveryJournal::new(&runtime.0).unwrap();
                        journal
                            .reserve_host_registration(OTHER.into(), intent.registration.clone())
                            .unwrap();
                    })
                    .join()
                    .unwrap();
            });
            Ok(lookup)
        },
    );
    assert_eq!(success(&result)["acknowledged"], false);
    assert_eq!(runtime.pending().request_key, OTHER);
    let stale = bridge(
        Operation::Retry,
        json!({"entryId":intent.entry_id}),
        &runtime.0,
        |_| panic!("a stale entry must never query the daemon"),
    );
    assert_eq!(stale["error"]["code"], "CLIENT_RECOVERY_REQUIRED");
}

#[test]
fn a_retry_miss_rechecks_the_pending_intent_before_writing() {
    let runtime = PrivateRuntime::new();
    let _daemon = RunningDaemon::start(&runtime);
    let intent = establish_pending(&runtime);
    let result = bridge(
        Operation::Retry,
        json!({"entryId":intent.entry_id}),
        &runtime.0,
        |command| {
            assert_eq!(command, lookup_frame());
            let miss = ipc::call(&runtime.0, command)?;
            assert_eq!(success(&miss)["receipt"], Value::Null);
            let raced = ipc::call(&runtime.0, register_frame(ORIGINAL))?;
            assert_eq!(
                success(&runtime.call(
                    Operation::Acknowledge,
                    ack_args(&intent.entry_id, success(&raced))
                ))["acknowledged"],
                true
            );
            let journal = ClientRecoveryJournal::new(&runtime.0)?;
            journal.reserve_host_registration(OTHER.into(), intent.registration.clone())?;
            Ok(miss)
        },
    );
    assert_eq!(result["error"]["code"], "CLIENT_RECOVERY_REQUIRED");
    assert_eq!(runtime.pending().request_key, OTHER);
}

#[test]
fn fixed_arguments_and_bounds_are_enforced_before_any_core_dispatch() {
    let runtime = PrivateRuntime::new();
    let mut too_large = registration();
    too_large["name"] = json!("x".repeat(MAX_PARAMS));
    let mut invalid_registration = registration();
    invalid_registration["endpoint"]["extra"] = json!(true);
    let invalid = [
        (
            Operation::Register,
            json!({"registration":registration(),"idempotencyKey":ORIGINAL,"expectedRevision":2}),
        ),
        (
            Operation::Register,
            json!({"registration":too_large,"idempotencyKey":ORIGINAL}),
        ),
        (
            Operation::Register,
            json!({"registration":invalid_registration,"idempotencyKey":ORIGINAL}),
        ),
        (
            Operation::Register,
            json!({"registration":registration(),"idempotencyKey":"new/key"}),
        ),
        (
            Operation::List,
            json!({"request":{"limit":1,"idempotency_key":OTHER}}),
        ),
        (
            Operation::Get,
            json!({"hostId":OTHER,"method":"hosts.register"}),
        ),
        (Operation::Pending, json!({"runtime":"/outside"})),
        (
            Operation::Reconcile,
            json!({"entryId":OTHER,"requestKey":ORIGINAL}),
        ),
        (
            Operation::Retry,
            json!({"entryId":OTHER,"registration":registration()}),
        ),
        (
            Operation::Acknowledge,
            json!({"entryId":ORIGINAL,"hostId":OTHER,"createdSequence":1,"receipt":{}}),
        ),
        (
            Operation::Acknowledge,
            json!({"entryId":ORIGINAL,"hostId":OTHER,"createdSequence":MAX_CREATED_SEQUENCE+1}),
        ),
    ];
    for (operation, args) in invalid {
        let refused = bridge(operation, args, &runtime.0, |_| {
            panic!("invalid arguments dispatched Core")
        });
        assert_eq!(refused["error"]["code"], "INVALID_REQUEST");
        assert_eq!(refused["error"]["side_effect_state"], "not_started");
        assert!(!runtime.0.join(HOST_JOURNAL).exists());
    }

    let no_journal = runtime.0.join("missing");
    let refused = bridge(Operation::Register, register_args(), &no_journal, |_| {
        panic!("write without durable intent")
    });
    assert_eq!(refused["error"]["code"], "CLIENT_RECOVERY_UNAVAILABLE");
}
