use super::controller;
use objc2::{rc::Retained, runtime::AnyObject, MainThreadMarker, Message};
use objc2_app_kit::{NSButton, NSTextField, NSView, NSWindow};
use objc2_foundation::NSRect;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Manager, RunEvent, WebviewWindow};

struct State {
    expected: Value,
    output: PathBuf,
    caller: Mutex<Option<WebviewWindow>>,
    phase: Mutex<Phase>,
    done: AtomicBool,
    failed: AtomicBool,
    injected: AtomicBool,
}
type CaptureResult = Arc<Mutex<Option<Result<(), String>>>>;
struct Phase {
    name: &'static str,
    clicked: bool,
    page: usize,
    previous_number: Option<isize>,
    capture: Option<CaptureResult>,
    capture_worker: Option<std::thread::JoinHandle<()>>,
    sheet_seen: Option<(isize, Instant)>,
    observation_failure: Option<Value>,
}
fn write(path: &Path, value: &Value) -> Result<(), String> {
    let temporary = path.with_extension("json.partial");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())
}

fn fail(app: &tauri::AppHandle, reason: &str) {
    let state = app.state::<State>();
    if state.failed.swap(true, Ordering::AcqRel) {
        return;
    }
    let observation_failure = state
        .phase
        .lock()
        .ok()
        .and_then(|p| p.observation_failure.clone());
    let reason = observation_failure
        .as_ref()
        .and_then(|v| v["reason"].as_str())
        .unwrap_or(reason);
    let _ = write(
        &state.output.join("owner-dialog.json"),
        &json!({"status":"failed","reason":reason,"observation_failure":observation_failure,"dimension":"macos_own_process_real_sheet","provider_model_calls":"not_run"}),
    );
    state.done.store(true, Ordering::Release);
    app.exit(1);
}
fn invoke_script(expected: &Value) -> String {
    format!("(async()=>{{try{{const response=await window.__TAURI_INTERNALS__.invoke('grants_create',{});await window.__TAURI_INTERNALS__.invoke('owner_dialog_report',{{response}})}}catch(error){{await window.__TAURI_INTERNALS__.invoke('owner_dialog_report',{{response:{{bridge_error:String(error)}}}})}}}})()",expected["invoke"])
}
fn record_report(
    window: WebviewWindow,
    app: &tauri::AppHandle,
    response: Value,
) -> Result<(), String> {
    let state = app.state::<State>();
    if window.label() != "main" {
        return Err("Report from unexpected framework caller".into());
    }
    let mut phase = state.phase.lock().map_err(|_| "Phase lock")?;
    if !phase.clicked {
        return Err("Real expected OS button was not clicked".into());
    }
    write(
        &state.output.join(format!("{}-reply.json", phase.name)),
        &response,
    )?;
    if phase.name == "cancel" {
        if response["error"]["code"] != "PERMISSION_DENIED"
            || response["error"]["side_effect_state"] != "not_started"
            || response.get("result").is_some()
        {
            drop(phase);
            return Err("Cancel response mismatch".into());
        }
        *phase = Phase {
            name: "approve",
            clicked: false,
            page: 0,
            previous_number: None,
            capture: None,
            capture_worker: None,
            sheet_seen: None,
            observation_failure: None,
        };
        drop(phase);
        window
            .eval(invoke_script(&state.expected))
            .map_err(|e| e.to_string())?;
    } else {
        if response.get("error").is_some()
            || response["result"]["approval_id"] != state.expected["proposal"]["id"]
        {
            drop(phase);
            return Err("Grant response mismatch".into());
        }
        write(
            &state.output.join("owner-dialog.json"),
            &json!({"status":"os_button_paths_completed_database_check_pending","dimension":"macos_own_process_real_sheet","provider_model_calls":"not_run","automated_fixture_not_human_owner":true}),
        )?;
        state.done.store(true, Ordering::Release);
        app.exit(0);
    }
    Ok(())
}
#[tauri::command]
fn owner_dialog_report(
    window: WebviewWindow,
    app: tauri::AppHandle,
    response: Value,
) -> Result<(), String> {
    let result = record_report(window, &app, response);
    if let Err(error) = &result {
        fail(&app, error);
    }
    result
}

