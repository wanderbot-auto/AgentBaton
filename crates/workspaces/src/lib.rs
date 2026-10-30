//! Owner-selected workspace operations. Worktrees organize writes; they are not OS sandboxes.
//! Import/isolation APIs never remove source directories, implicitly execute repository scripts, or stash/reset source edits.
//! Fixed validator execution is separate and requires a trusted-host-approved command contract.

pub mod adoption;
pub mod command_validation;
pub mod data_workspace;
pub mod deterministic;
pub mod source_capture;
pub mod validation;

use serde::{Deserialize, Serialize};
use std::ffi::{CString, OsStr};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum WorkspaceError {
    Io(io::Error),
    InvalidPath,
    NotRepository,
    UnsafeRepository,
    UnverifiedIndex,
    Git(String),
    Dirty,
    StaleHead,
    TooLarge,
    NotRegularFile,
    ResourceLimit,
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "workspace I/O: {e}"),
            Self::InvalidPath => write!(f, "invalid or non-contained workspace path"),
            Self::NotRepository => write!(f, "not a Git working tree"),
            Self::UnsafeRepository => write!(
                f,
                "repository requires unapproved configuration indirection, executable filters or external object access"
            ),
            Self::UnverifiedIndex => write!(f, "repository index hides tracked paths through assume-unchanged, skip-worktree or sparse checkout; explicit owner review is required"),
            Self::Git(_) => write!(f, "Git operation failed; inspect repository state"),
            Self::Dirty => write!(f, "target has uncommitted or untracked files"),
            Self::StaleHead => write!(f, "target HEAD changed; validate the new candidate"),
            Self::TooLarge => write!(f, "artifact exceeds the approved byte limit"),
            Self::NotRegularFile => write!(f, "artifact must be a regular file"),
            Self::ResourceLimit => write!(f, "Git operation exceeded time or output limit"),
        }
    }
}

