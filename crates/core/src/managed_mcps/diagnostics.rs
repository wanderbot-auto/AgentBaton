//! Standalone, owner-confirmed MCP stdio diagnostics. No Provider or session is
//! opened here. Only trusted in-process code may turn a review into authority.
use super::*;
use crate::registry::Project;
use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
use std::{
    collections::BTreeSet,
    fs::{self, DirBuilder},
    io::{self, Read, Write},
    os::unix::{fs::DirBuilderExt, process::ExitStatusExt},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

const JOURNAL_SCOPE: &str = "managed_mcp_diagnostic:v1";
const PENDING: &str = "pending";
const REVIEW_TTL: Duration = Duration::from_secs(60);
const IO_TIMEOUT: Duration = Duration::from_secs(4);
const EXIT_GRACE: Duration = Duration::from_millis(150);
const KILL_GRACE: Duration = Duration::from_millis(350);
const TICK: Duration = Duration::from_millis(5);
const MAX_FRAME: usize = 64 * 1024;
const MAX_STDOUT: usize = 4 * MAX_FRAME;
const MAX_STDERR: usize = 4 * MAX_FRAME;
const MAX_NOTIFICATIONS: usize = 32;
const MAX_PAGES: usize = 4;
const MAX_TOOLS: usize = 128;
const PROTOCOL: &str = "2025-06-18";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpDiagnosticRequest {
    pub project_id: String,
    pub project_revision: u64,
    pub connection_id: String,
    pub connection_revision: u64,
    pub server_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpDiagnosticReview {
    pub diagnostic_id: String,
    pub request: ManagedMcpDiagnosticRequest,
    pub project_name: String,
    pub project_digest: String,
    pub connection_name: String,
    pub connection_digest: String,
    pub config_revision: String,
    pub runtime_plan_digest: String,
    pub host_id: String,
    pub transport: String,
    pub server: ManagedMcpSummary,
    pub executable_identity: Value,
    pub arguments: Vec<String>,
    pub actions: [String; 3],
    pub environment: String,
    pub io_timeout_ms: u64,
    pub expires_at_ms: i64,
    pub warning: String,
}

#[derive(Debug)]
pub struct PendingMcpDiagnostic {
    review: ManagedMcpDiagnosticReview,
    review_digest: String,
    expires: Instant,
}

impl PendingMcpDiagnostic {
    pub fn review(&self) -> &ManagedMcpDiagnosticReview {
        &self.review
    }

    pub fn review_digest(&self) -> &str {
        &self.review_digest
    }

    /// Must be called only by the trusted owner-confirmation adapter after
    /// displaying the complete review and receiving affirmative consent.
    /// A renderer-provided digest, configuration approval, or Grant is not consent.
    pub fn confirm_after_owner_dialog(
        self,
        accepted_digest: Option<&str>,
    ) -> Result<OwnerConfirmedMcpDiagnostic> {
        if accepted_digest != Some(self.review_digest()) {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Active MCP diagnosis requires separate confirmation of this exact review",
            ));
        }
        self.check_expiry()?;
        Ok(OwnerConfirmedMcpDiagnostic(self))
    }

    #[cfg(test)]
    pub(super) fn expire_for_test(&mut self) {
        self.expires = Instant::now();
    }

    fn check_expiry(&self) -> Result<()> {
        if Instant::now() >= self.expires || ms()? >= self.review.expires_at_ms {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Active MCP diagnostic confirmation expired",
            ));
        }
        Ok(())
    }
}

