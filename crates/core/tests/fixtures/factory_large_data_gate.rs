//! Linux generated-fixture-only fault injection. No product hooks or endpoints.
use super::*;
use std::{
    io::Write,
    os::unix::fs::MetadataExt,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub(super) struct ReadGate {
    root: PathBuf,
    library: PathBuf,
    artifacts: PathBuf,
    held: Option<std::fs::File>,
    started: Instant,
}
fn write_private(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}
impl ReadGate {
    pub(super) fn compile(base: &Path, mode: &str, daemon: &Path, raw_daemon: &Path) -> Self {
        let started = Instant::now();
        let root = base.join("generated-read-gate");
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let source = root.join("generated-gate.c");
        let library = root.join("generated-gate.so");
        let body = format!(
            "#define CONTROL_DIR {}\n{}",
            serde_json::to_string(&root).unwrap(),
            include_str!("factory_data_read_gate.c")
        );
        write_private(&source, body.as_bytes());
        let compiler = Command::new("cc")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .arg("--version")
            .output()
            .unwrap();
        assert!(compiler.status.success());
        let args = vec![
            "-shared".to_string(),
            "-fPIC".into(),
            "-O2".into(),
            "-std=c11".into(),
            "-pthread".into(),
            source.display().to_string(),
            "-ldl".into(),
            "-o".into(),
            library.display().to_string(),
        ];
        let result = Command::new("cc")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(&args)
            .output()
            .unwrap();
        let artifacts = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/factory-runtime-core/large-data-read-diagnostics-20261029")
            .join(format!("{mode}-{}", sha256(body.as_bytes())));
        fs::create_dir_all(&artifacts).unwrap();
        write_private(&artifacts.join("generated-gate.c"), body.as_bytes());
        write_private(&artifacts.join("cc.stdout"), &result.stdout);
        write_private(&artifacts.join("cc.stderr"), &result.stderr);
        write_private(&artifacts.join("cc-version.stdout"), &compiler.stdout);
        assert!(
            result.status.success(),
            "cc failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        fs::set_permissions(&library, fs::Permissions::from_mode(0o700)).unwrap();
        fs::copy(&library, artifacts.join("generated-gate.so")).unwrap();
        let record = json!({"scope":"generated own-daemon Linux synthetic post-read stall only","mode":mode,"cc_argv":args,"source_sha256":sha256(body.as_bytes()),"library_sha256":sha256(&fs::read(&library).unwrap()),"installed_stripped_daemon_path":daemon,"installed_stripped_daemon_sha256":sha256(&fs::read(daemon).unwrap()),"raw_daemon_path":raw_daemon,"raw_daemon_sha256":sha256(&fs::read(raw_daemon).unwrap()),"explicit_daemon_environment":{"LD_PRELOAD":library},"not_injected_into":["Cargo","rustc","Host","other daemons","Providers"],"hard_failure_pause_bound_seconds":10});
        write_private(
            &artifacts.join("build.json"),
            &serde_json::to_vec_pretty(&record).unwrap(),
        );
        Self {
            root,
            library,
            artifacts,
            held: None,
            started,
        }
    }
    pub(super) fn phase(
        &self,
        name: &str,
        runtime: &Path,
        scope: Option<&serde_json::Value>,
        extra: serde_json::Value,
    ) {
        let began = Instant::now();
        let epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let mut state = Vec::new();
        let database = runtime.join("baton.sqlite3");
        if database.exists() {
            let observe = || -> rusqlite::Result<Vec<serde_json::Value>> {
                let db = Connection::open_with_flags(
                    &database,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                )?;
                db.busy_timeout(Duration::from_millis(20))?;
                let mut query = db.prepare(
                    "SELECT 'factory_scopes',id,CASE WHEN octet_length(document)<=65536 THEN document END FROM factory_scopes
                     UNION ALL SELECT 'factory_runs',id,CASE WHEN octet_length(document)<=65536 THEN document END FROM factory_runs
                     UNION ALL SELECT 'factory_task_bindings',task_id,CASE WHEN octet_length(document)<=65536 THEN document END FROM factory_task_bindings
                     UNION ALL SELECT 'sessions',id,CASE WHEN octet_length(document)<=65536 THEN document END FROM sessions
                     UNION ALL SELECT 'tasks',id,CASE WHEN octet_length(document)<=65536 THEN document END FROM tasks
                     UNION ALL SELECT 'execution_grants',id,CASE WHEN octet_length(document)<=65536 THEN document END FROM execution_grants
                     UNION ALL SELECT 'connections',id,CASE WHEN octet_length(document)<=65536 THEN document END FROM connections
                     UNION ALL SELECT 'managed_workspaces',id,CASE WHEN octet_length(document)<=65536 THEN document END FROM managed_workspaces
                     UNION ALL SELECT 'projects',id,CASE WHEN octet_length(document)<=65536 THEN document END FROM projects
                     UNION ALL SELECT 'dispatch_intents',id,CASE WHEN octet_length(document)<=65536 THEN document END FROM dispatch_intents
                     UNION ALL SELECT 'workspace_leases',resource,json_object('holder',holder,'fencing_token',fencing_token,'expires_at_ms',expires_at) FROM workspace_leases
                     UNION ALL SELECT 'factory_scoped_handles',id,json_object('scope_id',scope_id,'session_id',session_id,'attempt_id',attempt_id,'fencing_token',fencing_token,'expires_at_ms',expires_at_ms,'revoked',revoked) FROM factory_scoped_handles LIMIT 33",
                )?;
                let rows = query
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                assert!(
                    rows.len() <= 32,
                    "diagnostics refuse an unexpected large fixture, never truncate"
                );
                Ok(rows
                    .into_iter()
                    .map(|(table, id, raw)| {
                        let mut selected = serde_json::Map::new();
                        if let Some(raw) = raw {
                            selected
                                .insert("document_sha256".into(), json!(sha256(raw.as_bytes())));
                            let document: serde_json::Value = serde_json::from_str(&raw).unwrap();
                            for key in [
                                "revision",
                                "state",
                                "revoked",
                                "expires_at_ms",
                                "fencing_token",
                                "attempt_id",
                                "intent_id",
                                "session_id",
                                "active_session_id",
                                "cancel_requested",
                                "spec_revision",
                                "turn_state",
                                "holder",
                                "last_error",
                            ] {
                                if let Some(value) = document.get(key) {
                                    selected.insert(key.into(), value.clone());
                                }
                            }
                            if let Some(handle) = document.get("handle") {
                                selected
                                    .insert("task_handle_state".into(), handle["state"].clone());
                                selected.insert(
                                    "task_handle_revision".into(),
                                    handle["revision"].clone(),
                                );
                                selected.insert(
                                    "task_handle_confirmed_stop".into(),
                                    handle["confirmed_stop"].clone(),
                                );
                            }
                        } else {
                            selected.insert("oversize_not_read".into(), json!(true));
                        }
                        json!({"table":table,"id":id,"observed":selected})
                    })
                    .collect())
            };
            match observe() {
                Ok(rows) => state = rows,
                Err(error) => state.push(json!({"observation_error":error.to_string()})),
            }
        }
        let expires = scope.and_then(|scope| scope["expires_at_ms"].as_i64());
        let record = json!({"stage":name,"fixture_elapsed_ms":self.started.elapsed().as_millis(),"epoch_ms":epoch_ms,"scope_id":scope.map(|scope|scope["id"].clone()),"scope_revision":scope.map(|scope|scope["revision"].clone()),"scope_expires_at_ms":expires,"scope_remaining_ms_at_phase":expires.map(|deadline|i128::from(deadline)-i128::try_from(epoch_ms).unwrap()),"source_deadlines":{"client_socket_read_timeout_ms":5000,"actor_ReadyRequest_budget_ms":60000,"owned_DataIO_budget_ms":30000,"actor_and_worker_exact_Instant":"not observable over wire; source constants only, never relabel client timeout as scope expiry"},"trusted_state":state,"extra":extra,"diagnostic_observation_ms":began.elapsed().as_millis()});
        let bytes = serde_json::to_vec_pretty(&record).unwrap();
        // No request or credential document is ever serialized. The credential-
        // bearing handle table is read only through explicit public metadata;
        // token/token_digest are never selected.
        assert!(!String::from_utf8_lossy(&bytes).contains("scoped_token"));
        write_private(&self.artifacts.join(format!("phase-{name}.json")), &bytes);
    }
    pub(super) fn configure_own_daemon(&self, command: &mut Command) {
        // Process-local Command environment only, never std::env::set_var.
        command.env("LD_PRELOAD", &self.library);
    }
    fn arm(&mut self, input: &Path) {
        let held = fs::OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(input)
            .unwrap();
        let m = held.metadata().unwrap();
        assert!(m.is_file());
        assert_eq!(m.nlink(), 1);
        assert_eq!(m.mode() & 0o7777, 0o600);
        assert_eq!(m.len(), 16 * 1024 * 1024);
        write_private(
            &self.root.join("identity"),
            format!("{} {} {}\n", m.dev(), m.ino(), m.len()).as_bytes(),
        );
        self.held = Some(held);
        write_private(&self.root.join("enabled"), b"generated fixture only\n");
    }
    fn entered(&self, daemon_id: u32) -> serde_json::Value {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(bytes) = fs::read(self.root.join("entered")) {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    assert_eq!(v["pid"], daemon_id);
                    assert_ne!(
                        v["tid"], v["pid"],
                        "large read must be off the daemon Actor thread"
                    );
                    assert_eq!(v["read_bytes"], 65536);
                    let m = self.held.as_ref().unwrap().metadata().unwrap();
                    assert_eq!(v["device"], m.dev());
                    assert_eq!(v["inode"], m.ino());
                    write_private(
                        &self.artifacts.join("actual-read-entered.json"),
                        &serde_json::to_vec_pretty(&v).unwrap(),
                    );
                    return v;
                }
            }
            assert!(
                Instant::now() < until,
                "no actual selected-file64KiB read; no in-flight coverage established"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub(super) fn stop_original_daemon(&self, child: &mut std::process::Child) {
        let pid = child.id();
        assert!(child.try_wait().unwrap().is_none());
        let began = Instant::now();
        // This Child is still owned and unreaped: its PID cannot be reused here.
        assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGTERM) }, 0);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            assert!(
                began.elapsed() < Duration::from_secs(5),
                "own daemon shutdown did not finish"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        write_private(&self.artifacts.join("owned-daemon-exit.json"),
            &serde_json::to_vec_pretty(&json!({"pid":pid,"status_code":status.code(),"elapsed_ms":began.elapsed().as_millis(),"signal":"SIGTERM sent only to original unreaped Child","all_descendants_gone":"not_claimed"})).unwrap());
        assert!(status.success(), "own daemon shutdown failed: {status}");
    }
    fn release(&self) {
        let path = self.root.join("release");
        if !path.exists() {
            write_private(&path, b"explicit fixture release\n");
        }
    }
}
impl Drop for ReadGate {
    fn drop(&mut self) {
        self.release();
    }
}
struct ReleaseOnFailure<'a>(&'a ReadGate);
impl Drop for ReleaseOnFailure<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