impl std::error::Error for WorkspaceError {}
impl From<io::Error> for WorkspaceError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceIdentity {
    pub canonical_root: PathBuf,
    /// Local directory identity, used to distinguish a replacement at an old path.
    pub device: u64,
    pub inode: u64,
    pub kind: WorkspaceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceKind {
    Directory,
    Git {
        common_dir: PathBuf,
        head: String,
        dirty: bool,
    },
}

fn git_command(root: &Path) -> Command {
    let mut command = Command::new("git");
    // Local operations must not inherit another process's Git repository/config selectors.
    for (key, _) in std::env::vars_os() {
        if key.as_bytes().starts_with(b"GIT_") {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        // Local inspection/isolation never authorizes promisor lazy fetches or
        // repository-configured remote helpers (including ext/SSH helpers).
        .env("GIT_NO_LAZY_FETCH", "1")
        // Approved commit IDs refer to raw objects and their recorded ancestry.
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_GRAFT_FILE", "/dev/null")
        .arg("--no-optional-locks")
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "submodule.recurse=false",
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.file.allow=never",
            "-c",
            "core.ignoreStat=false",
            "-c",
            "core.trustctime=true",
            "-c",
            "core.checkStat=default",
        ])
        .arg("-C")
        .arg(root);
    command
}

fn output(root: &Path, args: &[&OsStr]) -> Result<Output, WorkspaceError> {
    bounded_output(git_command(root).args(args))
}

fn bounded_output(command: &mut Command) -> Result<Output, WorkspaceError> {
    const MAX_OUTPUT: u64 = 4 * 1024 * 1024;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let read = |stream: Box<dyn Read + Send>| -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        stream.take(MAX_OUTPUT + 1).read_to_end(&mut bytes)?;
        Ok(bytes)
    };
    let out = std::thread::spawn(move || read(Box::new(stdout)));
    let err = std::thread::spawn(move || read(Box::new(stderr)));
    let started = Instant::now();
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status, false);
        }
        if started.elapsed() >= Duration::from_secs(15) {
            let _ = child.kill();
            break (child.wait()?, true);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = out.join().map_err(|_| WorkspaceError::ResourceLimit)??;
    let stderr = err.join().map_err(|_| WorkspaceError::ResourceLimit)??;
    if timed_out || stdout.len() as u64 > MAX_OUTPUT || stderr.len() as u64 > MAX_OUTPUT {
        return Err(WorkspaceError::ResourceLimit);
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod inspection_process_bounds {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn each_inspection_stream_has_a_real_raw_byte_limit() {
        for stream in ["stdout", "stderr"] {
            let code = if stream == "stdout" {
                "i=0; while [ \"$i\" -lt 5000 ]; do printf '%01000d' 0; i=$((i+1)); done"
            } else {
                "i=0; while [ \"$i\" -lt 5000 ]; do printf '%01000d' 0 >&2; i=$((i+1)); done"
            };
            assert!(
                matches!(
                    bounded_output(Command::new("/bin/sh").args(["-c", code])),
                    Err(WorkspaceError::ResourceLimit)
                ),
                "{stream}"
            );
        }
    }

    #[test]
    fn inspection_timeout_kills_and_reaps_the_actual_owned_command() {
        let dir = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let marker = dir.path().join("pid");
        // The static script writes its own PID, then exec replaces that exact
        // owned child. No detached descendants or unrelated PID is signalled.
        let started = Instant::now();
        assert!(matches!(
            bounded_output(
                Command::new("/bin/sh")
                    .args([
                        "-c",
                        "printf '%s' \"$$\" > \"$1\"; exec /bin/sleep 30",
                        "fixture"
                    ])
                    .arg(&marker)
            ),
            Err(WorkspaceError::ResourceLimit)
        ));
        assert!(started.elapsed() >= Duration::from_secs(15));
        assert!(started.elapsed() < Duration::from_secs(25));
        let pid = fs::read_to_string(marker)
            .unwrap()
            .parse::<libc::pid_t>()
            .unwrap();
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
    }
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, WorkspaceError> {
    let result = bounded_output(git_command(root).args(args))?;
    if !result.status.success() {
        return Err(WorkspaceError::Git(
            String::from_utf8_lossy(&result.stderr).into_owned(),
        ));
    }
    String::from_utf8(result.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|_| WorkspaceError::InvalidPath)
}

fn reject_config_indirection(root: &Path) -> Result<(), WorkspaceError> {
    // A future worktree gitdir can activate rules invisible in the source
    // context. Audit declarations without following them; do not rewrite them.
    let includes = bounded_output(git_command(root).args([
        "config",
        "--no-includes",
        "--get-regexp",
        "^include(if\\..*)?\\.path$",
    ]))?;
    if includes.status.success() {
        return Err(WorkspaceError::UnsafeRepository);
    }
    if includes.status.code() != Some(1) {
        // Preserve malformed/unreadable repository rejection before probing it;
        // a failed audit must never permit a later object or checkout operation.
        return Err(WorkspaceError::NotRepository);
    }
    Ok(())
}

fn reject_partial_clones(root: &Path) -> Result<(), WorkspaceError> {
    // Older Git versions may not honor GIT_NO_LAZY_FETCH. Refuse external object
    // sources before status/checkout/object inspection can invoke their helpers.
    let partial = bounded_output(git_command(root).args([
        "config",
        "--get-regexp",
        "^(extensions\\.partialclone|remote\\..*\\.promisor)$",
    ]))?;
    if partial.status.success() || partial.status.code() != Some(1) {
        return Err(WorkspaceError::UnsafeRepository);
    }
    Ok(())
}

fn reject_checkout_filters(root: &Path) -> Result<(), WorkspaceError> {
    // Even `status` can invoke clean/process filters when comparing a modified file.
    let filters = bounded_output(git_command(root).args(["config", "--get-regexp", "^filter\\."]))?;
    if filters.status.success() || !matches!(filters.status.code(), Some(1)) {
        return Err(WorkspaceError::UnsafeRepository);
    }
    Ok(())
}

/// One no-include config read replaces three independent declarations queries
/// in an inspection. Do not cache this across inspections/admission boundaries.
fn audit_inspection_config(root: &Path) -> Result<(), WorkspaceError> {
    let config =
        bounded_output(git_command(root).args(["config", "--no-includes", "--null", "--list"]))?;
    if !config.status.success() {
        return Err(WorkspaceError::NotRepository);
    }
    if !config.stdout.is_empty() && config.stdout.last() != Some(&0) {
        return Err(WorkspaceError::UnsafeRepository);
    }
    for record in config.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        // Git's -z list separates key/value with LF, records with NUL. Values
        // can themselves contain LF and must never be interpreted as keys.
        let key = record.split(|b| *b == b'\n').next().unwrap_or_default();
        if key == b"include.path"
            || (key.starts_with(b"includeif.")
                && key.ends_with(b".path")
                && key.len() >= b"includeif.".len() + b".path".len())
            || key == b"extensions.partialclone"
            || (key.starts_with(b"remote.")
                && key.ends_with(b".promisor")
                && key.len() >= b"remote.".len() + b".promisor".len())
            || key.starts_with(b"filter.")
        {
            return Err(WorkspaceError::UnsafeRepository);
        }
    }
    Ok(())
}

fn inspected_git_metadata(
    root: &Path,
) -> Result<Option<(PathBuf, String, PathBuf)>, WorkspaceError> {
    let batch = bounded_output(git_command(root).args([
        "rev-parse",
        "--path-format=absolute",
        "--show-toplevel",
        "--git-common-dir",
        "--verify",
        "HEAD^{commit}",
    ]))?;
    if batch.status.success() {
        let text = String::from_utf8(batch.stdout).map_err(|_| WorkspaceError::InvalidPath)?;
        let fields: Vec<_> = text.lines().collect();
        if fields.len() == 3 && is_oid(fields[2]) {
            return Ok(Some((
                fs::canonicalize(fields[0])?,
                fields[2].into(),
                fs::canonicalize(fields[1])?,
            )));
        }
        // Newlines in actual filesystem paths make the batched framing
        // ambiguous. Preserve the original independent path-reading behavior.
    }
    let probe = output(
        root,
        &[OsStr::new("rev-parse"), OsStr::new("--show-toplevel")],
    )?;
    if !probe.status.success() {
        return Ok(None);
    }
    let top = fs::canonicalize(
        String::from_utf8(probe.stdout)
            .map_err(|_| WorkspaceError::InvalidPath)?
            .trim(),
    )?;
    let head = git_text(root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let common = fs::canonicalize(git_text(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?)?;
    Ok(Some((top, head, common)))
}

/// Read flags and modes together. This preserves assume-unchanged,
/// skip-worktree, sparse-index and gitlink checks before running status.
fn reject_hidden_index_entries(root: &Path) -> Result<bool, WorkspaceError> {
    // `-v` renders assume-unchanged entries with lowercase status letters; `S` denotes
    // skip-worktree (including sparse-directory index entries). Read only: never clear flags.
    let entries =
        bounded_output(git_command(root).args(["ls-files", "--cached", "--stage", "-v", "-z"]))?;
    if !entries.status.success() {
        return Err(WorkspaceError::NotRepository);
    }
    let mut has_submodules = false;
    for entry in entries.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        if entry.len() < 3 || entry[1] != b' ' {
            return Err(WorkspaceError::UnverifiedIndex);
        }
        if entry[0].is_ascii_lowercase() || entry[0] == b'S' {
            return Err(WorkspaceError::UnverifiedIndex);
        }
        has_submodules |= entry[2..].starts_with(b"160000 ");
    }
    // Even if all files currently happen to be present, sparse configuration can change the
    // next checkout's coverage. Do not inherit it silently into a supposedly complete workspace.
    let sparse = bounded_output(git_command(root).args([
        "config",
        "--bool",
        "--get",
        "core.sparseCheckout",
    ]))?;
    if sparse.status.success() {
        if sparse.stdout != b"false\n" {
            return Err(WorkspaceError::UnverifiedIndex);
        }
    } else if sparse.status.code() != Some(1) {
        return Err(WorkspaceError::UnverifiedIndex);
    }
    Ok(has_submodules)
}

/// Import is read-only and does not initialize Git or execute repository scripts/hooks.
/// An unborn or corrupt repository is an error rather than silently treated as a data directory.
pub fn inspect(root: &Path) -> Result<WorkspaceIdentity, WorkspaceError> {
    let canonical_root = fs::canonicalize(root)?;
    let metadata = fs::metadata(&canonical_root)?;
    if !metadata.is_dir() {
        return Err(WorkspaceError::InvalidPath);
    }
    audit_inspection_config(&canonical_root)?;
    let kind = if let Some((top, head, common_dir)) = inspected_git_metadata(&canonical_root)? {
        // Explicit directory selection must not silently expand to an ancestor repository.
        if top != canonical_root {
            return Err(WorkspaceError::InvalidPath);
        }
        let has_submodules = reject_hidden_index_entries(&canonical_root)?;
        let dirty = !git_text(
            &canonical_root,
            &[
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--ignore-submodules=all",
            ],
        )?
        .is_empty();
        // Do not execute nested repositories' config/hooks merely to inspect their dirtiness.
        // Until explicit submodule policy exists, conservatively block adoption for any gitlink.
        let dirty = dirty || has_submodules;
        WorkspaceKind::Git {
            common_dir,
            head,
            dirty,
        }
    } else {
        match canonical_root.join(".git").symlink_metadata() {
            Ok(_) => return Err(WorkspaceError::NotRepository),
            Err(error) if error.kind() == io::ErrorKind::NotFound => WorkspaceKind::Directory,
            Err(error) => return Err(WorkspaceError::Io(error)),
        }
    };
    Ok(WorkspaceIdentity {
        canonical_root,
        device: metadata.dev(),
        inode: metadata.ino(),
        kind,
    })
}

/// Reassociation preserves identity only when the actual directory inode/device matches.
/// Copying a tree to another filesystem requires an explicit new workspace mapping.
pub fn reassociate(
    previous: &WorkspaceIdentity,
    new_root: &Path,
) -> Result<WorkspaceIdentity, WorkspaceError> {
    let current = inspect(new_root)?;
    if current.device != previous.device || current.inode != previous.inode {
        return Err(WorkspaceError::InvalidPath);
    }
    match (&previous.kind, &current.kind) {
        (WorkspaceKind::Directory, WorkspaceKind::Directory)
        | (WorkspaceKind::Git { .. }, WorkspaceKind::Git { .. }) => Ok(current),
        _ => Err(WorkspaceError::InvalidPath),
    }
}

fn is_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Create a detached, never force-overwritten managed checkout at an exact commit.
/// The owner selects `managed_root`; `name` is a single inert directory component.
pub fn create_isolated(
    source: &Path,
    managed_root: &Path,
    name: &str,
    commit: &str,
) -> Result<WorkspaceIdentity, WorkspaceError> {
    if name.is_empty()
        || name.starts_with('.')
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || !is_oid(commit)
    {
        return Err(WorkspaceError::InvalidPath);
    }
    let source = inspect(source)?;
    if !matches!(source.kind, WorkspaceKind::Git { .. }) {
        return Err(WorkspaceError::NotRepository);
    }
    let managed_root = fs::canonicalize(managed_root)?;
    if !managed_root.is_dir() || managed_root.starts_with(&source.canonical_root) {
        return Err(WorkspaceError::InvalidPath);
    }
    let destination = managed_root.join(name);
    if destination.symlink_metadata().is_ok() {
        return Err(WorkspaceError::InvalidPath);
    }
    // Checkout can invoke named clean/smudge/process filters. Reject their configuration rather
    // than running owner-unapproved code during worktree creation.
    reject_config_indirection(&source.canonical_root)?;
    reject_partial_clones(&source.canonical_root)?;
    reject_checkout_filters(&source.canonical_root)?;
    let verified = git_text(
        &source.canonical_root,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )?;
    if !verified.eq_ignore_ascii_case(commit) {
        return Err(WorkspaceError::InvalidPath);
    }
    let result = output(
        &source.canonical_root,
        &[
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--detach"),
            OsStr::new("--"),
            destination.as_os_str(),
            OsStr::new(commit),
        ],
    )?;
    if !result.status.success() {
        return Err(WorkspaceError::Git(
            String::from_utf8_lossy(&result.stderr).into_owned(),
        ));
    }
    inspect(&destination)
}

/// Same isolated-worktree checks, with the original trusted host stage clock.
/// Consult control.preparation_io even on Err: private worktree/object effects
/// may already exist. Nothing resets/removes source or silently retries.
pub fn create_isolated_controlled(
    source: &Path,
    managed_root: &Path,
    name: &str,
    commit: &str,
    control: &adoption::AdoptionControl<'_>,
) -> Result<WorkspaceIdentity, WorkspaceError> {
    let control = control.narrowed(Instant::now() + Duration::from_secs(15));
    control.check()?;
    if name.is_empty()
        || name.starts_with('.')
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || !is_oid(commit)
    {
        return Err(WorkspaceError::InvalidPath);
    }
    let source = inspect_controlled(source, &control)?;
    if !matches!(source.kind, WorkspaceKind::Git { .. }) {
        return Err(WorkspaceError::NotRepository);
    }
    control.check()?;
    let managed_root = fs::canonicalize(managed_root)?;
    control.check()?;
    if !managed_root.is_dir() || managed_root.starts_with(&source.canonical_root) {
        return Err(WorkspaceError::InvalidPath);
    }
    let destination = managed_root.join(name);
    if destination.symlink_metadata().is_ok() {
        return Err(WorkspaceError::InvalidPath);
    }
    // Exact original config declaration probes, all under the same control.
    let includes = output_with_control(
        git_command(&source.canonical_root).args([
            "config",
            "--no-includes",
            "--get-regexp",
            "^include(if\\..*)?\\.path$",
        ]),
        Some(&control),
    )?;
    if includes.status.success() {
        return Err(WorkspaceError::UnsafeRepository);
    }
    if includes.status.code() != Some(1) {
        return Err(WorkspaceError::NotRepository);
    }
    let partial = output_with_control(
        git_command(&source.canonical_root).args([
            "config",
            "--get-regexp",
            "^(extensions\\.partialclone|remote\\..*\\.promisor)$",
        ]),
        Some(&control),
    )?;
    if partial.status.success() || partial.status.code() != Some(1) {
        return Err(WorkspaceError::UnsafeRepository);
    }
    let filters = output_with_control(
        git_command(&source.canonical_root).args(["config", "--get-regexp", "^filter\\."]),
        Some(&control),
    )?;
    if filters.status.success() || filters.status.code() != Some(1) {
        return Err(WorkspaceError::UnsafeRepository);
    }
    let verified = output_with_control(
        git_command(&source.canonical_root).args([
            "rev-parse",
            "--verify",
            &format!("{commit}^{{commit}}"),
        ]),
        Some(&control),
    )?;
    if !verified.status.success()
        || !String::from_utf8(verified.stdout)
            .map_err(|_| WorkspaceError::InvalidPath)?
            .trim()
            .eq_ignore_ascii_case(commit)
    {
        return Err(WorkspaceError::InvalidPath);
    }
    control.check()?;
    control.note_effects();
    let result = output_with_control(
        git_command(&source.canonical_root).args([
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--detach"),
            OsStr::new("--"),
            destination.as_os_str(),
            OsStr::new(commit),
        ]),
        Some(&control),
    )?;
    if !result.status.success() {
        return Err(WorkspaceError::Git(
            "controlled isolated checkout rejected".into(),
        ));
    }
    let identity = inspect_controlled(&destination, &control)?;
    control.check()?;
    Ok(identity)
}

/// Check immediately before adoption; this is evidence, not an atomic merge or authorization.
/// The caller must fence writes and recheck the exact head during the eventual integration.
pub fn check_adoption(
    target: &Path,
    expected_head: &str,
) -> Result<WorkspaceIdentity, WorkspaceError> {
    if !is_oid(expected_head) {
        return Err(WorkspaceError::InvalidPath);
    }
    let target = inspect(target)?;
    match &target.kind {
        WorkspaceKind::Git { head, dirty, .. } => {
            if head != expected_head {
                return Err(WorkspaceError::StaleHead);
            }
            if *dirty {
                return Err(WorkspaceError::Dirty);
            }
            Ok(target)
        }
        _ => Err(WorkspaceError::NotRepository),
    }
}

/// Read a regular artifact through directory descriptors, rejecting symlinks at every component.
/// `root` must be an owner-approved root. This does not constrain unrelated Provider processes.
pub fn read_artifact(
    root: &Path,
    relative: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, WorkspaceError> {
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(WorkspaceError::InvalidPath);
    }
    let components: Vec<_> = relative.components().collect();
    if components
        .iter()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(WorkspaceError::InvalidPath);
    }
    let mut directory = open_root_without_symlinks(root)?;
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err(WorkspaceError::InvalidPath);
        };
        let name = CString::new(name.as_bytes()).map_err(|_| WorkspaceError::InvalidPath)?;
        let last = index + 1 == components.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if last { 0 } else { libc::O_DIRECTORY };
        // SAFETY: name is NUL-terminated; the borrowed dirfd remains open throughout openat.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(WorkspaceError::Io(io::Error::last_os_error()));
        }
        // SAFETY: successful openat returns a newly owned file descriptor.
        let file = unsafe { File::from_raw_fd(fd) };
        if last {
            let metadata = file.metadata()?;
            if !metadata.is_file() {
                return Err(WorkspaceError::NotRegularFile);
            }
            if metadata.len() > max_bytes {
                return Err(WorkspaceError::TooLarge);
            }
            let limit = max_bytes.checked_add(1).ok_or(WorkspaceError::TooLarge)?;
            let mut bytes = Vec::new();
            file.take(limit).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > max_bytes {
                return Err(WorkspaceError::TooLarge);
            }
            return Ok(bytes);
        }
        directory = file;
    }
    Err(WorkspaceError::InvalidPath)
}

