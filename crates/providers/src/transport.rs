//! Owned Unix stdio transport. No shell, executable discovery, model credentials
//! or automatic retries. Process separation and working directories are not a
//! filesystem/network sandbox. Callers must validate installation approval.
use crate::installation::BinaryDigest;
use crate::{AcpClient, PermissionDecision, ProtocolError, TurnState, UpdateKind};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// A descriptor supplied after the application's trusted approval and binary
/// verification flow. A nonempty approval reference is provenance, not a grant:
/// the caller must resolve it against its authorization store. No discovery
/// result is automatically converted into this descriptor.
#[derive(Clone)]
pub struct ApprovedInstallation {
    pub executable: PathBuf,
    pub expected_sha256: BinaryDigest,
    pub arguments: Vec<OsString>,
    pub workspace: PathBuf,
    pub approval_reference: String,
    /// Explicit, approved environment only. Parent process credentials are never
    /// inherited. Include necessary runtime/proxy/trust settings intentionally.
    pub environment: BTreeMap<OsString, OsString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    InvalidInstallation,
    StartupFailed,
    BinaryChanged,
    Timeout,
    Io,
    Protocol,
    Closed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectState {
    None,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TransportFailure {
    pub kind: FailureKind,
    pub side_effect_state: SideEffectState,
    pub protocol_error: Option<ProtocolError>,
}
impl std::fmt::Display for TransportFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Provider transport {:?}; effects {:?}",
            self.kind, self.side_effect_state
        )
    }
}
impl std::error::Error for TransportFailure {}
impl TransportFailure {
    pub(crate) fn local(kind: FailureKind) -> Self {
        Self {
            kind,
            side_effect_state: SideEffectState::None,
            protocol_error: None,
        }
    }
    pub(crate) fn unknown(kind: FailureKind) -> Self {
        Self {
            kind,
            side_effect_state: SideEffectState::Unknown,
            protocol_error: None,
        }
    }
    pub(crate) fn protocol(error: ProtocolError) -> Self {
        Self {
            kind: FailureKind::Protocol,
            side_effect_state: SideEffectState::Unknown,
            protocol_error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Diagnostics {
    pub stderr_bytes_observed: u64,
    /// No stderr bytes are retained: only counts, so credentials cannot be
    /// printed by forwarding these diagnostics.
    pub stderr_discarded: bool,
    pub evidence_records_dropped: u64,
    pub evidence_bytes: usize,
    pub process_reaped: bool,
    pub update_records_dropped: u64,
    pub update_bytes: usize,
    pub outbound_bytes: usize,
}

/// Raw-free typed update; strings remain untrusted content and may contain
/// private user data. Apply project retention/redaction before durable storage.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderEvent {
    pub session_id: String,
    pub local_binding: String,
    pub kind: UpdateKind,
}
struct PendingPrompt {
    session: String,
    request_id: u64,
    deadline: Instant,
    cancel_sent: bool,
}
pub struct ProviderProcess {
    io: crate::process_io::OwnedProcessIo,
    client: AcpClient,
    timeout: Duration,
    workspace: PathBuf,
    pending_prompt: Option<PendingPrompt>,
    claude_managed: bool,
    session_configuration: Value,
}
impl ProviderProcess {
    pub fn start(
        installation: ApprovedInstallation,
        timeout: Duration,
    ) -> Result<Self, TransportFailure> {
        let workspace = installation.workspace.clone();
        let mut process = Self {
            io: crate::process_io::OwnedProcessIo::spawn(installation, timeout)?,
            client: AcpClient::new(),
            timeout,
            workspace,
            pending_prompt: None,
            claude_managed: false,
            session_configuration: crate::session_configuration::default_configuration(false),
        };
        let request = process
            .client
            .initialize()
            .map_err(TransportFailure::protocol)?;
        let result = process.exchange(&request)?;
        if let Err(error) = process.client.accept_initialize(result) {
            return process.fail(TransportFailure::protocol(error));
        }
        Ok(process)
    }
    pub(crate) fn start_claude(
        installation: ApprovedInstallation,
        timeout: Duration,
    ) -> Result<Self, TransportFailure> {
        let mut process = Self::start(installation, timeout)?;
        process.claude_managed = true;
        process.session_configuration = crate::session_configuration::default_configuration(true);
        Ok(process)
    }
    fn fail<T>(&mut self, failure: TransportFailure) -> Result<T, TransportFailure> {
        self.client.disconnected();
        self.terminate();
        Err(failure)
    }
    fn handle_frame(
        &mut self,
        frame: Value,
        request_id: u64,
    ) -> Result<Option<Value>, TransportFailure> {
        crate::validate_envelope(&frame).map_err(TransportFailure::protocol)?;
        self.io.evidence(&frame);
        if let Some(method) = frame.get("method").and_then(Value::as_str) {
            if method == "session/update" {
                let event = self
                    .client
                    .normalize_update(frame)
                    .map_err(TransportFailure::protocol)?;
                self.io.update(ProviderEvent {
                    session_id: event.session_id,
                    local_binding: event.local_binding,
                    kind: event.kind,
                })?;
            } else if method == "session/request_permission" {
                let response = self
                    .client
                    .permission_response(&frame, PermissionDecision::Cancelled)
                    .map_err(TransportFailure::protocol)?;
                // Fields are validated by permission_response, and local binding
                // comes from registration rather than provider identity claims.
                let session_id = frame["params"]["sessionId"].as_str().unwrap();
                let local_binding = self
                    .client
                    .sessions
                    .get(session_id)
                    .unwrap()
                    .binding
                    .clone();
                self.io.update(ProviderEvent {
                    session_id: session_id.into(),
                    local_binding,
                    kind: UpdateKind::PermissionDenied {
                        tool_call_id: frame["params"]["toolCall"]["toolCallId"]
                            .as_str()
                            .unwrap()
                            .into(),
                    },
                })?;
                self.io.enqueue_frame(&response)?;
            } else if let Some(id) = frame.get("id") {
                self.io.enqueue_frame(&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Client method unsupported"}}))?;
            }
            return Ok(None);
        }
        if frame.get("id").and_then(Value::as_u64) != Some(request_id) {
            return Err(TransportFailure::protocol(ProtocolError::UnmatchedResponse));
        }
        Ok(Some(frame))
    }
    fn receive_response(
        &mut self,
        request_id: u64,
        deadline: Instant,
    ) -> Result<Value, TransportFailure> {
        loop {
            let frame = self.io.next_frame(deadline)?;
            if let Some(response) = self.handle_frame(frame, request_id)? {
                return Ok(response);
            }
        }
    }
    fn exchange(&mut self, request: &Value) -> Result<Value, TransportFailure> {
        let deadline = Instant::now() + self.timeout;
        let id = request
            .get("id")
            .and_then(Value::as_u64)
            .ok_or(TransportFailure::protocol(ProtocolError::InvalidEnvelope))?;
        if let Err(error) = self.io.enqueue_frame(request) {
            return self.fail(error);
        }
        match self.receive_response(id, deadline) {
            Ok(value) => Ok(value),
            Err(error) => self.fail(error),
        }
    }
    /// Accept only the server-owned immutable snapshot projection before any
    /// session is opened. This method does not establish consent or authority.
    pub fn configure_session_configuration(
        &mut self,
        configuration: &Value,
    ) -> Result<(), TransportFailure> {
        if self.pending_prompt.is_some() || !self.client.sessions.is_empty() {
            return Err(TransportFailure::protocol(ProtocolError::UnexpectedState));
        }
        self.session_configuration =
            crate::session_configuration::validate(configuration, self.claude_managed)?;
        Ok(())
    }
    /// Create a session in the explicitly approved workspace, without enabling
    /// arbitrary additional roots or automatically connecting MCP servers.
    pub fn new_session(&mut self, local_binding: &str) -> Result<String, TransportFailure> {
        if self.pending_prompt.is_some() {
            return Err(TransportFailure::protocol(ProtocolError::UnexpectedState));
        }
        self.client.ready().map_err(TransportFailure::protocol)?;
        if local_binding.is_empty() || local_binding.len() > 256 {
            return Err(TransportFailure::local(FailureKind::InvalidInstallation));
        }
        let cwd = self
            .workspace
            .to_str()
            .ok_or(TransportFailure::local(FailureKind::InvalidInstallation))?;
        let id = self
            .client
            .request_id()
            .map_err(TransportFailure::protocol)?;
        let mut params = self.session_configuration.clone();
        params["cwd"] = json!(cwd);
        let response = self
            .exchange(&json!({"jsonrpc":"2.0","id":id,"method":"session/new","params":params}))?;
        if response.get("error").is_some() {
            return Err(TransportFailure::protocol(ProtocolError::ProviderRejected));
        }
        let native_id = match response
            .pointer("/result/sessionId")
            .and_then(Value::as_str)
        {
            Some(id) => id.to_owned(),
            None => return self.fail(TransportFailure::protocol(ProtocolError::InvalidParams)),
        };
        if let Err(error) = self.client.register_session(&native_id, local_binding) {
            return self.fail(TransportFailure::protocol(error));
        }
        Ok(native_id)
    }
    /// Set the deadline for the next worker-only handshake/session operation or
    /// turn. Pending turns retain their original deadline and cannot be extended.
    pub fn set_io_timeout(&mut self, timeout: Duration) -> Result<(), TransportFailure> {
        if timeout.is_zero() || timeout > Duration::from_secs(60) {
            return Err(TransportFailure::protocol(ProtocolError::InvalidParams));
        }
        if self.pending_prompt.is_some() {
            return Err(TransportFailure::protocol(ProtocolError::UnexpectedState));
        }
        self.timeout = timeout;
        Ok(())
    }
    pub fn begin_prompt(&mut self, native_id: &str, text: &str) -> Result<(), TransportFailure> {
        if self.pending_prompt.is_some() {
            return Err(TransportFailure::protocol(ProtocolError::UnexpectedState));
        }
        let request = self
            .client
            .prompt(native_id, text)
            .map_err(TransportFailure::protocol)?;
        let request_id = request["id"].as_u64().unwrap();
        self.pending_prompt = Some(PendingPrompt {
            session: native_id.into(),
            request_id,
            deadline: Instant::now() + self.timeout,
            cancel_sent: false,
        });
        if let Err(error) = self
            .io
            .enqueue_frame(&request)
            .and_then(|_| self.io.flush_outbound())
        {
            return self.fail(error);
        }
        Ok(())
    }
    /// Pump a bounded quantum while retaining the original monotonic turn
    /// deadline. A quantum expiration is Ok(None), never a stop/retry signal.
    /// Workers should check their cancellation/budget commands between calls.
    pub fn poll_prompt(
        &mut self,
        native_id: &str,
        quantum: Duration,
    ) -> Result<Option<TurnState>, TransportFailure> {
        if quantum.is_zero() || quantum > Duration::from_secs(1) {
            return Err(TransportFailure::protocol(ProtocolError::InvalidParams));
        }
        let (request_id, turn_deadline) = match &self.pending_prompt {
            Some(pending) if pending.session == native_id => (pending.request_id, pending.deadline),
            _ => return Err(TransportFailure::protocol(ProtocolError::UnexpectedState)),
        };
        let deadline = turn_deadline.min(Instant::now() + quantum);
        loop {
            let frame = match self.io.next_frame(deadline) {
                Ok(value) => value,
                Err(error)
                    if error.kind == FailureKind::Timeout && Instant::now() < turn_deadline =>
                {
                    return Ok(None)
                }
                Err(error) => return self.fail(error),
            };
            let response = match self.handle_frame(frame, request_id) {
                Ok(Some(response)) => response,
                Ok(None) => continue,
                Err(error) => return self.fail(error),
            };
            self.pending_prompt = None;
            return match self.client.accept_prompt_result(native_id, response) {
                Ok(state) => Ok(Some(state)),
                Err(error) => self.fail(TransportFailure::protocol(error)),
            };
        }
    }
    pub fn wait_prompt(&mut self, native_id: &str) -> Result<TurnState, TransportFailure> {
        loop {
            if let Some(state) = self.poll_prompt(native_id, Duration::from_millis(25))? {
                return Ok(state);
            }
        }
    }
    /// Queue the notification once. Successful queueing remains a cancellation
    /// request until a correlated native stop result confirms its outcome.
    pub fn cancel(&mut self, native_id: &str) -> Result<(), TransportFailure> {
        match &self.pending_prompt {
            Some(pending) if pending.session == native_id && pending.cancel_sent => return Ok(()),
            Some(pending) if pending.session == native_id => {}
            _ => return Err(TransportFailure::protocol(ProtocolError::UnexpectedState)),
        }
        let request = self
            .client
            .cancel(native_id)
            .map_err(TransportFailure::protocol)?;
        if let Err(error) = self
            .io
            .enqueue_frame(&request)
            .and_then(|_| self.io.flush_outbound())
        {
            return self.fail(error);
        }
        self.pending_prompt.as_mut().unwrap().cancel_sent = true;
        Ok(())
    }
    pub fn take_updates(&mut self) -> Vec<ProviderEvent> {
        self.io.take_updates()
    }
    pub fn turn_state(&self, native_id: &str) -> Result<&TurnState, ProtocolError> {
        self.client.state(native_id)
    }
    pub fn process_id(&self) -> u32 {
        self.io.process_id()
    }
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.io.diagnostics
    }
    /// Raw, untrusted, potentially sensitive protocol evidence. Never forward to
    /// logs/UI without privacy/redaction policy. Truncation is explicit.
    pub fn take_evidence(&mut self) -> Vec<Value> {
        self.io.take_evidence()
    }
    /// Only this transport's owned process group is signalled, before reaping its
    /// leader. Holding the unreaped child prevents PID reuse during signalling.
    /// Processes intentionally escaping the group require OS-level supervision;
    /// this implementation does not claim an OS sandbox.
    pub fn terminate(&mut self) {
        self.client.disconnected();
        self.io.terminate();
    }
}
impl Drop for ProviderProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}
pub(crate) fn nonblocking(fd: RawFd) -> std::io::Result<()> {
    // SAFETY: each fd belongs to a live owned pipe for this process.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

impl std::fmt::Debug for ApprovedInstallation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovedInstallation")
            .field("executable", &self.executable)
            .field("expected_sha256", &self.expected_sha256)
            .field("argument_count", &self.arguments.len())
            .field(
                "environment_names",
                &self.environment.keys().collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

/// Readiness-based idle wait. The two-millisecond cap bounds exit/deadline
/// observation, but a producer wakeup returns immediately instead of imposing a
/// two-millisecond throughput floor on every small pipe refill.
pub(crate) fn wait_for_io(
    stdout: Option<RawFd>,
    stderr: Option<RawFd>,
    stdin: Option<RawFd>,
    deadline: Instant,
) -> std::io::Result<()> {
    let mut descriptors = [
        libc::pollfd {
            fd: stdout.unwrap_or(-1),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: stderr.unwrap_or(-1),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: stdin.unwrap_or(-1),
            events: libc::POLLOUT,
            revents: 0,
        },
    ];
    let nanos = deadline
        .saturating_duration_since(Instant::now())
        .as_nanos();
    let timeout_ms = nanos.div_ceil(1_000_000).min(2) as libc::c_int;
    // SAFETY: the supplied file descriptors are borrowed from live owned pipes;
    // poll only observes readiness and writes within the descriptors array.
    let result = unsafe {
        libc::poll(
            descriptors.as_mut_ptr(),
            descriptors.len() as libc::nfds_t,
            timeout_ms,
        )
    };
    if result < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() != ErrorKind::Interrupted {
            return Err(error);
        }
    }
    Ok(())
}
