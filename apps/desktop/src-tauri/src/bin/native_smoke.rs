//! Main-thread real WebView -> production Tauri invoke -> private daemon harness.
//! This separate binary is never built by the default application feature set.
#[cfg(not(debug_assertions))]
compile_error!("native-smoke is a debug-only integration harness; never ship it");

use agentbaton_core::ipc;
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};

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
    match (state.phase.load(Ordering::SeqCst), window.label()) {
        (0, "main") => {
            let screenshot = capture_screen(&state.output, &window);
            let committed = (|| {
                let mut report = state.report.lock().unwrap();
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
 try {
  const deadline = Date.now() + 10000;
  while (!document.querySelector('#root')?.children.length) {
   require(Date.now() < deadline, 'actual built frontend did not mount');
   await new Promise(r => setTimeout(r, 25));
  }
  const initial = result(await invoke('system_snapshot'));
  require(initial.tasks.length === 0, 'private runtime must start empty');
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
  await invoke('native_smoke_report',{result:{ok:true,domMounted:true,realComposerSubmitted:true,realTaskRendered:true,taskId:task.id,idempotent:true,invalidInputRejected:true,journal:true}});
 } catch (error) { await invoke('native_smoke_report',{result:{ok:false,error:String(error)}}); }
})();
"#;
const SECOND: &str = r#"
(async () => {
 const invoke = window.__TAURI_INTERNALS__.invoke;
 const require = (value,message) => { if(!value) throw Error(message); };
 const result = value => { require(!value.error, JSON.stringify(value.error)); return value.result; };
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
  await invoke('native_smoke_report',{result:{ok:true,domMounted:true,taskId:task.id,restartPersisted:true,draftCancelled:true,stopEvidence:true}});
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
    let daemon_path = std::env::current_exe()?
        .parent()
        .ok_or("no binary parent")?
        .join("batond");
    let daemon = OwnedDaemon::start(daemon_path, runtime.clone())?;
    let report = json!({"status":"running","stages":[],"private_fixture_runtime":runtime,"owned_daemon_pid":daemon.child.id(),"dimension":"real_native_webview_production_invoke_private_daemon","os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"commit":std::env::var("AGENTBATON_SMOKE_COMMIT").unwrap_or_else(|_|"unknown".into()),"native_dialog":"not_run_manual_acceptance_required","provider_models":"not_run_not_authorized"});
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
}