/// One-use native confirmation. There is no deserialization or public constructor.
#[derive(Debug)]
pub struct OwnerConfirmedMcpDiagnostic(PendingMcpDiagnostic);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpDiagnosticOutcome {
    Enumerated,
    Cancelled,
    TimedOut,
    ApprovalExpired,
    InputChanged,
    StartupFailed,
    ProtocolFailed,
    UnsupportedProtocol,
    ToolsUnsupported,
    ResourceLimit,
    ProcessFailed,
    IoFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum McpDiagnosticExit {
    NotStarted,
    Reaped {
        exit_code: Option<i32>,
        signal: Option<i32>,
        kill_requested: bool,
    },
    Unknown {
        kill_requested: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpDiagnosticReceipt {
    pub diagnostic_id: String,
    pub review_digest: String,
    pub evidence_source: String,
    pub outcome: McpDiagnosticOutcome,
    pub direct_child: McpDiagnosticExit,
    pub protocol_version: Option<String>,
    pub runtime_snapshot_digest: Option<String>,
    pub configured_tools_seen: Vec<String>,
    pub other_tool_count: Option<usize>,
    pub stderr_bytes_observed: usize,
}

impl ManagedMcpDiagnosticReceipt {
    /// In-process evidence of a completed list with a reaped direct child.
    /// Use a committed record or a separate journal check for durable evidence.
    pub fn verified(&self) -> bool {
        self.outcome == McpDiagnosticOutcome::Enumerated
            && matches!(self.direct_child, McpDiagnosticExit::Reaped { .. })
    }
}

/// Keeps ownership if the diagnostic gets no proof of direct-child exit.
/// The caller retains this in-process handle until a receipt commits and the
/// child is reaped. Never reconstruct a kill target from journal text or a PID.
#[must_use]
#[derive(Debug)]
pub struct ManagedMcpDiagnosticAttempt {
    receipt: ManagedMcpDiagnosticReceipt,
    child: Option<Child>,
    recorded: bool,
}

impl ManagedMcpDiagnosticAttempt {
    pub fn receipt(&self) -> &ManagedMcpDiagnosticReceipt {
        &self.receipt
    }

    pub fn has_unreaped_child(&self) -> bool {
        self.child.is_some()
    }

    pub fn journal_recorded(&self) -> bool {
        self.recorded
    }

    /// Poll only the exact retained child and retry a failed receipt commit.
    pub fn reconcile(&mut self, store: &mut Store) -> Result<()> {
        if let Some(child) = self.child.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                let McpDiagnosticExit::Unknown { kill_requested } = self.receipt.direct_child
                else {
                    unreachable!("only an unknown exit can own a live child handle");
                };
                self.receipt.direct_child = reaped(status, kill_requested);
                qualify_exit(&mut self.receipt);
                self.child = None;
                self.recorded = false;
            }
        }
        if !self.recorded {
            record_receipt(store, &self.receipt)?;
            self.recorded = true;
        }
        Ok(())
    }
}

impl Drop for ManagedMcpDiagnosticAttempt {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            // Unsettled evidence stays pending. This fallback has no authority
            // to signal a process group or to assert cleanup from a kill request.
            let _ = child.kill();
            let _ = child.try_wait();
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpDiagnosticRecord {
    pub review: ManagedMcpDiagnosticReview,
    pub review_digest: String,
    /// None after a committed launch intent with no durable result. Never
    /// interpret this state as a failed spawn or automatically repeat it.
    pub receipt: Option<ManagedMcpDiagnosticReceipt>,
}

impl Store {
    /// Explicit read-only preview. Merely listing or scanning capabilities does
    /// not call this API or run the approved executable.
    pub fn preview_managed_mcp_diagnostic(
        &self,
        request: &ManagedMcpDiagnosticRequest,
    ) -> Result<PendingMcpDiagnostic> {
        let expires = Instant::now() + REVIEW_TTL;
        let expires_at_ms = ms()? + REVIEW_TTL.as_millis() as i64;
        let review = review(
            &self.conn,
            request,
            &Uuid::new_v4().to_string(),
            expires_at_ms,
        )?;
        let review_digest = digest(&review)?;
        Ok(PendingMcpDiagnostic {
            review,
            review_digest,
            expires,
        })
    }

