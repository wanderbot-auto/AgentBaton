//! Main-thread real WebView -> production Tauri invoke -> private daemon harness.
//! This separate binary is never built by the default application feature set.
#[cfg(not(debug_assertions))]
compile_error!("native-smoke is a debug-only integration harness; never ship it");

use agentbaton_core::ipc;
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};

// Fixed original UTF-8 bytes, deliberately never parsed/stringified through a
// JS Number. A runtime WebView must supply its real WebCrypto digest evidence.
const WEBCRYPTO_INPUT: &str = "{\"text\":\"原文证据🔒\",\"inode\":1152921504606846979,\"u64\":9223372036854775809,\"max\":18446744073709551615}\n";
const WEBCRYPTO_UTF8_BYTES: u64 = 109;
const WEBCRYPTO_SHA256: &str = "d29817e7338bcd0db0c7f22a98e3faa82802bb73f11d67ad2c65e789d1dd2892";
fn webcrypto_expected(checkpoint: &str) -> Value {
    json!({"version":1,"checkpoint":checkpoint,"available":true,"source":"window.crypto.subtle.digest","algorithm":"SHA-256","encoding":"UTF-8","input":WEBCRYPTO_INPUT,"input_bytes":WEBCRYPTO_UTF8_BYTES,"sha256":WEBCRYPTO_SHA256})
}
fn validate_webcrypto(proof: &Value, checkpoint: &str) -> Result<(), String> {
    if proof != &webcrypto_expected(checkpoint) {
        return Err("Missing or inconsistent real WebView WebCrypto UTF-8 SHA-256 evidence".into());
    }
    Ok(())
}
fn record_webcrypto_checkpoint(
    report: &mut Value,
    checkpoint: &str,
    proof: &Value,
) -> Result<(), String> {
    validate_webcrypto(proof, checkpoint)?;
    match checkpoint {
        "before" if report.get("webcrypto_checkpoints").is_none() => {
            report["webcrypto_checkpoints"] = json!({"before":proof});
        }
        "after"
            if report["webcrypto_checkpoints"]
                .as_object()
                .is_some_and(|v| v.len() == 1) =>
        {
            validate_webcrypto(&report["webcrypto_checkpoints"]["before"], "before")?;
            report["webcrypto_checkpoints"]["after"] = proof.clone();
        }
        _ => return Err("Mixed, repeated or out-of-order WebCrypto checkpoint".into()),
    }
    Ok(())
}
fn validate_webcrypto_evidence(
    result: &Value,
    report: &Value,
    reopened: bool,
) -> Result<(), String> {
    if !report["webcrypto_checkpoints"]
        .as_object()
        .is_some_and(|v| v.len() == 2)
    {
        return Err("Incomplete first-WebView WebCrypto checkpoints".into());
    }
    validate_webcrypto(&report["webcrypto_checkpoints"]["before"], "before")?;
    validate_webcrypto(&report["webcrypto_checkpoints"]["after"], "after")?;
    validate_webcrypto(
        &result["webCrypto"],
        if reopened { "reopened" } else { "first" },
    )?;
    if reopened {
        validate_webcrypto(&report["first_webview"]["webCrypto"], "first")?;
    }
    Ok(())
}

const GOAL_READONLY_SQL: &str = r#"
import json,sqlite3,sys
from pathlib import Path
db=sqlite3.connect(Path(sys.argv[1]).resolve().as_uri()+'?mode=ro',uri=True,timeout=1,isolation_level=None)
db.execute('PRAGMA query_only=ON')
db.execute('BEGIN')
row=db.execute('SELECT (SELECT count(*) FROM goal_scopes),(SELECT count(*) FROM goal_plan_revisions),(SELECT count(*) FROM attempts),(SELECT COALESCE(max(sequence),0) FROM events)').fetchone()
print(json.dumps(dict(zip(('scope_count','plan_count','attempt_count','sequence'),row))))
db.close()
"#;

const MISSING_GOAL_RUN_ID: &str = "native-readonly-missing-goal-run";
const GOAL_EXECUTION_READONLY_SQL: &str = r#"
import json,sqlite3,sys
from pathlib import Path
db=sqlite3.connect(Path(sys.argv[1]).resolve().as_uri()+'?mode=ro',uri=True,timeout=1,isolation_level=None)
db.execute('PRAGMA query_only=ON')
db.execute('BEGIN')
schema=db.execute('PRAGMA user_version').fetchone()[0]
row=db.execute('SELECT (SELECT count(*) FROM goal_runs),(SELECT count(*) FROM goal_run_nodes),(SELECT count(*) FROM goal_run_reservations),(SELECT COALESCE(max(sequence),0) FROM events)').fetchone()
print(json.dumps(dict(schema_version=schema,**dict(zip(('run_count','node_count','reservation_count','sequence'),row)))))
db.close()
"#;

fn goal_execution_database_snapshot(runtime: &Path) -> Result<Value, String> {
    let mut command = Command::new("python3");
    command
        .args(["-I", "-S", "-c", GOAL_EXECUTION_READONLY_SQL])
        .arg(runtime.join("baton.sqlite3"));
    let bytes = bounded_goal_query(command)?;
    let snapshot: Value =
        serde_json::from_slice(&bytes).map_err(|_| "invalid readonly Goal execution counts")?;
    if !valid_goal_execution_counts(&snapshot) {
        return Err(
            "Goal execution fixture must have schema12 and no run/node/reservation authority"
                .into(),
        );
    }
    Ok(snapshot)
}
fn valid_goal_execution_counts(value: &Value) -> bool {
    value.as_object().is_some_and(|record| record.len() == 5)
        && value["schema_version"].as_u64() == Some(12)
        && ["run_count", "node_count", "reservation_count"]
            .iter()
            .all(|key| value[*key].as_u64() == Some(0))
        && value["sequence"].as_u64().is_some()
}
fn record_goal_execution_checkpoint(
    phase: u8,
    label: &str,
    report: &mut Value,
    checkpoint: &str,
    snapshot: Value,
) -> Result<(), String> {
    if phase != 0
        || label != "main"
        || report.get("error").is_some()
        || report["status"] == "failed"
        || !valid_goal_execution_counts(&snapshot)
        || !valid_goal_counts(&report["goal_readonly_database"][checkpoint])
        || report["goal_readonly_database"][checkpoint]["sequence"] != snapshot["sequence"]
    {
        return Err("readonly Goal execution checkpoint is outside the phase, schema or original Goal interval".into());
    }
    match checkpoint {
        "before" if report.get("goal_execution_readonly_database").is_none() => {
            report["goal_execution_readonly_database"] =
                json!({"read_only":true,"before":snapshot});
        }
        "after"
            if report["goal_execution_readonly_database"]["before"] == snapshot
                && report["goal_execution_readonly_database"]
                    .get("after")
                    .is_none() =>
        {
            report["goal_execution_readonly_database"]["after"] = snapshot;
            report["goal_execution_readonly_database"]["unchanged"] = json!(true);
        }
        _ => {
            return Err(
                "readonly Goal execution counts/sequence changed or checkpoint replayed".into(),
            )
        }
    }
    Ok(())
}
fn validate_goal_execution_evidence(result: &Value, report: &Value) -> Result<(), String> {
    let database = &report["goal_execution_readonly_database"];
    let sequence = &result["goalExecutionReadonlySequence"];
    let missing = &result["goalExecutionMissingRun"];
    if [
        "goalExecutionLists",
        "goalExecutionMissingRunDenied",
        "readonlyGoalExecutionNoMutation",
    ]
    .iter()
    .any(|key| result[*key] != true)
        || database["read_only"] != true
        || database["unchanged"] != true
        || !valid_goal_execution_counts(&database["before"])
        || !valid_goal_execution_counts(&database["after"])
        || database["before"] != database["after"]
        || !sequence.as_object().is_some_and(|values| {
            values.len() == 2 && values.contains_key("before") && values.contains_key("after")
        })
        || ["before", "after"].iter().any(|phase| {
            sequence[*phase].as_u64().is_none()
                || sequence[*phase] != database[*phase]["sequence"]
                || sequence[*phase] != report["goal_readonly_database"][*phase]["sequence"]
                || sequence[*phase] != result["goalReadonlySequence"][*phase]
        })
        || !missing.as_object().is_some_and(|values| values.len() == 3)
        || missing["run_id"] != MISSING_GOAL_RUN_ID
        || missing["code"] != "NOT_FOUND"
        || missing["side_effect_state"] != "not_started"
    {
        return Err("missing, fabricated or changed readonly Goal execution evidence".into());
    }
    Ok(())
}

const MISSING_GOAL_GRAPH_RUN_ID: &str = "native-readonly-missing-goal-graph-run";
const GOAL_GRAPH_COUNT_FIELDS: [&str; 6] = [
    "capability_count",
    "job_count",
    "candidate_count",
    "validation_count",
    "owner_decision_count",
    "acceptance_count",
];
const GOAL_GRAPH_READONLY_SQL: &str = r#"
import json,sqlite3,sys
from pathlib import Path
db=sqlite3.connect(Path(sys.argv[1]).resolve().as_uri()+'?mode=ro',uri=True,timeout=1,isolation_level=None)
db.execute('PRAGMA query_only=ON')
db.execute('BEGIN')
schema=db.execute('PRAGMA user_version').fetchone()[0]
row=db.execute('SELECT (SELECT count(*) FROM goal_graph_capabilities),(SELECT count(*) FROM goal_graph_jobs),(SELECT count(*) FROM goal_graph_candidates),(SELECT count(*) FROM goal_graph_validations),(SELECT count(*) FROM goal_graph_owner_decisions),(SELECT count(*) FROM goal_graph_acceptances),(SELECT COALESCE(max(sequence),0) FROM events)').fetchone()
print(json.dumps(dict(schema_version=schema,**dict(zip(('capability_count','job_count','candidate_count','validation_count','owner_decision_count','acceptance_count','sequence'),row)))))
db.close()
"#;

fn valid_goal_graph_counts(value: &Value) -> bool {
    value.as_object().is_some_and(|record| record.len() == 8)
        && value["schema_version"].as_u64() == Some(12)
        && GOAL_GRAPH_COUNT_FIELDS
            .iter()
            .all(|key| value[*key].as_u64() == Some(0))
        && value["sequence"].as_u64().is_some()
}
fn goal_graph_database_snapshot(runtime: &Path) -> Result<Value, String> {
    let mut command = Command::new("python3");
    command
        .args(["-I", "-S", "-c", GOAL_GRAPH_READONLY_SQL])
        .arg(runtime.join("baton.sqlite3"));
    let bytes = bounded_goal_query(command)?;
    let snapshot: Value =
        serde_json::from_slice(&bytes).map_err(|_| "invalid readonly Goal graph counts")?;
    if !valid_goal_graph_counts(&snapshot) {
        return Err("Goal graph fixture must have schema12 and six empty graph tables".into());
    }
    Ok(snapshot)
}
fn record_goal_graph_checkpoint(
    phase: u8,
    label: &str,
    report: &mut Value,
    checkpoint: &str,
    snapshot: Value,
) -> Result<(), String> {
    if phase != 0
        || label != "main"
        || report.get("error").is_some()
        || report["status"] == "failed"
        || !valid_goal_graph_counts(&snapshot)
        || !valid_goal_counts(&report["goal_readonly_database"][checkpoint])
        || !valid_goal_execution_counts(&report["goal_execution_readonly_database"][checkpoint])
        || snapshot["sequence"] != report["goal_readonly_database"][checkpoint]["sequence"]
        || snapshot["sequence"]
            != report["goal_execution_readonly_database"][checkpoint]["sequence"]
    {
        return Err(
            "readonly Goal graph checkpoint is outside the phase, schema or original intervals"
                .into(),
        );
    }
    match checkpoint {
        "before" if report.get("goal_graph_readonly_database").is_none() => {
            report["goal_graph_readonly_database"] = json!({"read_only":true,"before":snapshot});
        }
        "after"
            if report["goal_graph_readonly_database"]["before"] == snapshot
                && report["goal_graph_readonly_database"]
                    .get("after")
                    .is_none() =>
        {
            report["goal_graph_readonly_database"]["after"] = snapshot;
            report["goal_graph_readonly_database"]["unchanged"] = json!(true);
        }
        _ => {
            return Err("readonly Goal graph counts/sequence changed or checkpoint replayed".into())
        }
    }
    Ok(())
}
fn validate_goal_graph_evidence(result: &Value, report: &Value) -> Result<(), String> {
    // Graph evidence adds to the original independently observed execution
    // evidence; a later/fabricated graph interval cannot replace it.
    validate_goal_execution_evidence(result, report)?;
    let database = &report["goal_graph_readonly_database"];
    let sequence = &result["goalGraphReadonlySequence"];
    let missing = &result["goalGraphMissingRun"];
    if [
        "goalGraphLists",
        "goalGraphMissingRunDenied",
        "readonlyGoalGraphNoMutation",
    ]
    .iter()
    .any(|key| result[*key] != true)
        || database["read_only"] != true
        || database["unchanged"] != true
        || !valid_goal_graph_counts(&database["before"])
        || !valid_goal_graph_counts(&database["after"])
        || database["before"] != database["after"]
        || !sequence.as_object().is_some_and(|values| {
            values.len() == 2 && values.contains_key("before") && values.contains_key("after")
        })
        || ["before", "after"].iter().any(|phase| {
            !valid_goal_counts(&report["goal_readonly_database"][*phase])
                || sequence[*phase].as_u64().is_none()
                || sequence[*phase] != database[*phase]["sequence"]
                || sequence[*phase] != report["goal_readonly_database"][*phase]["sequence"]
                || sequence[*phase]
                    != report["goal_execution_readonly_database"][*phase]["sequence"]
                || sequence[*phase] != result["goalReadonlySequence"][*phase]
                || sequence[*phase] != result["goalExecutionReadonlySequence"][*phase]
        })
        || !missing.as_object().is_some_and(|values| values.len() == 3)
        || missing["run_id"] != MISSING_GOAL_GRAPH_RUN_ID
        || missing["code"] != "NOT_FOUND"
        || missing["side_effect_state"] != "not_started"
    {
        return Err("missing, fabricated or changed readonly Goal graph evidence".into());
    }
    Ok(())
}

