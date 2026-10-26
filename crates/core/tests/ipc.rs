use agentbaton_core::ipc::{call, Daemon};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{symlink, PermissionsExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};
struct Server {
    runtime: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Server {
    fn new(runtime: &Path) -> Self {
        let daemon = Daemon::bind(runtime).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let run_stop = stop.clone();
        let thread = std::thread::spawn(move || daemon.run(run_stop).unwrap());
        Self {
            runtime: runtime.into(),
            stop,
            thread: Some(thread),
        }
    }
    fn request(&self, command: Value) -> Value {
        call(&self.runtime, command).unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn root() -> (tempfile::TempDir, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().canonicalize().unwrap().join("runtime");
    (d, path)
}
fn raw(runtime: &Path, body: &[u8]) -> Value {
    let mut socket = UnixStream::connect(runtime.join("batond.sock")).unwrap();
    socket.write_all(body).unwrap();
    let mut result = String::new();
    BufReader::new(socket).read_line(&mut result).unwrap();
    serde_json::from_str(&result).unwrap()
}
#[test]
fn socket_workflow_single_writer_and_restart_history() {
    let (_dir, path) = root();
    let server = Server::new(&path);
    assert_eq!(Daemon::bind(&path).err().unwrap().code, "WORKSPACE_BUSY");
    let response=server.request(json!({"method":"tasks.create","params":{"goal":"real persistent task"},"idempotency_key":"k"}));
    let task = response["result"].clone();
    assert_eq!(task["state"], "draft");
    assert_eq!(
        server.request(
            json!({"method":"tasks.cancel","params":{"task_id":task["id"]},"expected_revision":55})
        )["error"]["code"],
        "STALE_REVISION"
    );
    let response = server.request(
        json!({"method":"tasks.cancel","params":{"task_id":task["id"]},"expected_revision":1}),
    );
    assert_eq!(response["result"]["state"], "cancelled");
    drop(server);
    let restarted = Server::new(&path);
    assert_eq!(
        restarted.request(json!({"method":"system.snapshot","params":{}}))["result"]["tasks"][0]
            ["id"],
        task["id"]
    );
    assert_eq!(
        restarted.request(json!({"method":"events.subscribe","params":{"cursor":0,"limit":10}}))
            ["result"]["events"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn unauthenticated_unknown_version_and_malformed_do_not_mutate() {
    let (_dir, path) = root();
    let server = Server::new(&path);
    let response=raw(&path,b"{\"api_version\":1,\"request_id\":\"x\",\"token\":\"wrong\",\"method\":\"tasks.create\",\"params\":{\"goal\":\"no\"},\"idempotency_key\":\"k\"}\n");
    assert_eq!(response["error"]["code"], "AUTH_REQUIRED");
    assert_eq!(response["revision"], 0);
    assert_eq!(
        raw(&path, b"not json\n")["error"]["code"],
        "INVALID_REQUEST"
    );
    assert_eq!(server.request(json!({"method":"tasks.create","params":{"goal":"x","caller":"admin"},"idempotency_key":"k"}))["error"]["code"],"INVALID_REQUEST");
    assert_eq!(
        server.request(json!({"method":"system.snapshot","params":{},"bad_field":true}))["error"]
            ["code"],
        "INVALID_REQUEST"
    );
    let token = std::fs::read_to_string(path.join("session.token")).unwrap();
    let mut version=serde_json::to_vec(&json!({"api_version":99,"request_id":"v","token":token,"method":"system.snapshot","params":{}})).unwrap();
    version.push(b'\n');
    assert_eq!(raw(&path, &version)["error"]["code"], "VERSION_UNSUPPORTED");
    assert!(
        server.request(json!({"method":"tasks.list","params":{}}))["result"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn runtime_symlink_and_open_permissions_rejected_without_chmod() {
    let (dir, path) = root();
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(Daemon::bind(&path).err().unwrap().code, "PERMISSION_DENIED");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o755
    );
    let alias = dir.path().join("alias");
    symlink(&path, &alias).unwrap();
    assert_eq!(
        Daemon::bind(&alias).err().unwrap().code,
        "PERMISSION_DENIED"
    );
}
#[test]
fn credential_symlink_is_not_read_or_overwritten() {
    let (dir, path) = root();
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let target = dir.path().join("private");
    std::fs::write(&target, "must remain").unwrap();
    symlink(&target, path.join("session.token")).unwrap();
    assert_eq!(Daemon::bind(&path).err().unwrap().code, "PERMISSION_DENIED");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "must remain");
}
#[test]
fn oversized_frame_does_not_kill_daemon() {
    let (_dir, path) = root();
    let server = Server::new(&path);
    let payload = vec![b'x'; 1_048_577];
    let mut socket = UnixStream::connect(path.join("batond.sock")).unwrap();
    socket
        .set_write_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    // Rejecting an oversized peer is allowed to close the socket before the
    // sender's large write returns. Darwin can report EPIPE even when all bytes
    // reached the bounded reader. Error delivery is best effort; parser tests
    // assert RESOURCE_LIMIT directly, independently of socket delivery.
    let mut fully_written = true;
    for chunk in payload.chunks(4096) {
        if let Err(error) = socket.write_all(chunk) {
            assert!(
                matches!(
                    error.kind(),
                    std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::NotConnected
                ),
                "unexpected oversized write error: {error}"
            );
            fully_written = false;
            break;
        }
    }
    if let Err(error) = socket.shutdown(std::net::Shutdown::Write) {
        assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
            ),
            "unexpected shutdown error: {error}"
        );
    }
    let mut response = String::new();
    match BufReader::new(socket).read_line(&mut response) {
        Ok(0) => {} // Peer rejected and closed without an error frame.
        Ok(_) => {
            let response: Value = serde_json::from_str(&response).unwrap();
            if fully_written {
                assert_eq!(response["error"]["code"], "RESOURCE_LIMIT");
            } else {
                // An interrupted write may leave a truncated transport,
                // rather than deliver the full oversized frame.
                assert!(
                    response["error"]["code"] == "RESOURCE_LIMIT"
                        || response["error"]["code"] == "PROVIDER_DISCONNECTED"
                );
            }
        }
        Err(error) => assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
            ),
            "unexpected oversized read error: {error}"
        ),
    }
    let snapshot = server.request(json!({"method":"system.snapshot","params":{}}));
    assert_eq!(snapshot["result"]["sequence"], 0);
    assert_eq!(snapshot["result"]["tasks"], json!([]));
}