    /// Runs bounded stdio I/O on the owning thread. An IPC adapter must schedule
    /// this outside its request loop and supply a trusted private runtime root.
    pub fn run_managed_mcp_diagnostic(
        &mut self,
        approval: OwnerConfirmedMcpDiagnostic,
        private_runtime: &Path,
        cancel: &AtomicBool,
    ) -> Result<ManagedMcpDiagnosticAttempt> {
        let pending = approval.0;
        pending.check_expiry()?;
        let cancelled_before_admission = cancel.load(Ordering::Acquire);
        if !cancelled_before_admission {
            private_directory(private_runtime)?;
        }

        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        recheck(&tx, &pending)?;
        pending.check_expiry()?;
        journal_intent(&tx, &pending)?;
        tx.commit().map_err(db_error)?;

        let mut attempt = ManagedMcpDiagnosticAttempt {
            receipt: ManagedMcpDiagnosticReceipt {
                diagnostic_id: pending.review.diagnostic_id.clone(),
                review_digest: pending.review_digest.clone(),
                evidence_source: "standalone_local_mcp_diagnostic".into(),
                outcome: McpDiagnosticOutcome::Cancelled,
                direct_child: McpDiagnosticExit::NotStarted,
                protocol_version: None,
                runtime_snapshot_digest: None,
                configured_tools_seen: vec![],
                other_tool_count: None,
                stderr_bytes_observed: 0,
            },
            child: None,
            recorded: false,
        };
        if !cancelled_before_admission && !cancel.load(Ordering::Acquire) {
            match private_paths(private_runtime, &pending.review.diagnostic_id) {
                Ok(paths) => launch(&self.conn, &pending, paths, cancel, &mut attempt),
                Err(_) => attempt.receipt.outcome = McpDiagnosticOutcome::StartupFailed,
            }
        }
        // A failed SQLite write must not discard a collected receipt or an
        // unreaped child. The durable launch marker keeps further runs blocked.
        let _ = attempt.reconcile(self);
        Ok(attempt)
    }

    /// An open intent covers live I/O, a pending receipt, or an unknown
    /// direct-child outcome. Update admission must consider this alongside
    /// the actor's live ownership state.
    pub fn has_unresolved_managed_mcp_diagnostic(&self) -> Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM idempotency WHERE scope=?1 AND key=?2)",
                params![JOURNAL_SCOPE, PENDING],
                |row| row.get(0),
            )
            .map_err(db_error)
    }

    pub fn get_managed_mcp_diagnostic(
        &self,
        diagnostic_id: &str,
    ) -> Result<ManagedMcpDiagnosticRecord> {
        if !Uuid::parse_str(diagnostic_id).is_ok_and(|id| id.to_string() == diagnostic_id) {
            return Err(invalid("Expected an exact diagnostic identifier"));
        }
        diagnostic_record(&self.conn, diagnostic_id)
    }
}

fn review(
    db: &Connection,
    request: &ManagedMcpDiagnosticRequest,
    id: &str,
    expires_at_ms: i64,
) -> Result<ManagedMcpDiagnosticReview> {
    key_valid(&request.project_id)?;
    let raw: Option<String> = db
        .query_row(
            "SELECT document FROM projects WHERE id=?1 AND octet_length(document)<=1048576",
            [&request.project_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let project: Project = decode(&raw.ok_or_else(|| changed())?)?;
    if project.id != request.project_id
        || project.revision != request.project_revision
        || project.archived
    {
        return Err(changed());
    }
    let connection = read_connection(db, &request.connection_id)?;
    if connection.revision != request.connection_revision || connection.probe_state != "ready" {
        return Err(changed());
    }
    if connection.host_id != "local" {
        return Err(CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "Active MCP diagnostics currently support only the managed local stdio transport",
        ));
    }
    let profile = profile(&connection)?;
    let server = profile.managed_mcp.ok_or_else(|| {
        CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "No supported managed stdio MCP server at this connection revision",
        )
    })?;
    if server.name != request.server_name {
        return Err(changed());
    }
    let source = source_binding(&server.executable)?;
    if source["executable_sha256"] != server.executable_sha256 {
        return Err(changed());
    }
    Ok(ManagedMcpDiagnosticReview {
        diagnostic_id: id.into(),
        request: request.clone(),
        project_digest: digest(&project)?,
        project_name: project.name,
        connection_digest: digest(&connection)?,
        connection_name: connection.name,
        config_revision: profile.config_revision,
        runtime_plan_digest: profile.runtime_plan_digest,
        host_id: connection.host_id,
        transport: "stdio".into(),
        server,
        executable_identity: source["identity"].clone(),
        // The existing managed Codex MCP plan accepts precisely empty argv.
        arguments: vec![],
        actions: ["initialize", "notifications/initialized", "tools/list"].map(str::to_owned),
        environment: "fresh_baton_managed_state_without_credentials".into(),
        io_timeout_ms: IO_TIMEOUT.as_millis() as u64,
        expires_at_ms,
        warning: "This starts the approved local executable under your operating system account. The working directory and managed environment are private. Native code can still access files, network, or start descendants using your account permissions. This diagnostic verifies the selected entry file at observation, and can only observe its direct child and the approved protocol operations. Interpreters, dependencies and later filesystem changes are outside that verification.".into(),
    })
}

