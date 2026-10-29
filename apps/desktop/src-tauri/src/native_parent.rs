//! Caller-owned production dialogs. The web renderer never supplies a parent,
//! window label, native handle, confirmation result or private finalize method.
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    mpsc, Arc, Mutex, OnceLock, Weak,
};
use std::time::{Duration, Instant};
use tauri::{Manager, WebviewWindow, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

const REVIEWING: u8 = 1;
const STARTED: u8 = 2;
const INVALID: u8 = 4;
const CLOSE_REQUESTED: u8 = 8;
const DESTROYED: u8 = 16;
const FINISHED: u8 = 32;
const MAIN_THREAD_DEADLINE: Duration = Duration::from_secs(5);

// Presentation bounds are independent of approval snapshot bounds. This permits
// the existing full 1MiB snapshot plus its host-authored explanatory envelope.
pub(super) const REVIEW_MESSAGE_BYTES: usize = 1_048_576 + 32_768;
pub(super) const REVIEW_ROW_ENCODED_BYTES: usize = 40;
pub(super) const REVIEW_ROWS_PER_PAGE: usize = 8;

/// Entire original message, reversibly encoded. Every display line has a
/// non-whitespace prefix: native text layout cannot eat original indentation.
/// JSON escapes make controls/bidi explicit; ordinary Chinese/emoji stay readable.
/// UTF-8 byte limits bound each row without splitting a Unicode scalar.
fn review_prompts(message: &str) -> Result<Vec<String>, String> {
    if message.len() > REVIEW_MESSAGE_BYTES {
        return Err("Complete native message exceeds the bounded presentation envelope".into());
    }
    let mut rows = Vec::new();
    let mut encoded = String::new();
    let mut start = 0;
    let mut end = 0;
    for (offset, ch) in message.char_indices() {
        let escaped = if ch == '\u{061c}'
            || ch == '\x7f'
            || ('\u{0080}'..='\u{009f}').contains(&ch)
            || ('\u{200b}'..='\u{200f}').contains(&ch)
            || ('\u{2028}'..='\u{202e}').contains(&ch)
            || ('\u{2060}'..='\u{206f}').contains(&ch)
            || ch == '\u{feff}'
        {
            format!("\\u{:04x}", ch as u32)
        } else {
            let quoted = serde_json::to_string(&ch.to_string()).map_err(|e| e.to_string())?;
            quoted[1..quoted.len() - 1].to_owned()
        };
        if !encoded.is_empty() && encoded.len() + escaped.len() > REVIEW_ROW_ENCODED_BYTES {
            rows.push((start, offset, std::mem::take(&mut encoded)));
            start = offset;
        }
        encoded.push_str(&escaped);
        end = offset + ch.len_utf8();
    }
    if !encoded.is_empty() || rows.is_empty() {
        rows.push((start, end, encoded));
    }
    // Verify the exact roundtrip before opening even the first native page.
    let mut restored = String::new();
    for (start, end, encoded) in &rows {
        let decoded: String = serde_json::from_str(&format!("\"{encoded}\""))
            .map_err(|_| "Native review encoding could not be restored")?;
        if message.get(*start..*end) != Some(decoded.as_str()) {
            return Err("Native review encoding changed original bytes".into());
        }
        restored.push_str(&decoded);
    }
    if restored != message {
        return Err("Native review encoding omitted original bytes".into());
    }
    let total = rows.len().div_ceil(REVIEW_ROWS_PER_PAGE);
    let mut prompts = Vec::with_capacity(total + 1);
    for (index, page) in rows.chunks(REVIEW_ROWS_PER_PAGE).enumerate() {
        let body = page
            .iter()
            .enumerate()
            .map(|(row, (_, _, text))| {
                format!(
                    "L{:05} \"{}\"",
                    index * REVIEW_ROWS_PER_PAGE + row + 1,
                    text
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        prompts.push(format!("Native review page {}/{}\nOriginal UTF-8 bytes {}..{} of {}.\nRead JSON strings in L order; escapes preserve ALL text.\nOK reads this page only. Cancel stops this review.\n{}", index + 1, total, page[0].0, page.last().unwrap().1, message.len(), body));
    }
    prompts.push(format!("FINAL display confirmation\nAll {} pages / {} original UTF-8 bytes were reviewed.\nOK confirms the original request exactly as displayed.\nAny further required reviews still apply.\nCancel stops without private approval.", total, message.len()));
    Ok(prompts)
}

fn present_complete(message: &str, mut show: impl FnMut(&str) -> bool) -> Result<bool, String> {
    let prompts = review_prompts(message)?;
    Ok(prompts.iter().all(|prompt| show(prompt)))
}

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
            raw_window_handle::RawWindowHandle::AppKit(_)
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
        match present_complete(message, |page| self.confirm_page(title, page)) {
            Ok(confirmed) => confirmed,
            Err(error) => {
                self.fail(error);
                false
            }
        }
    }
    fn confirm_page(&self, title: &str, message: &str) -> bool {
        let (sender, receiver) = mpsc::sync_channel(1);
        let caller = self.caller.clone();
        let life = self.life.clone();
        let title = title.to_owned();
        let message = message.to_owned();
        let review_started = Arc::new(AtomicBool::new(false));
        let started_on_main = review_started.clone();
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
            // This flag is monotonic for this one page. Lifecycle returns to
            // idle after a human response; it is NOT a startup-timeout signal.
            started_on_main.store(true, Ordering::Release);
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
            if !review_started.load(Ordering::Acquire)
                && waiting_since.elapsed() >= MAIN_THREAD_DEADLINE
                && self.life.expire_waiting()
            {
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
    let message = format!("{message}. This invocation did not dispatch private finalize/decline. Existing pending proposals and previously committed history were not rolled back. Verify using the original idempotency key.");
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

    fn restore_prompts(prompts: &[String]) -> String {
        prompts[..prompts.len() - 1]
            .iter()
            .flat_map(|page| page.lines())
            .filter(|line| line.starts_with('L'))
            .map(|line| serde_json::from_str::<String>(line.split_once(' ').unwrap().1).unwrap())
            .collect()
    }
    #[test]
    fn bounded_utf8_pages_preserve_indent_controls_unicode_and_complete_bytes() {
        let original = "  first\n\t\t你好🔒\r\n\0\u{007f}\u{202e}last  ";
        let prompts = review_prompts(original).unwrap();
        assert_eq!(restore_prompts(&prompts), original);
        assert!(prompts
            .last()
            .unwrap()
            .starts_with("FINAL display confirmation"));
        for page in &prompts {
            assert!(page.len() < 1024);
            assert!(page.lines().count() <= REVIEW_ROWS_PER_PAGE + 4);
            assert!(page
                .lines()
                .all(|line| !line.starts_with(char::is_whitespace)));
        }
        assert!(prompts.concat().contains("\\u202e"));
        assert!(prompts.concat().contains("你好🔒"));
    }
    #[test]
    fn all_twelve_unicode_bidi_controls_are_visible_escaped_and_lossless() {
        let controls = [
            '\u{061c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
            '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        ];
        assert_eq!(controls.len(), 12);
        for control in controls {
            let original = format!("recipient: abc{control}123 你好🔒");
            let prompts = review_prompts(&original).unwrap();
            let displayed = prompts.concat();
            assert!(!displayed.contains(control));
            assert!(displayed.contains(&format!("\\u{:04x}", control as u32)));
            assert!(displayed.contains("你好🔒"));
            assert_eq!(restore_prompts(&prompts), original);
        }
    }
    #[test]
    fn original_large_limits_are_preserved_and_never_truncated() {
        for bytes in [0, 16_384, 1_048_576, REVIEW_MESSAGE_BYTES] {
            let original = "x".repeat(bytes);
            let prompts = review_prompts(&original).unwrap();
            assert_eq!(restore_prompts(&prompts), original);
            assert!(prompts.iter().all(|page| page.len() < 1024));
        }
        let mut calls = 0;
        assert!(
            present_complete(&"x".repeat(REVIEW_MESSAGE_BYTES + 1), |_| {
                calls += 1;
                true
            })
            .is_err()
        );
        assert_eq!(calls, 0);
    }
    #[test]
    fn cancel_any_page_or_final_short_circuits_without_private_dispatch() {
        let original = "    bound proposal \n".repeat(80);
        let count = review_prompts(&original).unwrap().len();
        assert!(count > 2);
        for cancel_at in 0..count {
            let mut calls = 0;
            let confirmed = present_complete(&original, |_| {
                let index = calls;
                calls += 1;
                index != cancel_at
            })
            .unwrap();
            assert!(!confirmed);
            assert_eq!(calls, cancel_at + 1);
        }
        let mut shown = Vec::new();
        assert!(present_complete(&original, |page| {
            shown.push(page.to_owned());
            true
        })
        .unwrap());
        assert_eq!(shown, review_prompts(&original).unwrap());
    }
    #[test]
    fn canonical_json_row_vector_matches_the_independent_host_runner() {
        let prompts = review_prompts(" \t你好🔒\n").unwrap();
        assert_eq!(prompts[0], "Native review page 1/1\nOriginal UTF-8 bytes 0..13 of 13.\nRead JSON strings in L order; escapes preserve ALL text.\nOK reads this page only. Cancel stops this review.\nL00001 \" \\t你好🔒\\n\"");
    }
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
    fn human_completion_idle_state_is_not_a_startup_timeout_signal() {
        let life = Lifecycle::default();
        let review_started = AtomicBool::new(false);
        assert!(life.begin_review());
        review_started.store(true, Ordering::Release);
        assert!(life.complete_review());
        // Even after the startup deadline, a queued genuine callback remains
        // valid: this page already started, despite the idle lifecycle state.
        let expired = !review_started.load(Ordering::Acquire) && life.expire_waiting();
        assert!(!expired);
        assert!(life.begin_finalize());
    }
    #[test]
    fn unavailable_before_dispatch_is_an_explicit_non_authority_result() {
        let result = unavailable("no caller");
        assert_eq!(result["error"]["code"], "NATIVE_PARENT_UNAVAILABLE");
        assert_eq!(result["error"]["side_effect_state"], "not_started");
        assert!(result.get("result").is_none());
    }
}