fn open_root_without_symlinks(root: &Path) -> Result<File, WorkspaceError> {
    // Require the owner-approved canonical path, rather than resolving a mutable symlink now.
    if !root.is_absolute()
        || root
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(WorkspaceError::InvalidPath);
    }
    let mut current = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/")?;
    for component in root.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        let name = CString::new(name.as_bytes()).map_err(|_| WorkspaceError::InvalidPath)?;
        // SAFETY: CString is NUL-terminated and current's directory descriptor remains live.
        let fd = unsafe {
            libc::openat(
                current.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(WorkspaceError::Io(io::Error::last_os_error()));
        }
        // SAFETY: newly returned descriptor is exclusively owned.
        current = unsafe { File::from_raw_fd(fd) };
    }
    Ok(current)
}

#[cfg(test)]
mod raw_ancestry_tests {
    use super::*;

    #[test]
    fn legacy_grafts_cannot_replace_raw_commit_parents() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        git_text(&root, &["init", "-b", "fixture"]).unwrap();
        git_text(&root, &["config", "user.name", "Fixture"]).unwrap();
        git_text(&root, &["config", "user.email", "fixture@example.invalid"]).unwrap();
        let mut commits = Vec::new();
        for content in ["A", "B", "C"] {
            fs::write(root.join("file.txt"), content).unwrap();
            git_text(&root, &["add", "file.txt"]).unwrap();
            git_text(&root, &["commit", "-m", content]).unwrap();
            commits.push(git_text(&root, &["rev-parse", "HEAD"]).unwrap());
        }
        let graft = root.join(".git/info/grafts");
        fs::write(&graft, format!("{} {}\n", commits[2], commits[0])).unwrap();
        let graft_before = fs::read(&graft).unwrap();
        let index_before = fs::read(root.join(".git/index")).unwrap();
        // Establish that the fixture actually changes ordinary Git ancestry.
        let mut ordinary = git_command(&root);
        ordinary.env("GIT_GRAFT_FILE", &graft);
        let altered =
            bounded_output(ordinary.args(["show", "-s", "--format=%P", &commits[2]])).unwrap();
        assert!(altered.status.success());
        assert_eq!(
            String::from_utf8(altered.stdout).unwrap().trim(),
            commits[0]
        );
        assert_eq!(
            git_text(&root, &["show", "-s", "--format=%P", &commits[2]]).unwrap(),
            commits[1]
        );
        assert_eq!(fs::read(&graft).unwrap(), graft_before);
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), index_before);
    }
}