fn contains(outer: NSRect, inner: NSRect) -> bool {
    let a = [
        outer.origin.x,
        outer.origin.y,
        outer.size.width,
        outer.size.height,
        inner.origin.x,
        inner.origin.y,
        inner.size.width,
        inner.size.height,
    ];
    a.iter().all(|v| v.is_finite())
        && inner.size.width > 0.
        && inner.size.height > 0.
        && inner.origin.x >= outer.origin.x - 0.5
        && inner.origin.y >= outer.origin.y - 0.5
        && inner.origin.x + inner.size.width <= outer.origin.x + outer.size.width + 0.5
        && inner.origin.y + inner.size.height <= outer.origin.y + outer.size.height + 0.5
}
fn view_visible(view: &NSView, sheet: &NSWindow) -> bool {
    !view.isHiddenOrHasHiddenAncestor()
        && view.window().is_some_and(|w| std::ptr::eq(&*w, sheet))
        && contains(view.visibleRect(), view.bounds())
}
struct Observation {
    value: Value,
    ok: Retained<NSButton>,
    cancel: Retained<NSButton>,
    number: isize,
}
struct SheetDiagnostic {
    // Set only after caller pointer, attached-sheet parent, visibility and stable
    // window identity checks. Never obtained from a file or foreign window scan.
    number: isize,
    value: Value,
}
fn rect(rect: NSRect) -> Value {
    json!({"x":rect.origin.x,"y":rect.origin.y,"width":rect.size.width,"height":rect.size.height})
}
fn observe(
    caller: &WebviewWindow,
    expected: &Value,
    phase: &str,
    index: usize,
    previous_number: Option<isize>,
    sheet_seen: &mut Option<(isize, Instant)>,
    diagnostic: &mut Option<SheetDiagnostic>,
) -> Result<Option<Observation>, String> {
    let _main = MainThreadMarker::new().ok_or("NSAlert observation must be on main")?;
    let pointer = caller.ns_window().map_err(|e| e.to_string())?;
    if pointer.is_null() {
        return Err("Actual caller NSWindow unavailable".into());
    }
    // SAFETY: Tauri supplies its live caller NSWindow; this runs on the event
    // thread, and a retained reference covers this observation only.
    let parent =
        unsafe { Retained::retain(pointer.cast::<NSWindow>()) }.ok_or("Cannot retain caller")?;
    let Some(sheet) = parent.attachedSheet() else {
        return Ok(None);
    };
    let parent_matches = sheet
        .sheetParent()
        .is_some_and(|p| std::ptr::eq(&*p, &*parent));
    if !parent_matches {
        return Err("Attached sheet belongs to a foreign parent".into());
    }
    if !sheet.isVisible() {
        return Ok(None);
    }
    let number = sheet.windowNumber();
    if previous_number == Some(number) {
        return Ok(None); // Wait for the next real sheet; never reuse the prior click.
    }
    let page_message = expected["presentation"][index]
        .as_str()
        .ok_or("Missing presentation page")?;
    match sheet_seen {
        None => {
            *sheet_seen = Some((number, Instant::now()));
            return Ok(None);
        }
        Some((seen, _)) if *seen != number => {
            return Err("Attached sheet changed during the armed phase".into())
        }
        Some((_, when)) if when.elapsed() < Duration::from_millis(500) => return Ok(None),
        _ => {}
    }
    *diagnostic = Some(SheetDiagnostic {
        number,
        value: json!({"phase":phase,"validation_stage":"content_view","window_number":number,
            "parent_window_number":parent.windowNumber(),"parent_matches":parent_matches,
            "parent_visible":parent.isVisible(),"sheet_visible":sheet.isVisible(),
            "parent_frame":rect(parent.frame()),"sheet_frame":rect(sheet.frame()),
            "on_screen":sheet.screen().is_some_and(|screen|contains(screen.visibleFrame(),sheet.frame())),
            "expected_message":controller::diagnostic_text(page_message, &mut 0)?,
            "expected_title":controller::diagnostic_text("AgentBaton — Native owner approval", &mut 0)?,
            "title_matches":0,"message_matches":0,"views":[],"buttons":[],
            "limits":{"views":256,"class_bytes":256,"field_text_utf8_bytes":controller::DIAGNOSTIC_TEXT_BYTES,"total_text_utf8_bytes":controller::DIAGNOSTIC_TOTAL_TEXT_BYTES},
            "automated_fixture_not_human_owner":true,"diagnostic_not_click_authority":true}),
    });
    let diagnostic = &mut diagnostic.as_mut().unwrap().value;
    let content = sheet.contentView().ok_or("Sheet has no content")?;
    content.displayIfNeeded();
    let mut queue = vec![(content, None)];
    let mut text_bytes = 0;
    diagnostic["validation_stage"] = json!("view_traversal");
    let mut count = 0;
    let mut message = Vec::new();
    let mut title = Vec::new();
    let mut title_matches = 0;
    let mut message_matches = 0;
    let mut buttons = Vec::new();
    let mut ok = None;
    let mut cancel = None;
    let mut complete = true;
    while let Some((view, parent_index)) = queue.pop() {
        count += 1;
        if count > 256 {
            return Err("Own sheet view traversal limit".into());
        }
        let object: &AnyObject = &view;
        let class = object.class().name();
        if class.to_bytes().len() > 256 {
            return Err("Own sheet diagnostic class byte limit".into());
        }
        let index = diagnostic["views"].as_array().unwrap().len();
        diagnostic["views"].as_array_mut().unwrap().push(json!({
            "index":index,"parent_index":parent_index,"class":class.to_string_lossy(),
            "window_matches":view.window().is_some_and(|w|std::ptr::eq(&*w,&*sheet)),
            "hidden_or_hidden_ancestor":view.isHiddenOrHasHiddenAncestor(),
            "visible":view_visible(&view,&sheet),"frame":rect(view.frame()),
            "bounds":rect(view.bounds()),"visible_rect":rect(view.visibleRect())}));
        if let Some(text) = object.downcast_ref::<NSTextField>() {
            let native_text = text.stringValue();
            diagnostic["views"][index]["text_utf16_units"] = json!(native_text.len_utf16());
            if native_text.len_utf16() > controller::DIAGNOSTIC_TEXT_BYTES {
                return Err("Own sheet diagnostic text UTF-16 preallocation limit".into());
            }
            let value = native_text.to_string();
            diagnostic["views"][index]["text_utf8_bytes"] = json!(value.len());
            diagnostic["views"][index]["text"] =
                controller::diagnostic_text(&value, &mut text_bytes)?;
            title_matches += usize::from(value == "AgentBaton — Native owner approval");
            message_matches += usize::from(value == page_message);
            diagnostic["title_matches"] = json!(title_matches);
            diagnostic["message_matches"] = json!(message_matches);
            if value == page_message || value == "AgentBaton — Native owner approval" {
                let bounds = text.bounds();
                let cell = text.cell().ok_or("Text has no measurable cell")?;
                let mut unconstrained = bounds;
                unconstrained.size.height = 1_000_000.;
                let required = cell.cellSizeForBounds(unconstrained);
                complete &= view_visible(text, &sheet)
                    && required.height.is_finite()
                    && required.width.is_finite()
                    && required.height <= bounds.size.height + 0.5
                    && required.width <= bounds.size.width + 0.5;
                diagnostic["views"][index]["required_cell_size"] =
                    json!({"width":required.width,"height":required.height});
                if value == "AgentBaton — Native owner approval" {
                    title.push(value);
                } else {
                    message.push(value);
                }
            }
        }
        if let Some(button) = object.downcast_ref::<NSButton>() {
            let native_name = button.title();
            if native_name.len_utf16() > 256 {
                return Err("Own sheet diagnostic button title limit".into());
            }
            let name = native_name.to_string();
            if name.len() > 256 {
                return Err("Own sheet diagnostic button title byte limit".into());
            }
            buttons.push(json!({"title":name,"enabled":button.isEnabled(),"visible":view_visible(button,&sheet)}));
            diagnostic["buttons"] = json!(buttons);
            let retained = button.retain();
            if name == "OK" {
                if ok.replace(retained).is_some() {
                    return Err("Ambiguous OK".into());
                }
            } else if name == "Cancel" && cancel.replace(retained).is_some() {
                return Err("Ambiguous Cancel".into());
            }
        }
        for child in &view.subviews() {
            if count + queue.len() >= 256 {
                return Err("Own sheet view traversal limit".into());
            }
            queue.push((child.retain(), Some(index)));
        }
    }
    diagnostic["validation_stage"] = json!("exact_title_message_counts");
    if title.len() != 1 || message.len() != 1 {
        return Err("Exact full native title/message not uniquely present".into());
    }
    let on_screen = sheet
        .screen()
        .is_some_and(|screen| contains(screen.visibleFrame(), sheet.frame()));
    let number = sheet.windowNumber();
    let value = json!({"phase":phase,"presentation_index":index,"presentation_count":expected["presentation"].as_array().unwrap().len(),"proposal_id":expected["proposal"]["id"],"proposal_revision":expected["proposal"]["revision"],"title":title[0],"message":message[0],"parent_matches":parent_matches,"visible":parent.isVisible() && sheet.isVisible() && on_screen,"complete_text_visible":complete,"window_number":number,"buttons":buttons,"capture_api":"/usr/sbin/screencapture -x -l exact_attached_sheet_window_number","click_api":"NSControl.performClick(None)","automated_fixture_not_human_owner":true});
    diagnostic["validation_stage"] = json!("controller_validate");
    controller::validate(&value, expected, phase, index)?;
    Ok(Some(Observation {
        value,
        ok: ok.ok_or("No OK")?,
        cancel: cancel.ok_or("No Cancel")?,
        number,
    }))
}
struct OwnedCapture(Child);
impl Drop for OwnedCapture {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
fn capture(number: isize, path: PathBuf) -> Result<(), String> {
    let mut child = OwnedCapture(
        Command::new("/usr/sbin/screencapture")
            .args(["-x", "-l", &number.to_string()])
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?,
    );
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                return Err("Actual sheet capture failed; no permissions were changed".into());
            }
            let bytes = fs::read(path).map_err(|e| e.to_string())?;
            if bytes.len() < 64 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
                return Err("Sheet capture did not produce PNG pixels".into());
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            let _ = child.0.kill();
            let _ = child.0.wait();
            return Err("Sheet capture deadline".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn start_capture(phase: &mut Phase, number: isize, path: PathBuf) {
    // Exactly one owned capture process/thread per phase, including failure.
    // Existing normal capture is reused, never detached or replaced.
    if phase.capture.is_none() {
        let result = Arc::new(Mutex::new(None));
        phase.capture = Some(result.clone());
        phase.capture_worker = Some(std::thread::spawn(move || {
            let captured = capture(number, path);
            *result
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(captured);
        }));
    }
}
fn save_failure(state: &State, phase: &mut Phase) {
    let path = state.output.join(format!(
        "{}-page-{:04}-sheet-diagnostic.json",
        phase.name,
        phase.page + 1
    ));
    if let Some(failure) = &mut phase.observation_failure {
        if let Err(error) = write(&path, failure) {
            // The final owner-dialog failure report also retains this secondary
            // error; it must not replace the earliest observation rejection.
            failure["diagnostic_write_error"] = json!(error);
        }
    }
}
fn poll_failure(state: &State, phase: &mut Phase) -> Result<(), String> {
    let captured = phase.capture.as_ref().and_then(|r| {
        r.lock()
            .map(|r| r.clone())
            .unwrap_or_else(|_| Some(Err("Capture lock".into())))
    });
    let pending = phase.capture.is_some() && captured.is_none();
    if let Some(failure) = &mut phase.observation_failure {
        failure["capture"] = match captured {
            None if pending => json!({"status":"pending"}),
            None => {
                json!({"status":"not_started","reason":"No verified positive owned sheet window number"})
            }
            Some(Ok(())) => {
                json!({"status":"captured_diagnostic_only","path":format!("{}-page-{:04}-sheet.png",phase.name,phase.page + 1)})
            }
            Some(Err(error)) => json!({"status":"failed","secondary_error":error}),
        };
    }
    if !pending {
        if let Some(worker) = phase.capture_worker.take() {
            if worker.join().is_err() {
                phase.observation_failure.as_mut().unwrap()["capture_join_error"] =
                    json!("Sheet capture worker panicked");
            }
        }
    }
    save_failure(state, phase);
    if pending {
        return Ok(());
    }
    Err(phase.observation_failure.as_ref().unwrap()["reason"]
        .as_str()
        .unwrap()
        .to_owned())
}
fn poll(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<State>();
    if state.done.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut phase = state.phase.lock().map_err(|_| "Phase lock")?;
    if phase.clicked {
        return Ok(());
    }
    if phase.observation_failure.is_some() {
        return poll_failure(&state, &mut phase);
    }
    let caller = state
        .caller
        .lock()
        .map_err(|_| "Caller lock")?
        .clone()
        .ok_or("Missing actual caller")?;
    let name = phase.name;
    let mut diagnostic = None;
    let observed = match observe(
        &caller,
        &state.expected,
        name,
        phase.page,
        phase.previous_number,
        &mut phase.sheet_seen,
        &mut diagnostic,
    ) {
        Ok(Some(observed)) => observed,
        Ok(None) => return Ok(()),
        Err(reason) => {
            let number = diagnostic.as_ref().map(|d| d.number).filter(|n| *n > 0);
            phase.observation_failure = Some(json!({"status":"failed","reason":reason,
                "failure_phase":"observation_before_click","observation":diagnostic.map(|d|d.value),
                "capture_api":"/usr/sbin/screencapture -x -l exact_attached_sheet_window_number",
                "no_click_or_database_permit":true}));
            save_failure(&state, &mut phase);
            if let Some(number) = number {
                let path = state.output.join(format!(
                    "{}-page-{:04}-sheet.png",
                    phase.name,
                    phase.page + 1
                ));
                start_capture(&mut phase, number, path);
            }
            return poll_failure(&state, &mut phase);
        }
    };
    if phase.capture.is_none() {
        let path = state.output.join(format!(
            "{}-page-{:04}-sheet.png",
            phase.name,
            phase.page + 1
        ));
        start_capture(&mut phase, observed.number, path);
        return Ok(());
    }
    let captured = phase
        .capture
        .as_ref()
        .unwrap()
        .lock()
        .map_err(|_| "Capture lock")?
        .clone();
    match captured {
        None => return Ok(()),
        Some(Err(error)) => return Err(error),
        Some(Ok(())) => {}
    }
    if let Some(worker) = phase.capture_worker.take() {
        worker.join().map_err(|_| "Sheet capture worker panicked")?;
    }
    write(
        &state.output.join(format!(
            "{}-page-{:04}-sheet-ready.json",
            phase.name,
            phase.page + 1
        )),
        &observed.value,
    )?;
    let permit = state.output.join(format!(
        "{}-page-{:04}-database-permit.json",
        phase.name,
        phase.page + 1
    ));
    if !permit.exists() {
        return Ok(());
    }
    let permit: Value = serde_json::from_slice(&fs::read(permit).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if permit["phase"] != phase.name
        || permit["presentation_index"] != phase.page
        || permit["presentation_count"] != state.expected["presentation"].as_array().unwrap().len()
        || permit["proposal_id"] != state.expected["proposal"]["id"]
        || permit["proposal_revision"] != state.expected["proposal"]["revision"]
        || permit["before_click_sequence"] != state.expected["baseline_sequence"]
    {
        return Err(
            "Host database permit does not bind the same exact pending proposal/sequence".into(),
        );
    }
    // Mark before public action to prevent reentrant polling from clicking twice.
    let final_page = phase.page + 1 == state.expected["presentation"].as_array().unwrap().len();
    let button = if final_page && phase.name == "cancel" {
        observed.cancel
    } else {
        observed.ok
    };
    if final_page {
        phase.clicked = true;
    } else {
        phase.page += 1;
        phase.previous_number = Some(observed.number);
        phase.sheet_seen = None;
        phase.capture = None;
        // capture_worker was genuinely joined above before moving to next page.
    }
    drop(phase);
    // SAFETY: This is the enabled NSButton dynamically found in this exact live
    // attached sheet on main, after full text/layout/parent/proposal validation.
    // None is the documented nil sender. It runs the real target/action path;
    // no modal result/completion callback or Core finalize is manufactured.
    unsafe { button.performClick(None) };
    Ok(())
}
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = std::env::args_os()
        .nth(1)
        .ok_or("Expected generated host fixture file")?;
    if fs::metadata(&fixture)?.len() > 65536 {
        return Err("Generated Grant fixture exceeds 64 KiB bound".into());
    }
    let expected: Value = serde_json::from_slice(&fs::read(fixture)?)?;
    let intent = json!({"project_id":expected["invoke"]["projectId"],"workspace_id":expected["invoke"]["workspaceId"],"connection_id":expected["invoke"]["connectionId"],"max_attempts":1,"max_concurrency":1,"max_duration_ms":1000});
    if expected["proposal"]["id"]
        .as_str()
        .is_none_or(|s| s.is_empty())
        || expected["proposal"]["revision"]
            .as_u64()
            .is_none_or(|n| n == 0)
        || expected["baseline_sequence"].as_u64().is_none()
        || expected["proposal"]["snapshot"]["intent"] != intent
        || expected["invoke"]["idempotencyKey"] != "owner-os-grant"
        || expected["message"]
            .as_str()
            .is_none_or(|s| s.is_empty() || s.len() > 24576)
        || expected["presentation"].as_array().is_none_or(|pages| {
            pages.len() < 2
                || pages.len() > 1024
                || pages
                    .iter()
                    .any(|page| page.as_str().is_none_or(|s| s.is_empty() || s.len() > 2048))
        })
        || expected["kind"] != "native_grant_os_dialog_fixture"
        || expected["proposal"]["kind"] != "grant"
        || expected["proposal"]["consumed"] != false
        || expected["invoke"]["maxAttempts"] != 1
        || expected["invoke"]["maxConcurrency"] != 1
        || expected["invoke"]["maxDurationMs"] != 1000
    {
        return Err("Only the bounded generated Grant fixture is supported".into());
    }
    let runtime =
        PathBuf::from(expected["runtime"].as_str().ok_or("No runtime")?).canonicalize()?;
    let output = PathBuf::from(expected["output"].as_str().ok_or("No output")?).canonicalize()?;
    let mut context = tauri::generate_context!();
    context.config_mut().app.app_directories_override = Some(
        tauri::utils::config::AppDirectoriesOverride::Root(runtime.join("tauri-state")),
    );
    for window in &mut context.config_mut().app.windows {
        window.incognito = true;
        window.data_directory = Some("owner-smoke-webview".into());
    }
    let state = State {
        expected,
        output,
        caller: Mutex::new(None),
        phase: Mutex::new(Phase {
            name: "cancel",
            clicked: false,
            page: 0,
            previous_number: None,
            capture: None,
            capture_worker: None,
            sheet_seen: None,
            observation_failure: None,
        }),
        done: AtomicBool::new(false),
        failed: AtomicBool::new(false),
        injected: AtomicBool::new(false),
    };
    let app = agentbaton_desktop::desktop_builder(runtime)
        .manage(state)
        .setup(|app| {
            *app.state::<State>().caller.lock().unwrap() = Some(
                app.get_webview_window("main")
                    .ok_or("Configured caller missing")?,
            );
            Ok(())
        })
        .invoke_handler(|invoke| {
            if invoke.message.command() == "owner_dialog_report" {
                let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
                    tauri::generate_handler![owner_dialog_report];
                handler(invoke)
            } else {
                agentbaton_desktop::dispatch_production(invoke)
            }
        })
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished
                && webview.label() == "main"
            {
                let app = webview.app_handle();
                let state = app.state::<State>();
                if !state.injected.swap(true, Ordering::AcqRel) {
                    if let Err(error) = webview.eval(invoke_script(&state.expected)) {
                        fail(app, &error.to_string());
                    }
                }
            }
        })
        .build(context)?;
    let finishing = app.handle().clone();
    let handle = app.handle().clone();
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(75);
        loop {
            if handle.state::<State>().done.load(Ordering::Acquire) {
                return;
            }
            if Instant::now() >= deadline {
                fail(
                    &handle,
                    "Real Owner dialog deadline; no fallback driver or permission change",
                );
                return;
            }
            let target = handle.clone();
            if handle
                .run_on_main_thread(move || {
                    if let Err(error) = poll(&target) {
                        fail(&target, &error);
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
    finishing
        .state::<State>()
        .done
        .store(true, Ordering::Release);
    worker.join().map_err(|_| "OS dialog poller panicked")?;
    let capture = finishing
        .state::<State>()
        .phase
        .lock()
        .map_err(|_| "Phase lock")?
        .capture_worker
        .take();
    if let Some(capture) = capture {
        capture
            .join()
            .map_err(|_| "Sheet capture worker panicked")?;
    }
    if code != 0 {
        return Err("OS dialog smoke failed; inspect preserved evidence".into());
    }
    Ok(())
}