pub(super) fn run_inflight_case(
    gate: &mut ReadGate,
    runtime: &Path,
    workspace: &Path,
    scope: &serde_json::Value,
    selection: &serde_json::Value,
    daemon_id: u32,
    mode: &str,
) {
    let until = Instant::now() + Duration::from_secs(5);
    while !workspace.join("reports/pin-ready").exists() {
        if Instant::now() >= until {
            gate.phase("owned_marker_timeout",runtime,Some(scope),json!({"fixture_stderr":fs::read_to_string(workspace.join("reports/fixture-stderr.txt")).ok()}));
        }
        assert!(
            Instant::now() < until,
            "actual generated MCP parent did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    gate.phase("owned_marker_ready",runtime,Some(scope),json!({"owned_initial_mcp_status":fs::read_to_string(workspace.join("reports/pin-ready")).ok()}));
    let credential_path = fs::read_dir(runtime.join("factory-scoped-handles"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let credential: serde_json::Value =
        serde_json::from_slice(&fs::read(credential_path).unwrap()).unwrap();
    gate.phase(
        "credential_ready",
        runtime,
        Some(scope),
        json!({"handle_id":credential["handle_id"]}),
    );
    let raw = json!({"api_version":1,"request_id":"controlled-large-data-original-nonce","handle_id":credential["handle_id"],"scoped_token":credential["scoped_token"],"method":"get_task_status","params":{"task_id":scope["root_task_id"]}});
    gate.phase("initial_get_begin",runtime,Some(scope),json!({"request_id":"controlled-large-data-original-nonce","method":"get_task_status","client_deadline_epoch_ms":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()+5000}));
    let before = super::scoped_request(runtime, raw.clone());
    gate.phase(
        "initial_get_response",
        runtime,
        Some(scope),
        json!({"complete_response_envelope":before}),
    );
    assert!(
        before.get("error").is_none(),
        "must first prove the real handle/witness works: {before}"
    );
    gate.arm(&workspace.join("inputs/input.txt"));
    gate.phase(
        "armed",
        runtime,
        Some(scope),
        json!({"actual_gate_entered":false}),
    );
    let _cleanup = ReleaseOnFailure(gate);
    let target = runtime.to_path_buf();
    let pending = std::thread::spawn(move || super::scoped_request(&target, raw));
    let entered = gate.entered(daemon_id);
    gate.phase(
        "entered",
        runtime,
        Some(scope),
        json!({"actual_read_observation":entered}),
    );
    let thread_path = PathBuf::from(format!(
        "/proc/{daemon_id}/task/{}",
        entered["tid"].as_u64().unwrap()
    ));
    assert!(
        thread_path.exists(),
        "original read worker must actually be alive while held"
    );
    write_private(&gate.artifacts.join("owned-process-before.json"), &serde_json::to_vec_pretty(&json!({"daemon_id":daemon_id,"read_worker_tid":entered["tid"],"read_worker_stat":fs::read_to_string(thread_path.join("stat")).unwrap()})).unwrap());
    assert!(
        !pending.is_finished(),
        "a finished/Ready race is not in-flight coverage"
    );
    let db = Connection::open(runtime.join("baton.sqlite3")).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT active FROM budgets WHERE scope=?1",
            [format!("project:{}", scope["project_id"].as_str().unwrap())],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    let mut other = json!({"api_version":1,"request_id":"second-read-cannot-bypass-worker-slot","handle_id":credential["handle_id"],"scoped_token":credential["scoped_token"],"method":"get_task_result","params":{"task_id":scope["root_task_id"]}});
    let busy = super::scoped_request(runtime, other.take());
    assert_eq!(busy["error"]["code"], "WORKSPACE_BUSY", "{busy}");
    assert!(!pending.is_finished());
    let sibling = super::mixed_call(
        runtime,
        json!({"method":"tasks.create","params":{"project_id":scope["project_id"],"goal":"Owner-only pending worker occupancy probe; never approved"},"idempotency_key":"large-data-worker-slot-probe-task"}),
    );
    let started = Instant::now();
    let current = super::mixed_call(
        runtime,
        json!({"method":"factory_runs.get","params":{"scope_id":scope["id"]}}),
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "Owner history/status blocked on dataset byte I/O"
    );
    let status_elapsed_ms = started.elapsed().as_millis();
    let mut revoke_elapsed_ms = None;
    if mode == "read_gate_revoke" {
        let began = Instant::now();
        let revoked = super::mixed_call(
            runtime,
            json!({"method":"factory_scopes.revoke","params":{"scope_id":scope["id"]},"expected_revision":scope["revision"],"idempotency_key":"large-data-owned-revoke"}),
        );
        assert!(began.elapsed() < Duration::from_secs(3));
        revoke_elapsed_ms = Some(began.elapsed().as_millis());
        assert_eq!(revoked["revoked"], true);
    } else if mode == "read_gate_expiry" {
        let until = Instant::now() + Duration::from_secs(5);
        while i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap()
            < scope["expires_at_ms"].as_i64().unwrap()
        {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let mut stop_elapsed_ms = None;
    if mode == "read_gate_cancel" {
        let began = Instant::now();
        let stopped = super::mixed_call(
            runtime,
            json!({"method":"factory_runs.cancel","params":{"scope_id":scope["id"]},"expected_revision":current["run"]["revision"],"idempotency_key":"large-data-owned-stop"}),
        );
        stop_elapsed_ms = Some(began.elapsed().as_millis());
        assert!(began.elapsed() < Duration::from_secs(3));
        assert!(stopped["cancel_requested"] == true || stopped["state"] == "cancelled");
    }
    // Revoke and expiry cases deliberately DO NOT issue Ownercancel: otherwise
    // its explicit cancellation would mask whether their own boundary worked.
    let responsive = Instant::now();
    let _ = super::mixed_call(
        runtime,
        json!({"method":"factory_runs.get","params":{"scope_id":scope["id"]}}),
    );
    assert!(responsive.elapsed() < Duration::from_secs(3));
    assert!(
        !pending.is_finished(),
        "cancellation is not an invented join while the real read is still held"
    );
    let mut probe = selection.clone();
    probe["root_task_id"] = sibling["id"].clone();
    probe["expected_task_revision"] = sibling["revision"].clone();
    let busy=agentbaton_core::ipc::call(runtime,json!({"method":"factory_scopes.propose","params":probe,"idempotency_key":"must-retain-large-data-owned-worker-slot"})).unwrap();
    assert_eq!(
        busy["error"]["code"], "WORKSPACE_BUSY",
        "worker was prematurely released: {busy}"
    );
    assert!(!gate.root.join("left").exists());
    assert!(thread_path.exists());
    let held_active: i64 = db
        .query_row(
            "SELECT active FROM budgets WHERE scope=?1",
            [format!("project:{}", scope["project_id"].as_str().unwrap())],
            |r| r.get(0),
        )
        .unwrap();
    let held_leases: i64 = db
        .query_row(
            "SELECT count(*) FROM workspace_leases WHERE holder!=''",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        held_active, 1,
        "a stop request must not release the original still-owned attempt"
    );
    assert_eq!(
        held_leases, 1,
        "the original workspace must remain fenced before actual cleanup"
    );
    assert!(!gate.root.join("fixture_timeout").exists());
    gate.release();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !pending.is_finished() {
        assert!(
            Instant::now() < deadline,
            "cancelled original read did not complete after explicit release"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let denied = pending.join().unwrap();
    assert_eq!(denied["request_id"], "controlled-large-data-original-nonce");
    assert!(
        denied.get("error").is_some(),
        "released cancelled observation cannot authorize a result: {denied}"
    );
    assert!(denied.get("result").is_none());
    assert!(!gate.root.join("fixture_timeout").exists());
    assert!(gate.root.join("left").exists());
    assert!(
        !thread_path.exists(),
        "pending response must follow the original worker exit/join, not precede it"
    );
    write_private(&gate.artifacts.join("owned-process-after-read.json"), &serde_json::to_vec_pretty(&json!({"daemon_id":daemon_id,"read_worker_tid":entered["tid"],"original_worker_thread_present":false,"daemon_present":PathBuf::from(format!("/proc/{daemon_id}")).exists()})).unwrap());
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let (open,active,reaped):(i64,i64,i64)=db.query_row("SELECT (SELECT count(*) FROM dispatch_intents WHERE state!='stopped'),(SELECT COALESCE(sum(active),0) FROM budgets WHERE scope LIKE 'project:%'),(SELECT count(*) FROM events WHERE json_extract(document,'$.kind')='session.stopped' AND json_extract(document,'$.payload.process_reaped')=1 AND json_extract(document,'$.payload.no_process_started')=0)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        if open == 0 && active == 0 && reaped == 1 {
            break;
        }
        assert!(
            Instant::now() < until,
            "owned original process/worker not truly stopped"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        db.query_row("SELECT count(*) FROM factory_delegations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM factory_attempts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT consumed FROM budgets WHERE scope=?1",
            [format!("project:{}", scope["project_id"].as_str().unwrap())],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_leases WHERE holder!=''",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM factory_scopes", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let report = json!({"mode":mode,"actual_read_bytes_before_stall":65536,"dataset_bytes":16*1024*1024,"owner_status_responsive":true,"explicit_owner_cancel_issued":mode=="read_gate_cancel","approved_scope_ttl_ms":scope["selection"]["ttl_ms"],"owner_status_elapsed_ms":status_elapsed_ms,"owner_stop_elapsed_ms":stop_elapsed_ms,"owner_revoke_elapsed_ms":revoke_elapsed_ms,"active_before_release":held_active,"leases_before_release":held_leases,"read_worker_thread_gone_after_pending_reply":true,"worker_busy_until_explicit_release":true,"original_request_nonce":denied["request_id"],"read_response":denied,"attempts":1,"consumed":1,"children":0,"original_leader_reaped":true,"complete_descendant_containment":"not_established","kernel_read_interruption":"not_claimed","macOS":"not_run","ready_request_queue_deadline":"not_run; expiry mode tests approved scope expiry"});
    write_private(
        &gate.artifacts.join("outcome.json"),
        &serde_json::to_vec_pretty(&report).unwrap(),
    );
}
