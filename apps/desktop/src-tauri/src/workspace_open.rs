//! Directory-only OS handoff. The renderer never supplies a path or process configuration.
use agentbaton_core::{ipc, WorkspaceOpenContext, WorkspaceOpenSelection};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::WebviewWindow;

#[cfg(any(target_os = "macos", test))]
const OPENER: &str = "/usr/bin/open";
#[cfg(any(target_os = "macos", test))]
const TERMINAL: &str = "/System/Applications/Utilities/Terminal.app";
static ACTIVE: AtomicBool = AtomicBool::new(false);

fn refused(code: &str) -> Value {
    json!({"status":"refused","code":code})
}
fn unknown(code: &str) -> Value {
    json!({"status":"outcome_unknown","code":code})
}
fn request_selection(request: Value) -> Result<WorkspaceOpenSelection, ()> {
    let object = request.as_object().ok_or(())?;
    if object.len() != 2
        || object.get("terminal").and_then(Value::as_str) != Some("macos_terminal")
        || !object.contains_key("selection")
    {
        return Err(());
    }
    let selection: WorkspaceOpenSelection =
        serde_json::from_value(object["selection"].clone()).map_err(|_| ())?;
    selection.validate().map_err(|_| ())?;
    Ok(selection)
}
fn context_frame(selection: &WorkspaceOpenSelection) -> Value {
    json!({"method":"workspace_open.context","params":{"selection":selection}})
}
fn fresh_context(
    response: Value,
    expected: &WorkspaceOpenSelection,
) -> Result<WorkspaceOpenContext, &'static str> {
    if let Some(error) = response.get("error") {
        return Err(match error.get("code").and_then(Value::as_str) {
            Some("INVALID_REQUEST") => "invalid_selection",
            Some("INPUT_CHANGED") => "directory_changed",
            _ => "stale_context",
        });
    }
    let context: WorkspaceOpenContext =
        serde_json::from_value(response.get("result").ok_or("stale_context")?.clone())
            .map_err(|_| "stale_context")?;
    if context.selection != *expected {
        return Err("stale_context");
    }
    Ok(context)
}
fn trusted_origin(label: &str, scheme: &str, host: Option<&str>, port: Option<u16>) -> bool {
    label == "main"
        && (scheme == "tauri" && host == Some("localhost") && port.is_none()
            || scheme == "http" && host == Some("tauri.localhost") && port.is_none()
            || cfg!(debug_assertions)
                && scheme == "http"
                && host == Some("127.0.0.1")
                && port == Some(1420))
}
type WindowReadJob = Box<dyn FnOnce() + Send + 'static>;
fn observe_window_until(
    deadline: std::time::Instant,
    now: impl Fn() -> std::time::Instant + Clone + Send + 'static,
    dispatch: impl FnOnce(WindowReadJob) -> Result<(), ()>,
    observe: impl FnOnce() -> bool + Send + 'static,
) -> bool {
    use std::sync::{mpsc, Arc};
    if now() >= deadline {
        return false;
    }
    let active = Arc::new(AtomicBool::new(true));
    struct Observation(Arc<AtomicBool>);
    impl Drop for Observation {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }
    let _observation = Observation(active.clone());
    let sample_now = now.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    // This queued job can only sample the caller. It cannot fetch Core
    // context, validate a directory, finalize or start the OS handoff.
    if dispatch(Box::new(move || {
        if !active.load(Ordering::Acquire) || sample_now() >= deadline {
            return;
        }
        let trusted = observe();
        let timely = active.load(Ordering::Acquire) && sample_now() < deadline;
        let _ = sender.try_send(trusted && timely);
    }))
    .is_err()
    {
        return false;
    }
    let Some(remaining) = deadline
        .checked_duration_since(now())
        .filter(|d| !d.is_zero())
    else {
        return false;
    };
    receiver
        .recv_timeout(remaining)
        .is_ok_and(|trusted| trusted && now() < deadline)
}
fn trusted_window(window: &WebviewWindow) -> bool {
    use std::time::{Duration, Instant};
    let caller = window.clone();
    observe_window_until(
        Instant::now() + Duration::from_secs(5),
        Instant::now,
        |read| window.run_on_main_thread(read).map_err(|_| ()),
        move || {
            caller.url().is_ok_and(|url| {
                url.username().is_empty()
                    && url.password().is_none()
                    && trusted_origin(caller.label(), url.scheme(), url.host_str(), url.port())
            })
        },
    )
}
fn closed_result(value: &Value) -> bool {
    let Some(v) = value.as_object() else {
        return false;
    };
    v.len() == 2
        && v.contains_key("status")
        && v.contains_key("code")
        && matches!(
            (v["status"].as_str(), v["code"].as_str()),
            (Some("handoff_accepted"), Some("accepted"))
                | (
                    Some("refused"),
                    Some(
                        "unsupported_platform"
                            | "invalid_selection"
                            | "stale_context"
                            | "directory_changed"
                            | "native_window_invalid"
                            | "opener_unavailable"
                    )
                )
                | (
                    Some("outcome_unknown"),
                    Some("opener_failed" | "opener_timeout" | "transport_unknown")
                )
        )
}
/// Called only by Root's narrow main command hook. No daemon startup or write bridge.
pub(super) async fn handoff(directory: PathBuf, window: WebviewWindow, request: Value) -> Value {
    tauri::async_runtime::spawn_blocking(move || handoff_sync(directory, window, request))
        .await
        .unwrap_or_else(|_| unknown("transport_unknown"))
}
fn handoff_sync(directory: PathBuf, window: WebviewWindow, request: Value) -> Value {
    let selection = match request_selection(request) {
        Ok(v) => v,
        Err(()) => return refused("invalid_selection"),
    };
    if !trusted_window(&window) {
        return refused("native_window_invalid");
    }
    if !cfg!(target_os = "macos") {
        return refused("unsupported_platform");
    }
    if ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return refused("stale_context");
    }
    struct Flight;
    impl Drop for Flight {
        fn drop(&mut self) {
            ACTIVE.store(false, Ordering::Release);
        }
    }
    let _flight = Flight;
    let started = AtomicBool::new(false);
    let current_window = window.clone();
    let value = super::native_parent::with_caller(window, |parent| {
        #[cfg(target_os = "macos")]
        if !clear_previous_owned_opener() {
            return Ok(refused("stale_context"));
        }
        let response = match ipc::call(&directory, context_frame(&selection)) {
            Ok(v) => v,
            Err(_) => return Ok(refused("stale_context")),
        };
        let context = match fresh_context(response, &selection) {
            Ok(v) => v,
            Err(code) => return Ok(refused(code)),
        };
        let pin = match agentbaton_core::pin_open_context(&context) {
            Ok(v) => v,
            Err(_) => return Ok(refused("directory_changed")),
        };
        parent.finalize(|| {
            if !trusted_window(&current_window) {
                return Ok(refused("native_window_invalid"));
            }
            #[cfg(target_os = "macos")]
            if !opener_usable() {
                return Ok(refused("opener_unavailable"));
            }
            if pin.recheck().is_err() {
                return Ok(refused("directory_changed"));
            }
            #[cfg(target_os = "macos")]
            {
                Ok(open_directory(&context.canonical_path, &started))
            }
            #[cfg(not(target_os = "macos"))]
            {
                Ok(refused("unsupported_platform"))
            }
        })
    });
    match value {
        Ok(v) if closed_result(&v) => v,
        _ if started.load(Ordering::Acquire) => unknown("transport_unknown"),
        _ => refused("native_window_invalid"),
    }
}
#[cfg(any(target_os = "macos", test))]
fn opener_command(path: &str) -> std::process::Command {
    use std::process::{Command, Stdio};
    let mut command = Command::new(OPENER);
    command
        .args(["-a", TERMINAL, path])
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}
#[cfg(any(target_os = "macos", test))]
fn completed(success: bool, inside_deadline: bool) -> Value {
    if !inside_deadline {
        unknown("opener_timeout")
    } else if success {
        json!({"status":"handoff_accepted","code":"accepted"})
    } else {
        unknown("opener_failed")
    }
}
#[cfg(target_os = "macos")]
fn owned_slot() -> &'static std::sync::Mutex<Option<std::process::Child>> {
    static OWNED: std::sync::Mutex<Option<std::process::Child>> = std::sync::Mutex::new(None);
    &OWNED
}
#[cfg(target_os = "macos")]
fn clear_previous_owned_opener() -> bool {
    let mut slot = owned_slot().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(child) = slot.as_mut() {
        if !matches!(child.try_wait(), Ok(Some(_))) {
            return false;
        }
        *slot = None;
    }
    true
}
#[cfg(target_os = "macos")]
fn retain_after_interruption(mut child: std::process::Child) {
    // A fresh owned-child observation precedes signalling. A wait error is
    // uncertainty about ownership, so never signal that numeric PID on error.
    match child.try_wait() {
        Ok(Some(_)) => return,
        Ok(None) => {
            // Only our opener, never Terminal or any external process group.
            let _ = child.kill();
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
        }
        Err(_) => (),
    }
    // No unbounded wait, detached reaper, new deadline or repeated signal.
    *owned_slot().lock().unwrap_or_else(|p| p.into_inner()) = Some(child);
}

