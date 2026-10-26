//! Main-thread real WebView -> production Tauri invoke -> private daemon harness.
//! This separate binary is never built by the default application feature set.
#[cfg(not(debug_assertions))]
compile_error!("native-smoke is a debug-only integration harness; never ship it");

use agentbaton_core::ipc;
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
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
            if self.child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err("owned daemon exited before authentication".into());
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
    daemon: Arc<Mutex<OwnedDaemon>>,
    report: Arc<Mutex<Value>>,
    output: PathBuf,
}
impl Smoke {
    fn fail(&self, app: &tauri::AppHandle, reason: impl ToString) {
        self.phase.store(4, Ordering::SeqCst);
        if let Ok(mut report) = self.report.lock() {
            report["status"] = json!("failed");
            report["error"] = json!(reason.to_string());
            let _ = fs::write(
                self.output.join("native-smoke.json"),
                serde_json::to_vec_pretty(&*report).unwrap(),
            );
        }
        app.exit(1);
    }
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
    match state.phase.load(Ordering::SeqCst) {
        0 => {
            state.report.lock().unwrap()["first_webview"] = result;
            state.report.lock().unwrap()["screenshot"] =
                json!(capture_screen(&state.output, &window));
            state.phase.store(1, Ordering::SeqCst);
            window.close().map_err(|e| e.to_string())?;
        }
        2 => {
            let expected = state.report.lock().unwrap()["first_webview"]["taskId"].clone();
            if result["taskId"] != expected {
                state.fail(&app, "reopened WebView saw a different task");
                return Err("task mismatch".into());
            }
            state.report.lock().unwrap()["reopened_webview"] = result;
            state.report.lock().unwrap()["status"] = json!("checks_passed_cleanup_pending");
            fs::write(
                state.output.join("native-smoke.json"),
                serde_json::to_vec_pretty(&*state.report.lock().unwrap()).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            state.phase.store(3, Ordering::SeqCst);
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
fn reopen(app: &tauri::AppHandle) -> Result<(), String> {
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
    daemon.restart()?;
    state.report.lock().unwrap()["owned_daemon_pid"] = json!(daemon.child.id());
    state.report.lock().unwrap()["owned_daemon_restarted"] = json!(true);
    state.phase.store(2, Ordering::SeqCst);
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("AgentBaton — Native integration")
        .inner_size(1180., 800.)
        .incognito(true)
        .data_directory(daemon.runtime.join("webview-cache"))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or("provide an existing evidence directory")?;
    let output = fs::canonicalize(output)?;
    if !output.is_dir() {
        return Err("evidence directory is not a directory".into());
    }
    let temporary = tempfile::Builder::new().prefix("baton-native-").tempdir()?;
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
    let report = json!({"status":"running","private_fixture_runtime":runtime,"owned_daemon_pid":daemon.child.id(),"dimension":"real_native_webview_production_invoke_private_daemon","os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"commit":std::env::var("AGENTBATON_SMOKE_COMMIT").unwrap_or_else(|_|"unknown".into()),"native_dialog":"not_run_manual_acceptance_required","provider_models":"not_run_not_authorized"});
    fs::write(
        output.join("native-smoke.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    let daemon = Arc::new(Mutex::new(daemon));
    let _cleanup_guard = CleanupGuard(daemon.clone());
    let report = Arc::new(Mutex::new(report));
    let phase = Arc::new(AtomicU8::new(0));
    let smoke = Smoke {
        phase: phase.clone(),
        daemon: daemon.clone(),
        report: report.clone(),
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
                let script = match state.phase.load(Ordering::SeqCst) {
                    0 => FIRST,
                    2 => SECOND,
                    _ => return,
                };
                if let Err(error) = webview.eval(script) {
                    state.fail(app, error);
                }
            }
        })
        .build(context)?;
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if handle.state::<Smoke>().phase.load(Ordering::SeqCst) >= 3 {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        handle
            .state::<Smoke>()
            .fail(&handle, "native WebView integration exceeded60s deadline");
    });
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.run_return(|app, event| match event {
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::Destroyed,
                ..
            } if label == "main" && app.state::<Smoke>().phase.load(Ordering::SeqCst) == 1 => {
                if let Err(error) = reopen(app) {
                    app.state::<Smoke>().fail(app, error);
                }
            }
            RunEvent::ExitRequested {
                api, code: None, ..
            } if app.state::<Smoke>().phase.load(Ordering::SeqCst) < 3 => api.prevent_exit(),
            _ => {}
        })
    }));
    let code = outcome.unwrap_or(1);
    if code != 0 {
        phase.store(4, Ordering::SeqCst);
    }
    let mut daemon = daemon.lock().unwrap_or_else(|error| error.into_inner());
    if daemon.child.try_wait()?.is_none() {
        daemon.child.kill()?;
    }
    daemon.child.wait()?;
    let mut report = report.lock().unwrap_or_else(|error| error.into_inner());
    report["owned_daemon_cleanup"] = json!(true);
    if code == 0 && phase.load(Ordering::SeqCst) == 3 {
        report["status"] = json!("passed");
    } else {
        report["status"] = json!("failed");
    }
    fs::write(
        output.join("native-smoke.json"),
        serde_json::to_vec_pretty(&*report)?,
    )?;
    drop(report);
    drop(daemon);
    drop(temporary);
    if code != 0 || phase.load(Ordering::SeqCst) != 3 {
        return Err("native smoke failed; inspect native-smoke.json".into());
    }
    Ok(())
}
