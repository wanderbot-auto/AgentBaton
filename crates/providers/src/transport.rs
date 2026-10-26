//! Owned Unix stdio transport. No shell, executable discovery, model credentials
//! or automatic retries. Process separation and working directories are not a
//! filesystem/network sandbox. Callers must validate installation approval.
use crate::installation::{verify_executable, BinaryDigest, InstallationError};
use crate::{
    read_frame, AcpClient, PermissionDecision, ProtocolError, TurnState, UpdateKind,
    MAX_FRAME_BYTES,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::io::{Cursor, ErrorKind, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

const EVIDENCE_LIMIT: usize = MAX_FRAME_BYTES;
const RECORD_LIMIT: usize = 128;

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
            "ACP transport {:?}; effects {:?}",
            self.kind, self.side_effect_state
        )
    }
}
impl std::error::Error for TransportFailure {}
impl TransportFailure {
    fn local(kind: FailureKind) -> Self {
        Self {
            kind,
            side_effect_state: SideEffectState::None,
            protocol_error: None,
        }
    }
    fn unknown(kind: FailureKind) -> Self {
        Self {
            kind,
            side_effect_state: SideEffectState::Unknown,
            protocol_error: None,
        }
    }
    fn protocol(error: ProtocolError) -> Self {
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
struct PendingFrame {
    bytes: Vec<u8>,
    offset: usize,
}
pub struct ProviderProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    buffered: Vec<u8>,
    scan_from: usize,
    eof: bool,
    stderr_eof: bool,
    client: AcpClient,
    timeout: Duration,
    workspace: PathBuf,
    pending_prompt: Option<PendingPrompt>,
    outbound: VecDeque<PendingFrame>,
    updates: Vec<ProviderEvent>,
    evidence: Vec<Value>,
    diagnostics: Diagnostics,
}
impl ProviderProcess {
    pub fn start(
        installation: ApprovedInstallation,
        timeout: Duration,
    ) -> Result<Self, TransportFailure> {
        if !installation.executable.is_absolute()
            || !installation.workspace.is_absolute()
            || installation.approval_reference.trim().is_empty()
            || installation.approval_reference.len() > 256
            || installation.arguments.len() > 128
            || installation.arguments.iter().any(|s| s.len() > 64 * 1024)
            || timeout.is_zero()
            || timeout > Duration::from_secs(60)
            || !installation.workspace.is_dir()
            || !installation.executable.is_file()
        {
            return Err(TransportFailure::local(FailureKind::InvalidInstallation));
        }
        verify_executable(&installation.executable, &installation.expected_sha256).map_err(
            |error| {
                TransportFailure::local(match error {
                    InstallationError::DigestMismatch | InstallationError::FileChanged => {
                        FailureKind::BinaryChanged
                    }
                    _ => FailureKind::InvalidInstallation,
                })
            },
        )?;
        let mut command = Command::new(&installation.executable);
        command
            .args(&installation.arguments)
            .current_dir(&installation.workspace)
            .env_clear()
            .envs(&installation.environment)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = command
            .spawn()
            .map_err(|_| TransportFailure::local(FailureKind::StartupFailed))?;
        // All three pipes are guaranteed by the explicit Stdio::piped settings.
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let mut process = Self {
            child,
            stdin: Some(stdin),
            stdout,
            stderr,
            buffered: Vec::new(),
            scan_from: 0,
            eof: false,
            stderr_eof: false,
            client: AcpClient::new(),
            timeout,
            workspace: installation.workspace,
            pending_prompt: None,
            outbound: VecDeque::new(),
            updates: Vec::new(),
            evidence: Vec::new(),
            diagnostics: Diagnostics::default(),
        };
        for fd in [
            process.stdin.as_ref().unwrap().as_raw_fd(),
            process.stdout.as_raw_fd(),
            process.stderr.as_raw_fd(),
        ] {
            if nonblocking(fd).is_err() {
                return process.fail(TransportFailure::unknown(FailureKind::Io));
            }
        }
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
    fn fail<T>(&mut self, failure: TransportFailure) -> Result<T, TransportFailure> {
        self.client.disconnected();
        self.terminate();
        Err(failure)
    }
    fn evidence(&mut self, value: &Value) {
        let bytes = serde_json::to_vec(value)
            .map(|v| v.len())
            .unwrap_or(EVIDENCE_LIMIT + 1);
        if self.evidence.len() >= RECORD_LIMIT
            || self.diagnostics.evidence_bytes.saturating_add(bytes) > EVIDENCE_LIMIT
        {
            self.diagnostics.evidence_records_dropped =
                self.diagnostics.evidence_records_dropped.saturating_add(1);
            return;
        }
        self.diagnostics.evidence_bytes += bytes;
        self.evidence.push(value.clone());
    }
    fn drain_stderr(&mut self) -> Result<bool, TransportFailure> {
        if self.stderr_eof {
            return Ok(false);
        }
        let mut progressed = false;
        let mut chunk = [0u8; 4096];
        // Bound work per loop: an infinite stderr producer cannot starve stdout
        // parsing, timeout checks or cancellation.
        for _ in 0..16 {
            match self.stderr.read(&mut chunk) {
                Ok(0) => {
                    self.stderr_eof = true;
                    break;
                }
                Ok(n) => {
                    progressed = true;
                    self.diagnostics.stderr_bytes_observed = self
                        .diagnostics
                        .stderr_bytes_observed
                        .saturating_add(n as u64);
                    self.diagnostics.stderr_discarded = true;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return Err(TransportFailure::unknown(FailureKind::Io)),
            }
        }
        Ok(progressed)
    }
    fn enqueue_frame(&mut self, message: &Value) -> Result<(), TransportFailure> {
        let mut bytes = serde_json::to_vec(message)
            .map_err(|_| TransportFailure::protocol(ProtocolError::MalformedFrame))?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(TransportFailure::protocol(ProtocolError::FrameTooLarge));
        }
        if self.outbound.len() >= 32
            || self.diagnostics.outbound_bytes.saturating_add(bytes.len()) > MAX_FRAME_BYTES
        {
            return Err(TransportFailure::protocol(ProtocolError::ResourceLimit));
        }
        self.diagnostics.outbound_bytes += bytes.len();
        self.outbound.push_back(PendingFrame { bytes, offset: 0 });
        Ok(())
    }
    fn flush_outbound(&mut self) -> Result<(), TransportFailure> {
        for _ in 0..16 {
            let Some(frame) = self.outbound.front_mut() else {
                return Ok(());
            };
            let end = frame.bytes.len().min(frame.offset + 4096);
            match self
                .stdin
                .as_mut()
                .ok_or(TransportFailure::unknown(FailureKind::Closed))?
                .write(&frame.bytes[frame.offset..end])
            {
                Ok(0) => return Err(TransportFailure::unknown(FailureKind::Closed)),
                Ok(n) => {
                    frame.offset += n;
                    self.diagnostics.outbound_bytes -= n;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(()),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return Err(TransportFailure::unknown(FailureKind::Io)),
            }
            if frame.offset == frame.bytes.len() {
                self.outbound.pop_front();
            }
        }
        Ok(())
    }
    fn update(&mut self, event: ProviderEvent) -> Result<(), TransportFailure> {
        let bytes = serde_json::to_vec(&event)
            .map(|v| v.len())
            .unwrap_or(EVIDENCE_LIMIT + 1);
        if self.updates.len() >= RECORD_LIMIT
            || self.diagnostics.update_bytes.saturating_add(bytes) > EVIDENCE_LIMIT
        {
            if matches!(&event.kind, UpdateKind::PermissionDenied { .. }) {
                return Err(TransportFailure::protocol(ProtocolError::ResourceLimit));
            }
            self.diagnostics.update_records_dropped =
                self.diagnostics.update_records_dropped.saturating_add(1);
            return Ok(());
        }
        self.diagnostics.update_bytes += bytes;
        self.updates.push(event);
        Ok(())
    }
    fn next_frame(&mut self, deadline: Instant) -> Result<Value, TransportFailure> {
        loop {
            if Instant::now() >= deadline {
                return Err(TransportFailure::unknown(FailureKind::Timeout));
            }
            let stderr_progress = self.drain_stderr()?;
            let outbound_before = self.diagnostics.outbound_bytes;
            self.flush_outbound()?;
            let outbound_progress = self.diagnostics.outbound_bytes < outbound_before;
            if let Some(end) = self.buffered[self.scan_from..]
                .iter()
                .position(|b| *b == b'\n')
                .map(|offset| self.scan_from + offset)
            {
                if end + 1 > MAX_FRAME_BYTES {
                    return Err(TransportFailure::protocol(ProtocolError::FrameTooLarge));
                }
                let frame: Vec<_> = self.buffered.drain(..=end).collect();
                self.scan_from = 0;
                return read_frame(&mut Cursor::new(frame))
                    .map_err(TransportFailure::protocol)?
                    .ok_or(TransportFailure::unknown(FailureKind::Closed));
            }
            self.scan_from = self.buffered.len();
            if self.buffered.len() > MAX_FRAME_BYTES {
                return Err(TransportFailure::protocol(ProtocolError::FrameTooLarge));
            }
            if self.eof {
                return Err(TransportFailure::unknown(FailureKind::Closed));
            }
            let mut chunk = [0u8; 4096];
            match self.stdout.read(&mut chunk) {
                Ok(0) => self.eof = true,
                Ok(n) => self.buffered.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    if !stderr_progress && !outbound_progress {
                        wait_for_io(
                            Some(self.stdout.as_raw_fd()),
                            (!self.stderr_eof).then(|| self.stderr.as_raw_fd()),
                            (!self.outbound.is_empty())
                                .then(|| self.stdin.as_ref().unwrap().as_raw_fd()),
                            deadline,
                        )
                        .map_err(|_| TransportFailure::unknown(FailureKind::Io))?;
                    }
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return Err(TransportFailure::unknown(FailureKind::Io)),
            }
        }
    }
    fn handle_frame(
        &mut self,
        frame: Value,
        request_id: u64,
    ) -> Result<Option<Value>, TransportFailure> {
        self.evidence(&frame);
        if let Some(method) = frame.get("method").and_then(Value::as_str) {
            if method == "session/update" {
                let event = self
                    .client
                    .normalize_update(frame)
                    .map_err(TransportFailure::protocol)?;
                self.update(ProviderEvent {
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
                self.update(ProviderEvent {
                    session_id: session_id.into(),
                    local_binding,
                    kind: UpdateKind::PermissionDenied {
                        tool_call_id: frame["params"]["toolCall"]["toolCallId"]
                            .as_str()
                            .unwrap()
                            .into(),
                    },
                })?;
                self.enqueue_frame(&response)?;
            } else if let Some(id) = frame.get("id") {
                self.enqueue_frame(&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Client method unsupported"}}))?;
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
            let frame = self.next_frame(deadline)?;
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
        if let Err(error) = self.enqueue_frame(request) {
            return self.fail(error);
        }
        match self.receive_response(id, deadline) {
            Ok(value) => Ok(value),
            Err(error) => self.fail(error),
        }
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
        let response=self.exchange(&json!({"jsonrpc":"2.0","id":id,"method":"session/new","params":{"cwd":cwd,"mcpServers":[]}}))?;
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
            .enqueue_frame(&request)
            .and_then(|_| self.flush_outbound())
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
            let frame = match self.next_frame(deadline) {
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
            .enqueue_frame(&request)
            .and_then(|_| self.flush_outbound())
        {
            return self.fail(error);
        }
        self.pending_prompt.as_mut().unwrap().cancel_sent = true;
        Ok(())
    }
    pub fn take_updates(&mut self) -> Vec<ProviderEvent> {
        self.diagnostics.update_bytes = 0;
        std::mem::take(&mut self.updates)
    }
    pub fn turn_state(&self, native_id: &str) -> Result<&TurnState, ProtocolError> {
        self.client.state(native_id)
    }
    pub fn process_id(&self) -> u32 {
        self.child.id()
    }
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }
    /// Raw, untrusted, potentially sensitive protocol evidence. Never forward to
    /// logs/UI without privacy/redaction policy. Truncation is explicit.
    pub fn take_evidence(&mut self) -> Vec<Value> {
        self.diagnostics.evidence_bytes = 0;
        std::mem::take(&mut self.evidence)
    }
    /// Only this transport's owned process group is signalled, before reaping its
    /// leader. Holding the unreaped child prevents PID reuse during signalling.
    /// Processes intentionally escaping the group require OS-level supervision;
    /// this implementation does not claim an OS sandbox.
    pub fn terminate(&mut self) {
        self.client.disconnected();
        self.stdin.take();
        self.outbound.clear();
        self.diagnostics.outbound_bytes = 0;
        if !self.diagnostics.process_reaped {
            // SAFETY: command.process_group(0) creates a group led by our direct
            // child; we never reap it before this group signal.
            unsafe {
                libc::kill(-(self.child.id() as i32), libc::SIGKILL);
            }
            let _ = self.child.kill();
            self.diagnostics.process_reaped = self.child.wait().is_ok();
        }
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