/// Private control-aware output. A known joined exit status is retained even
/// when cancellation races completion; the write caller decides its meaning.
pub(crate) struct ControlledGitOutput {
    pub(crate) output: Output,
    pub(crate) interrupted: bool,
    pub(crate) io: adoption::ControlledIntegrationIo,
}
struct ControlledGitChild {
    child: std::process::Child,
    reaped: bool,
    signal_attempted: bool,
}
impl ControlledGitChild {
    fn finished(&self) -> io::Result<bool> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                libc::id_t::from(self.child.id()),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { info.si_pid() } != 0)
    }
    fn signal_own_group(&mut self) {
        // Never reap the leader before group signals end: its unreaped PID
        // prevents targeting a later unrelated process with a reused number.
        let original_child_is_owned = match self.finished() {
            Ok(_) => true,
            // waitid(WNOWAIT) did not consume the child on EINTR. Keep its
            // original identity and deliver cleanup instead of waiting live.
            Err(error) => error.kind() == io::ErrorKind::Interrupted,
        };
        if !self.reaped && !self.signal_attempted && original_child_is_owned {
            self.signal_attempted = true;
            if let Ok(pid) = i32::try_from(self.child.id()) {
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
            }
            let _ = self.child.kill();
        }
    }
    fn reap(&mut self) -> io::Result<std::process::ExitStatus> {
        let status = self.child.wait()?;
        self.reaped = true;
        Ok(status)
    }
}
impl Drop for ControlledGitChild {
    fn drop(&mut self) {
        if !self.reaped {
            self.signal_own_group();
            if self.signal_attempted || self.finished().is_ok() {
                let _ = self.reap();
            }
        }
    }
}
struct ControlledCapture<T> {
    pipe: Option<T>,
    bytes: Vec<u8>,
    eof: bool,
    failed: bool,
    truncated: bool,
}
impl<T: Read + AsRawFd> ControlledCapture<T> {
    fn new(pipe: T) -> io::Result<Self> {
        let fd = pipe.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            pipe: Some(pipe),
            bytes: vec![],
            eof: false,
            failed: false,
            truncated: false,
        })
    }
    fn pump(&mut self) -> bool {
        let Some(pipe) = self.pipe.as_mut() else {
            return false;
        };
        let mut buffer = [0u8; 65536];
        match pipe.read(&mut buffer) {
            Ok(0) => {
                self.eof = true;
                self.pipe.take();
                true
            }
            Ok(n) => {
                let remaining = 4_194_304usize.saturating_sub(self.bytes.len());
                self.bytes.extend_from_slice(&buffer[..remaining.min(n)]);
                if n > remaining {
                    self.truncated = true;
                    self.pipe.take();
                }
                true
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                false
            }
            Err(_) => {
                self.failed = true;
                self.pipe.take();
                true
            }
        }
    }
}
pub(crate) fn controlled_output(
    command: &mut Command,
    control: &adoption::AdoptionControl<'_>,
) -> Result<ControlledGitOutput, WorkspaceError> {
    use std::os::unix::process::CommandExt;
    control.check()?;
    let started = Instant::now();
    control.note_git_attempt();
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let mut owned = ControlledGitChild {
        child,
        reaped: false,
        signal_attempted: false,
    };
    #[cfg(test)]
    controlled_spawn_hook::apply(owned.child.id());
    let mut stdout = ControlledCapture::new(owned.child.stdout.take().expect("piped stdout"))?;
    let mut stderr = ControlledCapture::new(owned.child.stderr.take().expect("piped stderr"))?;
    let deadline = control.deadline().min(started + Duration::from_secs(15));
    let mut interrupted = false;
    loop {
        let out_progress = stdout.pump();
        let err_progress = stderr.pump();
        if stdout.failed || stderr.failed || stdout.truncated || stderr.truncated {
            interrupted = true;
            break;
        }
        let finished = match owned.finished() {
            Ok(value) => value,
            // A signal cannot skip the original deadline/cancel check.
            Err(error) if error.kind() == io::ErrorKind::Interrupted => false,
            Err(error) => return Err(WorkspaceError::Io(error)),
        };
        if finished && stdout.eof && stderr.eof {
            break;
        }
        if control.check().is_err() || Instant::now() >= deadline {
            interrupted = true;
            break;
        }
        if !out_progress && !err_progress {
            std::thread::sleep(
                Duration::from_millis(2).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    }
    // This always targets our own group before consuming the leader's identity,
    // including a leader that exited while another inherited pipe stayed open.
    owned.signal_own_group();
    let status = owned.reap()?;
    // Nonblocking bounded final drain only: never invent a new cleanup clock or
    // wait indefinitely for an escaped descendant. Missing EOF stays unknown.
    while stdout.pump() {}
    while stderr.pump() {}
    let io = adoption::ControlledIntegrationIo {
        elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        stdout_retained_bytes: Some(stdout.bytes.len() as u64),
        stderr_retained_bytes: Some(stderr.bytes.len() as u64),
        stdout_eof: stdout.eof,
        stderr_eof: stderr.eof,
        owned_leader_reaped: Some(owned.reaped),
        owned_group_signal_attempted: Some(owned.signal_attempted),
        interrupted,
        read_failed: Some(stdout.failed || stderr.failed),
    };
    stdout.pipe.take();
    stderr.pipe.take();
    control.note_git_io(&io);
    Ok(ControlledGitOutput {
        output: Output {
            status,
            stdout: stdout.bytes,
            stderr: stderr.bytes,
        },
        interrupted,
        io,
    })
}

// Prepared local-ref transaction; no caller-controlled protocol is accepted.
// The validate callback runs only after Git acknowledges prepare while keeping
// the target lock, so cooperative ref writers cannot replace its type between
// this check and commit. This is not protection from arbitrary same-UID writes.
pub(crate) struct PreparedRefOutput {
    pub(crate) result: ControlledGitOutput,
    pub(crate) commit_acknowledged: bool,
    pub(crate) abort_acknowledged: bool,
    pub(crate) validation_error: Option<adoption::AdoptionError>,
}
fn ref_transaction_poll(
    owned: &ControlledGitChild,
    stdout: &mut ControlledCapture<std::process::ChildStdout>,
    stderr: &mut ControlledCapture<std::process::ChildStderr>,
    control: &adoption::AdoptionControl<'_>,
    expected_ack: Option<&[u8]>,
) -> Result<(), WorkspaceError> {
    loop {
        let progress = stdout.pump() | stderr.pump();
        if stdout.truncated || stderr.truncated {
            return Err(WorkspaceError::TooLarge);
        }
        if stdout.failed || stderr.failed {
            return Err(WorkspaceError::UnsafeRepository);
        }
        if let Some(expected) = expected_ack {
            if stdout.bytes == expected {
                return Ok(());
            }
            if !expected.starts_with(&stdout.bytes) {
                return Err(WorkspaceError::UnsafeRepository);
            }
        }
        let finished = match owned.finished() {
            Ok(value) => value,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => false,
            Err(e) => return Err(e.into()),
        };
        if finished && stdout.eof && stderr.eof {
            return if expected_ack.is_none() {
                Ok(())
            } else {
                Err(WorkspaceError::Git(
                    "prepared ref protocol unavailable or refused".into(),
                ))
            };
        }
        control.check()?;
        if !progress {
            std::thread::sleep(
                Duration::from_millis(2)
                    .min(control.deadline().saturating_duration_since(Instant::now())),
            );
        }
    }
}
fn ref_transaction_write(
    stdin: &mut std::process::ChildStdin,
    frame: &[u8],
    control: &adoption::AdoptionControl<'_>,
) -> Result<(), WorkspaceError> {
    use std::io::Write;
    let mut remaining = frame;
    while !remaining.is_empty() {
        control.check()?;
        match stdin.write(remaining) {
            Ok(0) => return Err(WorkspaceError::UnsafeRepository),
            Ok(n) => remaining = &remaining[n..],
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                std::thread::sleep(
                    Duration::from_millis(2)
                        .min(control.deadline().saturating_duration_since(Instant::now())),
                );
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub(crate) fn prepared_ref_output(
    command: &mut Command,
    target: &str,
    candidate: &str,
    expected: &str,
    control: &adoption::AdoptionControl<'_>,
    validate_locked: impl FnOnce(&adoption::AdoptionControl<'_>) -> Result<(), adoption::AdoptionError>,
) -> Result<PreparedRefOutput, WorkspaceError> {
    use std::os::unix::process::CommandExt;
    // The fixed update frame cannot contain another protocol command.
    if !target.starts_with("refs/heads/")
        || target.chars().any(char::is_control)
        || target.len() > 4096
        || !is_oid(candidate)
        || !is_oid(expected)
    {
        return Err(WorkspaceError::InvalidPath);
    }
    control.check()?;
    let started = Instant::now();
    let bounded = control.narrowed(started + Duration::from_secs(15));
    control.note_git_attempt();
    let child = command
        .args([
            "update-ref",
            "--no-deref",
            "--stdin",
            "-m",
            "AgentBaton validated local adoption",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let mut owned = ControlledGitChild {
        child,
        reaped: false,
        signal_attempted: false,
    };
    let mut stdin = owned.child.stdin.take().expect("piped stdin");
    let flags = unsafe { libc::fcntl(stdin.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(stdin.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(io::Error::last_os_error().into());
    }
    let mut stdout = ControlledCapture::new(owned.child.stdout.take().expect("piped stdout"))?;
    let mut stderr = ControlledCapture::new(owned.child.stderr.take().expect("piped stderr"))?;
    let mut commit_may_have_been_sent = false;
    let mut validation_error = None;
    let mut interrupted = false;
    let operation = (|| -> Result<(), WorkspaceError> {
        ref_transaction_write(
            &mut stdin,
            format!("start\nupdate {target} {candidate} {expected}\nprepare\n").as_bytes(),
            &bounded,
        )?;
        ref_transaction_poll(
            &owned,
            &mut stdout,
            &mut stderr,
            &bounded,
            Some(b"start: ok\nprepare: ok\n"),
        )?;
        bounded.check()?;
        validation_error = validate_locked(&bounded).err();
        if validation_error.is_some() {
            ref_transaction_write(&mut stdin, b"abort\n", &bounded)?;
        } else {
            // Mark before the first possible byte; failed/partial commit writes
            // must retain unknown history instead of reporting not_started.
            bounded.check()?;
            commit_may_have_been_sent = true;
            ref_transaction_write(&mut stdin, b"commit\n", &bounded)?;
        }
        Ok(())
    })();
    if operation.is_err() {
        interrupted = true;
    }
    drop(stdin);
    if operation.is_ok()
        && ref_transaction_poll(&owned, &mut stdout, &mut stderr, &bounded, None).is_err()
    {
        interrupted = true;
    }
    // Keep the unreaped leader identity until all owned-group signals end.
    owned.signal_own_group();
    let status = owned.reap()?;
    while stdout.pump() {}
    while stderr.pump() {}
    let complete = !stdout.truncated
        && !stderr.truncated
        && !stdout.failed
        && !stderr.failed
        && stdout.eof
        && stderr.eof;
    let commit_acknowledged = commit_may_have_been_sent
        && status.success()
        && stdout.bytes == b"start: ok\nprepare: ok\ncommit: ok\n";
    let abort_acknowledged = !commit_may_have_been_sent
        && status.success()
        && complete
        && stdout.bytes == b"start: ok\nprepare: ok\nabort: ok\n";
    interrupted |= bounded.check().is_err();
    let io = adoption::ControlledIntegrationIo {
        elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        stdout_retained_bytes: Some(stdout.bytes.len() as u64),
        stderr_retained_bytes: Some(stderr.bytes.len() as u64),
        stdout_eof: stdout.eof,
        stderr_eof: stderr.eof,
        owned_leader_reaped: Some(owned.reaped),
        owned_group_signal_attempted: Some(owned.signal_attempted),
        interrupted,
        read_failed: Some(stdout.failed || stderr.failed || stdout.truncated || stderr.truncated),
    };
    control.note_git_io(&io);
    Ok(PreparedRefOutput {
        result: ControlledGitOutput {
            output: Output {
                status,
                stdout: stdout.bytes,
                stderr: stderr.bytes,
            },
            interrupted,
            io,
        },
        commit_acknowledged,
        abort_acknowledged,
        validation_error,
    })
}

/// Controlled-only runner sharing the original clock and IO observations.
/// None preserves the established default runner and result classification.
fn output_with_control(
    command: &mut Command,
    control: Option<&adoption::AdoptionControl<'_>>,
) -> Result<Output, WorkspaceError> {
    match control {
        None => bounded_output(command),
        Some(control) => {
            control.check()?;
            let result = controlled_output(command, control)?;
            control.check()?;
            if result.interrupted || !result.io.io_complete() {
                return Err(WorkspaceError::ResourceLimit);
            }
            Ok(result.output)
        }
    }
}
/// Git-only inspection under one original monotonic control. The ordinary
/// public inspect API is unchanged. Every existing safety predicate is retained.
pub(crate) fn inspect_controlled(
    root: &Path,
    control: &adoption::AdoptionControl<'_>,
) -> Result<WorkspaceIdentity, WorkspaceError> {
    control.check()?;
    let canonical_root = fs::canonicalize(root)?;
    let metadata = fs::metadata(&canonical_root)?;
    if !metadata.is_dir() {
        return Err(WorkspaceError::InvalidPath);
    }
    let run = |args: &[&str]| -> Result<Output, WorkspaceError> {
        control.check()?;
        let observation = controlled_output(git_command(&canonical_root).args(args), control)?;
        control.check()?;
        if observation.interrupted {
            return Err(WorkspaceError::ResourceLimit);
        }
        Ok(observation.output)
    };
    let config = run(&["config", "--no-includes", "--null", "--list"])?;
    if !config.status.success() || (!config.stdout.is_empty() && config.stdout.last() != Some(&0)) {
        return Err(WorkspaceError::NotRepository);
    }
    for record in config.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let key = record.split(|b| *b == b'\n').next().unwrap_or_default();
        if key == b"include.path"
            || (key.starts_with(b"includeif.")
                && key.ends_with(b".path")
                && key.len() >= b"includeif.".len() + b".path".len())
            || key == b"extensions.partialclone"
            || (key.starts_with(b"remote.")
                && key.ends_with(b".promisor")
                && key.len() >= b"remote.".len() + b".promisor".len())
            || key.starts_with(b"filter.")
        {
            return Err(WorkspaceError::UnsafeRepository);
        }
    }
    let batch = run(&[
        "rev-parse",
        "--path-format=absolute",
        "--show-toplevel",
        "--git-common-dir",
        "--verify",
        "HEAD^{commit}",
    ])?;
    let parsed = if batch.status.success() {
        let text = String::from_utf8(batch.stdout).map_err(|_| WorkspaceError::InvalidPath)?;
        let fields = text.lines().collect::<Vec<_>>();
        if fields.len() == 3 && is_oid(fields[2]) {
            Some((
                fs::canonicalize(fields[0])?,
                fields[2].to_owned(),
                fs::canonicalize(fields[1])?,
            ))
        } else {
            None
        }
    } else {
        None
    };
    let (top, head, common_dir) = if let Some(fields) = parsed {
        fields
    } else {
        let text = |args: &[&str]| -> Result<String, WorkspaceError> {
            let output = run(args)?;
            if !output.status.success() {
                return Err(WorkspaceError::NotRepository);
            }
            String::from_utf8(output.stdout)
                .map(|s| s.trim().to_owned())
                .map_err(|_| WorkspaceError::InvalidPath)
        };
        (
            fs::canonicalize(text(&["rev-parse", "--show-toplevel"])?)?,
            text(&["rev-parse", "--verify", "HEAD^{commit}"])?,
            fs::canonicalize(text(&[
                "rev-parse",
                "--path-format=absolute",
                "--git-common-dir",
            ])?)?,
        )
    };
    if top != canonical_root || !is_oid(&head) {
        return Err(WorkspaceError::InvalidPath);
    }
    let entries = run(&["ls-files", "--cached", "--stage", "-v", "-z"])?;
    if !entries.status.success() {
        return Err(WorkspaceError::NotRepository);
    }
    let mut submodules = false;
    for entry in entries.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        if entry.len() < 3 || entry[1] != b' ' || entry[0].is_ascii_lowercase() || entry[0] == b'S'
        {
            return Err(WorkspaceError::UnverifiedIndex);
        }
        submodules |= entry[2..].starts_with(b"160000 ");
    }
    let sparse = run(&["config", "--bool", "--get", "core.sparseCheckout"])?;
    if (sparse.status.success() && sparse.stdout != b"false\n")
        || (!sparse.status.success() && sparse.status.code() != Some(1))
    {
        return Err(WorkspaceError::UnverifiedIndex);
    }
    let status = run(&[
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
        "--ignore-submodules=all",
    ])?;
    if !status.status.success() {
        return Err(WorkspaceError::NotRepository);
    }
    control.check()?;
    Ok(WorkspaceIdentity {
        canonical_root,
        device: metadata.dev(),
        inode: metadata.ino(),
        kind: WorkspaceKind::Git {
            common_dir,
            head,
            dirty: submodules || !status.stdout.is_empty(),
        },
    })
}

#[cfg(test)]
mod controlled_spawn_hook {
    type Hook = Box<dyn FnOnce(u32)>;
    std::thread_local! {static HOOK:std::cell::RefCell<Option<Hook>>=std::cell::RefCell::new(None);}
    pub(super) struct Guard;
    pub(super) fn install(hook: impl FnOnce(u32) + 'static) -> Guard {
        HOOK.with(|slot| {
            assert!(slot.borrow().is_none());
            *slot.borrow_mut() = Some(Box::new(hook));
        });
        Guard
    }
    pub(super) fn apply(pid: u32) {
        let hook = HOOK.with(|slot| slot.borrow_mut().take());
        if let Some(hook) = hook {
            hook(pid);
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            HOOK.with(|slot| {
                slot.borrow_mut().take();
            });
        }
    }
}
#[cfg(all(test, target_os = "linux"))]
mod controlled_pipe_tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    };
    #[test]
    fn exited_leader_with_an_open_inherited_pipe_still_obeys_the_original_deadline() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let release = root.join("release");
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let start = Instant::now();
        let deadline = start + Duration::from_secs(1);
        let release_file = release.clone();
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let (writer_tx, writer_rx) = mpsc::sync_channel(1);
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        let owner = std::thread::spawn(move || {
            // The parent opens the inherited writer before this owned runner can
            // reap the leader. No potentially reused PID is opened or signalled.
            let _hook = controlled_spawn_hook::install(move |pid| {
                let writer = File::options()
                    .write(true)
                    .open(format!("/proc/{pid}/fd/1"));
                writer_tx.send(writer).unwrap();
                ack_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            });
            let control = adoption::AdoptionControl::for_request(&flag, deadline);
            let result = controlled_output(
                Command::new("/bin/sh").args([
                    OsStr::new("-c"),
                    OsStr::new("while [ ! -f \"$1\" ]; do :; done; exit 0"),
                    OsStr::new("fixture"),
                    release_file.as_os_str(),
                ]),
                &control,
            );
            let _ = done_tx.send(());
            result
        });
        let held_writer = writer_rx
            .recv_timeout(Duration::from_secs(2))
            .ok()
            .and_then(Result::ok);
        let Some(held_writer) = held_writer else {
            cancel.store(true, Ordering::Release);
            let _ = ack_tx.send(());
            let _ = owner.join();
            panic!("fixture could not retain its own pipe writer");
        };
        if fs::write(&release, b"release").is_err() {
            cancel.store(true, Ordering::Release);
            drop(held_writer);
            let _ = ack_tx.send(());
            let _ = owner.join();
            panic!("fixture release failed");
        }
        let _ = ack_tx.send(());
        let completed = done_rx.recv_timeout(Duration::from_secs(2));
        drop(held_writer);
        let result = owner.join().unwrap().unwrap();
        assert!(
            completed.is_ok(),
            "an open pipe must not cause an unbounded reader join"
        );
        assert!(start.elapsed() < Duration::from_secs(3));
        assert!(
            result.output.status.success(),
            "the actual original leader exited0 before the controlled pipe deadline"
        );
        assert!(result.interrupted);
        assert_eq!(result.io.owned_leader_reaped(), Some(true));
        assert!(!result.io.stdout_eof());
        assert!(!result.io.io_complete());
        assert_eq!(result.io.stdout_retained_bytes(), Some(0));
    }
}
