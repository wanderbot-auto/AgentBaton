//! Owned Unix stdio transport. No shell, executable discovery, model credentials
//! or automatic retries. Process separation and working directories are not a
//! filesystem/network sandbox. Callers must validate installation approval.
use crate::{read_frame, AcpClient, PermissionDecision, ProtocolError, TurnState, MAX_FRAME_BYTES};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
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
#[derive(Debug, Clone)]
pub struct ApprovedInstallation {
    pub executable: PathBuf,
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
}

pub struct ProviderProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    stderr_eof: bool,
    buffered: Vec<u8>,
    scan_from: usize,
    eof: bool,
    client: AcpClient,
    timeout: Duration,
    workspace: PathBuf,
    pending_prompt: Option<(String, u64)>,
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
            stderr_eof: false,
            buffered: Vec::new(),
            scan_from: 0,
            eof: false,
            client: AcpClient::new(),
            timeout,
            workspace: installation.workspace,
            pending_prompt: None,
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
        let mut chunk = [0u8; 4096];
        let mut progressed = false;
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
    fn wait_ready(&self, deadline: Instant, writing: bool) -> Result<(), TransportFailure> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(TransportFailure::unknown(FailureKind::Timeout));
        }
        let (fd, events) = if writing {
            (
                self.stdin
                    .as_ref()
                    .ok_or(TransportFailure::unknown(FailureKind::Closed))?
                    .as_raw_fd(),
                libc::POLLOUT,
            )
        } else {
            (self.stdout.as_raw_fd(), libc::POLLIN)
        };
        let mut descriptors = [
            libc::pollfd {
                fd,
                events,
                revents: 0,
            },
            libc::pollfd {
                // poll ignores a negative descriptor; EOF must not produce
                // an always-ready HUP loop while stdout is still idle.
                fd: if self.stderr_eof {
                    -1
                } else {
                    self.stderr.as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // Wake when a small pipe refills instead of imposing a 2ms throughput
        // floor on every producer/consumer scheduling handoff.
        let timeout = remaining.min(Duration::from_millis(2)).as_millis().max(1) as i32;
        // SAFETY: both descriptors are borrowed from live owned process pipes.
        let result = unsafe {
            libc::poll(
                descriptors.as_mut_ptr(),
                descriptors.len() as libc::nfds_t,
                timeout,
            )
        };
        if result < 0 && std::io::Error::last_os_error().kind() != ErrorKind::Interrupted {
            return Err(TransportFailure::unknown(FailureKind::Io));
        }
        Ok(())
    }
    fn write_frame(&mut self, message: &Value, deadline: Instant) -> Result<(), TransportFailure> {
        let mut bytes = serde_json::to_vec(message)
            .map_err(|_| TransportFailure::protocol(ProtocolError::MalformedFrame))?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(TransportFailure::protocol(ProtocolError::FrameTooLarge));
        }
        let mut offset = 0;
        while offset < bytes.len() {
            if Instant::now() >= deadline {
                return Err(TransportFailure::unknown(FailureKind::Timeout));
            }
            let stderr_progress = self.drain_stderr()?;
            match self
                .stdin
                .as_mut()
                .ok_or(TransportFailure::unknown(FailureKind::Closed))?
                .write(&bytes[offset..])
            {
                Ok(0) => return Err(TransportFailure::unknown(FailureKind::Closed)),
                Ok(n) => offset += n,
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    if stderr_progress {
                        std::thread::yield_now();
                    } else {
                        self.wait_ready(deadline, true)?;
                    }
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return Err(TransportFailure::unknown(FailureKind::Io)),
            }
        }
        Ok(())
    }
    fn next_frame(&mut self, deadline: Instant) -> Result<Value, TransportFailure> {
        loop {
            if Instant::now() >= deadline {
                return Err(TransportFailure::unknown(FailureKind::Timeout));
            }
            let stderr_progress = self.drain_stderr()?;
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
                    // Small Darwin pipes need more drain iterations. Sleeping
                    // after useful stderr work artificially throttles a noisy
                    // child below its unchanged response deadline.
                    if stderr_progress {
                        std::thread::yield_now();
                    } else {
                        self.wait_ready(deadline, false)?;
                    }
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return Err(TransportFailure::unknown(FailureKind::Io)),
            }
        }
    }
    fn receive_response(
        &mut self,
        request_id: u64,
        deadline: Instant,
    ) -> Result<Value, TransportFailure> {
        loop {
            let frame = self.next_frame(deadline)?;
            self.evidence(&frame);
            if let Some(method) = frame.get("method").and_then(Value::as_str) {
                if method == "session/update" {
                    self.client
                        .normalize_update(frame)
                        .map_err(TransportFailure::protocol)?;
                } else if method == "session/request_permission" {
                    // Until a trusted permission-policy callback is integrated,
                    // every request is denied rather than inferred from labels.
                    let response = self
                        .client
                        .permission_response(&frame, PermissionDecision::Cancelled)
                        .map_err(TransportFailure::protocol)?;
                    self.write_frame(&response, deadline)?;
                } else if let Some(id) = frame.get("id") {
                    self.write_frame(&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Client method unsupported"}}),deadline)?;
                }
                continue;
            }
            if frame.get("id").and_then(Value::as_u64) != Some(request_id) {
                return Err(TransportFailure::protocol(ProtocolError::UnmatchedResponse));
            }
            return Ok(frame);
        }
    }
    fn exchange(&mut self, request: &Value) -> Result<Value, TransportFailure> {
        let deadline = Instant::now() + self.timeout;
        let id = request
            .get("id")
            .and_then(Value::as_u64)
            .ok_or(TransportFailure::protocol(ProtocolError::InvalidEnvelope))?;
        if let Err(error) = self.write_frame(request, deadline) {
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
    pub fn begin_prompt(&mut self, native_id: &str, text: &str) -> Result<(), TransportFailure> {
        if self.pending_prompt.is_some() {
            return Err(TransportFailure::protocol(ProtocolError::UnexpectedState));
        }
        let request = self
            .client
            .prompt(native_id, text)
            .map_err(TransportFailure::protocol)?;
        let id = request["id"].as_u64().unwrap();
        self.pending_prompt = Some((native_id.into(), id));
        if let Err(error) = self.write_frame(&request, Instant::now() + self.timeout) {
            return self.fail(error);
        }
        Ok(())
    }
    pub fn wait_prompt(&mut self, native_id: &str) -> Result<TurnState, TransportFailure> {
        let id = match &self.pending_prompt {
            Some((session, id)) if session == native_id => *id,
            _ => return Err(TransportFailure::protocol(ProtocolError::UnexpectedState)),
        };
        let response = match self.receive_response(id, Instant::now() + self.timeout) {
            Ok(value) => value,
            Err(error) => return self.fail(error),
        };
        self.pending_prompt = None;
        match self.client.accept_prompt_result(native_id, response) {
            Ok(state) => Ok(state),
            Err(error) => self.fail(TransportFailure::protocol(error)),
        }
    }
    pub fn cancel(&mut self, native_id: &str) -> Result<(), TransportFailure> {
        let request = self
            .client
            .cancel(native_id)
            .map_err(TransportFailure::protocol)?;
        if let Err(error) = self.write_frame(&request, Instant::now() + self.timeout) {
            return self.fail(error);
        }
        Ok(())
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
fn nonblocking(fd: RawFd) -> std::io::Result<()> {
    // SAFETY: each fd belongs to a live owned pipe for this process.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
