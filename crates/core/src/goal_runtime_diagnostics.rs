//! Explicit debug-only scalar observations. This module is not compiled into
//! release builds and never produces an authority value or changes scheduling.
use std::{
    collections::{BTreeMap, VecDeque},
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const MAX_LINES: usize = 512;
const MAX_BYTES: usize = 131072;
const MAX_LINE: usize = 256;
const PER_STAGE: usize = 8;
static ACTOR_ACTIVE: AtomicBool = AtomicBool::new(false);
pub(crate) fn hashing_enabled() -> bool {
    ACTOR_ACTIVE.load(Ordering::Relaxed) && enabled()
}
#[derive(Clone, PartialEq, Eq)]
struct Signature {
    flags: [bool; 6],
    count: usize,
    code: &'static str,
}
struct Stage {
    signature: Signature,
    last: Instant,
    immediate: usize,
    tail: VecDeque<(u64, String)>,
}
struct Trace {
    start: Instant,
    wall_ms: u64,
    lines: usize,
    bytes: usize,
    last_read: Option<Instant>,
    stages: BTreeMap<&'static str, Stage>,
}
fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}
impl Trace {
    fn new(now: Instant) -> Self {
        Self {
            start: now,
            wall_ms: millis(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default(),
            ),
            lines: 0,
            bytes: 0,
            last_read: None,
            stages: BTreeMap::new(),
        }
    }
    fn reserve(&mut self, line: &str) -> bool {
        if line.len() > MAX_LINE
            || self.lines >= MAX_LINES
            || self.bytes.saturating_add(line.len()) > MAX_BYTES
        {
            return false;
        }
        self.lines += 1;
        self.bytes += line.len();
        true
    }
    fn record(
        &mut self,
        stage: &'static str,
        now: Instant,
        mut elapsed: Option<Duration>,
        flags: [bool; 6],
        count: usize,
        code: &'static str,
    ) -> Option<String> {
        if !allowed_stage(stage) {
            return None;
        }
        if stage == "input_read_issued" {
            self.last_read = Some(now);
        }
        if stage == "input_read_done" {
            elapsed = self
                .last_read
                .take()
                .map(|start| now.saturating_duration_since(start));
        }
        let signature = Signature { flags, count, code };
        if self.stages.get(stage).is_some_and(|old| {
            old.signature == signature
                && (elapsed.is_none_or(|d| d < Duration::from_millis(100))
                    || now.saturating_duration_since(old.last) < Duration::from_secs(1))
        }) {
            return None;
        }
        let at = millis(now.saturating_duration_since(self.start));
        let valid = match stage {
            "input_read_issued" => 3,
            "provider_prompt_failure" => 7,
            "companion_read_hash"
            | "companion_sha256"
            | "mcp_external_recheck"
            | "mcp_provider_verify"
            | "factory_query_check"
            | "factory_git_snapshot"
            | "factory_phase"
            | "factory_admission_phase"
            | "goal_budget_denied" => 0,
            _ => 63,
        };
        let line=format!("AB_GOAL_DIAG stage={stage} wall_ms={} at_ms={at} elapsed_ms={} flags={}{}{}{}{}{} valid={valid} count={count} code={code}\n",self.wall_ms,elapsed.map(millis).unwrap_or(0),u8::from(flags[0]),u8::from(flags[1]),u8::from(flags[2]),u8::from(flags[3]),u8::from(flags[4]),u8::from(flags[5]));
        if line.len() > MAX_LINE {
            return None;
        }
        let bucket = self.stages.entry(stage).or_insert_with(|| Stage {
            signature: signature.clone(),
            last: now,
            immediate: 0,
            tail: VecDeque::new(),
        });
        bucket.signature = signature;
        bucket.last = now;
        if bucket.immediate < PER_STAGE {
            bucket.immediate += 1;
            return self.reserve(&line).then_some(line);
        }
        if bucket.tail.len() == PER_STAGE {
            bucket.tail.pop_front();
        }
        bucket.tail.push_back((at, line));
        None
    }
    fn drain(&mut self) -> Vec<String> {
        let mut rows = self
            .stages
            .values_mut()
            .flat_map(|s| s.tail.drain(..))
            .collect::<Vec<_>>();
        rows.sort_by_key(|(at, _)| *at);
        rows.into_iter()
            .filter_map(|(_, line)| self.reserve(&line).then_some(line))
            .collect()
    }
}
fn allowed_stage(stage: &str) -> bool {
    matches!(
        stage,
        "actor_run_enter"
            | "companion_read_hash"
            | "companion_sha256"
            | "mcp_external_recheck"
            | "mcp_provider_verify"
            | "factory_query_check"
            | "factory_git_snapshot"
            | "goal_meter_before"
            | "goal_meter_observed"
            | "goal_budget_denied"
            | "provider_prompt_failure"
            | "dispatch_task_rejected"
            | "factory_tick_begin"
            | "factory_tick_end"
            | "factory_phase"
            | "factory_admission_phase"
            | "goal_tick_begin"
            | "goal_tick_end"
            | "goal_after_stops"
            | "goal_pending_empty"
            | "goal_pending_reply"
            | "goal_pending_disconnect"
            | "goal_lane_busy"
            | "goal_needs_advance"
            | "goal_selected"
            | "factory_preflight"
            | "graph_preflight"
            | "data_context"
            | "ready_error"
            | "input_read_issued"
            | "input_read_done"
            | "request_handled"
    )
}
fn error_code(code: Option<&str>) -> &'static str {
    match code {
        None => "none",
        Some("WORKSPACE_BUSY") => "WORKSPACE_BUSY",
        Some("STALE_REVISION") => "STALE_REVISION",
        Some("INPUT_CHANGED") => "INPUT_CHANGED",
        Some("AUTH_REQUIRED") => "AUTH_REQUIRED",
        Some("RESOURCE_LIMIT") => "RESOURCE_LIMIT",
        Some("DEADLINE_EXCEEDED") => "DEADLINE_EXCEEDED",
        Some("OUTCOME_UNKNOWN") => "OUTCOME_UNKNOWN",
        Some("BUDGET_UNKNOWN") => "BUDGET_UNKNOWN",
        Some("INTERNAL_ERROR") => "INTERNAL_ERROR",
        Some("PERMISSION_DENIED") => "PERMISSION_DENIED",
        Some("CAPABILITY_UNSUPPORTED") => "CAPABILITY_UNSUPPORTED",
        Some("CONFIG_CONFLICT") => "CONFIG_CONFLICT",
        Some("INTERNAL_ADVANCE_INTERRUPTED") => "INTERNAL_ADVANCE_INTERRUPTED",
        Some(_) => "other_code",
    }
}
pub(crate) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED
        .get_or_init(|| std::env::var("AGENTBATON_GOAL_DIAGNOSTICS").ok().as_deref() == Some("1"))
}
fn state() -> &'static Mutex<Trace> {
    static STATE: OnceLock<Mutex<Trace>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(Trace::new(Instant::now())))
}
pub(crate) fn emit(
    stage: &'static str,
    elapsed: Option<Duration>,
    flags: [bool; 6],
    count: usize,
    code: Option<&str>,
) {
    if !enabled() {
        return;
    }
    let line = state().lock().ok().and_then(|mut trace| {
        trace.record(
            stage,
            Instant::now(),
            elapsed,
            flags,
            count,
            error_code(code),
        )
    });
    if let Some(line) = line {
        let _ = std::io::stderr().lock().write_all(line.as_bytes());
    }
}
// Fixed enum-like tags only; never output raw method or request text.
pub(crate) fn method_tag(raw: &str) -> usize {
    if !enabled() {
        return 0;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return 0;
    };
    match value["method"].as_str() {
        Some("goal_graph_runs.dispatch") => 1,
        Some("goal_runs.dispatch") => 2,
        Some("factory_runs.get") => 3,
        Some("factory_child_goals.list") => 4,
        Some("factory_child_goals.get") => 5,
        Some("goal_node_approvals.propose") => 6,
        Some("approvals.finalize") => 7,
        Some("approvals.decline") => 8,
        _ => 0,
    }
}
// Classify only exact existing static diagnostics. Never render any message,
// raw wire text, scope identifier or error detail into the scalar trace.
pub(crate) fn budget_reason_tag(message: Option<&str>) -> usize {
    match message {
        Some("Historical actual runtime is unknown") => 1,
        Some("Historical actual output is unknown") => 2,
        Some("Provider token/cost usage or bounded reservation is unavailable") => 3,
        Some("Ancestor token/cost usage or reservation is unknown or exceeds its finite bound") => 4,
        Some("Shared reservation origin or measurement scope is inconsistent; no missing history was treated as zero") => 5,
        Some("Shared reservation is not bound to the actual session/attempt/root") => 6,
        Some("Factory-derived accounting differs from its exact joined/unknown node evidence") => 7,
        Some("Factory-derived Goal reservation lacks its exact atomic node/attempt receipt") => 8,
        Some("Factory ancestor runtime is unknown") => 9,
        Some("Factory ancestor output is unknown") => 10,
        Some("The original shared runtime observation is unknown; do not reset it with a new session/root") => 11,
        Some("An ancestor has unknown runtime") => 12,
        _ => 0,
    }
}
pub(crate) fn provider_failure_tag(kind: agentbaton_providers::transport::FailureKind) -> usize {
    use agentbaton_providers::transport::FailureKind;
    match kind {
        FailureKind::Timeout => 1,
        FailureKind::Protocol => 2,
        FailureKind::Closed => 3,
        FailureKind::Io => 4,
        _ => 0,
    }
}
pub(crate) struct FlushOnDrop;
impl FlushOnDrop {
    pub(crate) fn enter() -> Self {
        if enabled() {
            ACTOR_ACTIVE.store(true, Ordering::Relaxed);
        }
        Self
    }
}
impl Drop for FlushOnDrop {
    fn drop(&mut self) {
        if !enabled() {
            return;
        }
        ACTOR_ACTIVE.store(false, Ordering::Relaxed);
        let rows = state()
            .lock()
            .ok()
            .map(|mut trace| trace.drain())
            .unwrap_or_default();
        for line in rows {
            let _ = std::io::stderr().lock().write_all(line.as_bytes());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_failure_tags_are_fixed_and_never_include_payloads() {
        use agentbaton_providers::transport::FailureKind;
        for (kind, tag) in [
            (FailureKind::Timeout, 1),
            (FailureKind::Protocol, 2),
            (FailureKind::Closed, 3),
            (FailureKind::Io, 4),
            (FailureKind::BinaryChanged, 0),
        ] {
            assert_eq!(provider_failure_tag(kind), tag);
        }
        let now = Instant::now();
        let mut trace = Trace::new(now);
        let line = trace
            .record(
                "provider_prompt_failure",
                now,
                Some(Duration::from_millis(1000)),
                [true, true, true, false, false, false],
                1,
                "none",
            )
            .unwrap();
        assert!(line.contains("flags=111000 valid=7 count=1"));
        assert!(line.len() <= MAX_LINE);
    }
    #[test]
    fn budget_reason_tags_accept_only_fixed_static_messages() {
        assert_eq!(
            budget_reason_tag(Some("Historical actual runtime is unknown")),
            1
        );
        assert_eq!(
            budget_reason_tag(Some("Historical actual output is unknown")),
            2
        );
        assert_eq!(
            budget_reason_tag(Some(
                "Shared reservation is not bound to the actual session/attempt/root"
            )),
            6
        );
        assert_eq!(
            budget_reason_tag(Some("private-token-shaped-user-error")),
            0
        );
        assert_eq!(budget_reason_tag(None), 0);
        let now = Instant::now();
        let mut trace = Trace::new(now);
        let line = trace
            .record(
                "goal_budget_denied",
                now,
                None,
                [false; 6],
                budget_reason_tag(Some("private-token-shaped-user-error")),
                "BUDGET_UNKNOWN",
            )
            .unwrap();
        assert!(!line.contains("private-token"));
        assert!(line.contains("valid=0 count=0"));
    }
    #[test]
    fn repeated_ticks_keep_bounded_late_stage_tail_instead_of_spending_global_budget() {
        let start = Instant::now();
        let mut trace = Trace::new(start);
        let mut emitted = vec![];
        for i in 0..10000 {
            if let Some(line) = trace.record(
                "goal_needs_advance",
                start + Duration::from_millis(i),
                None,
                [i % 2 == 0, false, false, false, false, false],
                0,
                "none",
            ) {
                emitted.push(line);
            }
        }
        assert_eq!(emitted.len(), 8);
        let tail = trace.drain();
        assert_eq!(tail.len(), 8);
        assert!(tail.last().unwrap().contains("at_ms=9999"));
        assert!(trace.lines <= MAX_LINES && trace.bytes <= MAX_BYTES);
    }
    #[test]
    fn fixed_stage_and_code_allowlists_never_print_request_or_error_payloads() {
        let now = Instant::now();
        let mut trace = Trace::new(now);
        assert!(trace
            .record("not-a-fixed-stage", now, None, [false; 6], 0, "none")
            .is_none());
        let line = trace
            .record(
                "ready_error",
                now,
                None,
                [false; 6],
                0,
                error_code(Some("private-token-shaped-user-error")),
            )
            .unwrap();
        assert!(!line.contains("private-token"));
        assert!(line.contains("other_code"));
        assert!(line.len() <= MAX_LINE);
        // One fixed stage shares the existing eight-first/eight-tail bucket
        // across all six phases; no phase adds an unbounded trace dimension.
        for phase in 1..=6 {
            let line = trace
                .record(
                    "factory_phase",
                    now,
                    Some(Duration::from_millis(7406)),
                    [false; 6],
                    phase,
                    "none",
                )
                .unwrap();
            assert!(line.contains("stage=factory_phase"));
            assert!(line.contains("elapsed_ms=7406 flags=000000 valid=0"));
            assert!(line.contains(&format!("count={phase} code=none")));
            assert!(line.len() <= MAX_LINE);
        }
        for phase in 1..=8 {
            let line = trace
                .record(
                    "factory_admission_phase",
                    now,
                    Some(Duration::from_millis(4969)),
                    [false; 6],
                    phase,
                    "none",
                )
                .unwrap();
            assert!(line.contains("stage=factory_admission_phase"));
            assert!(line.contains("elapsed_ms=4969 flags=000000 valid=0"));
            assert!(line.contains(&format!("count={phase} code=none")));
            assert!(line.len() <= MAX_LINE);
        }
        const { assert!(32 * PER_STAGE * 2 <= MAX_LINES) };
    }
}
