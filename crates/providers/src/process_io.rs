//! Shared, codec-independent owned subprocess and bounded nonblocking pump.
use crate::installation::{verify_executable, InstallationError};
use crate::transport::{
    nonblocking, wait_for_io, ApprovedInstallation, Diagnostics, FailureKind, ProviderEvent,
    TransportFailure,
};
use crate::{ProtocolError, UpdateKind, MAX_FRAME_BYTES};
use serde_json::Value;
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};
const EVIDENCE_LIMIT: usize = MAX_FRAME_BYTES;
const RECORD_LIMIT: usize = 128;
struct PendingFrame {
    bytes: Vec<u8>,
    offset: usize,
}
pub(crate) struct OwnedProcessIo {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    buffered: Vec<u8>,
    scan_from: usize,
    eof: bool,
    stderr_eof: bool,
    outbound: VecDeque<PendingFrame>,
    updates: Vec<ProviderEvent>,
    evidence: Vec<Value>,
    pub(crate) diagnostics: Diagnostics,
}
impl OwnedProcessIo {
    pub(crate) fn spawn(
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
                process.terminate();
                return Err(TransportFailure::unknown(FailureKind::Io));
            }
        }
        Ok(process)
    }
    pub(crate) fn evidence(&mut self, value: &Value) {
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
    pub(crate) fn enqueue_frame(&mut self, message: &Value) -> Result<(), TransportFailure> {
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
    pub(crate) fn flush_outbound(&mut self) -> Result<(), TransportFailure> {
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
    pub(crate) fn update(&mut self, event: ProviderEvent) -> Result<(), TransportFailure> {
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
    pub(crate) fn next_frame(&mut self, deadline: Instant) -> Result<Value, TransportFailure> {
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
                return crate::framing::parse(&frame)
                    .map_err(|_| TransportFailure::protocol(ProtocolError::MalformedFrame));
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

    pub(crate) fn process_id(&self) -> u32 {
        self.child.id()
    }
    pub(crate) fn take_updates(&mut self) -> Vec<ProviderEvent> {
        self.diagnostics.update_bytes = 0;
        std::mem::take(&mut self.updates)
    }
    pub(crate) fn take_evidence(&mut self) -> Vec<Value> {
        self.diagnostics.evidence_bytes = 0;
        std::mem::take(&mut self.evidence)
    }
    pub fn terminate(&mut self) {
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
impl Drop for OwnedProcessIo {
    fn drop(&mut self) {
        self.terminate();
    }
}
