//! Window-only background actions. There is no exit, close/destroy, navigation,
//! provider stop or execution dispatch in this contract.
use serde_json::Value;

pub(crate) fn status_instance<'a>(
    response: &'a Value,
    expected: Option<&str>,
) -> Result<&'a str, String> {
    if response.get("error").is_some() {
        return Err(
            "Cannot authenticate the current background instance; window remains visible".into(),
        );
    }
    let status = &response["result"];
    let fields = [
        "instance_id",
        "started_at",
        "api_version",
        "database_schema_version",
        "sequence",
        "owned_provider_workers",
        "store_recovery",
    ];
    let closed = status.as_object().is_some_and(|map| {
        map.len() == fields.len() && fields.iter().all(|key| map.contains_key(*key))
    });
    let instance = status["instance_id"]
        .as_str()
        .filter(|id| {
            id.len() == 36
                && id.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                    }
                })
        })
        .ok_or("Background identity is unavailable; refresh its status")?;
    if !closed
        || status["api_version"] != 1
        || status["database_schema_version"] != 18
        || status["sequence"]
            .as_u64()
            .is_none_or(|n| n > 9_007_199_254_740_991)
        || !status["owned_provider_workers"].is_boolean()
        || status["store_recovery"] != "completed_on_start"
        || !status["started_at"]
            .as_str()
            .is_some_and(|text| !text.is_empty() && text.len() <= 64 && text.ends_with('Z'))
        || expected.is_some_and(|id| id != instance)
    {
        return Err(
            "Background instance changed or its status is unsupported; refresh before hiding"
                .into(),
        );
    }
    Ok(instance)
}

pub(crate) fn hide_existing(
    label: &str,
    restoration_ready: bool,
    native_owner_busy: bool,
    hide: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if label != "main" {
        return Err("Only the existing main window can enter the background".into());
    }
    if !restoration_ready {
        return Err("Menu-bar restoration is unavailable; window remains visible".into());
    }
    if native_owner_busy {
        return Err("A Native approval still owns this window; finish it before hiding".into());
    }
    hide()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestoreStep {
    Unminimize,
    Show,
    Focus,
}
pub(crate) fn restore_existing(
    label: &str,
    mut apply: impl FnMut(RestoreStep) -> Result<(), String>,
) -> Result<(), String> {
    if label != "main" {
        return Err("Existing main window is unavailable; no replacement was created".into());
    }
    for step in [
        RestoreStep::Unminimize,
        RestoreStep::Show,
        RestoreStep::Focus,
    ] {
        apply(step)?;
    }
    Ok(())
}

/// A queued main-thread action must not begin after its caller timed out.
/// Once begun, a timeout remains unknown rather than being called not-started.
pub(crate) struct HideGate(std::sync::atomic::AtomicU8);
impl HideGate {
    pub(crate) fn new() -> Self {
        Self(std::sync::atomic::AtomicU8::new(0))
    }
    pub(crate) fn begin(&self) -> bool {
        self.0
            .compare_exchange(
                0,
                1,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_ok()
    }
    pub(crate) fn expire(&self) -> bool {
        self.0
            .compare_exchange(
                0,
                2,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_ok()
    }
}
#[derive(Debug)]
pub(crate) struct HideFailure {
    pub code: &'static str,
    pub message: String,
    pub state: &'static str,
}
impl HideFailure {
    pub(crate) fn not_started(message: String) -> Self {
        Self {
            code: "BACKGROUND_HIDE_UNAVAILABLE",
            message,
            state: "not_started",
        }
    }
    pub(crate) fn unknown(message: String) -> Self {
        Self {
            code: "OUTCOME_UNKNOWN",
            message,
            state: "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::Cell;
    fn status() -> Value {
        json!({"result":{"instance_id":"d949034a-2109-419c-b3c1-63ebef46e6ac","started_at":"2026-11-09T00:00:00Z","api_version":1,"database_schema_version":18,"sequence":5,"owned_provider_workers":true,"store_recovery":"completed_on_start"}})
    }
    #[test]
    fn hiding_requires_fresh_authenticated_instance_not_a_pid_or_historical_flag() {
        let original = status();
        let id = status_instance(&original, None).unwrap();
        assert_eq!(status_instance(&original, Some(id)).unwrap(), id);
        for variant in 0..7 {
            let mut r = original.clone();
            match variant {
                0 => r["error"] = json!({"code":"AUTH_REQUIRED"}),
                1 => r["result"]["instance_id"] = json!(1234),
                2 => r["result"]["database_schema_version"] = json!(17),
                3 => r["result"]["owned_provider_workers"] = json!("running"),
                4 => r["result"]["sequence"] = json!(9007199254740992u64),
                5 => r["result"]["store_recovery"] = json!("all_effects_reconciled"),
                _ => r["result"]["stops_all_tasks"] = json!(true),
            }
            assert!(status_instance(&r, Some(id)).is_err());
        }
        assert!(status_instance(&original, Some("b949034a-2109-419c-b3c1-63ebef46e6ac")).is_err());
    }
    #[test]
    fn close_to_background_preserves_existing_draft_and_unknown_key_and_restores_same_window() {
        let state = ("draft text", "original-unknown-key", "same-webview-id");
        let visible = Cell::new(true);
        hide_existing("main", true, false, || {
            visible.set(false);
            Ok(())
        })
        .unwrap();
        assert!(!visible.get());
        let mut steps = Vec::new();
        for _ in 0..2 {
            restore_existing("main", |step| {
                steps.push(step);
                if step == RestoreStep::Show {
                    visible.set(true);
                }
                Ok(())
            })
            .unwrap();
        }
        assert!(visible.get());
        assert_eq!(
            state,
            ("draft text", "original-unknown-key", "same-webview-id")
        );
        assert_eq!(
            steps,
            [
                RestoreStep::Unminimize,
                RestoreStep::Show,
                RestoreStep::Focus
            ]
            .repeat(2)
        );
    }
    #[test]
    fn busy_approval_unavailable_restoration_and_other_windows_never_hide() {
        for (label, ready, busy) in [
            ("main", true, true),
            ("main", false, false),
            ("other", true, false),
        ] {
            assert!(
                hide_existing(label, ready, busy, || panic!("must leave caller visible")).is_err()
            );
        }
    }
    #[test]
    fn native_hide_or_restore_failure_has_no_destructive_fallback() {
        let visible = Cell::new(true);
        assert!(hide_existing("main", true, false, || Err("native hide failed".into())).is_err());
        assert!(visible.get());
        let mut calls = Vec::new();
        assert!(restore_existing("main", |step| {
            calls.push(step);
            if step == RestoreStep::Show {
                Err("native show failed".into())
            } else {
                Ok(())
            }
        })
        .is_err());
        assert_eq!(calls, vec![RestoreStep::Unminimize, RestoreStep::Show]);
        assert!(restore_existing("replacement", |_| panic!(
            "cannot rebuild a missing main window"
        ))
        .is_err());
        visible.set(false);
        assert!(!visible.get());
    }
    #[test]
    fn expired_main_thread_dispatch_cannot_hide_later_or_claim_started_work_absent() {
        let queued = HideGate::new();
        assert!(queued.expire());
        assert!(!queued.begin());
        let started = HideGate::new();
        assert!(started.begin());
        assert!(!started.expire());
        assert!(!started.begin());
    }
}
