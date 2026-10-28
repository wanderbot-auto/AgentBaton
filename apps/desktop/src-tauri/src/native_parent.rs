//! Caller-owned production dialogs. The web renderer never supplies a parent,
//! window label, native handle, confirmation result or private finalize method.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    mpsc, Arc, Mutex, OnceLock, Weak,
};
use std::time::{Duration, Instant};
use tauri::wry::raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use tauri::{Manager, WebviewWindow, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

const REVIEWING: u8 = 1;
const STARTED: u8 = 2;
const INVALID: u8 = 4;
const CLOSE_REQUESTED: u8 = 8;
const DESTROYED: u8 = 16;
const FINISHED: u8 = 32;
const MAIN_THREAD_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Lifecycle {
    state: AtomicU8,
}
impl Lifecycle {
    fn available(&self) -> bool {
        self.state.load(Ordering::Acquire) == 0
    }
    fn begin_review(&self) -> bool {
        self.state
            .compare_exchange(0, REVIEWING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn complete_review(&self) -> bool {
        self.state
            .compare_exchange(REVIEWING, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn expire_waiting(&self) -> bool {
        self.state
            .compare_exchange(0, INVALID, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn begin_finalize(&self) -> bool {
        self.state
            .compare_exchange(0, STARTED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    fn invalidate(&self, destroyed: bool) -> bool {
        let flag = INVALID
            | if destroyed {
                DESTROYED
            } else {
                CLOSE_REQUESTED
            };
        let previous = self.state.fetch_or(flag, Ordering::AcqRel);
        // Ordinary close is deferred only once the owner already confirmed and
        // the private dispatch gate linearized. It is never deferred in review.
        !destroyed && previous & STARTED != 0 && previous & FINISHED == 0
    }
    fn invalid(&self) -> bool {
        self.state.load(Ordering::Acquire) & INVALID != 0
    }
    fn started(&self) -> bool {
        self.state.load(Ordering::Acquire) & STARTED != 0
    }
    fn finish(&self) -> bool {
        let before = self.state.fetch_or(FINISHED, Ordering::AcqRel);
        before & STARTED != 0 && before & CLOSE_REQUESTED != 0 && before & DESTROYED == 0
    }
}

/// Prevent stacked simultaneous sheets for one framework-owned window. The
/// label is obtained from that actual injected object and is ONLY an exclusion
/// key; it is never used to find/select/replace the dialog's parent.
struct Slot {
    label: String,
    life: Weak<Lifecycle>,
}
fn slots() -> &'static Mutex<HashMap<String, Weak<Lifecycle>>> {
    static SLOTS: OnceLock<Mutex<HashMap<String, Weak<Lifecycle>>>> = OnceLock::new();
    SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}
impl Slot {
    fn acquire(label: &str, life: &Arc<Lifecycle>) -> Result<Self, String> {
        let mut active = slots()
            .lock()
            .map_err(|_| "Native approval ownership lock unavailable")?;
        if active.contains_key(label) {
            return Err("A native approval already owns this caller window".into());
        }
        let life = Arc::downgrade(life);
        active.insert(label.to_owned(), life.clone());
        Ok(Self {
            label: label.to_owned(),
            life,
        })
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        if let Ok(mut active) = slots().lock() {
            if active
                .get(&self.label)
                .is_some_and(|life| life.ptr_eq(&self.life))
            {
                active.remove(&self.label);
            }
        }
    }
}

/// Installed once by the shipping/integration shared builder before windows
/// run. No asynchronous per-operation listener registration or leaked handlers.
/// The framework event label routes invalidation only; it never selects a
/// parent, supplies a handle or grants consent. Reused/stale labels can only
/// make an operation fail closed.
pub(crate) fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    let life = slots()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(window.label())
        .and_then(Weak::upgrade);
    if let Some(life) = life {
        match event {
            WindowEvent::CloseRequested { api, .. } => {
                if life.invalidate(false) {
                    api.prevent_close();
                }
            }
            WindowEvent::Destroyed => {
                life.invalidate(true);
            }
            _ => {}
        }
    }
}

/// These are the same two successful borrowed handles passed to parent().
/// Plugin.parent cannot silently re-query the caller and fall back to None.
struct CheckedParent<'a> {
    window: WindowHandle<'a>,
    display: DisplayHandle<'a>,
}
impl<'a> CheckedParent<'a> {
    fn acquire(caller: &'a WebviewWindow) -> Result<Self, String> {
        let window = caller
            .window_handle()
            .map_err(|error| format!("Caller window handle unavailable: {error}"))?;
        #[cfg(target_os = "macos")]
        if !matches!(
            window.as_raw(),
            tauri::wry::raw_window_handle::RawWindowHandle::AppKit(_)
        ) {
            return Err("Calling window did not supply an AppKit parent".into());
        }
        let display = caller
            .display_handle()
            .map_err(|error| format!("Caller display handle unavailable: {error}"))?;
        Ok(Self { window, display })
    }
}
impl HasWindowHandle for CheckedParent<'_> {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(self.window)
    }
}
impl HasDisplayHandle for CheckedParent<'_> {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(self.display)
    }
}

pub(crate) struct CallerOwner {
    caller: WebviewWindow,
    life: Arc<Lifecycle>,
    fault: Mutex<Option<String>>,
    _slot: Slot,
}
fn current_parent<'a>(
    caller: &'a WebviewWindow,
    life: &Lifecycle,
) -> Result<CheckedParent<'a>, String> {
    if !life.available() {
        return Err("Calling window closed or this approval is no longer active".into());
    }
    if !caller
        .is_visible()
        .map_err(|error| format!("Calling window is unavailable: {error}"))?
    {
        return Err("Calling window is not visible; no native approval was opened".into());
    }
    CheckedParent::acquire(caller)
}
impl CallerOwner {
    fn acquire(caller: WebviewWindow) -> Result<Self, String> {
        let life = Arc::new(Lifecycle::default());
        let slot = Slot::acquire(caller.label(), &life)?;
        let owner = Self {
            caller,
            life,
            fault: Mutex::new(None),
            _slot: slot,
        };
        owner.check(false)?;
        Ok(owner)
    }
    fn check(&self, finalizing: bool) -> Result<(), String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let caller = self.caller.clone();
        let life = self.life.clone();
        self.caller
            .run_on_main_thread(move || {
                let outcome = current_parent(&caller, &life).and_then(|_parent| {
                    if finalizing && !life.begin_finalize() {
                        Err("Caller closed before private finalization".into())
                    } else {
                        Ok(())
                    }
                });
                let _ = sender.send(outcome);
            })
            .map_err(|error| format!("Cannot validate caller on the UI thread: {error}"))?;
        receiver.recv_timeout(MAIN_THREAD_DEADLINE).map_err(|_| {
            self.life.state.fetch_or(INVALID, Ordering::AcqRel);
            "Caller validation did not complete; no private action was dispatched".to_string()
        })?
    }
    fn fail(&self, message: String) {
        self.life.state.fetch_or(INVALID, Ordering::AcqRel);
        if let Ok(mut fault) = self.fault.lock() {
            if fault.is_none() {
                *fault = Some(message);
            }
        }
    }
    pub(crate) fn confirm(&self, title: &str, message: &str) -> bool {
        let (sender, receiver) = mpsc::sync_channel(1);
        let caller = self.caller.clone();
        let life = self.life.clone();
        let title = title.to_owned();
        let message = message.to_owned();
        if let Err(error) = self.caller.run_on_main_thread(move || {
            // Wry executes plugin's nested run_on_main_thread inline here.
            // rfd retains NSWindow while constructing the real sheet before
            // this callback returns and another GUI event can close it.
            let prepared = current_parent(&caller, &life);
            let parent = match prepared {
                Ok(parent) => parent,
                Err(error) => {
                    let _ = sender.send(Err(error));
                    return;
                }
            };
            if !life.begin_review() {
                let _ = sender.send(Err("Caller closed before native review".into()));
                return;
            }
            let keep_caller = caller.clone();
            caller
                .app_handle()
                .dialog()
                .message(message)
                .title(title)
                .parent(&parent)
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancel)
                .show(move |confirmed| {
                    // Keep the actual framework caller alive through the real
                    // completion, in addition to rfd's retained AppKit parent.
                    let _caller = &keep_caller;
                    let outcome = if life.complete_review() {
                        Ok(confirmed)
                    } else {
                        Err("Caller closed during native review; confirmation is invalid".into())
                    };
                    let _ = sender.send(outcome);
                });
        }) {
            self.fail(format!("Cannot show caller-owned native approval: {error}"));
            return false;
        }
        let waiting_since = Instant::now();
        loop {
            if waiting_since.elapsed() >= MAIN_THREAD_DEADLINE && self.life.expire_waiting() {
                self.fail("Caller review did not start on the UI thread".into());
                return false;
            }
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(confirmed)) => return confirmed && !self.life.invalid(),
                Ok(Err(error)) => {
                    self.fail(error);
                    return false;
                }
                Err(mpsc::RecvTimeoutError::Timeout) if !self.life.invalid() => continue,
                Err(_) => {
                    self.fail(
                        "Calling window closed or native dialog completion became unavailable"
                            .into(),
                    );
                    return false;
                }
            }
        }
    }
    pub(crate) fn finalize(
        &self,
        send: impl FnOnce() -> Result<Value, String>,
    ) -> Result<Value, String> {
        if let Err(error) = self.check(true) {
            self.fail(error.clone());
            return Ok(unavailable(&error));
        }
        // This is the linearization point: caller valid and owner consented.
        // A close after this point may race a committed Core request; never
        // relabel that Core result/transport error as definitely not started.
        send()
    }
    fn finish(&self, result: Result<Value, String>) -> Result<Value, String> {
        if !self.life.started() && self.life.invalid() {
            let message = self
                .fault
                .lock()
                .ok()
                .and_then(|fault| fault.clone())
                .unwrap_or_else(|| "Calling window closed before private finalization".into());
            return Ok(unavailable(&message));
        }
        result
    }
}
impl Drop for CallerOwner {
    fn drop(&mut self) {
        if self.life.finish() {
            let _ = self.caller.close();
        }
    }
}
pub(crate) fn unavailable(message: &str) -> Value {
    json!({"request_id":null,"error":{"code":"NATIVE_PARENT_UNAVAILABLE","message":message,"retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":0,"warnings":[]})
}
pub(crate) fn with_caller(
    caller: WebviewWindow,
    operation: impl FnOnce(&CallerOwner) -> Result<Value, String>,
) -> Result<Value, String> {
    let owner = match CallerOwner::acquire(caller) {
        Ok(owner) => owner,
        Err(error) => return Ok(unavailable(&error)),
    };
    let result = operation(&owner);
    owner.finish(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn close_before_proposal_or_review_never_opens_a_dispatch_gate() {
        let life = Lifecycle::default();
        assert!(!life.invalidate(false));
        assert!(!life.available());
        assert!(!life.begin_review());
        assert!(!life.begin_finalize());
        assert!(!life.started());
    }
    #[test]
    fn close_during_or_between_pages_never_restores_confirmation() {
        let life = Lifecycle::default();
        assert!(life.begin_review());
        life.invalidate(true);
        assert!(!life.complete_review());
        assert!(!life.begin_review());
        assert!(!life.begin_finalize());
        assert!(!life.started());
    }
    #[test]
    fn close_after_last_page_before_finalize_fails_closed() {
        let life = Lifecycle::default();
        assert!(life.begin_review());
        assert!(life.complete_review());
        life.invalidate(false);
        assert!(!life.begin_finalize());
        assert!(!life.started());
    }
    #[test]
    fn genuine_completion_can_dispatch_once_but_never_twice() {
        let life = Lifecycle::default();
        assert!(life.begin_review());
        assert!(life.complete_review());
        assert!(life.begin_finalize());
        assert!(life.started());
        assert!(!life.begin_finalize());
        assert!(!life.begin_review());
    }
    #[test]
    fn close_after_dispatch_is_deferred_and_does_not_erase_possible_commit() {
        let life = Lifecycle::default();
        assert!(life.begin_finalize());
        assert!(life.invalidate(false));
        assert!(life.started());
        assert!(life.finish());
        assert!(life.started());
    }
    #[test]
    fn forced_destruction_after_dispatch_is_not_a_not_started_proof() {
        let life = Lifecycle::default();
        assert!(life.begin_finalize());
        assert!(!life.invalidate(true));
        assert!(life.started());
        assert!(!life.finish());
    }
    #[test]
    fn actual_framework_label_is_only_a_concurrency_key_not_a_parent_selector() {
        let life = Arc::new(Lifecycle::default());
        let first = Slot::acquire("unit-exclusive-window", &life).unwrap();
        assert!(Slot::acquire("unit-exclusive-window", &life).is_err());
        drop(first);
        assert!(Slot::acquire("unit-exclusive-window", &life).is_ok());
    }
    #[test]
    fn startup_deadline_cannot_later_open_a_review_or_dispatch() {
        let life = Lifecycle::default();
        assert!(life.expire_waiting());
        assert!(!life.begin_review());
        assert!(!life.begin_finalize());
        let reviewing = Lifecycle::default();
        assert!(reviewing.begin_review());
        assert!(!reviewing.expire_waiting()); // no deadline on human review
        assert!(reviewing.complete_review());
    }
    #[test]
    fn unavailable_before_dispatch_is_an_explicit_non_authority_result() {
        let result = unavailable("no caller");
        assert_eq!(result["error"]["code"], "NATIVE_PARENT_UNAVAILABLE");
        assert_eq!(result["error"]["side_effect_state"], "not_started");
        assert!(result.get("result").is_none());
    }
}
