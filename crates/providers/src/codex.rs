//! Codex App Server stdio dialect at the pinned protocol schema revision.
//! This is not ACP. Native declarations are not proof of identity or OS enforcement.
use crate::process_io::OwnedProcessIo;
use crate::transport::{
    ApprovedInstallation, Diagnostics, FailureKind, ProviderEvent, TransportFailure,
};
use crate::{ProtocolError, TurnState, UpdateKind, MAX_SESSIONS};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn protocol(error: ProtocolError) -> TransportFailure {
    TransportFailure::protocol(error)
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, TransportFailure> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty() && v.len() <= 4096)
        .ok_or_else(|| protocol(ProtocolError::InvalidParams))
}
/// Native envelopes intentionally do not require ACP's jsonrpc or protocolVersion.
/// Unknown object fields remain opaque; duplicate keys were rejected by the common frame parser.
pub fn validate_native_envelope(frame: &Value) -> Result<(), ProtocolError> {
    let object = frame.as_object().ok_or(ProtocolError::InvalidEnvelope)?;
    if object.get("jsonrpc").is_some_and(|v| v != "2.0") {
        return Err(ProtocolError::InvalidEnvelope);
    }
    if let Some(id) = object.get("id") {
        if !(id.is_string() || id.is_u64() || id.is_i64()) {
            return Err(ProtocolError::InvalidEnvelope);
        }
    }
    if let Some(method) = object.get("method") {
        if method.as_str().filter(|v| !v.is_empty()).is_none()
            || object.contains_key("result")
            || object.contains_key("error")
            || object.get("params").is_some_and(|v| !v.is_object())
        {
            return Err(ProtocolError::InvalidEnvelope);
        }
    } else {
        if !object.contains_key("id")
            || object.contains_key("result") == object.contains_key("error")
        {
            return Err(ProtocolError::InvalidEnvelope);
        }
        if let Some(error) = object.get("error") {
            if error.get("code").and_then(Value::as_i64).is_none()
                || error.get("message").and_then(Value::as_str).is_none()
            {
                return Err(ProtocolError::InvalidEnvelope);
            }
        }
    }
    Ok(())
}
struct Thread {
    binding: String,
    state: TurnState,
}
struct PendingTurn {
    thread: String,
    request_id: u64,
    turn_id: Option<String>,
    response_received: bool,
    terminal: Option<TurnState>,
    deadline: Instant,
    cancel_requested: bool,
    interrupt_sent: bool,
}
pub struct CodexProcess {
    io: OwnedProcessIo,
    timeout: Duration,
    workspace: PathBuf,
    next_id: u64,
    threads: BTreeMap<String, Thread>,
    pending: Option<PendingTurn>,
    control_ids: BTreeSet<u64>,
    completed_turns: BTreeSet<String>,
}
impl CodexProcess {
    pub fn start(
        installation: ApprovedInstallation,
        timeout: Duration,
    ) -> Result<Self, TransportFailure> {
        let workspace = installation.workspace.clone();
        let expected_home = installation
            .environment
            .get(std::ffi::OsStr::new("CODEX_HOME"))
            .map(PathBuf::from);
        let mut process = Self {
            io: OwnedProcessIo::spawn(installation, timeout)?,
            timeout,
            workspace,
            next_id: 0,
            threads: BTreeMap::new(),
            pending: None,
            control_ids: BTreeSet::new(),
            completed_turns: BTreeSet::new(),
        };
        let request=process.request("initialize",json!({"clientInfo":{"name":"agentbaton","title":"AgentBaton","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":false}}))?;
        let response = process.exchange(&request)?;
        let result = response
            .get("result")
            .ok_or_else(|| protocol(ProtocolError::ProviderRejected))?;
        for key in ["codexHome", "platformFamily", "platformOs", "userAgent"] {
            text(result, key)?;
        }
        let reported_home = PathBuf::from(text(result, "codexHome")?);
        if !reported_home.is_absolute() || expected_home.is_some_and(|home| home != reported_home) {
            return process.fail(protocol(ProtocolError::InvalidParams));
        }
        process.io.enqueue_frame(&json!({"method":"initialized"}))?;
        process.io.flush_outbound()?;
        Ok(process)
    }
    fn request(&mut self, method: &str, params: Value) -> Result<Value, TransportFailure> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| protocol(ProtocolError::ResourceLimit))?;
        Ok(json!({"id":self.next_id,"method":method,"params":params}))
    }
    fn fail<T>(&mut self, error: TransportFailure) -> Result<T, TransportFailure> {
        self.terminate();
        Err(error)
    }
    fn exchange(&mut self, request: &Value) -> Result<Value, TransportFailure> {
        let id = request["id"].as_u64().unwrap();
        let deadline = Instant::now() + self.timeout;
        self.io.enqueue_frame(request)?;
        loop {
            let frame = match self.io.next_frame(deadline) {
                Ok(f) => f,
                Err(e) => return self.fail(e),
            };
            if let Err(e) = validate_native_envelope(&frame) {
                return self.fail(protocol(e));
            }
            self.io.evidence(&frame);
            if frame.get("method").is_some() {
                if let Err(e) = self.notification_or_request(&frame) {
                    return self.fail(e);
                }
                continue;
            }
            if self.control_ids.remove(&frame["id"].as_u64().unwrap_or(0)) {
                continue;
            }
            if frame["id"].as_u64() != Some(id) {
                return self.fail(protocol(ProtocolError::UnmatchedResponse));
            }
            if frame.get("error").is_some() {
                return self.fail(protocol(ProtocolError::ProviderRejected));
            }
            return Ok(frame);
        }
    }
    /// Create a new native thread; no auth/config discovery or existing-session attachment.
    pub fn new_session(&mut self, binding: &str) -> Result<String, TransportFailure> {
        if self.pending.is_some() {
            return Err(protocol(ProtocolError::UnexpectedState));
        }
        if binding.is_empty() || binding.len() > 256 || self.threads.len() >= MAX_SESSIONS {
            return Err(protocol(ProtocolError::InvalidParams));
        }
        let request=self.request("thread/start",json!({"cwd":self.workspace,"approvalPolicy":"on-request","sandbox":"workspace-write","ephemeral":true}))?;
        let response = self.exchange(&request)?;
        let id = match self.validate_thread_response(&response) {
            Ok(id) => id,
            Err(error) => return self.fail(error),
        };
        self.threads.insert(
            id.clone(),
            Thread {
                binding: binding.into(),
                state: TurnState::Idle,
            },
        );
        Ok(id)
    }
    fn validate_thread_response(&self, response: &Value) -> Result<String, TransportFailure> {
        let result = response
            .get("result")
            .ok_or_else(|| protocol(ProtocolError::InvalidParams))?;
        let id = text(
            result
                .get("thread")
                .ok_or_else(|| protocol(ProtocolError::InvalidParams))?,
            "id",
        )?
        .to_owned();
        // Check the declared effective scope; this does not establish OS enforcement.
        if Path::new(text(result, "cwd")?) != self.workspace
            || result["approvalPolicy"] != "on-request"
            || result["sandbox"]["type"] != "workspaceWrite"
            || result["sandbox"]
                .get("networkAccess")
                .is_some_and(|v| v.as_bool() != Some(false))
        {
            return Err(protocol(ProtocolError::PermissionDenied));
        }
        if let Some(roots) = result["sandbox"].get("writableRoots") {
            let roots = roots
                .as_array()
                .ok_or_else(|| protocol(ProtocolError::InvalidParams))?;
            if roots
                .iter()
                .any(|r| r.as_str().is_none_or(|r| Path::new(r) != self.workspace))
            {
                return Err(protocol(ProtocolError::PermissionDenied));
            }
        }
        if self.threads.contains_key(&id) {
            return Err(protocol(ProtocolError::InvalidParams));
        }
        Ok(id)
    }
    pub fn set_io_timeout(&mut self, timeout: Duration) -> Result<(), TransportFailure> {
        if timeout.is_zero() || timeout > Duration::from_secs(60) {
            return Err(protocol(ProtocolError::InvalidParams));
        }
        if self.pending.is_some() {
            return Err(protocol(ProtocolError::UnexpectedState));
        }
        self.timeout = timeout;
        Ok(())
    }
    pub fn begin_prompt(&mut self, thread: &str, prompt: &str) -> Result<(), TransportFailure> {
        if self.pending.is_some() {
            return Err(protocol(ProtocolError::UnexpectedState));
        }
        if prompt.is_empty() || prompt.len() > 256 * 1024 {
            return Err(protocol(ProtocolError::InvalidParams));
        }
        if self.completed_turns.len() >= 4096 {
            return Err(protocol(ProtocolError::ResourceLimit));
        }
        let state = &self
            .threads
            .get(thread)
            .ok_or_else(|| protocol(ProtocolError::UnknownSession))?
            .state;
        if !matches!(
            state,
            TurnState::Idle | TurnState::Completed | TurnState::Cancelled | TurnState::Interrupted
        ) {
            return Err(protocol(ProtocolError::UnexpectedState));
        }
        let request=self.request("turn/start",json!({"threadId":thread,"input":[{"type":"text","text":prompt}],"cwd":self.workspace,"approvalPolicy":"on-request","sandboxPolicy":{"type":"workspaceWrite","writableRoots":[self.workspace],"networkAccess":false,"excludeSlashTmp":true,"excludeTmpdirEnvVar":true}}))?;
        self.pending = Some(PendingTurn {
            thread: thread.into(),
            request_id: request["id"].as_u64().unwrap(),
            turn_id: None,
            response_received: false,
            terminal: None,
            deadline: Instant::now() + self.timeout,
            cancel_requested: false,
            interrupt_sent: false,
        });
        self.threads.get_mut(thread).unwrap().state = TurnState::Running;
        if let Err(e) = self
            .io
            .enqueue_frame(&request)
            .and_then(|_| self.io.flush_outbound())
        {
            return self.fail(e);
        }
        Ok(())
    }
    fn observe_turn(&mut self, thread: &str, turn: &str) -> Result<(), TransportFailure> {
        let pending = self
            .pending
            .as_mut()
            .ok_or_else(|| protocol(ProtocolError::UnexpectedState))?;
        if pending.thread != thread
            || turn.is_empty()
            || turn.len() > 4096
            || pending.turn_id.as_ref().is_some_and(|v| v != turn)
        {
            return Err(protocol(ProtocolError::UnmatchedResponse));
        }
        if pending.turn_id.is_none() && self.completed_turns.contains(turn) {
            return Err(protocol(ProtocolError::UnmatchedResponse));
        }
        if pending.turn_id.is_none() {
            pending.turn_id = Some(turn.into());
        }
        Ok(())
    }
    fn emit(&mut self, thread: &str, kind: UpdateKind) -> Result<(), TransportFailure> {
        let binding = self
            .threads
            .get(thread)
            .ok_or_else(|| protocol(ProtocolError::UnknownSession))?
            .binding
            .clone();
        self.io.update(ProviderEvent {
            session_id: thread.into(),
            local_binding: binding,
            kind,
        })
    }
    fn notification_or_request(&mut self, frame: &Value) -> Result<(), TransportFailure> {
        let method = text(frame, "method")?;
        let params = frame.get("params").cloned().unwrap_or(json!({}));
        if let Some(id) = frame.get("id") {
            // Approval callbacks are bound to the exact active native turn, never
            // trusted merely because their operation labels resemble an approval.
            let result = match method {
                "item/commandExecution/requestApproval"
                | "item/fileChange/requestApproval"
                | "item/permissions/requestApproval" => {
                    let thread = text(&params, "threadId")?;
                    let turn = text(&params, "turnId")?;
                    self.observe_turn(thread, turn)?;
                    let item = text(&params, "itemId")?;
                    self.emit(
                        thread,
                        UpdateKind::PermissionDenied {
                            tool_call_id: item.into(),
                        },
                    )?;
                    match method {
                        "item/permissions/requestApproval" => {
                            json!({"permissions":{},"scope":"turn","strictAutoReview":true})
                        }
                        _ => {
                            json!({"decision":if self.pending.as_ref().unwrap().cancel_requested {"cancel"} else {"decline"}})
                        }
                    }
                }
                _ => {
                    self.io.enqueue_frame(&json!({"id":id,"error":{"code":-32601,"message":"Client method unsupported"}}))?;
                    return Ok(());
                }
            };
            self.io.enqueue_frame(&json!({"id":id,"result":result}))?;
            return self.maybe_interrupt();
        }
        match method {
            "turn/started" | "turn/completed" => {
                let thread = text(&params, "threadId")?;
                let turn = params
                    .get("turn")
                    .ok_or_else(|| protocol(ProtocolError::InvalidParams))?;
                let id = text(turn, "id")?;
                self.observe_turn(thread, id)?;
                if !turn["items"].is_array() {
                    return Err(protocol(ProtocolError::InvalidParams));
                }
                let status = text(turn, "status")?;
                if method == "turn/completed" {
                    let pending = self.pending.as_mut().unwrap();
                    let state = match status {
                        "completed" => TurnState::Completed,
                        "interrupted" if pending.cancel_requested => TurnState::Cancelled,
                        "interrupted" => TurnState::Interrupted,
                        "failed" => TurnState::OutcomeUnknown,
                        _ => return Err(protocol(ProtocolError::InvalidParams)),
                    };
                    if pending.terminal.is_some() {
                        return Err(protocol(ProtocolError::UnexpectedState));
                    }
                    pending.terminal = Some(state);
                } else if status != "inProgress" {
                    return Err(protocol(ProtocolError::InvalidParams));
                }
            }
            "item/agentMessage/delta" => {
                let thread = text(&params, "threadId")?;
                self.observe_turn(thread, text(&params, "turnId")?)?;
                text(&params, "itemId")?;
                let delta = params
                    .get("delta")
                    .and_then(Value::as_str)
                    .ok_or_else(|| protocol(ProtocolError::InvalidParams))?;
                self.emit(thread, UpdateKind::AgentText { text: delta.into() })?;
            }
            "item/started" | "item/completed" => {
                let thread = text(&params, "threadId")?;
                self.observe_turn(thread, text(&params, "turnId")?)?;
                let item = params
                    .get("item")
                    .ok_or_else(|| protocol(ProtocolError::InvalidParams))?;
                let id = text(item, "id")?;
                let kind =
                    match item["type"].as_str() {
                        Some(
                            "commandExecution" | "fileChange" | "mcpToolCall" | "dynamicToolCall",
                        ) if method == "item/started" => UpdateKind::ToolCall {
                            tool_call_id: id.into(),
                        },
                        Some(
                            "commandExecution" | "fileChange" | "mcpToolCall" | "dynamicToolCall",
                        ) => UpdateKind::ToolCallUpdate {
                            tool_call_id: id.into(),
                        },
                        _ => UpdateKind::Opaque {
                            native_kind: method.into(),
                        },
                    };
                self.emit(thread, kind)?;
            }
            _ => {
                // Preserve bounded raw evidence. Bind an opaque display event only
                // when this is actually a known thread; account/auth notices do
                // not establish trusted local identity or capabilities.
                if let Some(thread) = params.get("threadId").and_then(Value::as_str) {
                    if self.threads.contains_key(thread) {
                        // Late metadata may refer to an already completed turn.
                        // Opaque display evidence has no active-turn authority.
                        self.emit(
                            thread,
                            UpdateKind::Opaque {
                                native_kind: method.into(),
                            },
                        )?;
                    }
                }
            }
        }
        self.maybe_interrupt()
    }
    fn maybe_interrupt(&mut self) -> Result<(), TransportFailure> {
        let pending = self
            .pending
            .as_ref()
            .filter(|p| p.cancel_requested && !p.interrupt_sent && p.terminal.is_none());
        let Some((thread, turn)) =
            pending.and_then(|p| p.turn_id.as_ref().map(|t| (p.thread.clone(), t.clone())))
        else {
            return Ok(());
        };
        if self.control_ids.len() >= 128 {
            return Err(protocol(ProtocolError::ResourceLimit));
        }
        let request = self.request("turn/interrupt", json!({"threadId":thread,"turnId":turn}))?;
        self.control_ids.insert(request["id"].as_u64().unwrap());
        self.pending.as_mut().unwrap().interrupt_sent = true;
        self.io.enqueue_frame(&request)
    }
    pub fn cancel(&mut self, thread: &str) -> Result<(), TransportFailure> {
        let pending = self
            .pending
            .as_mut()
            .filter(|p| p.thread == thread)
            .ok_or_else(|| protocol(ProtocolError::UnexpectedState))?;
        pending.cancel_requested = true;
        self.threads.get_mut(thread).unwrap().state = TurnState::CancelRequested;
        if let Err(e) = self
            .maybe_interrupt()
            .and_then(|_| self.io.flush_outbound())
        {
            return self.fail(e);
        }
        Ok(())
    }
    pub fn poll_prompt(
        &mut self,
        thread: &str,
        quantum: Duration,
    ) -> Result<Option<TurnState>, TransportFailure> {
        if quantum.is_zero() || quantum > Duration::from_secs(1) {
            return Err(protocol(ProtocolError::InvalidParams));
        }
        let turn_deadline = self
            .pending
            .as_ref()
            .filter(|p| p.thread == thread)
            .ok_or_else(|| protocol(ProtocolError::UnexpectedState))?
            .deadline;
        let deadline = turn_deadline.min(Instant::now() + quantum);
        loop {
            if self
                .pending
                .as_ref()
                .is_some_and(|p| p.response_received && p.terminal.is_some())
            {
                let pending = self.pending.take().unwrap();
                let state = pending.terminal.unwrap();
                self.completed_turns.insert(pending.turn_id.unwrap());
                self.threads.get_mut(thread).unwrap().state = state.clone();
                return Ok(Some(state));
            }
            let frame = match self.io.next_frame(deadline) {
                Ok(f) => f,
                Err(e) if e.kind == FailureKind::Timeout && Instant::now() < turn_deadline => {
                    return Ok(None)
                }
                Err(e) => return self.fail(e),
            };
            if let Err(e) = validate_native_envelope(&frame) {
                return self.fail(protocol(e));
            }
            self.io.evidence(&frame);
            let handled: Result<(), TransportFailure> = (|| {
                if frame.get("method").is_some() {
                    return self.notification_or_request(&frame);
                }
                let id = frame["id"]
                    .as_u64()
                    .ok_or_else(|| protocol(ProtocolError::UnmatchedResponse))?;
                if self.control_ids.remove(&id) {
                    // An interrupt RPC acknowledgement is never a stop receipt.
                    if frame.get("error").is_some() {
                        return Err(protocol(ProtocolError::ProviderRejected));
                    }
                    return Ok(());
                }
                let pending = self.pending.as_ref().unwrap();
                if id != pending.request_id || pending.response_received {
                    return Err(protocol(ProtocolError::UnmatchedResponse));
                }
                if frame.get("error").is_some() {
                    return Err(protocol(ProtocolError::ProviderRejected));
                }
                let turn = frame
                    .pointer("/result/turn")
                    .ok_or_else(|| protocol(ProtocolError::InvalidParams))?;
                self.observe_turn(thread, text(turn, "id")?)?;
                if !turn["items"].is_array()
                    || !matches!(
                        turn["status"].as_str(),
                        Some("inProgress" | "completed" | "interrupted" | "failed")
                    )
                {
                    return Err(protocol(ProtocolError::InvalidParams));
                }
                self.pending.as_mut().unwrap().response_received = true;
                self.maybe_interrupt()
            })();
            if let Err(e) = handled {
                return self.fail(e);
            }
        }
    }
    pub fn wait_prompt(&mut self, thread: &str) -> Result<TurnState, TransportFailure> {
        loop {
            if let Some(s) = self.poll_prompt(thread, Duration::from_millis(25))? {
                return Ok(s);
            }
        }
    }
    pub fn process_id(&self) -> u32 {
        self.io.process_id()
    }
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.io.diagnostics
    }
    pub fn take_updates(&mut self) -> Vec<ProviderEvent> {
        self.io.take_updates()
    }
    pub fn take_evidence(&mut self) -> Vec<Value> {
        self.io.take_evidence()
    }
    pub fn turn_state(&self, thread: &str) -> Result<&TurnState, ProtocolError> {
        self.threads
            .get(thread)
            .map(|t| &t.state)
            .ok_or(ProtocolError::UnknownSession)
    }
    pub fn terminate(&mut self) {
        for thread in self.threads.values_mut() {
            if matches!(
                thread.state,
                TurnState::Running | TurnState::CancelRequested
            ) {
                thread.state = TurnState::OutcomeUnknown;
            }
        }
        self.io.terminate();
    }
}
impl Drop for CodexProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}
