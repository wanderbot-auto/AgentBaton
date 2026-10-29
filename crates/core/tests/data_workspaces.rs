//! Generated local fixtures. Owner finalization simulates the trusted host call;
//! no native dialog, vendor identity/model/network or Task acceptance is claimed.
use agentbaton_core::ipc::{call, Daemon};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
struct Fixture {
    _tmp: tempfile::TempDir,
    runtime: PathBuf,
    source: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<agentbaton_core::Result<()>>>,
}
impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let runtime = root.join("runtime");
        let source = root.join("source");
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(&source).unwrap();
        fs::write(source.join("dataset.csv"), b"value\n1\n").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let daemon = Daemon::bind(&runtime).unwrap();
        let flag = stop.clone();
        let thread = std::thread::spawn(move || daemon.run(flag));
        Self {
            _tmp: tmp,
            runtime,
            source,
            stop,
            thread: Some(thread),
        }
    }
    fn raw(&self, method: &str, params: Value, key: Option<&str>, revision: Option<u64>) -> Value {
        let mut q = json!({"method":method,"params":params});
        if let Some(k) = key {
            q["idempotency_key"] = json!(k)
        }
        if let Some(r) = revision {
            q["expected_revision"] = json!(r)
        }
        call(&self.runtime, q).unwrap()
    }
    fn ok(&self, method: &str, params: Value, key: Option<&str>, revision: Option<u64>) -> Value {
        let response = self.raw(method, params, key, revision);
        assert!(response.get("error").is_none(), "{method}: {response}");
        response["result"].clone()
    }
    fn project(&self) -> Value {
        self.ok(
            "projects.import",
            json!({"path":self.source,"name":"Generated non-Git dataset"}),
            Some("project"),
            None,
        )
    }
    fn preview(&self, project: &Value, key: &str) -> Value {
        let started = self.ok(
            "data_workspace_previews.start",
            selection(project),
            Some(key),
            Some(project["revision"].as_u64().unwrap()),
        );
        self.wait(
            "data_workspace_previews.get",
            "preview_id",
            started["id"].as_str().unwrap(),
            &["checking", "cancel_requested"],
        )
    }
    fn wait(&self, method: &str, field: &str, id: &str, active: &[&str]) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let value = self.ok(method, json!({field:id}), None, None);
            if !active.contains(&value["state"].as_str().unwrap()) {
                return value;
            }
            assert!(Instant::now() < deadline, "{method} remained {value}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn proposal(&self, preview: &Value, key: &str) -> Value {
        self.ok(
            "data_workspaces.propose",
            json!({"preview_id":preview["id"]}),
            Some(key),
            preview["revision"].as_u64(),
        )
    }
    fn approve(&self, proposal: &Value) -> Value {
        self.ok(
            "approvals.finalize",
            json!({"proposal_id":proposal["id"]}),
            Some("trusted-generated-host-confirmation"),
            proposal["revision"].as_u64(),
        )
    }
    fn create(&self, project: &Value) -> Value {
        let preview = self.preview(project, "preview");
        assert_eq!(preview["state"], "verified");
        let proposal = self.proposal(&preview, "copy-proposal");
        let started = self.approve(&proposal);
        self.wait(
            "data_workspaces.get",
            "creation_id",
            started["id"].as_str().unwrap(),
            &["creating", "cancel_requested"],
        )
    }
    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.join().unwrap().unwrap();
        }
    }
    fn restart(&mut self) {
        self.shutdown();
        self.stop = Arc::new(AtomicBool::new(false));
        let daemon = Daemon::bind(&self.runtime).unwrap();
        let flag = self.stop.clone();
        self.thread = Some(std::thread::spawn(move || daemon.run(flag)));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
fn selection(project: &Value) -> Value {
    json!({"project_id":project["id"],"files":[{"artifact_id":"dataset","relative_path":"dataset.csv","max_bytes":1024,"expected_sha256":null}],"limits":{"max_file_bytes":1024,"max_total_bytes":1024,"max_files":1}})
}
#[test]
fn actual_non_git_preview_owner_approval_copy_registry_and_same_key_are_durable() {
    let mut f = Fixture::new();
    let before = fs::metadata(f.source.join("dataset.csv")).unwrap();
    let project = f.project();
    assert_eq!(project["identity"]["kind"]["kind"], "directory");
    let preview = f.preview(&project, "preview");
    assert_eq!(preview["state"], "verified");
    assert_eq!(
        preview["snapshot"]["files"][0]["sha256"],
        agentbaton_workspaces::validation::sha256(b"value\n1\n")
    );
    let proposal = f.proposal(&preview, "copy");
    assert_eq!(proposal["kind"], "data_workspace");
    let destination = PathBuf::from(
        proposal["snapshot"]["intent"]["destination"]
            .as_str()
            .unwrap(),
    );
    assert!(!destination.exists());
    assert!(f
        .ok(
            "workspaces.list",
            json!({"project_id":project["id"]}),
            None,
            None
        )
        .as_array()
        .unwrap()
        .is_empty());
    let initial = f.approve(&proposal);
    let record = f.wait(
        "data_workspaces.get",
        "creation_id",
        initial["id"].as_str().unwrap(),
        &["creating", "cancel_requested"],
    );
    assert_eq!(record["state"], "ready");
    assert_eq!(record["scope"], "explicit_selected_dataset_copy_only");
    assert_eq!(record["filesystem_sandbox"], false);
    assert_eq!(
        fs::read(destination.join("inputs/dataset.csv")).unwrap(),
        b"value\n1\n"
    );
    assert_eq!(
        fs::metadata(destination.join("inputs/dataset.csv"))
            .unwrap()
            .mode()
            & 0o7777,
        0o600
    );
    let workspaces = f.ok(
        "workspaces.list",
        json!({"project_id":project["id"]}),
        None,
        None,
    );
    assert_eq!(workspaces[0]["id"], record["workspace_id"]);
    assert_eq!(workspaces[0]["identity"]["kind"]["kind"], "directory");
    assert_eq!(workspaces[0]["base_revision"], record["dataset_revision"]);
    let replay = f.approve(&proposal);
    assert_eq!(replay["id"], record["id"]);
    let rows = f.ok(
        "data_workspaces.list",
        json!({"project_id":project["id"]}),
        None,
        None,
    );
    assert_eq!(rows.as_array().unwrap().len(), 1);
    let after = fs::metadata(f.source.join("dataset.csv")).unwrap();
    assert_eq!(
        (
            before.ino(),
            before.mode(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec()
        ),
        (
            after.ino(),
            after.mode(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec()
        )
    );
    assert!(!f.source.join(".git").exists());
    f.restart();
    assert_eq!(
        f.ok(
            "data_workspaces.get",
            json!({"creation_id":record["id"]}),
            None,
            None
        )["state"],
        "ready"
    );
}
#[test]
fn renderer_authority_flags_roots_and_direct_copy_do_not_create_data_workspaces() {
    let f = Fixture::new();
    let p = f.project();
    for (field, value) in [
        ("source_root", json!(f.source)),
        ("destination", json!(f.runtime)),
        ("verified", json!(true)),
        ("approval", json!({"confirmed":true})),
    ] {
        let mut q = selection(&p);
        q[field] = value;
        assert_eq!(
            f.raw("data_workspace_previews.start", q, Some("forged"), Some(1))["error"]["code"],
            "INVALID_REQUEST"
        );
    }
    assert_eq!(
        f.raw(
            "data_workspaces.create",
            json!({"project_id":p["id"],"pass":true}),
            Some("direct"),
            Some(1)
        )["error"]["code"],
        "AUTH_REQUIRED"
    );
    assert!(f
        .ok(
            "data_workspace_previews.list",
            json!({"project_id":p["id"]}),
            None,
            None
        )
        .as_array()
        .unwrap()
        .is_empty());
    assert!(f
        .ok(
            "data_workspaces.list",
            json!({"project_id":p["id"]}),
            None,
            None
        )
        .as_array()
        .unwrap()
        .is_empty());
}
#[test]
fn approved_preview_drift_does_not_silently_resnapshot_or_replay_copy() {
    let f = Fixture::new();
    let p = f.project();
    let preview = f.preview(&p, "preview");
    let proposal = f.proposal(&preview, "copy");
    fs::write(f.source.join("dataset.csv"), b"value\n2\n").unwrap();
    let initial = f.approve(&proposal);
    let record = f.wait(
        "data_workspaces.get",
        "creation_id",
        initial["id"].as_str().unwrap(),
        &["creating", "cancel_requested"],
    );
    assert_eq!(record["state"], "failed");
    assert_eq!(record["error"]["code"], "INPUT_CHANGED");
    assert!(!PathBuf::from(record["destination"].as_str().unwrap()).exists());
    let replay = f.approve(&proposal);
    assert_eq!(replay["id"], record["id"]);
    assert_eq!(
        f.raw(
            "data_workspaces.propose",
            json!({"preview_id":preview["id"]}),
            Some("new-key"),
            preview["revision"].as_u64()
        )["error"]["code"],
        "OUTCOME_UNKNOWN"
    );
    assert_eq!(
        f.ok(
            "data_workspaces.list",
            json!({"project_id":p["id"]}),
            None,
            None
        )
        .as_array()
        .unwrap()
        .len(),
        1
    );
}
#[test]
fn ready_copy_survives_deleted_original_and_restart_without_touching_model_budgets() {
    let mut f = Fixture::new();
    let p = f.project();
    let record = f.create(&p);
    assert_eq!(record["state"], "ready");
    fs::remove_dir_all(&f.source).unwrap();
    f.restart();
    assert_eq!(
        f.ok(
            "data_workspaces.get",
            json!({"creation_id":record["id"]}),
            None,
            None
        )["state"],
        "ready"
    );
    let database = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        database
            .pragma_query_value::<u32, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        14
    );
    assert_eq!(
        database
            .query_row::<u32, _, _>("SELECT count(*) FROM budgets", [], |r| r.get(0))
            .unwrap(),
        0
    );
    assert_eq!(
        database
            .query_row::<u32, _, _>("SELECT count(*) FROM attempts", [], |r| r.get(0))
            .unwrap(),
        0
    );
    assert_eq!(
        fs::read(PathBuf::from(record["destination"].as_str().unwrap()).join("inputs/dataset.csv"))
            .unwrap(),
        b"value\n1\n"
    );
}
impl Fixture {
    fn generated_connection(&self) -> Value {
        let executable = self.runtime.join("generated-data-acp");
        let python = std::process::Command::new("/usr/bin/env")
            .args(["which", "python3"])
            .output()
            .unwrap();
        assert!(python.status.success());
        let python = String::from_utf8(python.stdout).unwrap();
        let body = r#"import sys,json,os,hashlib
if '--version' in sys.argv:
 print('generated-data-acp 0.1');sys.exit(0)
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize':
  print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':{'protocolVersion':1,'agentCapabilities':{}}}),flush=True)
 elif m=='session/new':
  assert r['params']['cwd']==os.getcwd()
  contract=json.load(open('.baton-data-ready.json'))
  revision=contract['snapshot']['dataset_revision']
  script=b'#!/bin/sh\nexit 0\n';environment=b'{"engine":"generated-fixture"}\n'
  for path,data in [('reproduce.sh',script),('environment.json',environment)]:
   fd=os.open(path,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600);os.write(fd,data);os.close(fd)
  table={'metadata':{'input_revision':revision,'dataset_revision':revision,'input_digests':{'dataset':hashlib.sha256(open('inputs/dataset.csv','rb').read()).hexdigest()},'table_artifact_id':'table','row_count':1,'dataset_rows':1,'units':{'x':'m'},'timezone':'UTC','seed':7,'scope':{'kind':'full'},'script_sha256':hashlib.sha256(script).hexdigest(),'environment_sha256':hashlib.sha256(environment).hexdigest()},'rows':[{'x':1.5}]}
  fd=os.open('table.json',os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600);os.write(fd,json.dumps(table).encode());os.close(fd)
  fd=os.open('fixture-observed-session-new',os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600);os.write(fd,b'actual generated ACP initialize then session/new');os.close(fd)
  print('malformed-generated-session-response',flush=True);sys.exit(0)
"#;
        fs::write(&executable, format!("#!{}\n{}", python.trim(), body)).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let proposal=self.ok("connections.propose",json!({"name":"Generated account-free data fixture","provider":"custom_acp","executable":executable,"arguments":[]}),Some("generated-connection"),None);
        let connection = self.approve(&proposal);
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let values = self.ok("connections.list", json!({}), None, None);
            let record = values
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"] == connection["id"])
                .unwrap();
            if record["probe_state"] == "ready" {
                return record.clone();
            }
            assert!(Instant::now() < end, "{record}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn grant(&self, p: &Value, copy: &Value, connection: &Value) -> Value {
        let proposal=self.ok("grants.propose",json!({"project_id":p["id"],"workspace_id":copy["workspace_id"],"connection_id":connection["id"],"max_attempts":2,"max_concurrency":1,"max_duration_ms":30000}),Some("data-grant"),None);
        self.approve(&proposal)
    }
}
#[test]
fn non_git_real_grant_generated_acp_stop_and_data_json_contract_are_bound_to_actual_copy() {
    let f = Fixture::new();
    let p = f.project();
    let copy = f.create(&p);
    assert_eq!(copy["state"], "ready");
    let connection = f.generated_connection();
    let task=f.ok("tasks.create",json!({"project_id":p["id"],"goal":"Validate generated data under approved fixed contract"}),Some("data-task"),None);
    let grant = f.grant(&p, &copy, &connection);
    let session=f.ok("sessions.create",json!({"task_id":task["id"],"workspace_id":copy["workspace_id"],"connection_id":connection["id"],"grant_id":grant["id"]}),Some("data-session"),task["revision"].as_u64());
    let stopped = f.wait(
        "sessions.get",
        "session_id",
        session["id"].as_str().unwrap(),
        &["starting", "ready", "running", "cancel_requested"],
    );
    assert!(
        matches!(stopped["state"].as_str(), Some("stopped" | "failed")),
        "{stopped}"
    );
    // A failed session label is not stop evidence. Wait for the actual driver
    // receipt and released lease before approving any artifact reads.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let db = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
        let state: String = db
            .query_row(
                "SELECT state FROM dispatch_intents WHERE id=?1",
                [stopped["intent_id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        let holder: String = db
            .query_row(
                "SELECT holder FROM workspace_leases WHERE resource=?1",
                [copy["workspace_id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        if state == "stopped" && holder.is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "No actual stop/lease evidence: {state}/{holder}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let destination = PathBuf::from(copy["destination"].as_str().unwrap());
    assert!(destination.join("fixture-observed-session-new").exists());
    assert_eq!(
        fs::read(f.source.join("dataset.csv")).unwrap(),
        b"value\n1\n"
    );
    let input_sha = agentbaton_workspaces::validation::sha256(b"value\n1\n");
    let selection = json!({"session_id":session["id"],"inputs":[{"artifact_id":"dataset","relative_path":"inputs/dataset.csv","max_bytes":1024,"expected_sha256":input_sha}],"outputs":[{"artifact_id":"table","relative_path":"table.json","max_bytes":4096,"expected_sha256":null},{"artifact_id":"script","relative_path":"reproduce.sh","max_bytes":1024,"expected_sha256":null},{"artifact_id":"environment","relative_path":"environment.json","max_bytes":1024,"expected_sha256":null}],"table":{"table_artifact_id":"table","columns":[{"name":"x","required":true,"nullable":false,"scalar":{"type":"number","min":0.0,"max":10.0}}],"dataset_rows":1,"analysis":{"script_artifact_id":"script","environment_artifact_id":"environment","units":{"x":"m"},"timezone":"UTC","seed":7,"scope":{"kind":"full"}},"limits":{"max_bytes":4096,"max_depth":16,"max_values":1000,"max_string_bytes":1024,"max_rows":10}}});
    let proposal = f.ok(
        "data_validation_policies.propose",
        selection.clone(),
        Some("data-policy"),
        None,
    );
    assert_eq!(proposal["kind"], "data_validation");
    let policy = f.approve(&proposal);
    assert_eq!(
        policy["file_contract"]["subject"]["input_revision"],
        copy["dataset_revision"]
    );
    let record = f.ok(
        "data_validations.run",
        json!({"policy_id":policy["id"]}),
        Some("data-check"),
        policy["revision"].as_u64(),
    );
    assert_eq!(record["state"], "contract_passed");
    assert_eq!(record["scope"], "data_contract_only");
    assert_eq!(record["task_acceptance"], "not_established");
    assert_eq!(record["integration_authorized"], false);
    let task_after = f.ok("tasks.get", json!({"task_id":task["id"]}), None, None);
    assert_eq!(task_after["state"], "outcome_unknown");
    let database = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
    let raw: String = database
        .query_row(
            "SELECT document FROM dispatch_intents WHERE id=?1",
            [stopped["intent_id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    let intent: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(intent["state"], "stopped");
    let (holder, epoch): (String, i64) = database
        .query_row(
            "SELECT holder,fencing_token FROM workspace_leases WHERE resource=?1",
            [copy["workspace_id"].as_str().unwrap()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(holder.is_empty());
    assert_eq!(epoch, stopped["fencing_token"].as_i64().unwrap());
    // A failed deterministic contract is a durable scoped receipt too, never
    // a Provider result or a task-level acceptance.
    let table_path = destination.join("table.json");
    let mut invalid: Value = serde_json::from_slice(&fs::read(&table_path).unwrap()).unwrap();
    invalid["rows"][0]["x"] = json!(99.0);
    fs::write(&table_path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    let failed = f.ok(
        "data_validations.run",
        json!({"policy_id":policy["id"]}),
        Some("data-contract-failed"),
        policy["revision"].as_u64(),
    );
    assert_eq!(failed["state"], "contract_failed");
    assert_eq!(failed["evidence"]["passed"], false);
    assert_eq!(failed["task_acceptance"], "not_established");
    let next_task = f.ok(
        "tasks.create",
        json!({"project_id":p["id"],"goal":"Generated later writer must fence old validation"}),
        Some("later-task"),
        None,
    );
    let next=f.ok("sessions.create",json!({"task_id":next_task["id"],"workspace_id":copy["workspace_id"],"connection_id":connection["id"],"grant_id":grant["id"]}),Some("later-session"),next_task["revision"].as_u64());
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let state: String = database
            .query_row(
                "SELECT state FROM dispatch_intents WHERE id=?1",
                [next["intent_id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        let (holder, fence): (String, i64) = database
            .query_row(
                "SELECT holder,fencing_token FROM workspace_leases WHERE resource=?1",
                [copy["workspace_id"].as_str().unwrap()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        if state == "stopped" && holder.is_empty() {
            assert!(fence > epoch);
            break;
        }
        assert!(
            Instant::now() < end,
            "Later actual owned writer did not stop: {state}/{holder}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let fenced = f.raw(
        "data_validations.run",
        json!({"policy_id":policy["id"]}),
        Some("after-real-later-writer"),
        policy["revision"].as_u64(),
    );
    assert_eq!(fenced["error"]["code"], "INPUT_CHANGED");
    assert!(fenced["error"]["message"]
        .as_str()
        .unwrap()
        .contains("later writer"));
    fs::write(destination.join("inputs/dataset.csv"), b"value\n9\n").unwrap();
    assert_eq!(
        f.raw(
            "data_validations.run",
            json!({"policy_id":policy["id"]}),
            Some("after-input-drift"),
            policy["revision"].as_u64()
        )["error"]["code"],
        "INPUT_CHANGED"
    );
    let historical = f.ok(
        "data_validations.run",
        json!({"policy_id":policy["id"]}),
        Some("data-check"),
        policy["revision"].as_u64(),
    );
    assert_eq!(historical["id"], record["id"]);
}

#[test]
fn known_private_runtime_is_rejected_before_dataset_preflight_or_copy_authority() {
    let f = Fixture::new();
    // Uses the normal authenticated IPC helper; no fixture reads/displays token bytes.
    let private = f.ok(
        "projects.import",
        json!({"path":f.runtime,"name":"Generated runtime exclusion fixture"}),
        Some("private-project"),
        None,
    );
    let response=f.raw("data_workspace_previews.start",json!({"project_id":private["id"],"files":[{"artifact_id":"must-not-read","relative_path":"session.token","max_bytes":1024,"expected_sha256":null}],"limits":{"max_file_bytes":1024,"max_total_bytes":1024,"max_files":1}}),Some("private-preview"),private["revision"].as_u64());
    assert_eq!(response["error"]["code"], "PERMISSION_DENIED");
    assert!(response.get("result").is_none());
    assert!(!response.to_string().contains("sha256"));
    assert!(f
        .ok(
            "data_workspace_previews.list",
            json!({"project_id":private["id"]}),
            None,
            None
        )
        .as_array()
        .unwrap()
        .is_empty());
    let ancestor = f.ok(
        "projects.import",
        json!({"path":f.runtime.parent().unwrap(),"name":"Generated ancestor exclusion fixture"}),
        Some("ancestor-project"),
        None,
    );
    let response=f.raw("data_workspace_previews.start",json!({"project_id":ancestor["id"],"files":[{"artifact_id":"must-not-read","relative_path":"runtime/baton.sqlite3","max_bytes":1024,"expected_sha256":null}],"limits":{"max_file_bytes":1024,"max_total_bytes":1024,"max_files":1}}),Some("ancestor-preview"),ancestor["revision"].as_u64());
    assert_eq!(response["error"]["code"], "PERMISSION_DENIED");
    assert!(f
        .ok(
            "data_workspace_previews.list",
            json!({"project_id":ancestor["id"]}),
            None,
            None
        )
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn moved_or_replaced_private_runtime_cannot_become_an_ordinary_dataset_source() {
    let mut f = Fixture::new();
    let original = f.runtime.clone();
    fs::write(
        original.join("generated-runtime-marker.txt"),
        b"benign generated marker, never token bytes",
    )
    .unwrap();
    let moved = original.with_file_name("moved");
    fs::rename(&original, &moved).unwrap();
    // A replacement at the old path must not satisfy the original directory pin.
    fs::create_dir(&original).unwrap();
    fs::set_permissions(&original, fs::Permissions::from_mode(0o700)).unwrap();
    f.runtime = moved;
    let p = f.ok(
        "projects.import",
        json!({"path":f.runtime,"name":"Generated moved-runtime boundary fixture"}),
        Some("moved-project"),
        None,
    );
    let response=f.raw("data_workspace_previews.start",json!({"project_id":p["id"],"files":[{"artifact_id":"benign-marker","relative_path":"generated-runtime-marker.txt","max_bytes":1024,"expected_sha256":null}],"limits":{"max_file_bytes":1024,"max_total_bytes":1024,"max_files":1}}),Some("moved-preview"),p["revision"].as_u64());
    if response.get("result").is_some() {
        let observed = f.wait(
            "data_workspace_previews.get",
            "preview_id",
            response["result"]["id"].as_str().unwrap(),
            &["checking", "cancel_requested"],
        );
        eprintln!(
            "CONTROL benign moved-runtime preview state={}",
            observed["state"]
        );
        assert_eq!(
            observed["state"], "verified",
            "old semantic control must actually complete its benign hash"
        );
    }
    assert_eq!(response["error"]["code"], "PERMISSION_DENIED");
    assert!(!response.to_string().contains("sha256"));
    assert!(f
        .ok(
            "data_workspace_previews.list",
            json!({"project_id":p["id"]}),
            None,
            None
        )
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn copy_intent_approval_and_journal_commit_atomically_before_any_copy() {
    let f = Fixture::new();
    let p = f.project();
    let preview = f.preview(&p, "preview");
    let proposal = f.proposal(&preview, "copy");
    let db = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_copy_prepare BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='data_workspace.copy_prepared' BEGIN SELECT RAISE(ABORT,'generated journal rollback fixture'); END;").unwrap();
    let response = f.raw(
        "approvals.finalize",
        json!({"proposal_id":proposal["id"]}),
        Some("host-confirm"),
        proposal["revision"].as_u64(),
    );
    assert!(response.get("error").is_some());
    assert_eq!(
        db.query_row::<u32, _, _>("SELECT count(*) FROM data_workspace_creations", [], |r| r
            .get(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row::<u32, _, _>("SELECT count(*) FROM managed_workspaces", [], |r| r.get(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row::<u32, _, _>("SELECT count(*) FROM data_workspace_receipts", [], |r| r
            .get(0))
            .unwrap(),
        0
    );
    assert!(db
        .query_row::<bool, _, _>(
            "SELECT result IS NULL FROM approval_proposals WHERE id=?1",
            [proposal["id"].as_str().unwrap()],
            |r| r.get(0)
        )
        .unwrap());
    let destination = PathBuf::from(
        proposal["snapshot"]["intent"]["destination"]
            .as_str()
            .unwrap(),
    );
    assert!(!destination.exists());
    db.execute_batch("DROP TRIGGER reject_copy_prepare")
        .unwrap();
    let initial = f.approve(&proposal);
    let record = f.wait(
        "data_workspaces.get",
        "creation_id",
        initial["id"].as_str().unwrap(),
        &["creating", "cancel_requested"],
    );
    assert_eq!(record["state"], "ready");
}
#[test]
fn missing_native_intent_or_ready_journal_receipt_never_borrows_disk_marker_authority() {
    for column in ["owner_sequence", "prepared_sequence", "finished_sequence"] {
        let f = Fixture::new();
        let p = f.project();
        let copy = f.create(&p);
        assert_eq!(copy["state"], "ready");
        let db = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
        let sql=format!("DELETE FROM events WHERE sequence=(SELECT {column} FROM data_workspace_receipts WHERE creation_id=?1)");
        db.execute(&sql, [copy["id"].as_str().unwrap()]).unwrap();
        let response=f.raw("grants.propose",json!({"project_id":p["id"],"workspace_id":copy["workspace_id"],"connection_id":"not-looked-up","max_attempts":1,"max_concurrency":1,"max_duration_ms":30000}),Some("missing-evidence"),None);
        assert_eq!(
            response["error"]["code"], "AUTH_REQUIRED",
            "{column}: {response}"
        );
        assert_eq!(
            db.query_row::<u32, _, _>("SELECT count(*) FROM execution_grants", [], |r| r.get(0))
                .unwrap(),
            0
        );
        assert_eq!(
            f.ok(
                "data_workspaces.get",
                json!({"creation_id":copy["id"]}),
                None,
                None
            )["state"],
            "ready"
        ); // historical view only
    }
}
#[test]
fn fresh_data_input_authority_restores_without_the_original_source_after_restart() {
    let mut f = Fixture::new();
    let p = f.project();
    let copy = f.create(&p);
    fs::remove_dir_all(&f.source).unwrap();
    f.restart();
    let connection = f.generated_connection();
    let grant = f.grant(&p, &copy, &connection);
    assert_eq!(grant["project_id"], p["id"]);
    assert_eq!(
        grant["workspace_bindings"][copy["workspace_id"].as_str().unwrap()]["kind"]["kind"],
        "directory"
    );
}
#[test]
fn schema10_copy_intent_survives_actual_process_exit_without_auto_adopting_a_marker() {
    if let Some(database) = std::env::var_os("BATON_DATA_CRASH_DB") {
        let mut store =
            agentbaton_core::storage::Store::open(std::path::Path::new(&database)).unwrap();
        let id = std::env::var("BATON_DATA_CRASH_PROPOSAL").unwrap();
        let result = store.finalize_proposal(&id, 1).unwrap();
        assert_eq!(result["state"], "creating");
        // Exact window: real finalizer committed intent/owner result, no I/O worker
        // was scheduled. Abrupt exit bypasses Store/daemon graceful shutdown.
        std::process::exit(77);
    }
    let mut f = Fixture::new();
    let p = f.project();
    let preview = f.preview(&p, "preview");
    let proposal = f.proposal(&preview, "copy");
    f.shutdown();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "schema10_copy_intent_survives_actual_process_exit_without_auto_adopting_a_marker",
            "--nocapture",
        ])
        .env("BATON_DATA_CRASH_DB", f.runtime.join("baton.sqlite3"))
        .env(
            "BATON_DATA_CRASH_PROPOSAL",
            proposal["id"].as_str().unwrap(),
        )
        .spawn()
        .unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(77));
            break;
        }
        if Instant::now() >= end {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("owned crash-window fixture timed out")
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let destination = PathBuf::from(
        proposal["snapshot"]["intent"]["destination"]
            .as_str()
            .unwrap(),
    );
    assert!(!destination.exists());
    // A forged disk marker cannot provide the absent actual copied-root pin.
    fs::create_dir(&destination).unwrap();
    fs::set_permissions(&destination, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        destination.join(".baton-data-ready.json"),
        b"{\"passed\":true,\"verified\":true}",
    )
    .unwrap();
    f.restart();
    let copies = f.ok(
        "data_workspaces.list",
        json!({"project_id":p["id"]}),
        None,
        None,
    );
    let copy = &copies[0];
    assert_eq!(copy["state"], "outcome_unknown");
    assert!(copy["contract"].is_null());
    let initial = f.approve(&proposal);
    assert_eq!(initial["id"], copy["id"]);
    assert_eq!(
        f.ok(
            "data_workspaces.get",
            json!({"creation_id":copy["id"]}),
            None,
            None
        )["state"],
        "outcome_unknown"
    );
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
    assert_eq!(
        f.raw(
            "data_workspaces.propose",
            json!({"preview_id":preview["id"]}),
            Some("blind-new-key"),
            preview["revision"].as_u64()
        )["error"]["code"],
        "OUTCOME_UNKNOWN"
    );
}

#[test]
fn legal_sixty_four_file_preview_and_owner_proposal_keep_every_selected_hash() {
    let f = Fixture::new();
    let p = f.project();
    let mut files = Vec::new();
    for i in 0..64 {
        let name = format!("file-{i:02}.csv");
        fs::write(f.source.join(&name), b"x").unwrap();
        files.push(json!({"artifact_id":format!("input-{i:02}"),"relative_path":name,"max_bytes":16,"expected_sha256":null}));
    }
    let started=f.ok("data_workspace_previews.start",json!({"project_id":p["id"],"files":files,"limits":{"max_file_bytes":16,"max_total_bytes":1024,"max_files":64}}),Some("max-files"),p["revision"].as_u64());
    let preview = f.wait(
        "data_workspace_previews.get",
        "preview_id",
        started["id"].as_str().unwrap(),
        &["checking", "cancel_requested"],
    );
    assert_eq!(preview["state"], "verified");
    let proposal = f.proposal(&preview, "max-files-owner");
    let selected = proposal["snapshot"]["snapshot"]["files"]
        .as_array()
        .unwrap();
    assert_eq!(selected.len(), 64);
    for file in selected {
        assert_eq!(
            file["sha256"],
            agentbaton_workspaces::validation::sha256(b"x")
        );
        assert_eq!(file["bytes"], 1);
    }
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/data-workspace-core");
    fs::create_dir_all(&fixture_dir).unwrap();
    let generated_json = serde_json::to_vec_pretty(&proposal).unwrap();
    let json_hash = agentbaton_workspaces::validation::sha256(&generated_json);
    fs::write(
        fixture_dir.join(format!(
            "generated-sixty-four-file-owner-proposal-{json_hash}.json"
        )),
        generated_json,
    )
    .unwrap();
    eprintln!(
        "DATA_OWNER max_files=64 complete_proposal_json_bytes={} native_dialog_not_executed",
        serde_json::to_vec(&proposal).unwrap().len()
    );
    assert!(!PathBuf::from(
        proposal["snapshot"]["intent"]["destination"]
            .as_str()
            .unwrap()
    )
    .exists());
}
#[test]
fn raw_oversize_preview_replay_refuses_before_decode_and_never_becomes_a_new_attempt() {
    let f = Fixture::new();
    let p = f.project();
    let preview = f.preview(&p, "raw-limit");
    let db = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
    db.execute(
        "UPDATE data_workspace_previews SET document=?1 WHERE id=?2",
        rusqlite::params![
            "x".repeat(4 * 1024 * 1024 + 1),
            preview["id"].as_str().unwrap()
        ],
    )
    .unwrap();
    let replay = f.raw(
        "data_workspace_previews.start",
        selection(&p),
        Some("raw-limit"),
        p["revision"].as_u64(),
    );
    assert_eq!(replay["error"]["code"], "RESOURCE_LIMIT");
    assert_eq!(
        db.query_row::<u32, _, _>("SELECT count(*) FROM data_workspace_previews", [], |r| r
            .get(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row::<u32, _, _>("SELECT count(*) FROM data_workspace_creations", [], |r| r
            .get(0))
            .unwrap(),
        0
    );
}

#[test]
fn cancellation_receipts_replay_exactly_while_get_returns_real_join_and_legacy_remains_unknown() {
    let mut f = Fixture::new();
    let p = f.project();
    // Generated bounded bytes make both actor-control and copy work real; the
    // winner is observed honestly rather than asserting that cancellation wins.
    let file = fs::File::create(f.source.join("large.bin")).unwrap();
    file.set_len(32 * 1024 * 1024).unwrap();
    drop(file);
    let chosen = json!({"project_id":p["id"],"files":[{"artifact_id":"large","relative_path":"large.bin","max_bytes":33554432,"expected_sha256":null}],"limits":{"max_file_bytes":33554432,"max_total_bytes":33554432,"max_files":1}});
    let initial = f.ok(
        "data_workspace_previews.start",
        chosen,
        Some("large-preview"),
        p["revision"].as_u64(),
    );
    let cancel = f.ok(
        "data_workspace_previews.cancel",
        json!({"preview_id":initial["id"]}),
        Some("preview-cancel-once"),
        initial["revision"].as_u64(),
    );
    let terminal = f.wait(
        "data_workspace_previews.get",
        "preview_id",
        initial["id"].as_str().unwrap(),
        &["checking", "cancel_requested"],
    );
    assert!(
        matches!(terminal["state"].as_str(), Some("cancelled" | "verified")),
        "{terminal}"
    );
    assert_eq!(
        f.ok(
            "data_workspace_previews.cancel",
            json!({"preview_id":initial["id"]}),
            Some("preview-cancel-once"),
            initial["revision"].as_u64()
        ),
        cancel
    );
    assert_eq!(
        f.raw(
            "data_workspace_previews.cancel",
            json!({"preview_id":initial["id"]}),
            Some("preview-cancel-once"),
            Some(999)
        )["error"]["code"],
        "CONFIG_CONFLICT"
    );
    let copy = f.create(&p);
    assert_eq!(copy["state"], "ready");
    assert_eq!(copy["io_completion"], "worker_joined");
    let receipt = f.ok(
        "data_workspaces.cancel",
        json!({"creation_id":copy["id"]}),
        Some("copy-cancel-once"),
        Some(1),
    );
    assert_eq!(receipt["state"], "ready");
    let db = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
    db.execute("UPDATE data_workspace_creations SET document=json_remove(document,'$.io_completion') WHERE id=?1",[copy["id"].as_str().unwrap()]).unwrap();
    // Only the legacy read view is synthetic; never manufacture Ready authority:
    // no fresh witness/proposal is used after this deliberately changed record.
    f.restart();
    let latest = f.ok(
        "data_workspaces.get",
        json!({"creation_id":copy["id"]}),
        None,
        None,
    );
    assert!(latest["io_completion"].is_null());
    assert_eq!(
        f.ok(
            "data_workspaces.cancel",
            json!({"creation_id":copy["id"]}),
            Some("copy-cancel-once"),
            Some(1)
        ),
        receipt
    );
    assert_eq!(
        f.raw(
            "data_workspaces.cancel",
            json!({"creation_id":copy["id"]}),
            Some("copy-cancel-once"),
            Some(2)
        )["error"]["code"],
        "CONFIG_CONFLICT"
    );
}
#[test]
fn actual_copy_cancellation_only_claims_cancelled_after_join_and_preserves_ready_race() {
    let f = Fixture::new();
    let p = f.project();
    let file = fs::File::create(f.source.join("copy.bin")).unwrap();
    file.set_len(32 * 1024 * 1024).unwrap();
    drop(file);
    let preparing = Instant::now();
    let initial=f.ok("data_workspace_previews.start",json!({"project_id":p["id"],"files":[{"artifact_id":"large","relative_path":"copy.bin","max_bytes":33554432,"expected_sha256":null}],"limits":{"max_file_bytes":33554432,"max_total_bytes":33554432,"max_files":1}}),Some("copy-large-preview"),p["revision"].as_u64());
    let start_elapsed = preparing.elapsed();
    eprintln!(
        "stage=large_preview_start elapsed_ms={} state={} io_receipt={}",
        start_elapsed.as_millis(),
        initial["state"],
        initial["io_completion"]
    );
    assert!(start_elapsed < Duration::from_secs(3));
    assert_eq!(initial["state"], "checking");
    assert!(initial["snapshot"].is_null());
    // This 32 MiB preparation may use the existing 30 s worker I/O budget.
    // Observe it for at most 35 s including IPC/scheduling margin; this does
    // not change the product deadline, ordinary waits or the copy/join wait.
    let preview_deadline = preparing + Duration::from_secs(35);
    let mut preview = std::thread::scope(|scope| {
        let snapshot = scope.spawn(|| {
            let requested = Instant::now();
            let value = f.ok("system.snapshot", json!({}), None, None);
            (value, requested.elapsed())
        });
        let read = scope.spawn(|| {
            let requested = Instant::now();
            let value = f.ok(
                "data_workspace_previews.get",
                json!({"preview_id":initial["id"]}),
                None,
                None,
            );
            (value, requested.elapsed())
        });
        let (snapshot, snapshot_elapsed) = snapshot.join().unwrap();
        let (value, read_elapsed) = read.join().unwrap();
        eprintln!(
            "stage=large_preview_parallel_reads snapshot_ms={} read_ms={} state={} io_receipt={}",
            snapshot_elapsed.as_millis(),
            read_elapsed.as_millis(),
            value["state"],
            value["io_completion"]
        );
        assert!(snapshot_elapsed < Duration::from_secs(3));
        assert!(read_elapsed < Duration::from_secs(3));
        assert!(snapshot["sequence"].is_u64());
        value
    });
    loop {
        let terminal = preview["state"] != "checking";
        let elapsed = preparing.elapsed();
        assert!(Instant::now() < preview_deadline,
            "stage=large_preview_preparation elapsed_ms={} terminal={} state={} revision={} io_receipt={} error_code={}",
            elapsed.as_millis(), terminal, preview["state"], preview["revision"], preview["io_completion"], preview["error"]["code"]);
        if terminal {
            eprintln!("stage=large_preview_preparation elapsed_ms={} terminal=true state={} revision={} io_receipt={} error_code={}", elapsed.as_millis(), preview["state"], preview["revision"], preview["io_completion"], preview["error"]["code"]);
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
        let requested = Instant::now();
        preview = f.ok(
            "data_workspace_previews.get",
            json!({"preview_id":initial["id"]}),
            None,
            None,
        );
        assert!(
            requested.elapsed() < Duration::from_secs(3),
            "stage=large_preview_poll response_ms={}",
            requested.elapsed().as_millis()
        );
    }
    assert_eq!(preview["state"], "verified");
    let proposal = f.proposal(&preview, "copy-large-proposal");
    let cancelling = Instant::now();
    let started = f.approve(&proposal);
    let receipt = f.ok(
        "data_workspaces.cancel",
        json!({"creation_id":started["id"]}),
        Some("cancel-copy-once"),
        started["revision"].as_u64(),
    );
    let current = f.wait(
        "data_workspaces.get",
        "creation_id",
        started["id"].as_str().unwrap(),
        &["creating", "cancel_requested"],
    );
    eprintln!(
        "stage=copy_cancel_join elapsed_ms={} terminal={} state={} io_receipt={}",
        cancelling.elapsed().as_millis(),
        !matches!(
            current["state"].as_str(),
            Some("creating" | "cancel_requested")
        ),
        current["state"],
        current["io_completion"]
    );
    assert_eq!(current["io_completion"], "worker_joined");
    match current["state"].as_str().unwrap() {
        "cancelled" => {
            assert_eq!(current["cancel_requested"], true);
            assert!(current["contract"].is_null());
            assert_eq!(current["error"]["code"], "CANCEL_REQUESTED");
            assert_eq!(current["error"]["side_effect_state"], "confirmed");
            if !current["observation"].is_null() {
                assert_eq!(current["observation"]["state"], "failed");
                assert_eq!(current["observation"]["error"], "cancel_requested");
            }
        }
        "ready" => {
            assert!(!current["contract"].is_null());
            assert!(PathBuf::from(current["destination"].as_str().unwrap())
                .join("inputs/copy.bin")
                .exists());
        }
        other => panic!("unexpected actual copy terminal: {other}: {current}"),
    }
    assert_eq!(
        f.ok(
            "data_workspaces.cancel",
            json!({"creation_id":started["id"]}),
            Some("cancel-copy-once"),
            started["revision"].as_u64()
        ),
        receipt
    );
    assert_eq!(
        fs::metadata(f.source.join("copy.bin")).unwrap().len(),
        33554432
    );
}
#[test]
fn terminal_journal_failure_after_real_copy_keeps_unknown_intent_and_never_reexecutes() {
    let mut f = Fixture::new();
    let p = f.project();
    let preview = f.preview(&p, "copy-journal-preview");
    let proposal = f.proposal(&preview, "copy-journal-proposal");
    let db = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_actual_copy_finish BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='data_workspace.copy_finished' BEGIN SELECT RAISE(ABORT,'generated terminal journal fault'); END;").unwrap();
    let initial = f.approve(&proposal);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !f.thread.as_ref().unwrap().is_finished() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(f.thread.take().unwrap().join().unwrap().is_err());
    let destination = PathBuf::from(initial["destination"].as_str().unwrap());
    assert_eq!(
        fs::read(destination.join("inputs/dataset.csv")).unwrap(),
        b"value\n1\n"
    );
    assert!(destination.join(".baton-data-ready.json").exists());
    assert_eq!(
        db.query_row(
            "SELECT state FROM data_workspace_creations WHERE id=?1",
            [initial["id"].as_str().unwrap()],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "creating"
    );
    db.execute_batch("DROP TRIGGER reject_actual_copy_finish")
        .unwrap();
    f.restart();
    let current = f.ok(
        "data_workspaces.get",
        json!({"creation_id":initial["id"]}),
        None,
        None,
    );
    assert_eq!(current["state"], "outcome_unknown");
    assert_eq!(current["io_completion"], "interrupted_no_join_receipt");
    assert!(current["contract"].is_null());
    assert_eq!(f.approve(&proposal), initial);
    assert_eq!(
        f.raw(
            "data_workspaces.propose",
            json!({"preview_id":preview["id"]}),
            Some("do-not-reexecute"),
            preview["revision"].as_u64()
        )["error"]["code"],
        "OUTCOME_UNKNOWN"
    );
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 3); // inputs + Creating + Ready, no cleanup/retry
}

#[test]
fn data_goal_approval_uses_fresh_private_selected_inputs_without_requiring_git_or_the_original_source(
) {
    let f = Fixture::new();
    let p = f.project();
    let copy = f.create(&p);
    let connection = f.generated_connection();
    let task=f.ok("tasks.create",json!({"project_id":p["id"],"goal":"Plan bounded data work from the real isolated dataset"}),Some("data-goal-task"),None);
    let grant = f.grant(&p, &copy, &connection);
    let budget = json!({"attempts":2,"parallel":1,"runtime_ms":1000,"output_bytes":4096,"children":0,"reworks":0,"tokens":null,"cost_microunits":null});
    let intent = json!({"task_id":task["id"],"expected_task_revision":task["revision"],"grant_id":grant["id"],"expected_grant_revision":grant["revision"],"max_nodes":1,"new_max_children":0,"new_max_depth":0,"root_budget":budget,"project_budget":budget,"inputs":[{"artifact_id":"dataset","relative_path":"inputs/dataset.csv","max_bytes":1024,"expected_sha256":agentbaton_workspaces::validation::sha256(b"value\n1\n")}],"outputs":[{"artifact":{"artifact_id":"report","relative_path":"reports/report.json","max_bytes":4096,"expected_sha256":null},"validators":[]}],"writable_prefixes":["reports"],"owner_acceptance_criteria":"Owner reviews the report; this approval does not execute a task or establish scientific correctness"});
    fs::remove_dir_all(&f.source).unwrap();
    let proposal = f.ok(
        "goal_scopes.propose",
        intent.clone(),
        Some("data-goal"),
        None,
    );
    assert_eq!(proposal["kind"], "goal");
    assert_eq!(
        proposal["snapshot"]["resolved"]["inputs"][0]["sha256"],
        agentbaton_workspaces::validation::sha256(b"value\n1\n")
    );
    let scope = f.approve(&proposal);
    assert_eq!(scope["task_id"], task["id"]);
    assert_eq!(
        f.ok("tasks.get", json!({"task_id":task["id"]}), None, None)["state"],
        "draft"
    );
    use agentbaton_core::goal_plan::*;
    let view: agentbaton_core::goal_store::GoalScopeView =
        serde_json::from_value(scope.clone()).unwrap();
    let contract = view.outputs["report"].clone();
    let node = PlanNode {
        id: "write-report".into(),
        parent_id: None,
        kind: NodeKind::Work,
        goal: "Produce a data report".into(),
        dependencies: vec![],
        workspace_id: view.workspace_id.clone(),
        executor: Executor::Connection {
            binding: Box::new(view.binding.clone()),
            program_sha256: view.program_sha256.clone(),
        },
        recipients: vec![],
        required_inputs: vec![RequiredInput {
            goal_task_id: view.task_id.clone(),
            artifact_id: "dataset".into(),
            contract_id: view.inputs["dataset"].contract_id.clone(),
            source: InputSource::ApprovedExternal {
                sha256: view.inputs["dataset"].sha256.clone(),
            },
        }],
        outputs: vec![PlannedOutput {
            artifact_id: "report".into(),
            contract_id: contract.clone(),
        }],
        required_validators: view.contracts[&contract].clone(),
        side_effect: SideEffect::WorkspaceWrite,
        write_set: vec![WriteTarget::Workspace {
            path: "reports/report.json".into(),
        }],
        budget: BudgetLimits {
            attempts: 1,
            parallel: 1,
            runtime_ms: 1000,
            output_bytes: 4096,
            children: 0,
            reworks: 0,
            tokens: None,
            cost_microunits: None,
        },
        reservation: AttemptReservation {
            runtime_ms: 500,
            output_bytes: 4096,
            tokens: None,
            cost_microunits: None,
        },
        retry: RetryPolicy {
            max_attempts: 1,
            max_reworks: 0,
            retry_classes: vec![FailureClass::BeforeDispatchTransient],
        },
        idempotency_key: "data-report-action".into(),
        external_action_id: None,
    };
    let plan = GoalPlanProposal {
        schema_version: 1,
        plan_id: "data-plan".into(),
        goal: view.goal.clone(),
        scope_revision: view.authority_revision,
        revision: 1,
        expected_revision: 0,
        acceptance_rules: view.required_acceptance_rules.clone(),
        nodes: vec![node],
    };
    let raw_plan = json!({"scope_id":view.id,"plan_json":serde_json::to_string(&plan).unwrap()});
    let submitted = f.ok(
        "goal_plans.propose",
        raw_plan.clone(),
        Some("data-plan"),
        Some(0),
    );
    assert_eq!(submitted["proposal"]["revision"], 1);
    let ready = f.ok(
        "goal_plans.ready_preview",
        json!({"scope_id":view.id}),
        None,
        Some(1),
    );
    assert_eq!(ready["read_only"], true);
    assert_eq!(ready["reservation_made"], false);
    assert_eq!(ready["dispatched"], false);
    assert_eq!(
        f.ok("tasks.get", json!({"task_id":task["id"]}), None, None)["state"],
        "draft"
    );
    // An exact historic owner proposal is recoverable even after input drift;
    // replay never renews approval, executes a command, or hashes unrelated data.
    fs::write(
        PathBuf::from(copy["destination"].as_str().unwrap()).join("inputs/dataset.csv"),
        b"changed\n",
    )
    .unwrap();
    assert_eq!(
        f.ok("goal_scopes.propose", intent, Some("data-goal"), None),
        proposal
    );
    assert_eq!(
        f.ok("goal_plans.propose", raw_plan, Some("data-plan"), Some(0)),
        submitted
    );
    assert_eq!(
        f.raw(
            "goal_plans.ready_preview",
            json!({"scope_id":view.id}),
            None,
            Some(1)
        )["error"]["code"],
        "INPUT_CHANGED"
    );
}

#[test]
fn invalid_path_caps_and_expected_hash_never_create_copy_authority_or_read_unselected_data() {
    let f = Fixture::new();
    let p = f.project();
    for path in ["../dataset.csv", "/dataset.csv", "a//b", "a/./b", "a/../b"] {
        let mut q = selection(&p);
        q["files"][0]["relative_path"] = json!(path);
        assert_eq!(
            f.raw(
                "data_workspace_previews.start",
                q,
                Some("bad-path"),
                p["revision"].as_u64()
            )["error"]["code"],
            "INVALID_REQUEST"
        );
    }
    for cap in [0, 67108865] {
        let mut q = selection(&p);
        q["limits"]["max_file_bytes"] = json!(cap);
        assert_eq!(
            f.raw(
                "data_workspace_previews.start",
                q,
                Some("bad-cap"),
                p["revision"].as_u64()
            )["error"]["code"],
            "INVALID_REQUEST"
        );
    }
    let mut wrong = selection(&p);
    wrong["files"][0]["expected_sha256"] = json!("0".repeat(64));
    let initial = f.ok(
        "data_workspace_previews.start",
        wrong,
        Some("wrong-sha"),
        p["revision"].as_u64(),
    );
    let current = f.wait(
        "data_workspace_previews.get",
        "preview_id",
        initial["id"].as_str().unwrap(),
        &["checking"],
    );
    assert_eq!(current["state"], "failed");
    assert!(current["snapshot"].is_null());
    assert_eq!(
        f.raw(
            "data_workspaces.propose",
            json!({"preview_id":current["id"]}),
            Some("no-failed-copy"),
            current["revision"].as_u64()
        )["error"]["code"],
        "INPUT_CHANGED"
    );
    assert!(f
        .ok(
            "data_workspaces.list",
            json!({"project_id":p["id"]}),
            None,
            None
        )
        .as_array()
        .unwrap()
        .is_empty());
}
#[test]
fn preview_history_limit_refuses_new_records_before_commit_but_keeps_existing_get_and_same_key() {
    let f = Fixture::new();
    let p = f.project();
    let original = f.preview(&p, "history-original");
    let db = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
    let (payload, document): (String, String) = db
        .query_row(
            "SELECT payload,document FROM data_workspace_previews WHERE id=?1",
            [original["id"].as_str().unwrap()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    // Explicit synthetic history pressure only: no owner proposal or copy is
    // fabricated. Every original guard/assertion still runs on actual APIs.
    for i in 1..100 {
        let id = format!("synthetic-history-{i}");
        let mut v: Value = serde_json::from_str(&document).unwrap();
        v["id"] = json!(id);
        db.execute(
            "INSERT INTO data_workspace_previews VALUES(?1,?2,'verified',?3,?4,?5)",
            rusqlite::params![
                id,
                p["id"].as_str().unwrap(),
                serde_json::to_string(&v).unwrap(),
                format!("synthetic-key-{i}"),
                payload
            ],
        )
        .unwrap();
    }
    assert_eq!(
        f.raw(
            "data_workspace_previews.start",
            selection(&p),
            Some("one-too-many"),
            p["revision"].as_u64()
        )["error"]["code"],
        "RESOURCE_LIMIT"
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM data_workspace_previews", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        100
    );
    assert_eq!(
        f.ok(
            "data_workspace_previews.start",
            selection(&p),
            Some("history-original"),
            p["revision"].as_u64()
        ),
        original
    );
    assert_eq!(
        f.ok(
            "data_workspace_previews.get",
            json!({"preview_id":original["id"]}),
            None,
            None
        ),
        original
    );
    assert_eq!(
        f.ok(
            "data_workspace_previews.list",
            json!({"project_id":p["id"]}),
            None,
            None
        )
        .as_array()
        .unwrap()
        .len(),
        100
    );
}

#[test]
fn native_copy_journal_sources_and_receipt_sequence_order_are_required_not_just_matching_payloads()
{
    let mut actual = Vec::new();
    for mode in [
        "prepared_source",
        "finished_source",
        "owner_copied_after_ready",
    ] {
        let f = Fixture::new();
        let p = f.project();
        let copy = f.create(&p);
        assert_eq!(copy["state"], "ready");
        let db = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
        if mode == "owner_copied_after_ready" {
            db.execute("INSERT INTO events(document) SELECT document FROM events WHERE sequence=(SELECT owner_sequence FROM data_workspace_receipts WHERE creation_id=?1)",[copy["id"].as_str().unwrap()]).unwrap();
            let late = db.last_insert_rowid();
            db.execute(
                "UPDATE data_workspace_receipts SET owner_sequence=?1 WHERE creation_id=?2",
                rusqlite::params![late, copy["id"].as_str().unwrap()],
            )
            .unwrap();
        } else {
            let column = if mode == "prepared_source" {
                "prepared_sequence"
            } else {
                "finished_sequence"
            };
            db.execute(&format!("UPDATE events SET document=json_set(document,'$.source','untrusted_generated_agent_output') WHERE sequence=(SELECT {column} FROM data_workspace_receipts WHERE creation_id=?1)"),[copy["id"].as_str().unwrap()]).unwrap();
        }
        let response=f.raw("grants.propose",json!({"project_id":p["id"],"workspace_id":copy["workspace_id"],"connection_id":"never-resolve-this-connection","max_attempts":1,"max_concurrency":1,"max_duration_ms":30000}),Some("tampered-chain"),None);
        actual.push((mode, response["error"]["code"].as_str().unwrap().to_owned()));
        assert_eq!(
            db.query_row("SELECT count(*) FROM execution_grants", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    eprintln!("GENERATED_JOURNAL_SOURCE_ORDER_CONTROL {actual:?}");
    assert!(actual.iter().all(|(_,code)|code=="AUTH_REQUIRED"),"source and positive commit ordering must reject before grant/connection admission: {actual:?}");
}

#[test]
fn committed_owner_copy_intent_refusal_reports_confirmed_and_reconciles_without_scheduling_a_new_copy(
) {
    let f = Fixture::new();
    let p = f.project();
    let preview = f.preview(&p, "postcommit-preview");
    let proposal = f.proposal(&preview, "postcommit-proposal");
    let db = rusqlite::Connection::open(f.runtime.join("baton.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER generated_archive_after_copy_intent AFTER INSERT ON data_workspace_creations BEGIN UPDATE projects SET document=json_set(document,'$.archived',json('true')) WHERE id=NEW.project_id; END;").unwrap();
    let response = f.raw(
        "approvals.finalize",
        json!({"proposal_id":proposal["id"]}),
        Some("trusted-owner-postcommit"),
        proposal["revision"].as_u64(),
    );
    let creation = f.ok(
        "data_workspaces.list",
        json!({"project_id":p["id"]}),
        None,
        None,
    )[0]
    .clone();
    eprintln!(
        "GENERATED_POSTCOMMIT_REFUSAL error={} durable_state={} io_completion={}",
        response["error"], creation["state"], creation["io_completion"]
    );
    assert_eq!(response["error"]["code"], "INPUT_CHANGED");
    assert_eq!(response["error"]["side_effect_state"], "confirmed");
    assert_eq!(
        response["error"]["details_ref"],
        format!("data-copy-intent:{}", creation["id"].as_str().unwrap())
    );
    assert_eq!(creation["state"], "failed");
    assert_eq!(creation["io_completion"], "not_scheduled");
    assert!(creation["contract"].is_null());
    assert!(!PathBuf::from(creation["destination"].as_str().unwrap()).exists());
    assert!(db
        .query_row(
            "SELECT result IS NOT NULL FROM approval_proposals WHERE id=?1",
            [proposal["id"].as_str().unwrap()],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
    let replay = f.approve(&proposal);
    assert_eq!(replay["id"], creation["id"]);
    assert_eq!(replay["state"], "creating");
    assert_eq!(
        f.ok(
            "data_workspaces.get",
            json!({"creation_id":creation["id"]}),
            None,
            None
        ),
        creation
    );
}
