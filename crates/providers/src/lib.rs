//! Bounded ACP wire contracts and an owned Unix stdio transport.
//! Vendor-specific drivers and live model compatibility remain unverified.
//!
//! The caller owns connection authentication, authorization, higher-level supervision,
//! credential isolation, redaction and durable event storage. Provider fields never
//! establish local identity or permission. Uninterpreted extensions remain opaque.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{BufRead, Read};

pub mod connections;
mod framing;
pub mod installation;
#[cfg(unix)]
pub mod transport;

pub const ACP_VERSION: u64 = 1;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_SESSIONS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolError {
    FrameTooLarge,
    MalformedFrame,
    IncompleteFrame,
    InvalidEnvelope,
    UnexpectedState,
    UnmatchedResponse,
    VersionUnsupported,
    CapabilityUnsupported,
    InvalidParams,
    UnknownSession,
    PermissionDenied,
    ResourceLimit,
    ProviderRejected,
    TransportClosed,
}
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Do not echo untrusted frame contents, tool arguments or credentials.
        write!(f, "ACP protocol error: {self:?}")
    }
}
impl std::error::Error for ProtocolError {}

/// Read a single newline-delimited JSON-RPC message without unbounded allocation.
/// A transport owner must impose read deadlines. EOF mid-frame is not completion.
pub fn read_frame(reader: &mut impl BufRead) -> Result<Option<Value>, ProtocolError> {
    let mut frame = Vec::new();
    reader
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut frame)
        .map_err(|_| ProtocolError::TransportClosed)?;
    if frame.is_empty() {
        return Ok(None);
    }
    if frame.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    if frame.last() != Some(&b'\n') {
        return Err(ProtocolError::IncompleteFrame);
    }
    let value: Value = framing::parse(&frame).map_err(|_| ProtocolError::MalformedFrame)?;
    validate_envelope(&value)?;
    Ok(Some(value))
}

