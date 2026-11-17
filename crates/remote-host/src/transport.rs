use crate::{
    protocol::*,
    registration::{ssh_path_syntax, validate_ssh_endpoint_syntax},
    store::private_dir,
};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

pub const IO_TIMEOUT: Duration = Duration::from_secs(2);
pub fn nonblocking(fd: i32) -> Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    require(
        flags >= 0 && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } >= 0,
        "io",
    )
}
fn ready(fd: i32, events: i16, deadline: Instant) -> Result<()> {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        require(!left.is_zero(), "timeout")?;
        let mut p = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let n = unsafe { libc::poll(&mut p, 1, left.as_millis().clamp(1, 2000) as i32) };
        if n > 0 {
            return Ok(());
        }
        if n < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return Err(Error::new("io"));
        }
    }
}
pub fn input_ready(fd: i32) -> Result<bool> {
    let mut p = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let n = unsafe { libc::poll(&mut p, 1, 100) };
    if n < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
        return Err(Error::new("io"));
    }
    Ok(n > 0)
}
pub fn write_frame<W: Write + AsRawFd>(out: &mut W, bytes: &[u8]) -> Result<()> {
    require(
        bytes.len() <= MAX_FRAME && !bytes.contains(&b'\n'),
        "oversize",
    )?;
    let mut frame = bytes.to_vec();
    frame.push(b'\n');
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut pos = 0;
    while pos < frame.len() {
        ready(out.as_raw_fd(), libc::POLLOUT, deadline)?;
        match out.write(&frame[pos..]) {
            Ok(0) => return Err(Error::new("disconnected")),
            Ok(n) => pos += n,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(Error::new("disconnected")),
        }
    }
    Ok(())
}
/// Byte-at-a-time reads avoid buffering arbitrary subsequent frames; poll bounds every wait.
pub fn read_frame<R: Read + AsRawFd>(input: &mut R) -> Result<Vec<u8>> {
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut bytes = Vec::new();
    loop {
        ready(input.as_raw_fd(), libc::POLLIN, deadline)?;
        let mut b = [0];
        match input.read(&mut b) {
            Ok(0) => {
                return Err(Error::new(if bytes.is_empty() {
                    "disconnected"
                } else {
                    "half_frame"
                }))
            }
            Ok(_) => {
                if b[0] == b'\n' {
                    return Ok(bytes);
                }
                bytes.push(b[0]);
                require(bytes.len() <= MAX_FRAME, "oversize")?;
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(Error::new("disconnected")),
        }
    }
}

/// SSH settings are supplied by trusted user configuration, never an RPC message.
#[derive(Clone)]
pub struct SshConfig {
    pub expected_host_identity: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub host_key_alias: String,
    pub known_hosts: PathBuf,
    pub known_hosts_digest: String,
    pub identity_file: PathBuf,
    pub remote_config: String,
}
impl SshConfig {
    pub fn argv(&self) -> Result<Vec<String>> {
        validate_ssh_endpoint_syntax(
            &self.expected_host_identity,
            &self.host,
            &self.user,
            self.port,
            &self.host_key_alias,
        )?;
        require(ssh_path_syntax(&self.remote_config), "ssh_remote_path")?;
        let known = self
            .known_hosts
            .to_str()
            .ok_or_else(|| Error::new("ssh_path"))?;
        let identity = self
            .identity_file
            .to_str()
            .ok_or_else(|| Error::new("ssh_path"))?;
        require(
            ssh_path_syntax(known) && ssh_path_syntax(identity),
            "ssh_path",
        )?;
        // Explicit private task/config directory, no default ~/.ssh discovery or config execution.
        for path in [&self.known_hosts, &self.identity_file] {
            private_dir(path.parent().ok_or_else(|| Error::new("ssh_path"))?)?;
            let m = path.symlink_metadata()?;
            require(
                m.is_file()
                    && !m.file_type().is_symlink()
                    && m.uid() == unsafe { libc::geteuid() }
                    && m.mode() & 0o077 == 0
                    && m.len() <= 65536,
                "ssh_file",
            )?;
        }
        let known_file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&self.known_hosts)?;
        let mut known_bytes = Vec::new();
        known_file.take(65537).read_to_end(&mut known_bytes)?;
        require(known_bytes.len() <= 65536, "ssh_file")?;
        require(
            digest(&known_bytes) == self.known_hosts_digest,
            "host_key_drift",
        )?;
        let mut args: Vec<String> = [
            "-F",
            "none",
            "-T",
            "-a",
            "-x",
            "-oBatchMode=yes",
            "-oStrictHostKeyChecking=yes",
            "-oUpdateHostKeys=no",
            "-oGlobalKnownHostsFile=/dev/null",
            "-oIdentitiesOnly=yes",
            "-oIdentityAgent=none",
            "-oCertificateFile=none",
            "-oAddKeysToAgent=no",
            "-oPasswordAuthentication=no",
            "-oKbdInteractiveAuthentication=no",
            "-oPreferredAuthentications=publickey",
            "-oClearAllForwardings=yes",
            "-oPermitLocalCommand=no",
            "-oControlMaster=no",
            "-oControlPath=none",
            "-oConnectTimeout=5",
            "-oConnectionAttempts=1",
            "-oServerAliveInterval=2",
            "-oServerAliveCountMax=2",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        args.extend([
            format!("-oUserKnownHostsFile={known}"),
            format!("-oHostKeyAlias={}", self.host_key_alias),
            "-i".into(),
            identity.into(),
            "-p".into(),
            self.port.to_string(),
            "-l".into(),
            self.user.clone(),
            "--".into(),
            self.host.clone(),
        ]);
        // OpenSSH joins remote argv through a shell. All variable characters are allowlisted;
        // executable is fixed, config path is absolute, no RPC payload enters this command.
        args.push(format!(
            "exec /usr/local/libexec/baton-host-companion --serve {}",
            self.remote_config
        ));
        Ok(args)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkState {
    Connected,
    Disconnected,
    ReconcileRequired,
}
/// A transport failure never asserts task failure or successful remote cancellation.
pub struct Client {
    child: Child,
    input: Option<ChildStdin>,
    output: ChildStdout,
    pub link: LinkState,
    pub cancel_requested: bool,
    reconciled: bool,
    expected_host: String,
    reconciled_binding: Option<(Binding, i64, String)>,
}
impl Client {
    pub fn local(binary: &Path, config: &Path) -> Result<Self> {
        let mut cmd = Command::new(binary);
        cmd.arg("--serve").arg(config);
        let trusted = crate::store::load_config(config)?;
        Self::spawn(cmd, trusted.host_identity)
    }
    pub fn ssh(config: &SshConfig) -> Result<Self> {
        let mut cmd = Command::new("/usr/bin/ssh");
        cmd.args(config.argv()?);
        Self::spawn(cmd, config.expected_host_identity.clone())
    }
    fn spawn(mut cmd: Command, expected_host: String) -> Result<Self> {
        let mut child = cmd
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child.stdin.take().ok_or_else(|| Error::new("io"))?;
        let output = child.stdout.take().ok_or_else(|| Error::new("io"))?;
        if let Err(e) = nonblocking(input.as_raw_fd()).and_then(|_| nonblocking(output.as_raw_fd()))
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
        Ok(Self {
            child,
            input: Some(input),
            output,
            link: LinkState::ReconcileRequired,
            cancel_requested: false,
            reconciled: false,
            expected_host,
            reconciled_binding: None,
        })
    }
    pub fn request(&mut self, request: &Request) -> Result<Reply> {
        require(
            request.host_identity == self.expected_host
                && request.binding.host == self.expected_host
                && request.binding.recipient == self.expected_host,
            "transport_host_binding",
        )?;
        if matches!(request.action, Action::Cancel) {
            self.cancel_requested = true;
        }
        require(
            (self.reconciled
                && self.reconciled_binding.as_ref()
                    == Some(&(
                        request.binding.clone(),
                        request.fence,
                        request.controller.clone(),
                    )))
                || matches!(
                    request.action,
                    Action::Reconcile { .. } | Action::Cancel | Action::Revoke
                ),
            "reconcile_required",
        )?;
        let result: Result<Reply> = (|| {
            write_frame(
                self.input
                    .as_mut()
                    .ok_or_else(|| Error::new("disconnected"))?,
                &serde_json::to_vec(request)?,
            )?;
            let response: Response = serde_json::from_slice(&read_frame(&mut self.output)?)?;
            require(
                response.version == VERSION && response.request_id == request.request_id,
                "response_binding",
            )?;
            let reply = response.result?;
            if let Reply::Snapshot { snapshot } = &reply {
                require(
                    snapshot.binding == request.binding && snapshot.fence == request.fence,
                    "response_binding",
                )?;
            }
            Ok(reply)
        })();
        match &result {
            Ok(Reply::Snapshot { snapshot }) => {
                self.reconciled = !snapshot.more;
                self.reconciled_binding = Some((
                    request.binding.clone(),
                    request.fence,
                    request.controller.clone(),
                ));
                self.link = if snapshot.more {
                    LinkState::ReconcileRequired
                } else {
                    LinkState::Connected
                };
            }
            Err(e)
                if [
                    "disconnected",
                    "half_frame",
                    "timeout",
                    "protocol",
                    "response_binding",
                    "oversize",
                ]
                .contains(&e.code.as_str()) =>
            {
                self.reconciled = false;
                self.link = LinkState::Disconnected;
            }
            _ => {}
        }
        result
    }
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
    pub fn close(&mut self) -> Result<()> {
        self.input.take(); // EOF: companion joins every bounded worker before it exits.
        let deadline = Instant::now() + Duration::from_secs(8);
        while self.child.try_wait()?.is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if self.child.try_wait()?.is_none() {
            unsafe {
                libc::kill(-(self.child.id() as i32), libc::SIGKILL);
            }
        }
        self.child.wait()?;
        self.link = LinkState::Disconnected;
        self.reconciled = false;
        Ok(())
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
