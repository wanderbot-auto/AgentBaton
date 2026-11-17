//! Actual authenticated Unix IPC, real SQLite and immutable local filesystem.
use agentbaton_core::{
    history_export::{published_path, sha, Preview, Receipt, Reference},
    ipc::{self, Daemon},
    storage::Store,
};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        net::UnixStream,
    },
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};
struct Running {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<agentbaton_core::Result<()>>>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap().unwrap();
    }
}
#[test]
fn authenticated_daemon_preview_publish_reconcile_and_reject_bad_envelopes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir(root.join("source")).unwrap();
    drop(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(root.join("baton.sqlite3"))
            .unwrap(),
    );
    let mut store = Store::open(&root.join("baton.sqlite3")).unwrap();
    let project = store
        .import_project(&root.join("source"), "IPC project", "project")
        .unwrap();
    let task = store
        .create_registered(
            &project.id,
            "Authorization: Bearer KNOWN-INTEGRATION-SECRET",
            "task",
        )
        .unwrap();
    let before = store.sequence().unwrap();
    drop(store);
    // Daemon bind secures its database before opening SQLite; native clients share
    // this same runtime, never a renderer-specified output path.
    let daemon = Daemon::bind(&root).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let _running = Running {
        stop,
        thread: Some(std::thread::spawn(move || daemon.run(signal))),
    };
    let selection = json!({"version":1,"project_id":project.id,"from_sequence":0,"to_sequence":null,"tasks":true,"messages":false,"events":false});
    let p: Preview = serde_json::from_value(
        ipc::call(
            &root,
            json!({"method":"history_exports.preview","params":selection}),
        )
        .unwrap()["result"]
            .clone(),
    )
    .unwrap();
    let reference: Reference = p.reference;
    let wire = ipc::call(
        &root,
        json!({"method":"history_exports.publish","params":reference}),
    )
    .unwrap();
    let receipt: Receipt = serde_json::from_value(wire["result"].clone()).unwrap();
    assert_eq!(receipt.state, "ready");
    let path = published_path(&root.join("history-exports"), &reference).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(sha(&bytes), reference.package_sha256);
    assert!(!String::from_utf8(bytes)
        .unwrap()
        .contains("KNOWN-INTEGRATION-SECRET"));
    assert_eq!(
        ipc::call(
            &root,
            json!({"method":"history_exports.get","params":reference})
        )
        .unwrap()["result"]["state"],
        "ready"
    );
    assert_eq!(ipc::call(&root,json!({"method":"history_exports.preview","params":selection,"idempotency_key":"unexpected"})).unwrap()["error"]["code"],"INVALID_REQUEST");
    let mut unsafe_selection = selection;
    unsafe_selection["destination"] = json!("/tmp/unapproved");
    assert_eq!(
        ipc::call(
            &root,
            json!({"method":"history_exports.preview","params":unsafe_selection})
        )
        .unwrap()["error"]["code"],
        "INVALID_REQUEST"
    );
    let mut stream = UnixStream::connect(root.join("batond.sock")).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    writeln!(stream,"{}",json!({"api_version":1,"request_id":"unauthorized","token":"wrong","method":"history_exports.publish","params":reference})).unwrap();
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&line).unwrap()["error"]["code"],
        "AUTH_REQUIRED"
    );
    let snapshot = ipc::call(&root, json!({"method":"system.snapshot","params":{}})).unwrap();
    assert_eq!(snapshot["result"]["sequence"], before);
    assert_eq!(snapshot["result"]["tasks"][0]["id"], task.id);
}