pub fn validate_envelope(value: &Value) -> Result<(), ProtocolError> {
    let obj = value.as_object().ok_or(ProtocolError::InvalidEnvelope)?;
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(ProtocolError::InvalidEnvelope);
    }
    if let Some(id) = obj.get("id") {
        if !id.is_string() && !id.is_i64() && !id.is_u64() {
            return Err(ProtocolError::InvalidEnvelope);
        }
    }
    if let Some(method) = obj.get("method") {
        if method.as_str().filter(|s| !s.is_empty()).is_none()
            || obj.contains_key("result")
            || obj.contains_key("error")
            || obj.get("params").is_some_and(|v| !v.is_object())
        {
            return Err(ProtocolError::InvalidEnvelope);
        }
    } else {
        if !obj.contains_key("id") || obj.contains_key("result") == obj.contains_key("error") {
            return Err(ProtocolError::InvalidEnvelope);
        }
        if let Some(error) = obj.get("error") {
            if error.get("code").and_then(Value::as_i64).is_none()
                || error.get("message").and_then(Value::as_str).is_none()
            {
                return Err(ProtocolError::InvalidEnvelope);
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    Idle,
    Running,
    CancelRequested,
    Cancelled,
    Completed,
    Interrupted,
    OutcomeUnknown,
}
#[derive(Debug, Clone)]
struct Session {
    attached: bool,
    /// Trusted local association, not a provider-supplied caller identity.
    binding: String,
    state: TurnState,
    prompt_request: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpdateKind {
    AgentText { text: String },
    AgentThought { text: String },
    ToolCall { tool_call_id: String },
    ToolCallUpdate { tool_call_id: String },
    PermissionDenied { tool_call_id: String },
    Opaque { native_kind: String },
}
#[derive(Debug, Clone, Serialize)]
pub struct NormalizedUpdate {
    pub session_id: String,
    pub local_binding: String,
    pub kind: UpdateKind,
    /// Untrusted raw protocol data; apply privacy policy before durable storage.
    pub raw: Value,
}

/// Single ACP connection with explicit handshake and request-response correlation.
/// It never invokes an executable or advertises unsupported filesystem/terminal APIs.
#[derive(Debug, Default)]
pub struct AcpClient {
    next_id: u64,
    initialize_id: Option<u64>,
    initialized: bool,
    capabilities: Value,
    sessions: BTreeMap<String, Session>,
}
impl AcpClient {
    pub fn new() -> Self {
        Self::default()
    }
    fn request_id(&mut self) -> Result<u64, ProtocolError> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(ProtocolError::ResourceLimit)?;
        Ok(self.next_id)
    }
    pub fn initialize(&mut self) -> Result<Value, ProtocolError> {
        if self.initialized || self.initialize_id.is_some() {
            return Err(ProtocolError::UnexpectedState);
        }
        let id = self.request_id()?;
        self.initialize_id = Some(id);
        Ok(
            json!({"jsonrpc":"2.0","id":id,"method":"initialize","params":{
                "protocolVersion":ACP_VERSION,
                "clientInfo":{"name":"AgentBaton","version":env!("CARGO_PKG_VERSION")},
                "clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false,"auth":{"terminal":false}}
            }}),
        )
    }
    pub fn accept_initialize(&mut self, response: Value) -> Result<(), ProtocolError> {
        validate_envelope(&response)?;
        if self.initialize_id.is_none() || self.initialized {
            return Err(ProtocolError::UnexpectedState);
        }
        if response.get("id").and_then(Value::as_u64) != self.initialize_id {
            return Err(ProtocolError::UnmatchedResponse);
        }
        if response.get("error").is_some() {
            self.initialize_id = None;
            return Err(ProtocolError::ProviderRejected);
        }
        let result = response.get("result").ok_or(ProtocolError::InvalidParams)?;
        if result.get("protocolVersion").and_then(Value::as_u64) != Some(ACP_VERSION) {
            self.initialize_id = None;
            return Err(ProtocolError::VersionUnsupported);
        }
        let capabilities = result
            .get("agentCapabilities")
            .cloned()
            .unwrap_or(json!({}));
        if !capabilities.is_object()
            || capabilities
                .get("loadSession")
                .is_some_and(|v| !v.is_boolean())
        {
            return Err(ProtocolError::InvalidParams);
        }
        self.capabilities = capabilities;
        self.initialized = true;
        self.initialize_id = None;
        Ok(())
    }
    fn ready(&self) -> Result<(), ProtocolError> {
        if self.initialized {
            Ok(())
        } else {
            Err(ProtocolError::UnexpectedState)
        }
    }
    pub fn capabilities(&self) -> Result<&Value, ProtocolError> {
        self.ready()?;
        Ok(&self.capabilities)
    }
    pub fn require_load_session(&self) -> Result<(), ProtocolError> {
        self.ready()?;
        if self
            .capabilities
            .get("loadSession")
            .and_then(Value::as_bool)
            == Some(true)
        {
            Ok(())
        } else {
            Err(ProtocolError::CapabilityUnsupported)
        }
    }
    /// Register only after the owning transport correlates a successful session/new
    /// response with its trusted local Attempt/Connection binding.
    pub fn register_session(
        &mut self,
        native_id: &str,
        local_binding: &str,
    ) -> Result<(), ProtocolError> {
        self.ready()?;
        if native_id.is_empty()
            || native_id.len() > 256
            || local_binding.is_empty()
            || local_binding.len() > 256
        {
            return Err(ProtocolError::InvalidParams);
        }
        if self.sessions.contains_key(native_id) {
            return Err(ProtocolError::UnexpectedState);
        }
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(ProtocolError::ResourceLimit);
        }
        self.sessions.insert(
            native_id.into(),
            Session {
                attached: true,
                binding: local_binding.into(),
                state: TurnState::Idle,
                prompt_request: None,
            },
        );
        Ok(())
    }
    pub fn prompt(&mut self, native_id: &str, text: &str) -> Result<Value, ProtocolError> {
        self.ready()?;
        if text.is_empty() || text.len() > MAX_FRAME_BYTES / 8 {
            return Err(ProtocolError::InvalidParams);
        }
        let session = self
            .sessions
            .get(native_id)
            .ok_or(ProtocolError::UnknownSession)?;
        if !session.attached
            || !matches!(
                session.state,
                TurnState::Idle
                    | TurnState::Completed
                    | TurnState::Cancelled
                    | TurnState::Interrupted
            )
        {
            return Err(ProtocolError::UnexpectedState);
        }
        let id = self.request_id()?;
        let session = self.sessions.get_mut(native_id).unwrap();
        session.state = TurnState::Running;
        session.prompt_request = Some(id);
        Ok(
            json!({"jsonrpc":"2.0","id":id,"method":"session/prompt","params":{"sessionId":native_id,"prompt":[{"type":"text","text":text}]}}),
        )
    }
    pub fn cancel(&mut self, native_id: &str) -> Result<Value, ProtocolError> {
        self.ready()?;
        let session = self
            .sessions
            .get_mut(native_id)
            .ok_or(ProtocolError::UnknownSession)?;
        if !session.attached
            || (session.state != TurnState::Running && session.state != TurnState::CancelRequested)
        {
            return Err(ProtocolError::UnexpectedState);
        }
        session.state = TurnState::CancelRequested;
        Ok(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":native_id}}))
    }
    pub fn state(&self, native_id: &str) -> Result<&TurnState, ProtocolError> {
        self.sessions
            .get(native_id)
            .map(|s| &s.state)
            .ok_or(ProtocolError::UnknownSession)
    }
    pub fn accept_prompt_result(
        &mut self,
        native_id: &str,
        response: Value,
    ) -> Result<TurnState, ProtocolError> {
        self.ready()?;
        validate_envelope(&response)?;
        let session = self
            .sessions
            .get_mut(native_id)
            .ok_or(ProtocolError::UnknownSession)?;
        if !session.attached
            || session.prompt_request.is_none()
            || response.get("id").and_then(Value::as_u64) != session.prompt_request
        {
            return Err(ProtocolError::UnmatchedResponse);
        }
        // An error cannot prove absence of side effects, even after cancellation.
        let state = if response.get("error").is_some() {
            TurnState::OutcomeUnknown
        } else {
            match response
                .pointer("/result/stopReason")
                .and_then(Value::as_str)
            {
                Some("cancelled") => TurnState::Cancelled,
                Some("end_turn") => TurnState::Completed,
                Some("max_tokens" | "max_turn_requests" | "refusal") => TurnState::Interrupted,
                _ => TurnState::OutcomeUnknown,
            }
        };
        session.state = state.clone();
        session.prompt_request = None;
        Ok(state)
    }
    pub fn disconnected(&mut self) {
        self.initialized = false;
        self.initialize_id = None;
        for session in self.sessions.values_mut() {
            session.attached = false;
            if matches!(
                session.state,
                TurnState::Running | TurnState::CancelRequested
            ) {
                session.state = TurnState::OutcomeUnknown;
            }
        }
    }
    pub fn normalize_update(&self, notification: Value) -> Result<NormalizedUpdate, ProtocolError> {
        self.ready()?;
        validate_envelope(&notification)?;
        if notification.get("method").and_then(Value::as_str) != Some("session/update")
            || notification.get("id").is_some()
        {
            return Err(ProtocolError::InvalidEnvelope);
        }
        let params = notification
            .get("params")
            .ok_or(ProtocolError::InvalidParams)?;
        let native_id = params
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or(ProtocolError::InvalidParams)?;
        let session = self
            .sessions
            .get(native_id)
            .ok_or(ProtocolError::UnknownSession)?;
        if !session.attached {
            return Err(ProtocolError::UnknownSession);
        }
        let update = params.get("update").ok_or(ProtocolError::InvalidParams)?;
        let native_kind = update
            .get("sessionUpdate")
            .and_then(Value::as_str)
            .ok_or(ProtocolError::InvalidParams)?;
        let opaque = || UpdateKind::Opaque {
            native_kind: native_kind.into(),
        };
        let kind = match native_kind {
            "agent_message_chunk" | "agent_thought_chunk" => {
                let content = update
                    .get("content")
                    .filter(|v| v.is_object())
                    .ok_or(ProtocolError::InvalidParams)?;
                if content.get("type").and_then(Value::as_str) == Some("text") {
                    let text = content
                        .get("text")
                        .and_then(Value::as_str)
                        .ok_or(ProtocolError::InvalidParams)?
                        .into();
                    if native_kind == "agent_message_chunk" {
                        UpdateKind::AgentText { text }
                    } else {
                        UpdateKind::AgentThought { text }
                    }
                } else {
                    opaque()
                }
            }
            "tool_call" | "tool_call_update" => {
                let id = update
                    .get("toolCallId")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .ok_or(ProtocolError::InvalidParams)?
                    .into();
                if native_kind == "tool_call" {
                    if update.get("title").and_then(Value::as_str).is_none() {
                        return Err(ProtocolError::InvalidParams);
                    }
                    UpdateKind::ToolCall { tool_call_id: id }
                } else {
                    UpdateKind::ToolCallUpdate { tool_call_id: id }
                }
            }
            _ => opaque(),
        };
        Ok(NormalizedUpdate {
            session_id: native_id.into(),
            local_binding: session.binding.clone(),
            kind,
            raw: notification,
        })
    }
}

/// A permission response is only a protocol mapping. The application must supply
/// a trusted, scope-checked decision. This layer never auto-approves a tool label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    AllowOnce { option_id: String },
    RejectOnce { option_id: String },
    Cancelled,
}
impl AcpClient {
    pub fn permission_response(
        &self,
        request: &Value,
        decision: PermissionDecision,
    ) -> Result<Value, ProtocolError> {
        self.ready()?;
        validate_envelope(request)?;
        if request.get("method").and_then(Value::as_str) != Some("session/request_permission") {
            return Err(ProtocolError::InvalidEnvelope);
        }
        let id = request.get("id").ok_or(ProtocolError::InvalidEnvelope)?;
        let params = request.get("params").ok_or(ProtocolError::InvalidParams)?;
        let native_id = params
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or(ProtocolError::InvalidParams)?;
        let session = self
            .sessions
            .get(native_id)
            .ok_or(ProtocolError::UnknownSession)?;
        if params
            .pointer("/toolCall/toolCallId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .is_none()
        {
            return Err(ProtocolError::InvalidParams);
        }
        if !session.attached
            || !matches!(
                session.state,
                TurnState::Running | TurnState::CancelRequested
            )
        {
            return Err(ProtocolError::UnexpectedState);
        }
        let options = params
            .get("options")
            .and_then(Value::as_array)
            .ok_or(ProtocolError::InvalidParams)?;
        if options.is_empty() || options.len() > 32 {
            return Err(ProtocolError::InvalidParams);
        }
        let mut ids = BTreeSet::new();
        for option in options {
            let option_id = option
                .get("optionId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .ok_or(ProtocolError::InvalidParams)?;
            if !ids.insert(option_id)
                || option.get("kind").and_then(Value::as_str).is_none()
                || option.get("name").and_then(Value::as_str).is_none()
            {
                return Err(ProtocolError::InvalidParams);
            }
        }
        let outcome = if session.state == TurnState::CancelRequested
            || decision == PermissionDecision::Cancelled
        {
            json!({"outcome":"cancelled"})
        } else {
            let (option_id, kind) = match &decision {
                PermissionDecision::AllowOnce { option_id } => (option_id, "allow_once"),
                PermissionDecision::RejectOnce { option_id } => (option_id, "reject_once"),
                PermissionDecision::Cancelled => unreachable!(),
            };
            if !options.iter().any(|o| {
                o.get("optionId").and_then(Value::as_str) == Some(option_id)
                    && o.get("kind").and_then(Value::as_str) == Some(kind)
            }) {
                return Err(ProtocolError::PermissionDenied);
            }
            json!({"outcome":"selected","optionId":option_id})
        };
        Ok(json!({"jsonrpc":"2.0","id":id,"result":{"outcome":outcome}}))
    }
}