/// A private test host query, never a renderer-selected executable, path or SQL.
/// It uses only Python's isolated standard library and a read-only transaction.
fn goal_database_snapshot(runtime: &Path) -> Result<Value, String> {
    let mut command = Command::new("python3");
    command
        .args(["-I", "-S", "-c", GOAL_READONLY_SQL])
        .arg(runtime.join("baton.sqlite3"));
    let bytes = bounded_goal_query(command)?;
    let snapshot: Value =
        serde_json::from_slice(&bytes).map_err(|_| "invalid readonly Goal counts")?;
    if !valid_goal_counts(&snapshot) {
        return Err("Goal fixture counts are not empty bounded integers".into());
    }
    Ok(snapshot)
}
fn bounded_goal_query(mut command: Command) -> Result<Vec<u8>, String> {
    struct OwnedQuery(Child);
    impl Drop for OwnedQuery {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = OwnedQuery(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "cannot start readonly Goal query")?,
    );
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or("missing readonly query output")?;
    let reader = std::thread::Builder::new()
        .name("native-readonly-goal-query".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            stdout.take(4097).read_to_end(&mut bytes).map(|_| bytes)
        })
        .map_err(|_| "cannot observe readonly Goal query")?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let outcome: Result<std::process::ExitStatus, String> = (|| loop {
        if let Some(status) = child
            .0
            .try_wait()
            .map_err(|_| "cannot observe readonly Goal query")?
        {
            break Ok(status);
        }
        if Instant::now() >= deadline {
            break Err("readonly Goal query exceeded its bound".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    })();
    if outcome.is_err() {
        let _ = child.0.kill();
        let _ = child.0.wait();
    }
    let bytes = reader
        .join()
        .map_err(|_| "readonly Goal query reader failed")?
        .map_err(|_| "readonly Goal query read failed")?;
    if !outcome?.success() || bytes.len() > 4096 {
        return Err("readonly Goal query failed or exceeded raw output bound".into());
    }
    Ok(bytes)
}
fn valid_goal_counts(value: &Value) -> bool {
    value.as_object().is_some_and(|record| record.len() == 4)
        && ["scope_count", "plan_count", "attempt_count"]
            .iter()
            .all(|key| value[*key].as_u64() == Some(0))
        && value["sequence"].as_u64().is_some()
}
fn prepare_readonly_goal_project(runtime: &Path) -> Result<String, String> {
    // Match the real daemon's private authority under umask 022 as well as 077.
    // Fresh ownership is mandatory: never chmod, replace or reuse an existing
    // database merely to make this disposable fixture pass admission.
    let database = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(runtime.join("baton.sqlite3"))
        .map_err(|_| "cannot create a fresh private fixture database")?;
    drop(database);
    let directory = runtime.join("goal-readonly-project");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(|_| "cannot prepare private project directory")?;
    fs::write(
        directory.join("generated.txt"),
        b"Generated readonly project fixture; no execution authority.\n",
    )
    .map_err(|_| "cannot prepare generated project file")?;
    let mut store = agentbaton_core::storage::Store::open(&runtime.join("baton.sqlite3"))
        .map_err(|_| "cannot initialize private fixture store")?;
    let project = store
        .import_project(
            &directory,
            "Native readonly Goal project",
            "native-smoke-readonly-project",
        )
        .map_err(|_| "cannot register generated readonly project")?;
    if !store
        .list()
        .map_err(|_| "cannot inspect private fixture tasks")?
        .is_empty()
    {
        return Err("readonly project setup unexpectedly created a task".into());
    }
    Ok(project.id)
}
fn record_goal_checkpoint(
    phase: u8,
    label: &str,
    report: &mut Value,
    checkpoint: &str,
    snapshot: Value,
) -> Result<(), String> {
    if phase != 0
        || label != "main"
        || report.get("error").is_some()
        || report["status"] == "failed"
        || !valid_goal_counts(&snapshot)
    {
        return Err("readonly Goal checkpoint is not valid in this native phase".into());
    }
    match checkpoint {
        "before" if report.get("goal_readonly_database").is_none() => {
            report["goal_readonly_database"] = json!({"read_only":true,"before":snapshot});
        }
        "after"
            if report["goal_readonly_database"]["before"] == snapshot
                && report["goal_readonly_database"].get("after").is_none() =>
        {
            report["goal_readonly_database"]["after"] = snapshot;
            report["goal_readonly_database"]["unchanged"] = json!(true);
        }
        _ => {
            return Err(
                "readonly Goal counts or sequence changed, or checkpoint was replayed".into(),
            )
        }
    }
    Ok(())
}

struct OwnedDaemon {
    child: Child,
    executable: PathBuf,
    runtime: PathBuf,
}
impl OwnedDaemon {
    fn start(executable: PathBuf, runtime: PathBuf) -> Result<Self, String> {
        let child = Command::new(&executable)
            .env("BATON_RUNTIME_DIR", &runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("daemon launch: {e}"))?;
        let mut owned = Self {
            child,
            executable,
            runtime,
        };
        owned.ready()?;
        Ok(owned)
    }
    fn ready(&mut self) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
                return Err(format!(
                    "owned daemon exited before authentication ({status})"
                ));
            }
            if let Ok(value) = ipc::call(
                &self.runtime,
                json!({"method":"system.snapshot","params":{}}),
            ) {
                if value.get("error").is_none() {
                    return Ok(value);
                }
            }
            if Instant::now() >= deadline {
                return Err("daemon readiness deadline".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn reject_second_writer(&self) -> Result<(), String> {
        let mut second = Command::new(&self.executable)
            .env("BATON_RUNTIME_DIR", &self.runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = second.try_wait().map_err(|e| e.to_string())? {
                return if status.success() {
                    Err("second writer unexpectedly succeeded".into())
                } else {
                    Ok(())
                };
            }
            if Instant::now() >= deadline {
                let _ = second.kill();
                let _ = second.wait();
                return Err("second writer did not reject within deadline".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn restart(&mut self) -> Result<(), String> {
        self.child.kill().map_err(|e| e.to_string())?;
        self.child.wait().map_err(|e| e.to_string())?;
        self.child = Command::new(&self.executable)
            .env("BATON_RUNTIME_DIR", &self.runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        self.ready()?;
        Ok(())
    }
}
impl Drop for OwnedDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
// Main-scope cleanup must run even if the platform event-loop setup panics.
// Managed App state and watchdog handles may outlive unwinding; an Arc drop alone
// is insufficient. Keep a separate guard that explicitly reaps this owned Child.
struct CleanupGuard(Arc<Mutex<OwnedDaemon>>);
impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let mut daemon = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let _ = daemon.child.kill();
        let _ = daemon.child.wait();
    }
}
struct Smoke {
    phase: Arc<AtomicU8>,
    injected: AtomicU8,
    daemon: Arc<Mutex<OwnedDaemon>>,
    report: Arc<Mutex<Value>>,
    failure_reason: Arc<Mutex<Option<String>>>,
    reopen_worker: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    output: PathBuf,
}
impl Smoke {
    fn stage(&self, stage: &str) -> Result<(), String> {
        let mut report = self.report.lock().unwrap();
        let stages = report["stages"]
            .as_array_mut()
            .ok_or("missing stage journal")?;
        if stages.len() >= 32 {
            return Err("native lifecycle stage limit".into());
        }
        stages.push(json!(stage));
        fs::write(
            self.output.join("native-smoke.json"),
            serde_json::to_vec_pretty(&*report).unwrap(),
        )
        .map_err(|error| error.to_string())
    }
    fn fail(&self, app: &tauri::AppHandle, reason: impl ToString) {
        record_failure(
            &self.phase,
            &self.failure_reason,
            &self.report,
            &reason.to_string(),
        );
        if let Ok(report) = self.report.lock() {
            let _ = fs::write(
                self.output.join("native-smoke.json"),
                serde_json::to_vec_pretty(&*report).unwrap(),
            );
        }
        app.exit(1);
    }
}
fn terminal_phase(phase: u8) -> bool {
    matches!(phase, 3 | 4)
}
fn claim_timeout(phase: &AtomicU8, failure_reason: &Mutex<Option<String>>, reason: &str) -> bool {
    let mut first = failure_reason
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    loop {
        let current = phase.load(Ordering::SeqCst);
        if terminal_phase(current) {
            return false;
        }
        if phase
            .compare_exchange(current, 4, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            if first.is_none() {
                *first = Some(reason.into());
            }
            return true;
        }
    }
}
fn record_failure(
    phase: &AtomicU8,
    failure_reason: &Mutex<Option<String>>,
    report: &Mutex<Value>,
    reason: &str,
) {
    // Publish failure before waiting for a handler's report lock, so a late
    // handler cannot commit an old successful phase after the deadline.
    let reason = {
        let mut first = failure_reason
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if first.is_none() {
            *first = Some(reason.into());
        }
        phase.store(4, Ordering::SeqCst);
        first.as_ref().unwrap().clone()
    };
    let mut report = report.lock().unwrap_or_else(|error| error.into_inner());
    report["status"] = json!("failed");
    if report.get("error").is_none() {
        report["error"] = json!(reason);
    }
}
fn commit_native_report(
    phase: &AtomicU8,
    report: &mut Value,
    expected: u8,
    result: Value,
) -> Result<(), String> {
    if report.get("error").is_some() || report["status"] == "failed" {
        return Err("native failure already recorded".into());
    }
    let next = match expected {
        0 => 1,
        2 if result["taskId"] == report["first_webview"]["taskId"] => 3,
        2 => return Err("reopened WebView saw a different task".into()),
        _ => return Err("unexpected report phase".into()),
    };
    phase
        .compare_exchange(expected, next, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "native phase changed before report commit".to_string())?;
    if expected == 0 {
        report["first_webview"] = result;
    } else {
        report["reopened_webview"] = result;
        report["status"] = json!("checks_passed_cleanup_pending");
    }
    Ok(())
}
fn finalize_native_report(
    code: i32,
    phase: &AtomicU8,
    failure_reason: &Mutex<Option<String>>,
    report: &mut Value,
) -> bool {
    let passed = code == 0
        && phase.load(Ordering::SeqCst) == 3
        && failure_reason
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_none()
        && report.get("error").is_none()
        && report["status"] == "checks_passed_cleanup_pending"
        && report["owned_daemon_cleanup"] == true;
    report["status"] = json!(if passed { "passed" } else { "failed" });
    passed
}
fn capture_screen(output: &Path, window: &tauri::WebviewWindow) -> &'static str {
    let _ = window;
    #[cfg(target_os = "macos")]
    let mut command = {
        if objc2::MainThreadMarker::new().is_none() {
            return "capture_main_thread_unavailable_not_passed";
        }
        let Ok(pointer) = window.ns_window() else {
            return "capture_window_unavailable_not_passed";
        };
        if pointer.is_null() {
            return "capture_window_unavailable_not_passed";
        }
        // SAFETY: Tauri returned this live NSWindow; the main-thread marker was
        // checked, and windowNumber is the public, integer-valued NSWindow API.
        let number: isize =
            unsafe { objc2::msg_send![pointer.cast::<objc2::runtime::AnyObject>(), windowNumber] };
        if number <= 0 {
            return "capture_window_unavailable_not_passed";
        }
        let mut command = Command::new("/usr/sbin/screencapture");
        command.args(["-x", "-l", &number.to_string()]);
        command
    };
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut c = Command::new("/usr/bin/import");
        c.args(["-window", "root"]);
        c
    };
    let Ok(mut child) = command
        .arg(output.join("native-screen.png"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return "capture_unavailable_not_passed";
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    "captured_actual_desktop"
                } else {
                    "capture_unavailable_not_passed"
                }
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return "capture_failed_not_passed";
            }
            Ok(None) => {}
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return "capture_timeout_not_passed";
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[tauri::command]
fn native_smoke_report(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Smoke>,
    result: Value,
) -> Result<(), String> {
    if serde_json::to_vec(&result)
        .map_err(|e| e.to_string())?
        .len()
        > 8192
    {
        state.fail(&app, "oversized harness report");
        return Err("oversized report".into());
    }
    if result["ok"] != true {
        state.fail(&app, format!("WebView phase failed: {result}"));
        return Err("WebView phase failed".into());
    }
    if let Some(checkpoint) = result.get("goalReadonlyCheckpoint") {
        let checked = (|| {
            let checkpoint = checkpoint
                .as_str()
                .filter(|value| ["before", "after"].contains(value))
                .ok_or("invalid readonly Goal checkpoint")?;
            if state.phase.load(Ordering::SeqCst) != 0 || window.label() != "main" {
                return Err("readonly Goal checkpoint outside first window".into());
            }
            let (snapshot, execution_snapshot, graph_snapshot) = {
                let daemon = state.daemon.lock().unwrap();
                (
                    goal_database_snapshot(&daemon.runtime)?,
                    goal_execution_database_snapshot(&daemon.runtime)?,
                    goal_graph_database_snapshot(&daemon.runtime)?,
                )
            };
            let mut report = state.report.lock().unwrap();
            record_webcrypto_checkpoint(&mut report, checkpoint, &result["webCrypto"])?;
            record_goal_checkpoint(
                state.phase.load(Ordering::SeqCst),
                window.label(),
                &mut report,
                checkpoint,
                snapshot,
            )?;
            record_goal_execution_checkpoint(
                state.phase.load(Ordering::SeqCst),
                window.label(),
                &mut report,
                checkpoint,
                execution_snapshot,
            )?;
            record_goal_graph_checkpoint(
                state.phase.load(Ordering::SeqCst),
                window.label(),
                &mut report,
                checkpoint,
                graph_snapshot,
            )?;
            fs::write(
                state.output.join("native-smoke.json"),
                serde_json::to_vec_pretty(&*report)
                    .map_err(|_| "cannot encode readonly Goal evidence")?,
            )
            .map_err(|_| "cannot persist readonly Goal evidence")?;
            Ok::<(), String>(())
        })();
        if let Err(error) = &checked {
            state.fail(&app, error);
        }
        return checked;
    }
    match (state.phase.load(Ordering::SeqCst), window.label()) {
        (0, "main") => {
            let screenshot = capture_screen(&state.output, &window);
            let committed = (|| {
                let mut report = state.report.lock().unwrap();
                let database = &report["goal_readonly_database"];
                if database["read_only"] != true
                    || database["unchanged"] != true
                    || database["before"] != database["after"]
                    || result["goalReadonlySequence"]["before"] != database["before"]["sequence"]
                    || result["goalReadonlySequence"]["after"] != database["after"]["sequence"]
                    || result["projectId"] != report["fixture_project_id"]
                {
                    return Err("missing or inconsistent readonly Goal database evidence".into());
                }
                validate_goal_execution_evidence(&result, &report)?;
                validate_goal_graph_evidence(&result, &report)?;
                validate_webcrypto_evidence(&result, &report, false)?;
                commit_native_report(&state.phase, &mut report, 0, result)?;
                report["screenshot"] = json!(screenshot);
                Ok::<(), String>(())
            })();
            if let Err(error) = committed {
                state.fail(&app, &error);
                return Err(error);
            }
            window.close().map_err(|e| e.to_string())?;
        }
        (2, "native-reopened") => {
            state.stage("reopened_report_received")?;
            let committed = (|| {
                let mut report = state.report.lock().unwrap();
                validate_webcrypto_evidence(&result, &report, true)?;
                commit_native_report(&state.phase, &mut report, 2, result)?;
                fs::write(
                    state.output.join("native-smoke.json"),
                    serde_json::to_vec_pretty(&*report).unwrap(),
                )
                .map_err(|error| error.to_string())
            })();
            if let Err(error) = committed {
                state.fail(&app, &error);
                return Err(error);
            }
            app.exit(0);
        }
        _ => {
            state.fail(&app, "out-of-order native report");
            return Err("unexpected report".into());
        }
    }
    Ok(())
}
const FIRST: &str = r#"
(async () => {
 const invoke = window.__TAURI_INTERNALS__.invoke;
 const require = (value, message) => { if (!value) throw Error(message); };
 const result = value => { require(!value.error, JSON.stringify(value.error)); return value.result; };
 const webCryptoProof = async checkpoint => {
  require(typeof window.TextEncoder==='function' && typeof window.crypto?.subtle?.digest==='function','real WebView WebCrypto/TextEncoder unavailable');
  const input="{\"text\":\"原文证据🔒\",\"inode\":1152921504606846979,\"u64\":9223372036854775809,\"max\":18446744073709551615}\n";
  const bytes=new window.TextEncoder().encode(input);
  const hashed=await window.crypto.subtle.digest('SHA-256',bytes);
  const sha256=Array.from(new Uint8Array(hashed),byte=>byte.toString(16).padStart(2,'0')).join('');
  return {version:1,checkpoint,available:true,source:'window.crypto.subtle.digest',algorithm:'SHA-256',encoding:'UTF-8',input,input_bytes:bytes.byteLength,sha256};
 };
 try {
  const deadline = Date.now() + 10000;
  while (!document.querySelector('#root')?.children.length) {
   require(Date.now() < deadline, 'actual built frontend did not mount');
   await new Promise(r => setTimeout(r, 25));
  }
  const initial = result(await invoke('system_snapshot'));
  require(initial.tasks.length === 0, 'private runtime must start empty');
  const projects=result(await invoke('projects_list'));
  require(Array.isArray(projects)&&projects.length===1&&projects[0].name==='Native readonly Goal project'&&!projects[0].archived,'generated registered readonly project fixture is absent');
  const fixtureProject=projects[0];
  let projectSelect;
  while(!(projectSelect=document.querySelector('.composer-project select'))||![...projectSelect.options].some(option=>option.value===fixtureProject.id)){require(Date.now()<deadline,'real project composer choice did not load');await new Promise(r=>setTimeout(r,25));}
  Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value').set.call(projectSelect,fixtureProject.id);
  projectSelect.dispatchEvent(new Event('change',{bubbles:true}));
  const goal='原生 WebView → IPC persistent task';
  const input=document.querySelector('[data-testid="goal-input"],#goal');
  const form=document.querySelector('[data-testid="goal-composer"],form.create-task');
  require(input && form, 'actual product goal composer is absent');
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(input,goal);
  input.dispatchEvent(new Event('input',{bubbles:true}));
  let button=form.querySelector('button[type="submit"]');
  while(button.disabled){require(Date.now()<deadline,'real goal submit never became enabled');await new Promise(r=>setTimeout(r,25));}
  button.click();
  let task;
  while(!task){
   const snapshot=result(await invoke('system_snapshot'));
   task=snapshot.tasks.find(t=>t.goal===goal);
   require(Date.now()<deadline,'actual UI submit did not create persistent task');
   if(!task)await new Promise(r=>setTimeout(r,25));
  }
  require(task.state === 'draft', 'create falsely marked dispatch');
  require(task.project_id===fixtureProject.id,'actual UI task did not bind the chosen registered fixture project');
  const rendered=()=>[...document.querySelectorAll('[data-task-id]')].some(node=>node.dataset.taskId===task.id)
   || [...document.querySelectorAll('.task-list strong,.inspector h2')].some(node=>node.textContent===goal);
  while(!rendered()){require(Date.now()<deadline,'UI did not render the real created task');await new Promise(r=>setTimeout(r,25));}
  const args={goal:'Native IPC idempotency fixture',idempotencyKey:'native-smoke-create'};
  const auxiliary=result(await invoke('tasks_create',args));
  const retry = result(await invoke('tasks_create',args));
  require(retry.id === auxiliary.id, 'idempotent retry duplicated task');
  const bad = await invoke('tasks_create',{goal:'',idempotencyKey:'native-smoke-invalid'});
  require(bad.error?.code === 'INVALID_REQUEST', 'real invalid input did not fail');
  const events = result(await invoke('events_subscribe',{cursor:0,limit:100}));
  require(events.events.some(e=>e.aggregate_id===task.id), 'real journal event absent');
  const beforeData=result(await invoke('system_snapshot')).sequence;
  const dataPolicies=result(await invoke('data_validation_policies_list',{taskId:task.id}));
  const dataRecords=result(await invoke('data_validations_list',{taskId:task.id}));
  require(Array.isArray(dataPolicies)&&dataPolicies.length===0&&Array.isArray(dataRecords)&&dataRecords.length===0,'fresh task must have no fabricated data approvals or receipts');
  const artifacts=document.querySelector('#tab-artifacts');
  require(artifacts,'actual task Artifacts tab missing');artifacts.click();
  let dataPanel;
  while(!(dataPanel=document.querySelector('[data-testid="data-contract-validation"]'))){require(Date.now()<deadline,'actual data contract panel did not mount');await new Promise(r=>setTimeout(r,25));}
  const folded=dataPanel.querySelector('details');
  require(folded&&!folded.open,'data contract controls must start folded');folded.querySelector('summary').click();
  while(dataPanel.querySelector('form>fieldset')?.disabled!==false){require(Date.now()<deadline,'real native data-list read did not reconcile successfully');await new Promise(r=>setTimeout(r,25));}
  require(!dataPanel.querySelector('[role="alert"]'),'real native data-list read failed');
  const approve=dataPanel.querySelector('button[type="submit"]');
  require(approve&&approve.disabled,'draft without stopped session must not enable native data approval');
  require(result(await invoke('system_snapshot')).sequence===beforeData,'read-only data controls unexpectedly mutated history');
  const beforeCommand=result(await invoke('system_snapshot')).sequence;
  const commandPolicies=result(await invoke('command_validation_policies_list',{taskId:task.id}));
  const commandRecords=result(await invoke('command_validations_list',{taskId:task.id}));
  require(Array.isArray(commandPolicies)&&commandPolicies.length===0&&Array.isArray(commandRecords)&&commandRecords.length===0,'fresh task must have no fabricated command approval or execution');
  let commandPanel;
  while(!(commandPanel=document.querySelector('[data-testid="command-validation"]'))){require(Date.now()<deadline,'actual command validation panel did not mount');await new Promise(r=>setTimeout(r,25));}
  const commandFolded=commandPanel.querySelector('details');
  require(commandFolded&&!commandFolded.open,'command validation controls must start folded');commandFolded.querySelector('summary').click();
  while(commandPanel.querySelector('form>fieldset')?.disabled!==false){require(Date.now()<deadline,'real native command-list read did not reconcile successfully');await new Promise(r=>setTimeout(r,25));}
  require(!commandPanel.querySelector('[role="alert"]'),'real native command-list read failed');
  const commandApprove=commandPanel.querySelector('button[type="submit"]');
  require(commandApprove&&commandApprove.disabled,'draft without stopped session must not enable native command approval');
  require(result(await invoke('system_snapshot')).sequence===beforeCommand,'read-only command controls unexpectedly mutated history');
  const beforeGoal=result(await invoke('system_snapshot')).sequence;
  require(typeof task.project_id==='string'&&task.project_id.length>0,'real fixture task lacks its Goal project reference');
  await invoke('native_smoke_report',{result:{ok:true,goalReadonlyCheckpoint:'before',webCrypto:await webCryptoProof('before')}});
  const goalGrants=result(await invoke('grants_list',{projectId:task.project_id}));
  const goalScopes=result(await invoke('goal_scopes_list',{taskId:task.id}));
  require(Array.isArray(goalGrants)&&goalGrants.length===0&&Array.isArray(goalScopes)&&goalScopes.length===0,'fresh task must have no fabricated root grant or Goal scope');
  const detailsTab=document.querySelector('#tab-details');require(detailsTab,'actual task Details tab missing');detailsTab.click();
  let goalPanel;
  while(!(goalPanel=document.querySelector('[data-testid="goal-planning"]'))){require(Date.now()<deadline,'actual Goal panel did not mount');await new Promise(r=>setTimeout(r,25));}
  const goalFolded=goalPanel.querySelector(':scope > details');
  require(goalFolded&&!goalFolded.open,'Goal controls must start folded');goalFolded.querySelector('summary').click();
  while(goalPanel.querySelector('form>fieldset')?.disabled!==false||goalPanel.querySelector('[aria-busy]')?.getAttribute('aria-busy')!=='false'){require(Date.now()<deadline,'real native Goal lists did not reconcile successfully');await new Promise(r=>setTimeout(r,25));}
  require(!goalPanel.querySelector('[role="alert"]'),'real native Goal scope-list read failed');
  require(/尚无独立原生批准的 Goal 范围|No native-approved Goal scope yet/.test(goalPanel.textContent),'actual empty Goal-scope state absent');
  const goalApprove=goalPanel.querySelector('button[type="submit"]');
  require(goalApprove&&goalApprove.matches(':disabled'),'no eligible root grant must not enable native Goal approval');
  const goalActionNames=['保存初始计划提议','保存修订计划提议','读取就绪预览','Store initial plan proposal','Store revised plan proposal','Read readiness preview'];
  require([...goalPanel.querySelectorAll('button')].filter(button=>goalActionNames.includes(button.textContent.trim())).every(button=>button.matches(':disabled')),'empty scope must not enable plan submission or readiness preview');
  require(!goalPanel.querySelector('textarea[spellcheck="false"]')||goalPanel.querySelector('textarea[spellcheck="false"]').matches(':disabled'),'empty scope must not enable plan JSON editing');
  require(!goalPanel.querySelector('[data-testid="goal-execution"]'),'scope-free Goal controls must not mount or activate execution UI');
  const beforeGoalExecution=result(await invoke('system_snapshot')).sequence;
  require(beforeGoalExecution===beforeGoal,'Goal execution reads must start inside the original readonly interval');
  const executionRuns=result(await invoke('goal_runs_list',{taskId:task.id}));
  require(Array.isArray(executionRuns)&&executionRuns.length===0,'fresh task must have no Goal execution runs');
  const absentRunId='native-readonly-missing-goal-run';
  const missingExecutionRun=await invoke('goal_runs_get',{runId:absentRunId});
  require(missingExecutionRun?.error?.code==='NOT_FOUND'&&missingExecutionRun.error.side_effect_state==='not_started'&&!Object.prototype.hasOwnProperty.call(missingExecutionRun,'result'),'a fixed missing run must return the actual Core denial without a fabricated result');
  const afterGoalExecution=result(await invoke('system_snapshot')).sequence;
  require(afterGoalExecution===beforeGoalExecution,'readonly execution list/get unexpectedly mutated history');
  const beforeGoalGraph=result(await invoke('system_snapshot')).sequence;
  require(beforeGoalGraph===beforeGoal&&beforeGoalGraph===beforeGoalExecution,'Graph reads must share the original Goal/execution readonly interval');
  const graphRuns=result(await invoke('goal_graph_runs_list',{taskId:task.id}));
  require(Array.isArray(graphRuns)&&graphRuns.length===0,'fresh task must have no Goal graph runs');
  const absentGraphRunId='native-readonly-missing-goal-graph-run';
  const missingGraphRun=await invoke('goal_graph_runs_get',{runId:absentGraphRunId});
  require(missingGraphRun?.error?.code==='NOT_FOUND'&&missingGraphRun.error.side_effect_state==='not_started'&&!Object.prototype.hasOwnProperty.call(missingGraphRun,'result'),'fixed missing Graph run must return the actual Core denial without a result');
  const afterGoalGraph=result(await invoke('system_snapshot')).sequence;
  require(afterGoalGraph===beforeGoalGraph,'readonly Graph list/get unexpectedly mutated history');
  const afterGoal=result(await invoke('system_snapshot')).sequence;
  require(afterGoal===beforeGoal,'read-only Goal controls unexpectedly mutated history');
  await invoke('native_smoke_report',{result:{ok:true,goalReadonlyCheckpoint:'after',webCrypto:await webCryptoProof('after')}});
  document.querySelector('#tab-conversation').click();
  while(document.querySelector('#tab-conversation')?.getAttribute('aria-selected')!=='true'){require(Date.now()<deadline,'return to conversation did not settle');await new Promise(r=>setTimeout(r,25));}
  await invoke('native_smoke_report',{result:{ok:true,webCrypto:await webCryptoProof('first'),domMounted:true,realComposerSubmitted:true,realTaskRendered:true,taskId:task.id,projectId:task.project_id,idempotent:true,invalidInputRejected:true,journal:true,dataContractLists:true,dataContractEmptyControls:true,readonlyDataNoMutation:true,commandContractLists:true,commandContractEmptyControls:true,readonlyCommandNoMutation:true,goalScopeLists:true,goalScopeEmptyControls:true,readonlyGoalNoMutation:true,goalReadonlySequence:{before:beforeGoal,after:afterGoal},goalExecutionLists:true,goalExecutionMissingRunDenied:true,readonlyGoalExecutionNoMutation:true,goalExecutionReadonlySequence:{before:beforeGoalExecution,after:afterGoalExecution},goalExecutionMissingRun:{run_id:absentRunId,code:missingExecutionRun.error.code,side_effect_state:missingExecutionRun.error.side_effect_state},goalGraphLists:true,goalGraphMissingRunDenied:true,readonlyGoalGraphNoMutation:true,goalGraphReadonlySequence:{before:beforeGoalGraph,after:afterGoalGraph},goalGraphMissingRun:{run_id:absentGraphRunId,code:missingGraphRun.error.code,side_effect_state:missingGraphRun.error.side_effect_state}}});
 } catch (error) { await invoke('native_smoke_report',{result:{ok:false,error:String(error)}}); }
})();
"#;
const SECOND: &str = r#"
(async () => {
 const invoke = window.__TAURI_INTERNALS__.invoke;
 const require = (value,message) => { if(!value) throw Error(message); };
 const result = value => { require(!value.error, JSON.stringify(value.error)); return value.result; };
 const webCryptoProof = async checkpoint => {
  require(typeof window.TextEncoder==='function' && typeof window.crypto?.subtle?.digest==='function','real WebView WebCrypto/TextEncoder unavailable');
  const input="{\"text\":\"原文证据🔒\",\"inode\":1152921504606846979,\"u64\":9223372036854775809,\"max\":18446744073709551615}\n";
  const bytes=new window.TextEncoder().encode(input);
  const hashed=await window.crypto.subtle.digest('SHA-256',bytes);
  const sha256=Array.from(new Uint8Array(hashed),byte=>byte.toString(16).padStart(2,'0')).join('');
  return {version:1,checkpoint,available:true,source:'window.crypto.subtle.digest',algorithm:'SHA-256',encoding:'UTF-8',input,input_bytes:bytes.byteLength,sha256};
 };
 try {
  const deadline=Date.now()+10000;
  while (!document.querySelector('#root')?.children.length) { require(Date.now()<deadline,'reopened frontend did not mount'); await new Promise(r=>setTimeout(r,25)); }
  const snapshot=result(await invoke('system_snapshot'));
  require(snapshot.tasks.length===2,'restart lost/duplicated persisted tasks');
  const task=snapshot.tasks.find(t=>t.goal==='原生 WebView → IPC persistent task'); require(task,'UI-created task absent'); require(task.state==='draft','restart changed task state');
  const cancelled=result(await invoke('tasks_cancel',{taskId:task.id,expectedRevision:task.revision,idempotencyKey:'native-smoke-cancel'}));
  require(cancelled.state==='cancelled','draft cancellation failed');
  const events=result(await invoke('events_subscribe',{cursor:0,limit:100}));
  require(events.events.at(-1)?.payload.stop_evidence==='no_attempt_dispatched','stop evidence absent');
  await invoke('native_smoke_report',{result:{ok:true,webCrypto:await webCryptoProof('reopened'),domMounted:true,taskId:task.id,projectId:task.project_id,restartPersisted:true,draftCancelled:true,stopEvidence:true}});
 } catch(error) { await invoke('native_smoke_report',{result:{ok:false,error:String(error)}}); }
})();
"#;
fn prepare_reopen(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let state = app.state::<Smoke>();
    let mut daemon = state.daemon.lock().unwrap();
    if daemon
        .child
        .try_wait()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Err("daemon stopped when native window closed".into());
    }
    let snapshot = daemon.ready()?;
    if snapshot["result"]["tasks"].as_array().map(Vec::len) != Some(2) {
        return Err("window close lost persisted task".into());
    }
    daemon.reject_second_writer()?;
    state.report.lock().unwrap()["window_close_daemon_survived"] = json!(true);
    state.report.lock().unwrap()["second_writer_rejected"] = json!(true);
    state.stage("closed_window_daemon_verified")?;
    daemon.restart()?;
    state.report.lock().unwrap()["owned_daemon_pid"] = json!(daemon.child.id());
    state.report.lock().unwrap()["owned_daemon_restarted"] = json!(true);
    state.stage("owned_daemon_restart_ready")?;
    // Release this mutex before the UI thread creates another WebView.
    Ok(daemon.runtime.join("webview-cache"))
}
fn claim_reopen(phase: &AtomicU8) -> Result<(), String> {
    phase
        .compare_exchange(1, 2, Ordering::SeqCst, Ordering::SeqCst)
        .map(|_| ())
        .map_err(|_| "duplicate or out-of-order reopen".into())
}
fn page_script(phase: u8, label: &str, injected: &AtomicU8) -> Option<&'static str> {
    let (bit, script) = match (phase, label) {
        (0, "main") => (1, FIRST),
        (2, "native-reopened") => (2, SECOND),
        _ => return None,
    };
    (injected.fetch_or(bit, Ordering::SeqCst) & bit == 0).then_some(script)
}
fn schedule_reopen(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<Smoke>();
    claim_reopen(&state.phase)?;
    state.stage("reopen_worker_requested")?;
    let worker = app.clone();
    let worker_handle = std::thread::Builder::new()
        .name("native-reopen".into())
        .spawn(move || {
            let cache = match prepare_reopen(&worker) {
                Ok(cache) => cache,
                Err(error) => {
                    worker.state::<Smoke>().fail(&worker, error);
                    return;
                }
            };
            if worker.state::<Smoke>().phase.load(Ordering::SeqCst) != 2 {
                return;
            }
            if let Err(error) = worker
                .state::<Smoke>()
                .stage("reopen_main_thread_dispatch_requested")
            {
                worker.state::<Smoke>().fail(&worker, error);
                return;
            }
            let main = worker.clone();
            // This call is deliberately made by the worker, so Tauri queues it
            // after the current Destroyed callback and old WebView teardown return.
            if let Err(error) = worker.run_on_main_thread(move || {
                let state = main.state::<Smoke>();
                if state.phase.load(Ordering::SeqCst) != 2 {
                    return;
                }
                if let Err(error) = state.stage("reopened_window_build_started") {
                    state.fail(&main, error);
                    return;
                }
                let result = WebviewWindowBuilder::new(
                    &main,
                    "native-reopened",
                    WebviewUrl::App("index.html".into()),
                )
                .title("AgentBaton — Native integration")
                .inner_size(1180., 800.)
                .incognito(true)
                .data_directory(cache)
                .build();
                if let Err(error) = result {
                    state.fail(&main, error);
                    return;
                }
                if let Err(error) = state.stage("reopened_window_build_completed") {
                    state.fail(&main, error);
                }
            }) {
                worker.state::<Smoke>().fail(&worker, error);
            }
        })
        .map_err(|error| error.to_string())?;
    *state.reopen_worker.lock().unwrap() = Some(worker_handle);
    Ok(())
}
fn private_fixture_runtime() -> std::io::Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix("baton-native-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or("provide an existing evidence directory")?;
    let output = fs::canonicalize(output)?;
    if !output.is_dir() {
        return Err("evidence directory is not a directory".into());
    }
    let temporary = private_fixture_runtime()?;
    let runtime = temporary.path().canonicalize()?;
    // Keep toolkit caches/settings inside this test's disposable namespace.
    // HOME and native Provider configuration/authentication are never changed.
    for (variable, name) in [
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_RUNTIME_DIR", "xdg-runtime"),
    ] {
        let path = runtime.join(name);
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        std::env::set_var(variable, path);
    }
    // Only fixed sibling daemon is allowed; no renderer-selected executable.
    // Simulate only selecting a generated directory through the real registry;
    // no OS file dialog, workspace, grant, Goal approval or model is fabricated.
    let fixture_project_id = prepare_readonly_goal_project(&runtime)?;
    let daemon_path = std::env::current_exe()?
        .parent()
        .ok_or("no binary parent")?
        .join("batond");
    let daemon = OwnedDaemon::start(daemon_path, runtime.clone())?;
    let report = json!({"status":"running","stages":[],"private_fixture_runtime":runtime,"fixture_project_id":fixture_project_id,"project_selection":"generated directory registered through real Core; OS file dialog not run","owned_daemon_pid":daemon.child.id(),"dimension":"real_native_webview_production_invoke_private_daemon","os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"commit":std::env::var("AGENTBATON_SMOKE_COMMIT").unwrap_or_else(|_|"unknown".into()),"native_dialog":"not_run_manual_acceptance_required","provider_models":"not_run_not_authorized"});
    fs::write(
        output.join("native-smoke.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    let daemon = Arc::new(Mutex::new(daemon));
    let _cleanup_guard = CleanupGuard(daemon.clone());
    let report = Arc::new(Mutex::new(report));
    let phase = Arc::new(AtomicU8::new(0));
    let failure_reason = Arc::new(Mutex::new(None));
    let reopen_worker = Arc::new(Mutex::new(None));
    let smoke = Smoke {
        phase: phase.clone(),
        injected: AtomicU8::new(0),
        daemon: daemon.clone(),
        report: report.clone(),
        failure_reason: failure_reason.clone(),
        reopen_worker: reopen_worker.clone(),
        output: output.clone(),
    };
    let mut context = tauri::generate_context!();
    context.config_mut().app.app_directories_override = Some(
        tauri::utils::config::AppDirectoriesOverride::Root(runtime.join("tauri-state")),
    );
    for window in &mut context.config_mut().app.windows {
        window.incognito = true;
        window.data_directory = Some("native-smoke-webview".into());
    }
    let app = agentbaton_desktop::desktop_builder(runtime)
        .manage(smoke)
        .invoke_handler(|invoke| {
            if invoke.message.command() == "native_smoke_report" {
                {
                    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
                        tauri::generate_handler![native_smoke_report];
                    handler(invoke)
                }
            } else {
                agentbaton_desktop::dispatch_production(invoke)
            }
        })
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let app = webview.app_handle();
                let state = app.state::<Smoke>();
                if let Some(script) = page_script(
                    state.phase.load(Ordering::SeqCst),
                    webview.label(),
                    &state.injected,
                ) {
                    if let Err(error) = state.stage(if webview.label() == "main" {
                        "first_page_finished_script_dispatched"
                    } else {
                        "reopened_page_finished_script_dispatched"
                    }) {
                        state.fail(app, error);
                        return;
                    }
                    if let Err(error) = webview.eval(script) {
                        state.fail(app, error);
                    }
                }
            }
        })
        .build(context)?;
    let handle = app.handle().clone();
    let watchdog = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if terminal_phase(handle.state::<Smoke>().phase.load(Ordering::SeqCst)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let state = handle.state::<Smoke>();
        if claim_timeout(
            &state.phase,
            &state.failure_reason,
            "native WebView integration exceeded60s deadline",
        ) {
            state.fail(&handle, "native WebView integration exceeded60s deadline");
        }
    });
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.run_return(|app, event| match event {
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::Destroyed,
                ..
            } if label == "main" && app.state::<Smoke>().phase.load(Ordering::SeqCst) == 1 => {
                if let Err(error) = schedule_reopen(app) {
                    app.state::<Smoke>().fail(app, error);
                }
            }
            RunEvent::ExitRequested {
                api, code: None, ..
            } if !terminal_phase(app.state::<Smoke>().phase.load(Ordering::SeqCst)) => {
                api.prevent_exit()
            }
            _ => {}
        })
    }));
    let code = outcome.unwrap_or(1);
    if code != 0 || phase.load(Ordering::SeqCst) != 3 {
        record_failure(
            &phase,
            &failure_reason,
            &report,
            "native event loop exited before successful completion",
        );
    }
    // No asynchronous failure publisher may outlive final acceptance/report IO.
    if watchdog.join().is_err() {
        record_failure(&phase, &failure_reason, &report, "native watchdog panicked");
    }
    let worker = reopen_worker.lock().unwrap().take();
    if let Some(worker) = worker {
        if worker.join().is_err() {
            record_failure(
                &phase,
                &failure_reason,
                &report,
                "native reopen worker panicked",
            );
        }
    }
    let mut daemon = daemon.lock().unwrap_or_else(|error| error.into_inner());
    if daemon.child.try_wait()?.is_none() {
        daemon.child.kill()?;
    }
    daemon.child.wait()?;
    let mut report = report.lock().unwrap_or_else(|error| error.into_inner());
    report["owned_daemon_cleanup"] = json!(true);
    let accepted = finalize_native_report(code, &phase, &failure_reason, &mut report);
    fs::write(
        output.join("native-smoke.json"),
        serde_json::to_vec_pretty(&*report)?,
    )?;
    drop(report);
    drop(daemon);
    drop(temporary);
    if !accepted {
        return Err("native smoke failed; inspect native-smoke.json".into());
    }
    Ok(())
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn generated_registered_project_setup_creates_no_goal_or_attempt_authority() {
        let directory = private_fixture_runtime().unwrap();
        // Match main(): macOS temporary roots may include a system symlink
        // alias, while the production daemon correctly requires the real path.
        let runtime = directory.path().canonicalize().unwrap();
        let id = prepare_readonly_goal_project(&runtime).unwrap();
        let store = agentbaton_core::storage::Store::open(&runtime.join("baton.sqlite3")).unwrap();
        assert!(store.list().unwrap().is_empty());
        let project = store.get_project(&id).unwrap();
        assert_eq!(project.name, "Native readonly Goal project");
        assert_eq!(
            serde_json::to_value(&project.identity.kind).unwrap(),
            json!({"kind":"directory"})
        );
        assert!(store.list_workspaces(&id).unwrap().is_empty());
        assert!(store.list_grants(&id).unwrap().is_empty());
        assert!(valid_goal_counts(
            &goal_database_snapshot(&runtime).unwrap()
        ));
        let mode = fs::metadata(runtime.join("baton.sqlite3"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        drop(store);
        let actual_daemon = ipc::Daemon::bind(&runtime);
        assert!(
            actual_daemon.is_ok(),
            "helper-created DB mode={mode:o}; actual authority rejection={:?}",
            actual_daemon.as_ref().err()
        );
        assert_eq!(mode, 0o600);
        drop(actual_daemon);
    }

    #[test]
    fn owned_runtime_alias_is_rejected_but_its_canonical_private_database_binds() {
        // The real runtime has exactly main()'s short owned layout. Put only
        // the denied alias in a separate namespace: nesting the actual runtime
        // adds unnecessary bytes to the platform's bounded Unix socket path.
        let owned_runtime = private_fixture_runtime().unwrap();
        let runtime = owned_runtime.path().canonicalize().unwrap();
        let alias_directory = private_fixture_runtime().unwrap();
        let alias_parent = alias_directory
            .path()
            .canonicalize()
            .unwrap()
            .join("alias-parent");
        let real_parent = runtime.parent().unwrap();
        std::os::unix::fs::symlink(real_parent, &alias_parent).unwrap();
        let alias_runtime = alias_parent.join(runtime.file_name().unwrap());
        assert_ne!(alias_runtime, runtime);
        assert_eq!(alias_runtime.canonicalize().unwrap(), runtime);
        prepare_readonly_goal_project(&runtime).unwrap();
        let database = runtime.join("baton.sqlite3");
        let before = fs::read(&database).unwrap();
        assert_eq!(
            fs::metadata(&database).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let denied = ipc::Daemon::bind(&alias_runtime)
            .err()
            .expect("a symlink alias must never gain daemon authority");
        assert_eq!(denied.code, "PERMISSION_DENIED");
        assert!(denied.message.contains("symbolic links"));
        assert_eq!(fs::read(&database).unwrap(), before);
        let socket_path_bytes = runtime
            .join("batond.sock")
            .as_os_str()
            .as_encoded_bytes()
            .len();
        let daemon = ipc::Daemon::bind(&runtime).unwrap_or_else(|error| {
            panic!("the same owned canonical runtime must bind; socket_path_bytes={socket_path_bytes}; {error:?}")
        });
        assert!(valid_goal_counts(
            &goal_database_snapshot(&runtime).unwrap()
        ));
        drop(daemon);
        assert_eq!(fs::read_link(&alias_parent).unwrap(), real_parent);
        assert_eq!(
            fs::metadata(database).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn project_fixture_refuses_existing_database_without_chmod_or_byte_changes() {
        let directory = private_fixture_runtime().unwrap();
        let database = directory.path().join("baton.sqlite3");
        // Deliberately use the ambient umask for this controlled preexisting
        // fixture: under CI 022 it is 0644. Subject code must leave it untouched.
        drop(
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o644)
                .open(&database)
                .unwrap(),
        );
        let mut store = agentbaton_core::storage::Store::open(&database).unwrap();
        store
            .create(
                "local",
                "Preserve actual existing fixture history",
                "existing",
            )
            .unwrap();
        drop(store);
        let bytes = fs::read(&database).unwrap();
        let mode = fs::metadata(&database).unwrap().permissions().mode() & 0o777;
        assert!(prepare_readonly_goal_project(directory.path()).is_err());
        assert_eq!(fs::read(&database).unwrap(), bytes);
        assert_eq!(
            fs::metadata(&database).unwrap().permissions().mode() & 0o777,
            mode
        );
    }

    #[test]
    fn actual_readonly_goal_counts_observe_sequence_changes_without_writes() {
        let directory = private_fixture_runtime().unwrap();
        drop(
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(directory.path().join("baton.sqlite3"))
                .unwrap(),
        );
        let mut store =
            agentbaton_core::storage::Store::open(&directory.path().join("baton.sqlite3")).unwrap();
        let before = goal_database_snapshot(directory.path()).unwrap();
        assert_eq!(
            before,
            json!({"scope_count":0,"plan_count":0,"attempt_count":0,"sequence":0})
        );
        assert_eq!(store.sequence().unwrap(), 0);
        let mut report = json!({"status":"running"});
        record_goal_checkpoint(0, "main", &mut report, "before", before.clone()).unwrap();
        record_goal_checkpoint(
            0,
            "main",
            &mut report,
            "after",
            goal_database_snapshot(directory.path()).unwrap(),
        )
        .unwrap();
        assert_eq!(report["goal_readonly_database"]["unchanged"], true);
        assert_eq!(store.sequence().unwrap(), 0);
        store
            .create("local", "Generated non-Goal task mutation", "fixture")
            .unwrap();
        let changed = goal_database_snapshot(directory.path()).unwrap();
        assert_eq!(changed["sequence"], store.sequence().unwrap());
        assert_ne!(changed, before);
        let mut report = json!({"status":"running"});
        record_goal_checkpoint(0, "main", &mut report, "before", before).unwrap();
        assert!(record_goal_checkpoint(0, "main", &mut report, "after", changed).is_err());
        assert!(report["goal_readonly_database"].get("after").is_none());
    }

    #[test]
    fn goal_checkpoints_reject_wrong_window_failure_replay_and_noninteger_counts() {
        let counts = json!({"scope_count":0,"plan_count":0,"attempt_count":0,"sequence":2});
        for (phase, label) in [(1, "main"), (2, "native-reopened"), (4, "main")] {
            let mut report = json!({"status":"running"});
            let before = report.clone();
            assert!(
                record_goal_checkpoint(phase, label, &mut report, "before", counts.clone())
                    .is_err()
            );
            assert_eq!(report, before);
        }
        for invalid in [json!(true), json!("0"), json!(-1), json!(0.0)] {
            let mut malformed = counts.clone();
            malformed["scope_count"] = invalid;
            assert!(!valid_goal_counts(&malformed));
        }
        let mut report = json!({"status":"running","error":"original failure"});
        assert!(record_goal_checkpoint(0, "main", &mut report, "before", counts.clone()).is_err());
        let mut report = json!({"status":"running"});
        record_goal_checkpoint(0, "main", &mut report, "before", counts.clone()).unwrap();
        assert!(record_goal_checkpoint(0, "main", &mut report, "before", counts.clone()).is_err());
        record_goal_checkpoint(0, "main", &mut report, "after", counts.clone()).unwrap();
        assert!(record_goal_checkpoint(0, "main", &mut report, "after", counts).is_err());
    }

    #[test]
    fn readonly_query_capture_and_owned_child_timeout_are_real_bounds() {
        let mut large = Command::new("/bin/sh");
        large.args(["-c", "printf '%05000d' 0"]);
        assert!(bounded_goal_query(large).is_err());
        let directory = private_fixture_runtime().unwrap();
        let pid_file = directory.path().join("owned-query.pid");
        let mut slow = Command::new("/bin/sh");
        slow.args([
            "-c",
            "printf '%s' \"$$\" > \"$1\"; exec /bin/sleep 30",
            "fixture",
        ])
        .arg(&pid_file);
        assert!(bounded_goal_query(slow)
            .unwrap_err()
            .contains("exceeded its bound"));
        let pid = fs::read_to_string(pid_file).unwrap();
        assert!(pid.bytes().all(|c| c.is_ascii_digit()));
        let observed = Command::new("/bin/ps")
            .args(["-p", &pid, "-o", "stat="])
            .output()
            .unwrap();
        assert!(
            observed.stdout.is_empty(),
            "owned query child was not reaped"
        );
    }

    #[test]
    fn fixture_runtime_allows_the_real_private_daemon_authority() {
        let directory = private_fixture_runtime().unwrap();
        let canonical = directory.path().canonicalize().unwrap();
        // This is actual core admission and storage initialization, not a
        // simulated GUI or a relaxed production permission check.
        let daemon = ipc::Daemon::bind(&canonical).unwrap();
        assert_eq!(
            fs::metadata(&canonical).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(canonical.join("batond.sock").exists());
        drop(daemon);
        assert!(!canonical.join("batond.sock").exists());
    }

    #[test]
    fn destroyed_window_can_schedule_exactly_one_reopen_in_the_valid_phase() {
        let phase = AtomicU8::new(0);
        assert!(claim_reopen(&phase).is_err());
        assert_eq!(phase.load(Ordering::SeqCst), 0);
        phase.store(1, Ordering::SeqCst);
        claim_reopen(&phase).unwrap();
        assert_eq!(phase.load(Ordering::SeqCst), 2);
        assert!(claim_reopen(&phase).is_err());
        phase.store(4, Ordering::SeqCst);
        assert!(claim_reopen(&phase).is_err());
    }

    #[test]
    fn each_real_window_phase_gets_its_script_once_and_wrong_windows_get_none() {
        let injected = AtomicU8::new(0);
        assert!(page_script(2, "main", &injected).is_none());
        assert!(page_script(0, "native-reopened", &injected).is_none());
        assert_eq!(page_script(0, "main", &injected), Some(FIRST));
        assert!(page_script(0, "main", &injected).is_none());
        assert_eq!(page_script(2, "native-reopened", &injected), Some(SECOND));
        assert!(page_script(2, "native-reopened", &injected).is_none());
        assert!(page_script(3, "native-reopened", &AtomicU8::new(0)).is_none());
    }

    #[test]
    fn late_success_cannot_erase_failure_published_while_the_report_lock_is_held() {
        for expected in [0, 2] {
            let phase = Arc::new(AtomicU8::new(expected));
            let reason = Arc::new(Mutex::new(None));
            let report = Arc::new(Mutex::new(
                json!({"status":"running","first_webview":{"taskId":"original"}}),
            ));
            let mut held_report = report.lock().unwrap();
            let failed_phase = phase.clone();
            let failed_reason = reason.clone();
            let failed_report = report.clone();
            let failure = std::thread::spawn(move || {
                record_failure(
                    &failed_phase,
                    &failed_reason,
                    &failed_report,
                    "original watchdog failure",
                );
            });
            let deadline = Instant::now() + Duration::from_secs(2);
            while phase.load(Ordering::SeqCst) != 4 {
                assert!(
                    Instant::now() < deadline,
                    "failure publication did not arrive"
                );
                std::thread::yield_now();
            }
            let before = held_report.clone();
            assert!(commit_native_report(
                &phase,
                &mut held_report,
                expected,
                json!({"ok":true,"taskId":"original"})
            )
            .is_err());
            assert_eq!(
                *held_report, before,
                "late successful report mutated evidence"
            );
            drop(held_report);
            // This may beat the first failure to the report mutex. Its generic
            // rejection must still retain the already reserved original reason.
            record_failure(&phase, &reason, &report, "late report rejected");
            failure.join().unwrap();
            let mut report = report.lock().unwrap();
            report["owned_daemon_cleanup"] = json!(true);
            assert_eq!(phase.load(Ordering::SeqCst), 4);
            assert_eq!(report["error"], "original watchdog failure");
            assert_eq!(report["status"], "failed");
            assert!(report.get("reopened_webview").is_none());
            assert!(!finalize_native_report(0, &phase, &reason, &mut report));
            assert_eq!(report["status"], "failed");
        }
    }

    #[test]
    fn completed_report_rejects_a_stale_timeout_and_final_acceptance_requires_no_failure() {
        let phase = AtomicU8::new(0);
        let reason = Mutex::new(None);
        let mut report = json!({"status":"running"});
        commit_native_report(
            &phase,
            &mut report,
            0,
            json!({"ok":true,"taskId":"original"}),
        )
        .unwrap();
        claim_reopen(&phase).unwrap();
        assert!(
            commit_native_report(&phase, &mut report, 2, json!({"ok":true,"taskId":"wrong"}))
                .is_err()
        );
        assert_eq!(phase.load(Ordering::SeqCst), 2);
        commit_native_report(
            &phase,
            &mut report,
            2,
            json!({"ok":true,"taskId":"original"}),
        )
        .unwrap();
        assert!(!claim_timeout(&phase, &reason, "stale timeout"));
        assert!(reason.lock().unwrap().is_none());
        report["owned_daemon_cleanup"] = json!(true);
        let clean = report.clone();
        assert!(finalize_native_report(0, &phase, &reason, &mut report));
        let mut errored = clean.clone();
        errored["error"] = json!("recorded failure");
        assert!(!finalize_native_report(0, &phase, &reason, &mut errored));
        let mut failed = clean.clone();
        failed["status"] = json!("failed");
        assert!(!finalize_native_report(0, &phase, &reason, &mut failed));
        *reason.lock().unwrap() = Some("published failure not copied to report yet".into());
        let mut unpublished = clean;
        assert!(!finalize_native_report(
            0,
            &phase,
            &reason,
            &mut unpublished
        ));
    }

    #[test]
    fn only_completed_or_failed_states_stop_the_watchdog() {
        for phase in [0, 1, 2, 5, 255] {
            assert!(!terminal_phase(phase));
            let phase = AtomicU8::new(phase);
            let reason = Mutex::new(None);
            assert!(claim_timeout(&phase, &reason, "deadline"));
            assert_eq!(phase.load(Ordering::SeqCst), 4);
            assert_eq!(reason.lock().unwrap().as_deref(), Some("deadline"));
        }
        assert!(terminal_phase(3));
        assert!(terminal_phase(4));
    }
    fn fresh_execution_fixture() -> (tempfile::TempDir, PathBuf, agentbaton_core::storage::Store) {
        let directory = private_fixture_runtime().unwrap();
        let runtime = directory.path().canonicalize().unwrap();
        drop(
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(runtime.join("baton.sqlite3"))
                .unwrap(),
        );
        let store = agentbaton_core::storage::Store::open(&runtime.join("baton.sqlite3")).unwrap();
        (directory, runtime, store)
    }
    fn readonly_report_fixture(sequence: u64) -> Value {
        let goal = json!({"scope_count":0,"plan_count":0,"attempt_count":0,"sequence":sequence});
        json!({"status":"running","goal_readonly_database":{"read_only":true,"unchanged":true,"before":goal,"after":goal}})
    }
    fn execution_counts_fixture(sequence: u64) -> Value {
        json!({"schema_version":12,"run_count":0,"node_count":0,"reservation_count":0,"sequence":sequence})
    }
    fn execution_result_fixture(sequence: u64) -> Value {
        json!({"goalExecutionLists":true,"goalExecutionMissingRunDenied":true,"readonlyGoalExecutionNoMutation":true,"goalReadonlySequence":{"before":sequence,"after":sequence},"goalExecutionReadonlySequence":{"before":sequence,"after":sequence},"goalExecutionMissingRun":{"run_id":MISSING_GOAL_RUN_ID,"code":"NOT_FOUND","side_effect_state":"not_started"}})
    }
    #[test]
    fn execution_readonly_actual_schema_counts_and_journal_observations_never_write() {
        let (_directory, runtime, mut store) = fresh_execution_fixture();
        let before = goal_execution_database_snapshot(&runtime).unwrap();
        assert_eq!(before, execution_counts_fixture(0));
        assert_eq!(store.sequence().unwrap(), 0);
        let mut report = readonly_report_fixture(0);
        record_goal_execution_checkpoint(0, "main", &mut report, "before", before.clone()).unwrap();
        record_goal_execution_checkpoint(
            0,
            "main",
            &mut report,
            "after",
            goal_execution_database_snapshot(&runtime).unwrap(),
        )
        .unwrap();
        validate_goal_execution_evidence(&execution_result_fixture(0), &report).unwrap();
        assert_eq!(store.sequence().unwrap(), 0);
        store
            .create(
                "local",
                "Generated sequence mutation, no execution grant",
                "readonly-exec-sequence",
            )
            .unwrap();
        let changed = goal_execution_database_snapshot(&runtime).unwrap();
        assert_eq!(changed["sequence"], store.sequence().unwrap());
        assert_eq!(changed["run_count"], 0);
        assert_ne!(changed, before);
        assert!(record_goal_execution_checkpoint(
            0,
            "main",
            &mut readonly_report_fixture(0),
            "before",
            changed
        )
        .is_err());
    }
    #[test]
    fn execution_readonly_schema_and_each_actual_table_drift_fail_closed() {
        let controls=[
            ("schema_version","PRAGMA user_version=10",10),
            ("schema_version","PRAGMA user_version=11",11),
            ("schema_version","PRAGMA user_version=13",13),
            ("run_count","INSERT INTO goal_runs VALUES('count-probe','scope','project','task',1,'queued','{}')",1),
            ("node_count","INSERT INTO goal_run_nodes VALUES('count-probe',1,'node',0,'not_started',NULL,NULL,'{}')",1),
            ("reservation_count","INSERT INTO goal_run_reservations VALUES('attempt','count-probe','node',NULL,'{}')",1),
        ];
        for (field, sql, expected) in controls {
            let (_directory, runtime, store) = fresh_execution_fixture();
            assert!(goal_execution_database_snapshot(&runtime).is_ok());
            // Deliberately malformed generated metadata, not a Core-approved
            // Goal, proves the independent query observes every actual table.
            let status=Command::new("python3").args(["-I","-S","-c","import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute(sys.argv[2]);db.commit();db.close()"]).arg(runtime.join("baton.sqlite3")).arg(sql).status().unwrap();
            assert!(status.success());
            assert!(goal_execution_database_snapshot(&runtime).is_err());
            let mut command = Command::new("python3");
            command
                .args(["-I", "-S", "-c", GOAL_EXECUTION_READONLY_SQL])
                .arg(runtime.join("baton.sqlite3"));
            let actual: Value =
                serde_json::from_slice(&bounded_goal_query(command).unwrap()).unwrap();
            assert_eq!(actual[field], expected);
            assert_eq!(actual["sequence"], 0);
            drop(store);
        }
    }
    #[test]
    fn execution_checkpoints_reject_wrong_phase_replay_changes_missing_and_fake_values() {
        let counts = execution_counts_fixture(2);
        for (phase, label) in [(1, "main"), (2, "native-reopened"), (0, "other")] {
            let mut report = readonly_report_fixture(2);
            let original = report.clone();
            assert!(record_goal_execution_checkpoint(
                phase,
                label,
                &mut report,
                "before",
                counts.clone()
            )
            .is_err());
            assert_eq!(report, original);
        }
        for field in [
            "schema_version",
            "run_count",
            "node_count",
            "reservation_count",
            "sequence",
        ] {
            for value in [
                json!(true),
                json!(false),
                json!("0"),
                json!(null),
                json!(-1),
                json!(0.0),
            ] {
                let mut invalid = counts.clone();
                invalid[field] = value;
                assert!(!valid_goal_execution_counts(&invalid));
            }
            let mut missing = counts.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(!valid_goal_execution_counts(&missing));
        }
        let mut report = readonly_report_fixture(2);
        record_goal_execution_checkpoint(0, "main", &mut report, "before", counts.clone()).unwrap();
        let original = report.clone();
        assert!(
            record_goal_execution_checkpoint(0, "main", &mut report, "before", counts.clone())
                .is_err()
        );
        assert_eq!(report, original);
        assert!(record_goal_execution_checkpoint(
            0,
            "main",
            &mut report,
            "after",
            execution_counts_fixture(3)
        )
        .is_err());
        assert_eq!(report, original);
        record_goal_execution_checkpoint(0, "main", &mut report, "after", counts.clone()).unwrap();
        let finished = report.clone();
        assert!(
            record_goal_execution_checkpoint(0, "main", &mut report, "after", counts.clone())
                .is_err()
        );
        assert_eq!(report, finished);
        let mut failed = readonly_report_fixture(2);
        failed["error"] = json!("original failure");
        assert!(
            record_goal_execution_checkpoint(0, "main", &mut failed, "before", counts).is_err()
        );
    }
    #[test]
    fn execution_final_evidence_requires_real_counts_strict_flags_and_exact_missing_reference() {
        let mut report = readonly_report_fixture(2);
        let counts = execution_counts_fixture(2);
        record_goal_execution_checkpoint(0, "main", &mut report, "before", counts.clone()).unwrap();
        record_goal_execution_checkpoint(0, "main", &mut report, "after", counts).unwrap();
        let result = execution_result_fixture(2);
        validate_goal_execution_evidence(&result, &report).unwrap();
        for field in [
            "goalExecutionLists",
            "goalExecutionMissingRunDenied",
            "readonlyGoalExecutionNoMutation",
        ] {
            for fake in [json!(false), json!("true"), json!(1), json!(null)] {
                let mut changed = result.clone();
                changed[field] = fake;
                assert!(validate_goal_execution_evidence(&changed, &report).is_err());
            }
            let mut missing = result.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(validate_goal_execution_evidence(&missing, &report).is_err());
        }
        for field in ["run_id", "code", "side_effect_state"] {
            let mut missing = result.clone();
            missing["goalExecutionMissingRun"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(validate_goal_execution_evidence(&missing, &report).is_err());
        }
        let mut wrong = result.clone();
        wrong["goalExecutionMissingRun"]["run_id"] = json!("foreign-run");
        assert!(validate_goal_execution_evidence(&wrong, &report).is_err());
        let mut wrong = result.clone();
        wrong["goalExecutionMissingRun"]["side_effect_state"] = json!("unknown");
        assert!(validate_goal_execution_evidence(&wrong, &report).is_err());
        for value in [json!(true), json!("2"), json!(2.0), json!(3)] {
            let mut wrong = result.clone();
            wrong["goalExecutionReadonlySequence"]["after"] = value;
            assert!(validate_goal_execution_evidence(&wrong, &report).is_err());
        }
        let mut missing = report.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("goal_execution_readonly_database");
        assert!(validate_goal_execution_evidence(&result, &missing).is_err());
        let mut changed = report.clone();
        changed["goal_execution_readonly_database"]["after"]["node_count"] = json!(1);
        assert!(validate_goal_execution_evidence(&result, &changed).is_err());
    }
    #[test]
    fn execution_actual_private_daemon_empty_list_and_fixed_missing_get_are_readonly() {
        let directory = private_fixture_runtime().unwrap();
        let runtime = directory.path().canonicalize().unwrap();
        let project = prepare_readonly_goal_project(&runtime).unwrap();
        let daemon = ipc::Daemon::bind(&runtime).unwrap();
        struct OwnedReadonlyDaemon {
            stop: Arc<std::sync::atomic::AtomicBool>,
            worker: Option<std::thread::JoinHandle<agentbaton_core::Result<()>>>,
        }
        impl Drop for OwnedReadonlyDaemon {
            fn drop(&mut self) {
                self.stop.store(true, Ordering::SeqCst);
                if let Some(worker) = self.worker.take() {
                    let _ = worker.join();
                }
            }
        }
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker = std::thread::spawn(move || daemon.run(worker_stop));
        let _owned = OwnedReadonlyDaemon {
            stop,
            worker: Some(worker),
        };
        // Original fixture task creation occurs before the readonly interval.
        let task=ipc::call(&runtime,json!({"method":"tasks.create","params":{"project_id":project,"goal":"Fresh readonly execution fixture"},"idempotency_key":"readonly-execution-task"})).unwrap();
        assert!(task.get("error").is_none(), "{task}");
        let task_id = task["result"]["id"].as_str().unwrap();
        let before = goal_execution_database_snapshot(&runtime).unwrap();
        let old_before = goal_database_snapshot(&runtime).unwrap();
        let listed = ipc::call(
            &runtime,
            json!({"method":"goal_runs.list","params":{"task_id":task_id}}),
        )
        .unwrap();
        assert_eq!(listed["result"], json!([]));
        let missing = ipc::call(
            &runtime,
            json!({"method":"goal_runs.get","params":{"run_id":MISSING_GOAL_RUN_ID}}),
        )
        .unwrap();
        assert_eq!(missing["error"]["code"], "NOT_FOUND");
        assert_eq!(missing["error"]["side_effect_state"], "not_started");
        assert!(missing.get("result").is_none());
        assert_eq!(goal_execution_database_snapshot(&runtime).unwrap(), before);
        assert_eq!(goal_database_snapshot(&runtime).unwrap(), old_before);
    }

    fn graph_counts_fixture(sequence: u64) -> Value {
        json!({"schema_version":12,"capability_count":0,"job_count":0,"candidate_count":0,"validation_count":0,"owner_decision_count":0,"acceptance_count":0,"sequence":sequence})
    }
    fn graph_report_fixture(sequence: u64) -> Value {
        let mut report = readonly_report_fixture(sequence);
        let counts = execution_counts_fixture(sequence);
        report["goal_execution_readonly_database"] =
            json!({"read_only":true,"unchanged":true,"before":counts,"after":counts});
        report
    }
    fn graph_result_fixture(sequence: u64) -> Value {
        let mut result = execution_result_fixture(sequence);
        for key in [
            "goalGraphLists",
            "goalGraphMissingRunDenied",
            "readonlyGoalGraphNoMutation",
        ] {
            result[key] = json!(true);
        }
        result["goalGraphReadonlySequence"] = json!({"before":sequence,"after":sequence});
        result["goalGraphMissingRun"] = json!({"run_id":MISSING_GOAL_GRAPH_RUN_ID,"code":"NOT_FOUND","side_effect_state":"not_started"});
        result
    }
    #[test]
    fn graph_readonly_actual_schema_six_counts_and_journal_never_write() {
        let (_directory, runtime, mut store) = fresh_execution_fixture();
        let before = goal_graph_database_snapshot(&runtime).unwrap();
        assert_eq!(before, graph_counts_fixture(0));
        assert_eq!(goal_database_snapshot(&runtime).unwrap()["sequence"], 0);
        assert_eq!(
            goal_execution_database_snapshot(&runtime).unwrap()["schema_version"],
            12
        );
        let mut report = graph_report_fixture(0);
        record_goal_graph_checkpoint(0, "main", &mut report, "before", before.clone()).unwrap();
        record_goal_graph_checkpoint(
            0,
            "main",
            &mut report,
            "after",
            goal_graph_database_snapshot(&runtime).unwrap(),
        )
        .unwrap();
        validate_goal_graph_evidence(&graph_result_fixture(0), &report).unwrap();
        assert_eq!(store.sequence().unwrap(), 0);
        store
            .create(
                "local",
                "Generated sequence drift, no Graph authority",
                "graph-sequence",
            )
            .unwrap();
        let changed = goal_graph_database_snapshot(&runtime).unwrap();
        assert_eq!(changed["sequence"], store.sequence().unwrap());
        assert_ne!(changed, before);
        for field in GOAL_GRAPH_COUNT_FIELDS {
            assert_eq!(changed[field], 0);
        }
        assert!(record_goal_graph_checkpoint(
            0,
            "main",
            &mut graph_report_fixture(0),
            "before",
            changed
        )
        .is_err());
    }
    #[test]
    fn graph_readonly_wrong_schema_and_each_real_table_drift_fail_closed() {
        let controls = [
            ("schema_version", "PRAGMA user_version=10", 10),
            ("schema_version", "PRAGMA user_version=11", 11),
            ("schema_version", "PRAGMA user_version=13", 13),
            ("capability_count", "INSERT INTO goal_graph_capabilities(run_id,approval_id,version,document) VALUES('probe','approval',1,'{}')", 1),
            ("job_count", "INSERT INTO goal_graph_jobs(id,run_id,node_id,plan_revision,state,attempt_id,workspace_id,document) VALUES('probe','run','node',1,'queued',NULL,'workspace','{}')", 1),
            ("candidate_count", "INSERT INTO goal_graph_candidates(id,run_id,node_id,plan_revision,attempt_id,document) VALUES('probe','run','node',1,'attempt','{}')", 1),
            ("validation_count", "INSERT INTO goal_graph_validations(id,run_id,node_id,plan_revision,attempt_id,document) VALUES('probe','run','node',1,'attempt','{}')", 1),
            ("owner_decision_count", "INSERT INTO goal_graph_owner_decisions(id,approval_id,run_id,node_id,plan_revision,document) VALUES('probe','approval','run','node',1,'{}')", 1),
            ("acceptance_count", "INSERT INTO goal_graph_acceptances(id,task_id,task_revision,run_id,plan_revision,document) VALUES('probe','task',1,'run',1,'{}')", 1),
        ];
        for (field, sql, expected) in controls {
            let (_directory, runtime, store) = fresh_execution_fixture();
            assert!(goal_graph_database_snapshot(&runtime).is_ok());
            // Deliberately malformed generated SQL metadata is a rejection
            // control, never a Core-approved capability, job or acceptance.
            let status = Command::new("python3").args(["-I","-S","-c","import sqlite3,sys;db=sqlite3.connect(sys.argv[1]);db.execute(sys.argv[2]);db.commit();db.close()"]).arg(runtime.join("baton.sqlite3")).arg(sql).status().unwrap();
            assert!(status.success());
            assert!(goal_graph_database_snapshot(&runtime).is_err());
            let mut command = Command::new("python3");
            command
                .args(["-I", "-S", "-c", GOAL_GRAPH_READONLY_SQL])
                .arg(runtime.join("baton.sqlite3"));
            let actual: Value =
                serde_json::from_slice(&bounded_goal_query(command).unwrap()).unwrap();
            assert_eq!(actual[field], expected);
            assert_eq!(actual["sequence"], 0);
            drop(store);
        }
    }
    #[test]
    fn graph_checkpoints_reject_phase_replay_type_and_cross_interval_substitution() {
        let counts = graph_counts_fixture(2);
        for (phase, label) in [(1, "main"), (2, "native-reopened"), (0, "other")] {
            let mut report = graph_report_fixture(2);
            let original = report.clone();
            assert!(record_goal_graph_checkpoint(
                phase,
                label,
                &mut report,
                "before",
                counts.clone()
            )
            .is_err());
            assert_eq!(report, original);
        }
        for field in [
            "schema_version",
            "capability_count",
            "job_count",
            "candidate_count",
            "validation_count",
            "owner_decision_count",
            "acceptance_count",
            "sequence",
        ] {
            let mut missing = counts.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(record_goal_graph_checkpoint(
                0,
                "main",
                &mut graph_report_fixture(2),
                "before",
                missing
            )
            .is_err());
            for fake in [json!(true), json!("0"), json!(0.0), json!(-1)] {
                let mut invalid = counts.clone();
                invalid[field] = fake;
                assert!(record_goal_graph_checkpoint(
                    0,
                    "main",
                    &mut graph_report_fixture(2),
                    "before",
                    invalid
                )
                .is_err());
            }
        }
        for group in ["goal_readonly_database", "goal_execution_readonly_database"] {
            let mut report = graph_report_fixture(2);
            report.as_object_mut().unwrap().remove(group);
            assert!(
                record_goal_graph_checkpoint(0, "main", &mut report, "before", counts.clone())
                    .is_err()
            );
        }
        let mut report = graph_report_fixture(2);
        assert!(
            record_goal_graph_checkpoint(0, "main", &mut report, "after", counts.clone()).is_err()
        );
        record_goal_graph_checkpoint(0, "main", &mut report, "before", counts.clone()).unwrap();
        let original = report.clone();
        assert!(
            record_goal_graph_checkpoint(0, "main", &mut report, "before", counts.clone()).is_err()
        );
        assert!(record_goal_graph_checkpoint(
            0,
            "main",
            &mut report,
            "after",
            graph_counts_fixture(3)
        )
        .is_err());
        assert_eq!(report, original);
        record_goal_graph_checkpoint(0, "main", &mut report, "after", counts.clone()).unwrap();
        let finished = report.clone();
        assert!(record_goal_graph_checkpoint(0, "main", &mut report, "after", counts).is_err());
        assert_eq!(report, finished);
        let mut failed = graph_report_fixture(2);
        failed["error"] = json!("original failure");
        assert!(record_goal_graph_checkpoint(
            0,
            "main",
            &mut failed,
            "before",
            graph_counts_fixture(2)
        )
        .is_err());
    }
    #[test]
    fn graph_final_evidence_requires_independent_schema12_and_exact_core_denial() {
        let mut report = graph_report_fixture(2);
        let counts = graph_counts_fixture(2);
        record_goal_graph_checkpoint(0, "main", &mut report, "before", counts.clone()).unwrap();
        record_goal_graph_checkpoint(0, "main", &mut report, "after", counts).unwrap();
        let result = graph_result_fixture(2);
        validate_goal_graph_evidence(&result, &report).unwrap();
        for field in [
            "goalGraphLists",
            "goalGraphMissingRunDenied",
            "readonlyGoalGraphNoMutation",
        ] {
            for fake in [json!(false), json!("true"), json!(1), json!(null)] {
                let mut changed = result.clone();
                changed[field] = fake;
                assert!(validate_goal_graph_evidence(&changed, &report).is_err());
            }
            let mut missing = result.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(validate_goal_graph_evidence(&missing, &report).is_err());
        }
        for field in ["run_id", "code", "side_effect_state"] {
            let mut missing = result.clone();
            missing["goalGraphMissingRun"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(validate_goal_graph_evidence(&missing, &report).is_err());
        }
        for (field, value) in [
            ("run_id", json!("foreign")),
            ("code", json!("PERMISSION_DENIED")),
            ("side_effect_state", json!("unknown")),
            ("result", json!({})),
        ] {
            let mut changed = result.clone();
            changed["goalGraphMissingRun"][field] = value;
            assert!(validate_goal_graph_evidence(&changed, &report).is_err());
        }
        let mut changed = report.clone();
        changed["goal_graph_readonly_database"]["after"]["acceptance_count"] = json!(1);
        assert!(validate_goal_graph_evidence(&result, &changed).is_err());
        let mut changed = report.clone();
        changed["goal_execution_readonly_database"]["before"]["schema_version"] = json!(11);
        assert!(validate_goal_graph_evidence(&result, &changed).is_err());
        let mut changed = report.clone();
        changed["goal_readonly_database"]["before"]["scope_count"] = json!(1);
        assert!(validate_goal_graph_evidence(&result, &changed).is_err());
        let mut changed_report = report.clone();
        let mut changed_result = result.clone();
        for phase in ["before", "after"] {
            changed_report["goal_graph_readonly_database"][phase]["sequence"] = json!(3);
            changed_result["goalGraphReadonlySequence"][phase] = json!(3);
        }
        assert!(validate_goal_graph_evidence(&changed_result, &changed_report).is_err());
    }
    #[test]
    fn graph_actual_private_daemon_empty_list_and_fixed_missing_get_are_readonly() {
        let directory = private_fixture_runtime().unwrap();
        let runtime = directory.path().canonicalize().unwrap();
        let project = prepare_readonly_goal_project(&runtime).unwrap();
        let daemon = ipc::Daemon::bind(&runtime).unwrap();
        struct OwnedGraphDaemon {
            stop: Arc<std::sync::atomic::AtomicBool>,
            worker: Option<std::thread::JoinHandle<agentbaton_core::Result<()>>>,
        }
        impl Drop for OwnedGraphDaemon {
            fn drop(&mut self) {
                self.stop.store(true, Ordering::SeqCst);
                if let Some(worker) = self.worker.take() {
                    let _ = worker.join();
                }
            }
        }
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = stop.clone();
        let _owned = OwnedGraphDaemon {
            stop,
            worker: Some(std::thread::spawn(move || daemon.run(flag))),
        };
        // Setup is outside the readonly interval; no Graph proposal/Owner/run.
        let task = ipc::call(&runtime,json!({"method":"tasks.create","params":{"project_id":project,"goal":"Fresh readonly Graph fixture"},"idempotency_key":"readonly-graph-task"})).unwrap();
        assert!(task.get("error").is_none(), "{task}");
        let task_id = task["result"]["id"].as_str().unwrap();
        let before = goal_graph_database_snapshot(&runtime).unwrap();
        let old_before = goal_database_snapshot(&runtime).unwrap();
        let execution_before = goal_execution_database_snapshot(&runtime).unwrap();
        assert_eq!(before["schema_version"], 12);
        let listed = ipc::call(
            &runtime,
            json!({"method":"goal_graph_runs.list","params":{"task_id":task_id}}),
        )
        .unwrap();
        assert!(listed.get("error").is_none(), "{listed}");
        assert_eq!(listed["result"], json!([]));
        let missing = ipc::call(
            &runtime,
            json!({"method":"goal_graph_runs.get","params":{"run_id":MISSING_GOAL_GRAPH_RUN_ID}}),
        )
        .unwrap();
        assert_eq!(missing["error"]["code"], "NOT_FOUND");
        assert_eq!(missing["error"]["side_effect_state"], "not_started");
        assert!(missing.get("result").is_none());
        assert_eq!(goal_graph_database_snapshot(&runtime).unwrap(), before);
        assert_eq!(goal_database_snapshot(&runtime).unwrap(), old_before);
        assert_eq!(
            goal_execution_database_snapshot(&runtime).unwrap(),
            execution_before
        );
    }
    #[test]
    fn webcrypto_raw_vector_retains_utf8_and_unsafe_integer_literals() {
        assert_eq!(WEBCRYPTO_INPUT.len() as u64, WEBCRYPTO_UTF8_BYTES);
        assert!(WEBCRYPTO_INPUT.contains("1152921504606846979"));
        assert!(WEBCRYPTO_INPUT.contains("18446744073709551615"));
        assert_eq!(
            WEBCRYPTO_SHA256,
            "d29817e7338bcd0db0c7f22a98e3faa82802bb73f11d67ad2c65e789d1dd2892"
        );
    }
    #[test]
    fn webcrypto_digest_algorithm_bytes_availability_and_types_fail_closed() {
        let proof = webcrypto_expected("before");
        validate_webcrypto(&proof, "before").unwrap();
        for (field, value) in [
            ("available", json!(false)),
            ("available", json!(1)),
            ("algorithm", json!("SHA-1")),
            ("input_bytes", json!(108)),
            ("input_bytes", json!("109")),
            ("sha256", json!("0".repeat(64))),
            ("input", json!("rounded JSON")),
            ("version", json!(true)),
            ("checkpoint", json!("after")),
        ] {
            let mut changed = proof.clone();
            changed[field] = value;
            assert!(validate_webcrypto(&changed, "before").is_err(), "{field}");
        }
        assert!(validate_webcrypto(&Value::Null, "before").is_err());
        let mut extra = proof.clone();
        extra["mocked"] = json!(true);
        assert!(validate_webcrypto(&extra, "before").is_err());
        for key in proof.as_object().unwrap().keys() {
            let mut missing = proof.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(validate_webcrypto(&missing, "before").is_err(), "{key}");
        }
    }
    #[test]
    fn webcrypto_checkpoint_order_and_context_cannot_be_mixed_or_replayed() {
        let mut report = json!({});
        assert!(
            record_webcrypto_checkpoint(&mut report, "after", &webcrypto_expected("after"))
                .is_err()
        );
        record_webcrypto_checkpoint(&mut report, "before", &webcrypto_expected("before")).unwrap();
        assert!(
            record_webcrypto_checkpoint(&mut report, "before", &webcrypto_expected("before"))
                .is_err()
        );
        assert!(
            record_webcrypto_checkpoint(&mut report, "after", &webcrypto_expected("before"))
                .is_err()
        );
        record_webcrypto_checkpoint(&mut report, "after", &webcrypto_expected("after")).unwrap();
        assert!(
            record_webcrypto_checkpoint(&mut report, "after", &webcrypto_expected("after"))
                .is_err()
        );
        validate_webcrypto_evidence(
            &json!({"webCrypto":webcrypto_expected("first")}),
            &report,
            false,
        )
        .unwrap();
        assert!(validate_webcrypto_evidence(
            &json!({"webCrypto":webcrypto_expected("before")}),
            &report,
            false
        )
        .is_err());
    }
    #[test]
    fn webcrypto_reopened_report_requires_its_own_tag_and_first_window_evidence() {
        let mut report = json!({});
        record_webcrypto_checkpoint(&mut report, "before", &webcrypto_expected("before")).unwrap();
        record_webcrypto_checkpoint(&mut report, "after", &webcrypto_expected("after")).unwrap();
        let second = json!({"webCrypto":webcrypto_expected("reopened")});
        assert!(validate_webcrypto_evidence(&second, &report, true).is_err());
        report["first_webview"] = json!({"webCrypto":webcrypto_expected("first")});
        validate_webcrypto_evidence(&second, &report, true).unwrap();
        report["webcrypto_checkpoints"]["before"] = webcrypto_expected("after");
        assert!(validate_webcrypto_evidence(&second, &report, true).is_err());
    }
}