fn recheck(db: &Connection, pending: &PendingMcpDiagnostic) -> Result<()> {
    let current = review(
        db,
        &pending.review.request,
        &pending.review.diagnostic_id,
        pending.review.expires_at_ms,
    )?;
    if digest(&current)? != pending.review_digest {
        return Err(changed());
    }
    Ok(())
}

fn journal_intent(tx: &Transaction<'_>, pending: &PendingMcpDiagnostic) -> Result<()> {
    let id = &pending.review.diagnostic_id;
    let previous: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM idempotency WHERE scope=?1 AND key IN (?2,?3))",
            params![JOURNAL_SCOPE, PENDING, id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if previous {
        return Err(CoreError::new(
            "DIAGNOSTIC_RECONCILIATION_REQUIRED",
            "A prior diagnostic intent must be reconciled before starting another",
        ));
    }
    tx.execute(
        "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,'null'),(?1,?4,?2,'null')",
        params![JOURNAL_SCOPE, id, encode(&pending.review)?, PENDING],
    )
    .map_err(db_error)?;
    event(
        tx,
        id,
        1,
        "mcp_diagnostic.launch_intent",
        json!({"review":pending.review,"review_digest":pending.review_digest}),
        "local_owner_confirmation",
    )
}

fn diagnostic_record(db: &Connection, id: &str) -> Result<ManagedMcpDiagnosticRecord> {
    let record: Option<(String, String)> = db
        .query_row(
            "SELECT payload,result FROM idempotency WHERE scope=?1 AND key=?2
             AND octet_length(payload)<=32768 AND octet_length(result)<=16384",
            params![JOURNAL_SCOPE, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(db_error)?;
    let (payload, result) =
        record.ok_or_else(|| CoreError::new("NOT_FOUND", "Diagnostic record not found"))?;
    let review: ManagedMcpDiagnosticReview = decode(&payload)?;
    let review_digest = digest(&review)?;
    let receipt: Option<ManagedMcpDiagnosticReceipt> = decode(&result)?;
    if review.diagnostic_id != id
        || receipt
            .as_ref()
            .is_some_and(|v| v.diagnostic_id != id || v.review_digest != review_digest)
    {
        return Err(changed());
    }
    Ok(ManagedMcpDiagnosticRecord {
        review,
        review_digest,
        receipt,
    })
}

fn record_receipt(store: &mut Store, receipt: &ManagedMcpDiagnosticReceipt) -> Result<()> {
    let tx = store
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    let id = &receipt.diagnostic_id;
    let before = diagnostic_record(&tx, id)?;
    let encoded = encode(receipt)?;
    let revision = if let Some(before) = before.receipt {
        if encode(&before)? == encoded {
            return Ok(());
        }
        if !matches!(before.direct_child, McpDiagnosticExit::Unknown { .. })
            || !matches!(receipt.direct_child, McpDiagnosticExit::Reaped { .. })
        {
            return Err(changed());
        }
        3
    } else {
        2
    };
    let marker: Option<String> = tx
        .query_row(
            "SELECT payload FROM idempotency WHERE scope=?1 AND key=?2",
            params![JOURNAL_SCOPE, PENDING],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    if marker.as_ref() != Some(id) || before.review_digest != receipt.review_digest {
        return Err(changed());
    }
    tx.execute(
        "UPDATE idempotency SET result=?1 WHERE scope=?2 AND key=?3",
        params![encoded, JOURNAL_SCOPE, id],
    )
    .map_err(db_error)?;
    if !matches!(receipt.direct_child, McpDiagnosticExit::Unknown { .. }) {
        tx.execute(
            "DELETE FROM idempotency WHERE scope=?1 AND key=?2 AND payload=?3",
            params![JOURNAL_SCOPE, PENDING, id],
        )
        .map_err(db_error)?;
    }
    event(
        &tx,
        id,
        revision,
        "mcp_diagnostic.observed",
        json!(receipt),
        "standalone_local_mcp_diagnostic",
    )?;
    tx.commit().map_err(db_error)
}

struct PrivatePaths {
    work: PathBuf,
    work_identity: (u64, u64),
    runtime: RuntimeRoot,
    descriptor: agentbaton_config::runtime::SnapshotDescriptor,
}

fn private_directory(path: &Path) -> Result<(u64, u64)> {
    let denied = || {
        CoreError::new(
            "PERMISSION_DENIED",
            "MCP diagnosis requires an owned private runtime directory",
        )
    };
    let meta = fs::symlink_metadata(path).map_err(|_| denied())?;
    if !path.is_absolute()
        || fs::canonicalize(path).map_err(|_| denied())? != path
        || !meta.is_dir()
        || meta.uid() != rustix::process::geteuid().as_raw()
        || meta.mode() & 0o077 != 0
    {
        return Err(denied());
    }
    Ok((meta.dev(), meta.ino()))
}

fn private_paths(runtime: &Path, id: &str) -> Result<PrivatePaths> {
    private_directory(runtime)?;
    let run = runtime.join(format!("mcp-diagnostic-{id}"));
    let work = run.join("work");
    let allocation_failed =
        || CoreError::new("INTERNAL_ERROR", "Cannot allocate private diagnostic state");
    let mut builder = DirBuilder::new();
    builder.mode(0o700);
    builder.create(&run).map_err(|_| allocation_failed())?;
    builder.create(&work).map_err(|_| allocation_failed())?;
    let work_identity = private_directory(&work)?;
    let root = RuntimeRoot::open(&run).map_err(crate::runtime_config::config_error)?;
    // Reuse the managed directory and environment rules with an empty plan.
    // No native configuration, tasks, or selected Skill bytes enter this runtime.
    let snapshot = root
        .materialize(
            crate::runtime_config::plan_for_provider("custom_acp")?,
            Ownership::BatonManaged,
        )
        .map_err(crate::runtime_config::config_error)?;
    Ok(PrivatePaths {
        work,
        work_identity,
        runtime: root,
        descriptor: snapshot.descriptor(),
    })
}

fn launch(
    db: &Connection,
    pending: &PendingMcpDiagnostic,
    paths: PrivatePaths,
    cancel: &AtomicBool,
    attempt: &mut ManagedMcpDiagnosticAttempt,
) {
    use McpDiagnosticOutcome::*;
    let fresh = paths
        .runtime
        .verify_before_launch(&paths.descriptor, RuntimeRequirements::default());
    let Ok(snapshot) = fresh else {
        attempt.receipt.outcome = InputChanged;
        return;
    };
    if private_directory(&paths.work).ok() != Some(paths.work_identity)
        || recheck(db, pending).is_err()
    {
        attempt.receipt.outcome = InputChanged;
        return;
    }
    if pending.check_expiry().is_err() {
        attempt.receipt.outcome = ApprovalExpired;
        return;
    }
    if cancel.load(Ordering::Acquire) {
        return;
    }

    attempt.receipt.runtime_snapshot_digest = Some(paths.descriptor.digest);
    let deadline = Instant::now() + IO_TIMEOUT;
    let child = Command::new(&pending.review.server.executable)
        .args(&pending.review.arguments)
        .current_dir(&paths.work)
        .env_clear()
        .envs(snapshot.environment())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let Ok(mut child) = child else {
        attempt.receipt.outcome = StartupFailed;
        return;
    };
    let mut io = ProbeIo::new(&mut child, cancel, deadline);
    attempt.receipt.outcome = io
        .nonblocking()
        .and_then(|()| io.enumerate(&pending.review.server, &mut attempt.receipt))
        .err()
        .unwrap_or(Enumerated);
    let failed = attempt.receipt.outcome != Enumerated;
    let exit = stop(&mut child, &mut io, failed);
    attempt.receipt.stderr_bytes_observed = io.stderr_bytes;
    attempt.receipt.direct_child = exit;
    if matches!(
        attempt.receipt.direct_child,
        McpDiagnosticExit::Unknown { .. }
    ) {
        attempt.child = Some(child);
    }
    qualify_exit(&mut attempt.receipt);
    if attempt.receipt.outcome == Enumerated {
        if cancel.load(Ordering::Acquire) {
            attempt.receipt.outcome = Cancelled;
        } else if io.stderr_bytes > MAX_STDERR {
            attempt.receipt.outcome = ResourceLimit;
        } else if recheck(db, pending).is_err() {
            attempt.receipt.outcome = InputChanged;
        }
    }
}

fn reaped(status: ExitStatus, kill_requested: bool) -> McpDiagnosticExit {
    McpDiagnosticExit::Reaped {
        exit_code: status.code(),
        signal: status.signal(),
        kill_requested,
    }
}

fn qualify_exit(receipt: &mut ManagedMcpDiagnosticReceipt) {
    if let McpDiagnosticExit::Reaped {
        exit_code,
        signal,
        kill_requested,
    } = receipt.direct_child
    {
        let clean = exit_code == Some(0);
        let forced = kill_requested && signal == Some(rustix::process::Signal::KILL.as_raw());
        if receipt.outcome == McpDiagnosticOutcome::Enumerated && !(clean || forced) {
            receipt.outcome = McpDiagnosticOutcome::ProcessFailed;
        }
    }
}

fn stop(child: &mut Child, io: &mut ProbeIo<'_>, abort: bool) -> McpDiagnosticExit {
    drop(io.stdin.take());
    if !abort {
        if let Some(status) = wait_for_exit(child, io, EXIT_GRACE) {
            return reaped(status, false);
        }
    } else if let Ok(Some(status)) = child.try_wait() {
        return reaped(status, false);
    }
    // The std::process::Child is still owned. Only it is signalled; a native
    // program's descendants may survive and cannot be certified by this result.
    let kill_requested = child.kill().is_ok();
    wait_for_exit(child, io, KILL_GRACE)
        .map(|status| reaped(status, kill_requested))
        .unwrap_or(McpDiagnosticExit::Unknown { kill_requested })
}

fn wait_for_exit(child: &mut Child, io: &mut ProbeIo<'_>, grace: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + grace;
    loop {
        io.drain_stderr().ok();
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(TICK),
            _ => return None,
        }
    }
}

type ProbeResult<T> = std::result::Result<T, McpDiagnosticOutcome>;

struct ProbeIo<'a> {
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    cancel: &'a AtomicBool,
    deadline: Instant,
    stdout_bytes: usize,
    stderr_bytes: usize,
    stderr_nonblocking: bool,
    notifications: usize,
    incoming: Vec<u8>,
}

impl<'a> ProbeIo<'a> {
    fn new(child: &mut Child, cancel: &'a AtomicBool, deadline: Instant) -> Self {
        Self {
            stdin: child.stdin.take(),
            stdout: child.stdout.take().expect("piped stdout"),
            stderr: child.stderr.take().expect("piped stderr"),
            cancel,
            deadline,
            stdout_bytes: 0,
            stderr_bytes: 0,
            stderr_nonblocking: false,
            notifications: 0,
            incoming: vec![],
        }
    }

    fn nonblocking(&mut self) -> ProbeResult<()> {
        use std::os::fd::AsFd;
        let enable = |pipe: std::os::fd::BorrowedFd<'_>| {
            let flags = fcntl_getfl(pipe).map_err(|_| McpDiagnosticOutcome::IoFailed)?;
            fcntl_setfl(pipe, flags | OFlags::NONBLOCK).map_err(|_| McpDiagnosticOutcome::IoFailed)
        };
        enable(self.stderr.as_fd())?;
        self.stderr_nonblocking = true;
        enable(self.stdout.as_fd())?;
        enable(self.stdin.as_ref().expect("piped stdin").as_fd())?;
        Ok(())
    }

    fn boundary(&mut self) -> ProbeResult<()> {
        if self.cancel.load(Ordering::Acquire) {
            return Err(McpDiagnosticOutcome::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(McpDiagnosticOutcome::TimedOut);
        }
        self.drain_stderr()
    }

    fn drain_stderr(&mut self) -> ProbeResult<()> {
        if !self.stderr_nonblocking {
            return Err(McpDiagnosticOutcome::IoFailed);
        }
        let mut chunk = [0u8; 4096];
        for _ in 0..4 {
            if self.stderr_bytes > MAX_STDERR {
                return Err(McpDiagnosticOutcome::ResourceLimit);
            }
            match self.stderr.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => self.stderr_bytes += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(McpDiagnosticOutcome::IoFailed),
            }
        }
        if self.stderr_bytes > MAX_STDERR {
            return Err(McpDiagnosticOutcome::ResourceLimit);
        }
        Ok(())
    }

    fn send(&mut self, frame: Value) -> ProbeResult<()> {
        let mut bytes = serde_json::to_vec(&frame).map_err(|_| McpDiagnosticOutcome::IoFailed)?;
        bytes.push(b'\n');
        let mut remaining = &bytes[..];
        while !remaining.is_empty() {
            self.boundary()?;
            match self.stdin.as_mut().expect("open input").write(remaining) {
                Ok(0) => return Err(McpDiagnosticOutcome::IoFailed),
                Ok(n) => remaining = &remaining[n..],
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(TICK),
                Err(_) => return Err(McpDiagnosticOutcome::IoFailed),
            }
        }
        Ok(())
    }

    fn read_frame(&mut self) -> ProbeResult<Value> {
        use McpDiagnosticOutcome::*;
        loop {
            self.boundary()?;
            if let Some(end) = self.incoming.iter().position(|&byte| byte == b'\n') {
                if end >= MAX_FRAME {
                    return Err(ResourceLimit);
                }
                let frame =
                    serde_json::from_slice(&self.incoming[..end]).map_err(|_| ProtocolFailed)?;
                self.incoming.drain(..=end);
                return Ok(frame);
            }
            if self.incoming.len() >= MAX_FRAME {
                return Err(ResourceLimit);
            }
            let mut chunk = [0u8; 4096];
            match self.stdout.read(&mut chunk) {
                Ok(0) => return Err(ProtocolFailed),
                Ok(n) => {
                    self.stdout_bytes += n;
                    if self.stdout_bytes > MAX_STDOUT {
                        return Err(ResourceLimit);
                    }
                    self.incoming.extend_from_slice(&chunk[..n]);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(TICK),
                Err(_) => return Err(IoFailed),
            }
        }
    }

    fn response(&mut self, expected: u64) -> ProbeResult<Value> {
        use McpDiagnosticOutcome::*;
        loop {
            let mut frame = self.read_frame()?;
            let envelope = frame.as_object_mut().ok_or(ProtocolFailed)?;
            if envelope.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
                return Err(ProtocolFailed);
            }
            if envelope.contains_key("method") {
                // Notifications are ignored, including their content. Requests
                // from the server never get an approval, result, or callback.
                if envelope.contains_key("id")
                    || envelope.contains_key("result")
                    || envelope.contains_key("error")
                    || !envelope
                        .get("method")
                        .and_then(Value::as_str)
                        .is_some_and(|method| method.starts_with("notifications/"))
                {
                    return Err(ProtocolFailed);
                }
                self.notifications += 1;
                if self.notifications > MAX_NOTIFICATIONS {
                    return Err(ResourceLimit);
                }
                continue;
            }
            if envelope.get("id").and_then(Value::as_u64) != Some(expected)
                || envelope.contains_key("error")
            {
                return Err(ProtocolFailed);
            }
            return envelope
                .remove("result")
                .filter(Value::is_object)
                .ok_or(ProtocolFailed);
        }
    }

    fn enumerate(
        &mut self,
        server: &ManagedMcpSummary,
        receipt: &mut ManagedMcpDiagnosticReceipt,
    ) -> ProbeResult<()> {
        use McpDiagnosticOutcome::*;
        self.send(json!({
            "jsonrpc":"2.0","id":1,"method":"initialize","params":{
                "protocolVersion":PROTOCOL,"capabilities":{},
                "clientInfo":{"name":"AgentBaton MCP diagnostic","version":env!("CARGO_PKG_VERSION")}
            }
        }))?;
        let init = self.response(1)?;
        let version = init["protocolVersion"].as_str().ok_or(ProtocolFailed)?;
        if !matches!(version, PROTOCOL | "2024-11-05" | "2025-03-26") {
            return Err(UnsupportedProtocol);
        }
        let capabilities = init["capabilities"].as_object().ok_or(ProtocolFailed)?;
        for key in ["name", "version"] {
            init["serverInfo"][key]
                .as_str()
                .filter(|v| !v.is_empty() && v.len() <= 256)
                .ok_or(ProtocolFailed)?;
        }
        receipt.protocol_version = Some(version.into());
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))?;
        match capabilities.get("tools") {
            Some(value) if value.is_object() => {}
            None => return Err(ToolsUnsupported),
            _ => return Err(ProtocolFailed),
        }

        let mut cursor: Option<String> = None;
        let mut cursors = BTreeSet::new();
        let mut seen = BTreeSet::new();
        for page in 0..MAX_PAGES {
            let id = 2 + page as u64;
            let params = cursor
                .as_ref()
                .map_or_else(|| json!({}), |v| json!({"cursor":v}));
            self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":params}))?;
            let result = self.response(id)?;
            let tools = result["tools"].as_array().ok_or(ProtocolFailed)?;
            if seen.len() + tools.len() > MAX_TOOLS {
                return Err(ResourceLimit);
            }
            for tool in tools {
                let name = tool["name"]
                    .as_str()
                    .filter(|v| !v.is_empty() && v.len() <= 256 && !v.chars().any(char::is_control))
                    .ok_or(ProtocolFailed)?;
                if !tool["inputSchema"].is_object()
                    || tool["inputSchema"]["type"] != "object"
                    || !seen.insert(name.to_owned())
                {
                    return Err(ProtocolFailed);
                }
            }
            let Some(next) = result.get("nextCursor") else {
                receipt.configured_tools_seen = server
                    .enabled_tools
                    .iter()
                    .filter(|name| seen.contains(*name))
                    .cloned()
                    .collect();
                receipt.other_tool_count = Some(seen.len() - receipt.configured_tools_seen.len());
                return Ok(());
            };
            let next = next
                .as_str()
                .filter(|v| !v.is_empty() && v.len() <= 1024)
                .ok_or(ProtocolFailed)?
                .to_owned();
            if !cursors.insert(next.clone()) {
                return Err(ProtocolFailed);
            }
            cursor = Some(next);
        }
        Err(ResourceLimit)
    }
}