#[cfg(target_os = "macos")]
fn opener_usable() -> bool {
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;
    let program = Path::new(OPENER);
    program
        .canonicalize()
        .is_ok_and(|p| p.as_os_str() == program.as_os_str())
        && std::fs::symlink_metadata(program).is_ok_and(|m| {
            m.is_file() && m.uid() == 0 && m.mode() & 0o022 == 0 && m.mode() & 0o111 != 0
        })
}
#[cfg(target_os = "macos")]
struct OwnedOpener {
    child: Option<std::process::Child>,
}
#[cfg(target_os = "macos")]
impl Drop for OwnedOpener {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            retain_after_interruption(child);
        }
    }
}
#[cfg(target_os = "macos")]
fn open_directory(path: &str, started: &AtomicBool) -> Value {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_secs(5);
    let child = match opener_command(path).spawn() {
        Ok(v) => v,
        Err(_) => return refused("opener_unavailable"),
    };
    let mut owned = OwnedOpener { child: Some(child) };
    started.store(true, Ordering::Release);
    loop {
        if Instant::now() >= deadline {
            return unknown("opener_timeout");
        }
        match owned
            .child
            .as_mut()
            .expect("Owned opener exists until observed reap")
            .try_wait()
        {
            Ok(Some(status)) => {
                owned.child = None;
                return completed(status.success(), Instant::now() < deadline);
            }
            Err(_) => return unknown("opener_failed"),
            Ok(None) => (),
        }
        std::thread::sleep(
            Duration::from_millis(10).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROJECT: &str = "11111111-1111-4111-8111-111111111111";
    fn request() -> Value {
        json!({"selection":{"kind":"project_source","project_id":PROJECT,"expected_project_revision":2},"terminal":"macos_terminal"})
    }
    #[test]
    fn only_closed_ids_revision_and_fixed_terminal_can_enter_native() {
        assert!(request_selection(request()).is_ok());
        for key in [
            "path",
            "program",
            "args",
            "env",
            "connection_id",
            "grant",
            "approval",
        ] {
            let mut v = request();
            v[key] = json!("private");
            assert!(request_selection(v).is_err());
        }
        for bad in [json!(0), json!(true), json!(-1), json!(1.5), json!("2")] {
            let mut v = request();
            v["selection"]["expected_project_revision"] = bad;
            assert!(request_selection(v).is_err());
        }
        for terminal in ["ghostty", "sh", "macos_terminal\n"] {
            let mut v = request();
            v["terminal"] = json!(terminal);
            assert!(request_selection(v).is_err());
        }
        let mut v = request();
        v["selection"]["path"] = json!("/tmp");
        assert!(request_selection(v).is_err());
    }
    #[test]
    fn context_must_be_exact_selected_record_and_not_a_success_error_mix() {
        let selection = request_selection(request()).unwrap();
        let context = json!({"selection":selection,"canonical_path":"/tmp","directory_identity":[{"device":"1","inode":"2"},{"device":"1","inode":"3"}]});
        assert!(fresh_context(json!({"result":context}), &selection).is_ok());
        let mut wrong = context.clone();
        wrong["selection"]["expected_project_revision"] = json!(3);
        assert!(fresh_context(json!({"result":wrong}), &selection).is_err());
        let mut extra = context.clone();
        extra["program"] = json!(OPENER);
        assert!(fresh_context(json!({"result":extra}), &selection).is_err());
        assert!(fresh_context(
            json!({"result":context,"error":{"code":"INPUT_CHANGED"}}),
            &selection
        )
        .is_err());
    }
    #[test]
    fn main_window_and_fixed_origin_are_required() {
        assert!(trusted_origin("main", "tauri", Some("localhost"), None));
        for (label, scheme, host, port) in [
            ("other", "tauri", Some("localhost"), None),
            ("main", "https", Some("evil.invalid"), None),
            ("main", "tauri", Some("localhost"), Some(5)),
            ("main", "file", None, None),
        ] {
            assert!(!trusted_origin(label, scheme, host, port));
        }
    }
    #[test]
    fn window_observation_zero_deadline_or_dispatch_failure_refuses_without_reading() {
        use std::sync::Arc;
        use std::time::{Duration, Instant};
        let reads = Arc::new(AtomicBool::new(false));
        let sample = reads.clone();
        assert!(!observe_window_until(
            Instant::now(),
            Instant::now,
            |_| panic!("Expired observation must not be dispatched"),
            move || {
                sample.store(true, Ordering::Release);
                true
            }
        ));
        let sample = reads.clone();
        assert!(!observe_window_until(
            Instant::now() + Duration::from_secs(1),
            Instant::now,
            |_| Err(()),
            move || {
                sample.store(true, Ordering::Release);
                true
            }
        ));
        assert!(!reads.load(Ordering::Acquire));
    }
    #[test]
    fn unexecuted_or_late_main_callback_has_no_read_or_handoff_authority() {
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};
        let queued: Arc<Mutex<Option<WindowReadJob>>> = Arc::new(Mutex::new(None));
        let captured = queued.clone();
        let reads = Arc::new(AtomicBool::new(false));
        let sample = reads.clone();
        let start = Instant::now();
        assert!(!observe_window_until(
            start + Duration::from_millis(1),
            move || start,
            move |read| {
                *captured.lock().unwrap() = Some(read);
                Ok(())
            },
            move || {
                sample.store(true, Ordering::Release);
                true
            }
        ));
        assert!(!reads.load(Ordering::Acquire));
        // Run the actual captured job after the receiver refused. Its lifetime
        // guard is revoked: it does not even invoke the readonly URL sampler.
        queued.lock().unwrap().take().unwrap()();
        assert!(!reads.load(Ordering::Acquire));
    }
    #[test]
    fn immediate_main_observation_can_succeed_without_opening_anything() {
        use std::time::{Duration, Instant};
        let start = Instant::now();
        assert!(observe_window_until(
            start + Duration::from_secs(1),
            move || start,
            |read| {
                read();
                Ok(())
            },
            || true
        ));
        assert!(!observe_window_until(
            start + Duration::from_secs(1),
            move || start,
            |read| {
                read();
                Ok(())
            },
            || false
        ));
    }
    #[test]
    fn sampler_that_finishes_at_deadline_cannot_restore_trust_or_renew_window() {
        use std::sync::{atomic::AtomicU64, Arc};
        use std::time::{Duration, Instant};
        let start = Instant::now();
        let elapsed = Arc::new(AtomicU64::new(0));
        let clock = elapsed.clone();
        let sample = elapsed.clone();
        assert!(!observe_window_until(
            start + Duration::from_millis(10),
            move || start + Duration::from_millis(clock.load(Ordering::Acquire)),
            |read| {
                read();
                Ok(())
            },
            move || {
                sample.store(10, Ordering::Release);
                true
            }
        ));
        assert_eq!(elapsed.load(Ordering::Acquire), 10);
    }
    #[test]
    fn separate_absolute_directory_argument_and_empty_child_environment_survive_special_text() {
        for path in [
            "/tmp/space directory",
            "/tmp/你好",
            "/tmp/new\nline",
            "/tmp/-leading;$(agent)",
        ] {
            let command = opener_command(path);
            assert_eq!(command.get_program(), std::ffi::OsStr::new(OPENER));
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                vec![
                    std::ffi::OsStr::new("-a"),
                    std::ffi::OsStr::new(TERMINAL),
                    std::ffi::OsStr::new(path)
                ]
            );
            assert_eq!(
                command.get_envs().collect::<Vec<_>>(),
                vec![(
                    std::ffi::OsStr::new("PATH"),
                    Some(std::ffi::OsStr::new("/usr/bin:/bin:/usr/sbin:/sbin"))
                )]
            );
        }
    }
    #[test]
    fn only_zero_exit_inside_original_wait_proves_handoff_and_never_terminal_cwd() {
        assert_eq!(
            completed(true, true),
            json!({"status":"handoff_accepted","code":"accepted"})
        );
        assert_eq!(completed(false, true), unknown("opener_failed"));
        assert_eq!(completed(true, false), unknown("opener_timeout"));
        assert_eq!(completed(false, false), unknown("opener_timeout"));
        for value in [
            refused("opener_timeout"),
            json!({"status":"handoff_accepted","code":"opener_failed"}),
            json!({"status":"handoff_accepted","code":"accepted","session":"private"}),
        ] {
            assert!(!closed_result(&value));
        }
    }
}
