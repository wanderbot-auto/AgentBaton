use agentbaton_core::ipc::{call, Daemon};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Command,
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
    fn start(runtime: &Path) -> Self {
        let daemon = Daemon::bind(runtime).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let run = stop.clone();
        let thread = std::thread::spawn(move || daemon.run(run).unwrap());
        Self {
            runtime: runtime.into(),
            stop,
            thread: Some(thread),
        }
    }
    fn request(&self, cmd: Value) -> Value {
        call(&self.runtime, cmd).unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn git(root: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().into()
}
fn repo(root: &Path) -> String {
    std::fs::create_dir(root).unwrap();
    git(root, &["init", "-q"]);
    std::fs::write(root.join("file"), "baseline").unwrap();
    git(root, &["add", "file"]);
    git(root, &["commit", "-qm", "initial"]);
    git(root, &["rev-parse", "HEAD"])
}
fn result(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}
#[test]
fn imported_projects_isolated_workspace_restart_and_archive_use_real_ipc() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = root.join("source");
    let head = repo(&source);
    std::fs::write(source.join("file"), "user dirty edit").unwrap();
    std::fs::write(source.join("untracked"), "keep").unwrap();
    let runtime = root.join("runtime");
    let server = Server::start(&runtime);
    let project=result(server.request(json!({"method":"projects.import","params":{"path":source,"name":"Real project"},"idempotency_key":"import"})));
    assert_eq!(project["identity"]["kind"]["dirty"], true);
    assert_eq!(result(server.request(json!({"method":"projects.import","params":{"path":source,"name":"Real project"},"idempotency_key":"import"})))["id"],project["id"]);
    let workspace=result(server.request(json!({"method":"workspaces.create","params":{"project_id":project["id"],"expected_head":head},"expected_revision":project["revision"],"idempotency_key":"isolate"})));
    assert_eq!(workspace["state"], "ready");
    let destination = PathBuf::from(workspace["root"].as_str().unwrap());
    assert!(destination.starts_with(runtime.join("workspaces")));
    assert_eq!(
        std::fs::read_to_string(destination.join("file")).unwrap(),
        "baseline"
    );
    assert_eq!(
        std::fs::read_to_string(source.join("file")).unwrap(),
        "user dirty edit"
    );
    assert_eq!(
        std::fs::read_to_string(source.join("untracked")).unwrap(),
        "keep"
    );
    assert_eq!(result(server.request(json!({"method":"workspaces.create","params":{"project_id":project["id"],"expected_head":head},"expected_revision":project["revision"],"idempotency_key":"isolate"})))["id"],workspace["id"]);
    let task=result(server.request(json!({"method":"tasks.create","params":{"project_id":project["id"],"goal":"registered project task"},"idempotency_key":"task"})));
    assert_eq!(task["project_id"], project["id"]);
    drop(server);
    let server = Server::start(&runtime);
    assert_eq!(
        result(server.request(json!({"method":"projects.list","params":{}})))[0]["id"],
        project["id"]
    );
    assert_eq!(
        result(
            server
                .request(json!({"method":"workspaces.list","params":{"project_id":project["id"]}}))
        )[0]["id"],
        workspace["id"]
    );
    let current = result(server.request(json!({"method":"projects.list","params":{}})))[0].clone();
    let archived=result(server.request(json!({"method":"projects.archive","params":{"project_id":project["id"]},"expected_revision":current["revision"],"idempotency_key":"archive"})));
    assert_eq!(archived["archived"], true);
    assert_eq!(result(server.request(json!({"method":"tasks.create","params":{"project_id":project["id"],"goal":"registered project task"},"idempotency_key":"task"})))["id"],task["id"]);
    assert!(source.join("file").exists());
    assert!(destination.join("file").exists());
    assert_eq!(server.request(json!({"method":"tasks.create","params":{"project_id":project["id"],"goal":"must refuse"},"idempotency_key":"blocked"}))["error"]["code"],"PERMISSION_DENIED");
}
#[test]
fn invalid_registry_requests_and_stale_head_do_not_create_workspaces() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = root.join("source");
    let head = repo(&source);
    let server = Server::start(&root.join("runtime"));
    assert_eq!(server.request(json!({"method":"tasks.create","params":{"project_id":"unknown","goal":"no"},"idempotency_key":"key"}))["error"]["code"],"NOT_FOUND");
    let p=result(server.request(json!({"method":"projects.import","params":{"path":source,"name":"P"},"idempotency_key":"import"})));
    assert_eq!(server.request(json!({"method":"workspaces.create","params":{"project_id":p["id"],"expected_head":head,"destination":"/tmp/not-approved"},"expected_revision":1,"idempotency_key":"bad"}))["error"]["code"],"INVALID_REQUEST");
    assert_eq!(server.request(json!({"method":"projects.archive","params":{"project_id":p["id"]},"expected_revision":99,"idempotency_key":"stale"}))["error"]["code"],"STALE_REVISION");
    std::fs::write(source.join("file"), "new commit").unwrap();
    git(&source, &["add", "file"]);
    git(&source, &["commit", "-qm", "advance"]);
    let response=server.request(json!({"method":"workspaces.create","params":{"project_id":p["id"],"expected_head":head},"expected_revision":1,"idempotency_key":"old-head"}));
    assert!(response.get("error").is_some(), "{response}");
    assert!(result(
        server.request(json!({"method":"workspaces.list","params":{"project_id":p["id"]}}))
    )
    .as_array()
    .unwrap()
    .is_empty());
}
#[test]
fn simultaneous_clients_reuse_identical_workspace_creation_key() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = root.join("source");
    let head = repo(&source);
    let server = Server::start(&root.join("runtime"));
    let p=result(server.request(json!({"method":"projects.import","params":{"path":source,"name":"P"},"idempotency_key":"import"})));
    let cmd = json!({"method":"workspaces.create","params":{"project_id":p["id"],"expected_head":head},"expected_revision":p["revision"],"idempotency_key":"shared-key"});
    let mut joins = vec![];
    for _ in 0..2 {
        let runtime = server.runtime.clone();
        let cmd = cmd.clone();
        joins.push(std::thread::spawn(move || {
            result(call(&runtime, cmd).unwrap())
        }));
    }
    let a = joins.remove(0).join().unwrap();
    let b = joins.remove(0).join().unwrap();
    assert_eq!(a["id"], b["id"]);
    assert_eq!(
        result(server.request(json!({"method":"workspaces.list","params":{"project_id":p["id"]}})))
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
