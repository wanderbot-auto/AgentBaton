//! In-memory provenance for one owner-requested planning turn. Never serialized,
//! reconstructed from actor events, or usable as Goal/Factory authority.
use super::WorkerReceipt;
use crate::{CoreError, Result, RuntimeBinding};
use agentbaton_providers::{transport::ProviderEvent, TurnState, UpdateKind};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

#[derive(Clone)]
pub(crate) struct PlanningCapture(Arc<Inner>);
struct Inner {
    conversation: String,
    turn: String,
    prompt: String,
    runtime: RuntimeBinding,
    limit: u64,
    observed: AtomicU64,
    state: Mutex<State>,
}
#[derive(Default)]
struct State {
    claimed: bool,
    native: Option<String>,
    started: bool,
    invalid: bool,
    text: String,
    terminal: Option<TurnState>,
    finished: bool,
}
pub(crate) struct PlanningCaptureResult {
    pub(crate) text: Option<String>,
    /// Sum of serialized raw-free normalized ProviderEvents, including thought,
    /// tool and metadata updates. Not token, filesystem, or monetary usage.
    pub(crate) observed_output_bytes: u64,
    pub(crate) complete: bool,
    /// Whether observed_output_bytes is a complete normalized-turn meter,
    /// independent of later JSON/plan semantic validation. Otherwise it is only
    /// a lower bound and must never settle a Known output budget.
    pub(crate) measurement_complete: bool,
    pub(crate) prompt_started: bool,
    pub(crate) terminal_state: Option<String>,
}
impl PlanningCapture {
    pub(crate) fn new(
        conversation_id: &str,
        turn_id: &str,
        fixed_prompt: &str,
        runtime: &RuntimeBinding,
        response_bytes: u64,
    ) -> Result<Self> {
        if conversation_id.is_empty()
            || conversation_id.len() > 128
            || turn_id.is_empty()
            || turn_id.len() > 128
            || fixed_prompt.is_empty()
            || fixed_prompt.len() > 262_144
            || !(1..=65_536).contains(&response_bytes)
            || runtime.session_id != conversation_id
        {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "Planning capture requires an exact bounded turn and runtime binding",
            ));
        }
        Ok(Self(Arc::new(Inner {
            conversation: conversation_id.into(),
            turn: turn_id.into(),
            prompt: fixed_prompt.into(),
            runtime: runtime.clone(),
            limit: response_bytes,
            observed: AtomicU64::new(0),
            state: Mutex::new(State::default()),
        })))
    }
    pub(super) fn claim(&self, id: &str) -> bool {
        let Ok(mut state) = self.0.state.lock() else {
            return false;
        };
        if id != self.0.conversation || state.claimed || state.finished || state.invalid {
            state.invalid = true;
            return false;
        }
        state.claimed = true;
        true
    }
    pub(super) fn ready(&self, id: &str, native: &str) -> bool {
        let Ok(mut state) = self.0.state.lock() else {
            return false;
        };
        if !state.claimed
            || id != self.0.conversation
            || native.is_empty()
            || state.native.is_some()
            || state.finished
        {
            state.invalid = true;
            return false;
        }
        state.native = Some(native.into());
        true
    }
    pub(super) fn begin(&self, id: &str, native: &str, prompt: &str, ordinary: bool) -> bool {
        let Ok(mut state) = self.0.state.lock() else {
            return false;
        };
        if !ordinary
            || id != self.0.conversation
            || state.native.as_deref() != Some(native)
            || prompt != self.0.prompt
            || state.started
            || state.invalid
            || state.finished
        {
            state.invalid = true;
            return false;
        }
        state.started = true;
        true
    }
    pub(super) fn update(&self, update: &ProviderEvent) -> bool {
        // Count even the update that exceeds the cap or has a wrong binding.
        let bytes = serde_json::to_vec(update)
            .map(|b| b.len() as u64)
            .unwrap_or(u64::MAX);
        let mut before = self.0.observed.load(Ordering::Relaxed);
        let total = loop {
            let total = before.saturating_add(bytes);
            match self.0.observed.compare_exchange_weak(
                before,
                total,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break total,
                Err(current) => before = current,
            }
        };
        let Ok(mut state) = self.0.state.lock() else {
            return false;
        };
        if total > self.0.limit
            || !state.started
            || state.invalid
            || state.finished
            || state.terminal.is_some()
            || update.local_binding != self.0.conversation
            || state.native.as_deref() != Some(update.session_id.as_str())
        {
            state.invalid = true;
            state.text.clear();
            return false;
        }
        if !matches!(
            &update.kind,
            UpdateKind::AgentText { .. } | UpdateKind::AgentThought { .. }
        ) {
            state.invalid = true;
            state.text.clear();
            return false;
        }
        if let UpdateKind::AgentText { text } = &update.kind {
            if state.text.len().saturating_add(text.len()) > self.0.limit as usize {
                state.invalid = true;
                state.text.clear();
                return false;
            }
            state.text.push_str(text);
        }
        true
    }
    pub(super) fn invalidate(&self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.invalid = true;
            state.text.clear();
        }
    }
    pub(super) fn terminal(&self, terminal: TurnState) -> bool {
        let Ok(mut state) = self.0.state.lock() else {
            return false;
        };
        if state.terminal.is_some() || !state.started || state.invalid {
            state.invalid = true;
            return false;
        }
        state.terminal = Some(terminal.clone());
        if terminal != TurnState::Completed {
            state.invalid = true;
            state.text.clear();
            return false;
        }
        true
    }
    pub(super) fn worker_finished(&self, receipt: &WorkerReceipt, cancelled: bool) {
        if let Ok(mut state) = self.0.state.lock() {
            if cancelled
                || !receipt.process_reaped
                || receipt.never_started
                || receipt.reason != "planning_completed"
                || state.terminal != Some(TurnState::Completed)
            {
                state.invalid = true;
                state.text.clear();
            }
            state.finished = true;
        }
    }
    /// Called only after joining the owned worker. A restart has no such Arc;
    /// no actor journal entry can manufacture this provenance.
    pub(crate) fn finish(
        &self,
        receipt: &WorkerReceipt,
        conversation_id: &str,
        turn_id: &str,
        fixed_prompt: &str,
        runtime: &RuntimeBinding,
    ) -> PlanningCaptureResult {
        let mut result = PlanningCaptureResult {
            text: None,
            observed_output_bytes: self.0.observed.load(Ordering::Relaxed),
            complete: false,
            measurement_complete: false,
            prompt_started: false,
            terminal_state: None,
        };
        let Ok(state) = self.0.state.lock() else {
            return result;
        };
        result.prompt_started = state.started;
        result.terminal_state = state
            .terminal
            .as_ref()
            .and_then(|v| serde_json::to_value(v).ok())
            .and_then(|v| v.as_str().map(str::to_owned));
        result.complete = state.finished
            && !state.invalid
            && state.started
            && state.terminal == Some(TurnState::Completed)
            && receipt.process_reaped
            && !receipt.never_started
            && receipt.reason == "planning_completed"
            && conversation_id == self.0.conversation
            && turn_id == self.0.turn
            && fixed_prompt == self.0.prompt
            && runtime == &self.0.runtime;
        result.measurement_complete = result.complete
            || (state.finished
                && receipt.never_started
                && !state.started
                && result.observed_output_bytes == 0
                && conversation_id == self.0.conversation
                && turn_id == self.0.turn
                && fixed_prompt == self.0.prompt
                && runtime == &self.0.runtime);
        if result.complete {
            result.text = Some(state.text.clone());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::{run_worker, Outcome, Work, WorkerLaunch};
    use agentbaton_providers::{
        connections::ProtocolRoute, installation::executable_digest,
        transport::ApprovedInstallation,
    };
    use std::{
        collections::BTreeMap,
        fs,
        os::unix::fs::PermissionsExt,
        sync::{atomic::AtomicBool, mpsc},
        time::Duration,
    };
    const PROMPT: &str = "fixed bounded planning request";
    fn runtime() -> RuntimeBinding {
        RuntimeBinding {
            session_id: "planning".into(),
            attempt_id: "attempt".into(),
            connection_config_revision: "revision".into(),
            plan_digest: "plan".into(),
            snapshot: agentbaton_config::runtime::SnapshotDescriptor {
                id: "snapshot".into(),
                digest: "digest".into(),
                root_device: 1,
                root_inode: 2,
                snapshot_device: 1,
                snapshot_inode: 3,
            },
        }
    }
    fn event(kind: UpdateKind) -> ProviderEvent {
        ProviderEvent {
            session_id: "native".into(),
            local_binding: "planning".into(),
            kind,
        }
    }
    fn generated(
        mode: &str,
        limit: u64,
        duplicate: bool,
        prompt: &str,
        full_channel: bool,
    ) -> (PlanningCaptureResult, WorkerReceipt) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let python = std::process::Command::new("python3")
            .args(["-c", "import sys;print(sys.executable)"])
            .output()
            .unwrap();
        assert!(python.status.success());
        let exe = root.join("generated-planning");
        let body = r#"import json,sys,os
for line in sys.stdin:
 r=json.loads(line);m=r['method']
 if m=='initialize': result={'protocolVersion':1,'agentCapabilities':{}}
 elif m=='session/new': result={'sessionId':'native'}
 elif m=='session/prompt':
  assert r['params']['prompt'][0]['text']=='fixed bounded planning request'
  with open('prompts','a')as f:f.write('one\n')
  if MODE=='complete':
   print(json.dumps({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'native','update':{'sessionUpdate':'agent_thought_chunk','content':{'type':'text','text':'not response'}}}}),flush=True)
  parts=['{"title":','"bounded"}'] if MODE!='oversize' else ['x'*2000]
  for text in parts:
   print(json.dumps({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'wrong' if MODE=='wrong_native' else 'native','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':text}}}}),flush=True)
  if MODE=='cancel':continue
  if MODE=='malformed':
   print('invalid json',flush=True);continue
  result={'stopReason':'unsupported' if MODE=='unsupported' else 'end_turn'}
 elif m=='session/cancel':continue
 else:raise AssertionError(m)
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':result}),flush=True)
"#;
        fs::write(
            &exe,
            format!(
                "#!{}\nMODE={}\n{}",
                String::from_utf8(python.stdout).unwrap().trim(),
                serde_json::to_string(mode).unwrap(),
                body
            ),
        )
        .unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
        let installation = ApprovedInstallation {
            expected_sha256: executable_digest(&exe).unwrap(),
            executable: exe,
            arguments: vec![],
            workspace: root.clone(),
            approval_reference: "generated-fixture".into(),
            environment: BTreeMap::new(),
        };
        let capture = PlanningCapture::new("planning", "turn", PROMPT, &runtime(), limit).unwrap();
        let launch = WorkerLaunch::planning_conversation(
            ProtocolRoute::CustomAcp,
            installation,
            None,
            0,
            Duration::from_secs(3),
            capture.clone(),
            super::super::managed_mcps::ManagedMcpLaunch::test_empty(),
        );
        let (commands, receiver) = mpsc::sync_channel(4);
        commands.send(Work::Prompt(prompt.into())).unwrap();
        if duplicate {
            commands.send(Work::Prompt(PROMPT.into())).unwrap();
        }
        let (events, receive) = mpsc::sync_channel(if full_channel { 1 } else { 32 });
        let cancel = Arc::new(AtomicBool::new(mode == "pre_cancel"));
        let worker_cancel = cancel.clone();
        let worker = std::thread::spawn(move || {
            run_worker("planning".into(), launch, receiver, worker_cancel, events)
        });
        if mode == "cancel" {
            loop {
                if matches!(
                    receive.recv_timeout(Duration::from_secs(3)).unwrap(),
                    Outcome::Update(..)
                ) {
                    cancel.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
        let receipt = worker.join().unwrap();
        assert!(receipt.process_reaped || receipt.never_started);
        if let Ok(prompts) = fs::read_to_string(root.join("prompts")) {
            assert_eq!(prompts.lines().count(), 1);
        }
        (
            capture.finish(&receipt, "planning", "turn", PROMPT, &runtime()),
            receipt,
        )
    }
    #[test]
    fn generated_planning_complete_is_correlated_captured_and_reaped() {
        let (result, receipt) = generated("complete", 4096, false, PROMPT, false);
        assert_eq!(receipt.reason, "planning_completed");
        assert!(result.complete);
        assert!(result.measurement_complete && result.prompt_started);
        assert_eq!(result.text.as_deref(), Some("{\"title\":\"bounded\"}"));
        let expected = ["{\"title\":", "\"bounded\"}"]
            .into_iter()
            .map(|text| {
                serde_json::to_vec(&event(UpdateKind::AgentText { text: text.into() }))
                    .unwrap()
                    .len() as u64
            })
            .sum::<u64>();
        let expected = expected
            + serde_json::to_vec(&event(UpdateKind::AgentThought {
                text: "not response".into(),
            }))
            .unwrap()
            .len() as u64;
        assert_eq!(result.observed_output_bytes, expected);
        assert_eq!(result.terminal_state.as_deref(), Some("completed"));
    }
    #[test]
    fn planning_cancelled_before_spawn_has_explicit_known_zero_meter_only() {
        let (result, receipt) = generated("pre_cancel", 4096, false, PROMPT, false);
        assert!(receipt.never_started);
        assert!(!receipt.process_reaped);
        assert!(!result.complete && result.text.is_none() && !result.prompt_started);
        assert!(result.measurement_complete);
        assert_eq!(result.observed_output_bytes, 0);
    }
    #[test]
    fn generated_planning_refuses_partial_backpressure_binding_and_second_turn() {
        for (mode, limit, duplicate, prompt, full) in [
            ("oversize", 512, false, PROMPT, false),
            ("complete", 4096, false, PROMPT, true),
            ("malformed", 4096, false, PROMPT, false),
            ("cancel", 4096, false, PROMPT, false),
            ("unsupported", 4096, false, PROMPT, false),
            ("wrong_native", 4096, false, PROMPT, false),
            ("complete", 4096, true, PROMPT, false),
            ("complete", 4096, false, "changed prompt", false),
        ] {
            let (result, _) = generated(mode, limit, duplicate, prompt, full);
            assert!(!result.complete, "{mode}");
            assert!(!result.measurement_complete, "{mode}");
            assert!(result.text.is_none());
            if matches!(mode, "oversize" | "malformed" | "cancel") || full {
                assert!(result.observed_output_bytes > 0);
            }
            if mode == "oversize" {
                assert!(result.observed_output_bytes > limit);
            }
        }
    }
    #[test]
    fn capture_is_bounded_and_launch_binding_can_only_be_claimed_once() {
        for limit in [0, 65_537] {
            assert!(PlanningCapture::new("planning", "turn", PROMPT, &runtime(), limit).is_err());
        }
        let capture = PlanningCapture::new("planning", "turn", PROMPT, &runtime(), 4096).unwrap();
        assert!(capture.claim("planning"));
        assert!(!capture.claim("planning"));
        assert!(!capture.begin("planning", "native", PROMPT, true));
        let capture = PlanningCapture::new("planning", "turn", PROMPT, &runtime(), 4096).unwrap();
        assert!(!capture.claim("other"));
        assert!(!capture.claim("planning"));
    }

    #[test]
    fn capture_requires_finish_exact_runtime_and_no_gap_late_or_duplicate_terminal() {
        for bad in [
            "gap",
            "late",
            "duplicate",
            "runtime",
            "cancel",
            "binding",
            "unfinished",
            "tool",
        ] {
            let runtime = runtime();
            let capture = PlanningCapture::new("planning", "turn", PROMPT, &runtime, 4096).unwrap();
            assert!(capture.claim("planning"));
            assert!(capture.ready("planning", "native"));
            assert!(capture.begin("planning", "native", PROMPT, true));
            let thought = event(UpdateKind::AgentThought {
                text: "not response".into(),
            });
            assert!(capture.update(&thought));
            assert!(capture.update(&event(UpdateKind::AgentText { text: "{}".into() })));
            if bad == "tool" {
                assert!(!capture.update(&event(UpdateKind::ToolCall {
                    tool_call_id: "unapproved".into()
                })));
            }
            let accepted = capture.terminal(TurnState::Completed);
            assert_eq!(accepted, bad != "tool");
            match bad {
                "gap" => capture.invalidate(),
                "late" => assert!(!capture.update(&event(UpdateKind::AgentText {
                    text: "late".into()
                }))),
                "duplicate" => assert!(!capture.terminal(TurnState::Completed)),
                _ => {}
            }
            let mut receipt = WorkerReceipt::conversation_panicked();
            receipt.process_reaped = true;
            receipt.reason = "planning_completed".into();
            if bad != "unfinished" {
                capture.worker_finished(&receipt, bad == "cancel");
            }
            let mut checked = runtime.clone();
            if bad == "runtime" {
                checked.attempt_id = "changed".into();
            }
            let result = capture.finish(
                &receipt,
                "planning",
                if bad == "binding" { "other" } else { "turn" },
                PROMPT,
                &checked,
            );
            assert!(!result.complete, "{bad}");
            assert!(!result.measurement_complete, "{bad}");
            assert!(result.text.is_none());
            assert!(result.observed_output_bytes > 0);
        }
    }
}
