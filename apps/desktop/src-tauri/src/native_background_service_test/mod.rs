//! Separate generated-tree test only: no OS service registration or login proof.
pub(crate) mod presentation;
mod sheet;
use crate::background_service::smoke::{Case, Fixture, Mode, Operation};
use agentbaton_core::ipc;
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Manager, RunEvent, WebviewWindow};

struct OwnedDaemon {
    child: Child,
    runtime: PathBuf,
}
impl OwnedDaemon {
    fn start(runtime: PathBuf) -> Result<Self, String> {
        let program = std::env::current_exe()
            .map_err(|_| "Smoke executable")?
            .parent()
            .ok_or("Smoke executable parent")?
            .join("batond");
        let m = fs::symlink_metadata(&program).map_err(|_| "Fixed sibling batond missing")?;
        if !m.is_file()
            || m.nlink() != 1
            || m.mode() & 0o6022 != 0
            || m.mode() & 0o111 == 0
            || ![0, unsafe { libc::geteuid() }].contains(&m.uid())
        {
            return Err("Unsafe fixed sibling daemon".into());
        }
        let child = Command::new(program)
            .env("BATON_RUNTIME_DIR", &runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Fixture daemon spawn")?;
        let mut owned = Self { child, runtime };
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if owned
                .child
                .try_wait()
                .map_err(|_| "Fixture daemon wait")?
                .is_some()
            {
                return Err("Fixture daemon exited before readiness".into());
            }
            if owned.snapshot().is_ok() {
                break;
            }
            if Instant::now() >= deadline {
                return Err("Fixture daemon readiness deadline".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(owned)
    }
    fn snapshot(&self) -> Result<Value, String> {
        let v = ipc::call(
            &self.runtime,
            json!({"method":"system.snapshot","params":{}}),
        )
        .map_err(|_| "Fixture snapshot transport")?;
        if v.get("error").is_some() {
            return Err("Fixture snapshot refusal".into());
        }
        crate::require_current_database_schema(&v)?;
        Ok(v)
    }
    fn alive(&mut self) -> bool {
        self.child.try_wait().is_ok_and(|s| s.is_none())
    }
    fn stop(&mut self) -> bool {
        if self.child.try_wait().is_ok_and(|s| s.is_some()) {
            return true;
        }
        let _ = self.child.kill();
        let until = Instant::now() + Duration::from_secs(3);
        while Instant::now() < until {
            if self.child.try_wait().is_ok_and(|s| s.is_some()) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
}
impl Drop for OwnedDaemon {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
struct State {
    fixture: Arc<Fixture>,
    caller: Mutex<Option<WebviewWindow>>,
    output: PathBuf,
    daemon: Mutex<OwnedDaemon>,
    capture: Mutex<Option<sheet::Capture>>,
    done: AtomicBool,
    failed: AtomicBool,
    page_ready: AtomicBool,
    screenshots: Mutex<usize>,
}
fn fail(app: &tauri::AppHandle, code: &'static str) {
    let s = app.state::<State>();
    s.fixture.fail(code);
    s.failed.store(true, Ordering::Release);
    s.done.store(true, Ordering::Release);
    app.exit(1);
}
#[tauri::command]
fn native_background_service_fixture_report(
    window: WebviewWindow,
    app: tauri::AppHandle,
    observation: Value,
) -> Result<(), String> {
    let result = (|| {
        if objc2::MainThreadMarker::new().is_none() {
            return Err("Report outside main thread".into());
        }
        let s = app.state::<State>();
        let caller = s.caller.lock().map_err(|_| "Caller lock")?;
        let caller = caller.as_ref().ok_or("Caller missing")?;
        if window.label() != caller.label()
            || window.ns_window().map_err(|_| "Report caller handle")?
                != caller.ns_window().map_err(|_| "Expected caller handle")?
        {
            return Err("Foreign report caller".into());
        }
        let obj = observation.as_object().ok_or("Report not object")?;
        if obj.len() != 2
            || serde_json::to_vec(&observation)
                .map_err(|_| "Report encode")?
                .len()
                > 128
        {
            return Err("Unknown/oversized report".into());
        }
        let index = obj
            .get("index")
            .and_then(Value::as_u64)
            .filter(|i| *i < 16)
            .ok_or("Invalid report index")?;
        let seen = obj
            .get("seen")
            .and_then(Value::as_bool)
            .ok_or("Invalid report flag")?;
        s.fixture.ui_report(index as usize, seen)
    })();
    if result.is_err() {
        fail(&app, "renderer_report_rejected");
    }
    result
}
fn script(index: usize, op: Operation, direct: bool) -> String {
    let action = match op {
        Operation::Status => "status",
        Operation::Enable => "enable",
        Operation::Disable => "disable",
    };
    format!(
        r#"(async()=>{{try{{
const until=performance.now()+15000;
const wait=async f=>{{while(performance.now()<until){{const v=f();if(v)return v;await new Promise(r=>setTimeout(r,25));}}throw Error('DOM observation deadline');}};
const find=(parent,text)=>[...parent.querySelectorAll('button')].find(b=>b.textContent.trim()===text);
if({index}===0){{
 const language=await wait(()=>document.querySelector('.language select'));
 Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value').set.call(language,'en');language.dispatchEvent(new Event('change',{{bubbles:true}}));
 const settings=await wait(()=>document.querySelector('button[title="Settings"]'));settings.click();
 const summary=await wait(()=>[...document.querySelectorAll('summary')].find(e=>e.textContent==='Background and login'));summary.click();
}}else{{
 const panel=await wait(()=>document.querySelector('section[aria-label="Start background work at next login"]'));
 if({direct}){{if(!find(panel,'Enable at next login')?.disabled)throw Error('Conflict UI must remain disabled');await window.__TAURI_INTERNALS__.invoke('background_service_enable');}}
 else{{const text={{status:'Refresh setting',enable:'Enable at next login',disable:'Disable at next login'}}['{action}'];const button=await wait(()=>{{const b=find(panel,text);return b&&!b.disabled?b:null;}});button.click();}}
}}

}}catch(error){{await window.__TAURI_INTERNALS__.invoke('native_background_service_fixture_report',{{observation:{{index:{index},seen:false}}}});}}}})()"#
    )
}
fn observation_script(index: usize, text: &str, disabled: bool) -> String {
    let text = serde_json::to_string(text).expect("fixed observation string");
    format!(
        r#"(async()=>{{const until=performance.now()+15000;let seen=false;while(performance.now()<until){{const p=document.querySelector('section[aria-label="Start background work at next login"]');const buttons=p?[...p.querySelectorAll('button')]:[];const action=buttons.find(b=>/^(Enable|Disable) at next login$/.test(b.textContent.trim()));if(p?.getAttribute('aria-busy')==='false'&&p.querySelector('[role="status"]')?.textContent==={text}&&action?.disabled==={disabled}){{seen=true;break;}}await new Promise(r=>setTimeout(r,25));}}await window.__TAURI_INTERNALS__.invoke('native_background_service_fixture_report',{{observation:{{index:{index},seen}}}});}})()"#
    )
}
fn poll(app: &tauri::AppHandle) -> Result<(), String> {
    let s = app.state::<State>();
    if !s.daemon.lock().map_err(|_| "Daemon lock")?.alive() {
        return Err("Owned fixture daemon changed or stopped".into());
    }
    let caller = s
        .caller
        .lock()
        .map_err(|_| "Caller lock")?
        .clone()
        .ok_or("Main caller missing")?;
    s.fixture.advance_observed()?;
    if s.fixture.done() {
        s.done.store(true, Ordering::Release);
        app.exit(0);
        return Ok(());
    }
    if !s.page_ready.load(Ordering::Acquire) {
        return Ok(());
    }
    if let Some((index, op, direct)) = s.fixture.launch()? {
        caller
            .eval(script(index, op, direct))
            .map_err(|_| "DOM script dispatch")?;
    }
    if let Some((index, text, disabled)) = s.fixture.observe_ui()? {
        caller
            .eval(observation_script(index, text, disabled))
            .map_err(|_| "DOM observation dispatch")?;
    }
    if let Some((index, page, pages)) = s.fixture.sheet()? {
        let Some(expected) = pages.get(page) else {
            return Ok(());
        };
        let Some(observed) = sheet::observe(&caller, expected)? else {
            return Ok(());
        };
        let mut capture = s.capture.lock().map_err(|_| "Capture lock")?;
        if capture.is_none() {
            *capture = Some(sheet::Capture::start(
                &s.output,
                index,
                page,
                observed.number,
            )?);
            return Ok(());
        }
        let c = capture.as_mut().ok_or("Capture missing")?;
        if (c.index, c.page, c.number) != (index, page, observed.number) {
            return Err("Stale/foreign sheet capture".into());
        }
        if !c.poll()? {
            return Ok(());
        }
        capture.take();
        *s.screenshots.lock().map_err(|_| "Screenshot count lock")? += 1;
        // Rust owns phase, expected complete page, live exact sheet and FS check.
        // The renderer cannot produce this authorization or choose a button.
        let cancel = s.fixture.before_click(index, page, observed.number)?;
        drop(capture);
        let button = if cancel { observed.cancel } else { observed.ok };
        // SAFETY: this is the retained enabled NSButton from the actual live
        // caller's attached sheet, observed on main, never a fake callback.
        unsafe { button.performClick(None) };
    }
    Ok(())
}
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("Only fixed case and existing evidence output arguments accepted".into());
    }
    let case = Case::parse(&args[0])?;
    let output = fs::canonicalize(&args[1])?;
    let m = fs::symlink_metadata(&output)?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o022 != 0 {
        return Err("Evidence output is not private-owned".into());
    }
    let fixture = Fixture::create(case)?;
    let mut initial = fixture.evidence();
    initial["status"] = json!("running");
    fs::write(
        output.join("background-service.json"),
        serde_json::to_vec_pretty(&initial)?,
    )?;
    let daemon = OwnedDaemon::start(fixture.core_runtime())?;
    let mut context = tauri::generate_context!();
    context.config_mut().app.app_directories_override =
        Some(tauri::utils::config::AppDirectoriesOverride::Root(
            fixture.core_runtime().join("tauri-state"),
        ));
    for w in &mut context.config_mut().app.windows {
        w.incognito = true;
        w.data_directory = Some("background-service-webview".into());
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(crate::native_parent::on_window_event)
        .manage(crate::Runtime {
            directory: fixture.core_runtime(),
            background_mode: Mode::Fixture(Some(fixture.clone())),
        })
        .manage(State {
            fixture: fixture.clone(),
            caller: Mutex::new(None),
            output: output.clone(),
            daemon: Mutex::new(daemon),
            capture: Mutex::new(None),
            done: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            page_ready: AtomicBool::new(false),
            screenshots: Mutex::new(0),
        })
        .setup(|app| {
            let main = app
                .get_webview_window("main")
                .ok_or("Main caller missing")?;
            *app.state::<State>()
                .caller
                .lock()
                .map_err(|_| "Caller lock")? = Some(main);
            Ok(())
        })
        .on_page_load(|window, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished
                && window.label() == "main"
            {
                window
                    .app_handle()
                    .state::<State>()
                    .page_ready
                    .store(true, Ordering::Release);
            }
        })
        .invoke_handler(|invoke| {
            if invoke.message.command() == "native_background_service_fixture_report" {
                {
                    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
                        tauri::generate_handler![native_background_service_fixture_report];
                    handler(invoke)
                }
            } else {
                crate::dispatch_production(invoke)
            }
        })
        .build(context)?;
    let handle = app.handle().clone();
    let finishing = app.handle().clone();
    // An automation alarm, not a product confirmation timeout or retry.
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(75);
        while !handle.state::<State>().done.load(Ordering::Acquire) {
            if Instant::now() >= deadline {
                fail(&handle, "native_background_fixture_alarm");
                return;
            }
            let target = handle.clone();
            if handle
                .run_on_main_thread(move || {
                    if let Err(_e) = poll(&target) {
                        fail(&target, "native_background_observation_failed");
                    }
                })
                .is_err()
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let code = app.run_return(|app, event| {
        if let RunEvent::ExitRequested {
            api, code: None, ..
        } = event
        {
            if !app.state::<State>().done.load(Ordering::Acquire) {
                api.prevent_exit();
            }
        }
    });
    let s = finishing.state::<State>();
    s.done.store(true, Ordering::Release);
    worker.join().map_err(|_| "Poller join failed")?;
    let capture_clean = s
        .capture
        .lock()
        .map_err(|_| "Capture lock")?
        .as_mut()
        .is_none_or(|c| c.stop());
    let alive = {
        let mut daemon = s.daemon.lock().map_err(|_| "Daemon lock")?;
        daemon.alive() && daemon.snapshot().is_ok()
    };
    let owned_reaped = s.daemon.lock().map_err(|_| "Daemon lock")?.stop();
    let mut record = fixture.evidence();
    record["owned_daemon_unchanged_and_alive_before_cleanup"] = json!(alive);
    record["owned_daemon_cleanup_reaped"] = json!(owned_reaped);
    record["capture_cleanup_reaped"] = json!(capture_clean);
    record["screenshots"] = json!(*s.screenshots.lock().map_err(|_| "Screenshot count")?);
    record["os"] = json!(std::env::consts::OS);
    record["architecture"] = json!(std::env::consts::ARCH);
    if code != 0 || s.failed.load(Ordering::Acquire) || !capture_clean || !alive || !owned_reaped {
        record["status"] = json!("failed");
    }
    let temporary = output.join("background-service.json.partial");
    fs::write(&temporary, serde_json::to_vec_pretty(&record)?)?;
    fs::rename(temporary, output.join("background-service.json"))?;
    if record["status"] != "passed" {
        return Err("Native background registration fixture failed; preserved evidence; never replay automatically".into());
    }
    Ok(())
}
