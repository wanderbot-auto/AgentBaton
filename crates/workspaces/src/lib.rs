//! Owner-selected workspace operations. Worktrees organize writes; they are not OS sandboxes.
//! No API here removes a source directory, executes its scripts, or stashes/resets source edits.

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
                "repository requires unapproved executable filters or external object access"
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

fn reject_hidden_index_entries(root: &Path) -> Result<(), WorkspaceError> {
    // `-v` renders assume-unchanged entries with lowercase status letters; `S` denotes
    // skip-worktree (including sparse-directory index entries). Read only: never clear flags.
    let entries = bounded_output(git_command(root).args(["ls-files", "--cached", "-v", "-z"]))?;
    if !entries.status.success() {
        return Err(WorkspaceError::NotRepository);
    }
    for entry in entries.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        if entry.len() < 3 || entry[1] != b' ' {
            return Err(WorkspaceError::UnverifiedIndex);
        }
        if entry[0].is_ascii_lowercase() || entry[0] == b'S' {
            return Err(WorkspaceError::UnverifiedIndex);
        }
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
    Ok(())
}

/// Import is read-only and does not initialize Git or execute repository scripts/hooks.
/// An unborn or corrupt repository is an error rather than silently treated as a data directory.
pub fn inspect(root: &Path) -> Result<WorkspaceIdentity, WorkspaceError> {
    let canonical_root = fs::canonicalize(root)?;
    let metadata = fs::metadata(&canonical_root)?;
    if !metadata.is_dir() {
        return Err(WorkspaceError::InvalidPath);
    }
    let probe = output(
        &canonical_root,
        &[OsStr::new("rev-parse"), OsStr::new("--show-toplevel")],
    )?;
    let kind = if probe.status.success() {
        let top = fs::canonicalize(
            String::from_utf8(probe.stdout)
                .map_err(|_| WorkspaceError::InvalidPath)?
                .trim(),
        )?;
        // Explicit directory selection must not silently expand to an ancestor repository.
        if top != canonical_root {
            return Err(WorkspaceError::InvalidPath);
        }
        reject_partial_clones(&canonical_root)?;
        reject_checkout_filters(&canonical_root)?;
        reject_hidden_index_entries(&canonical_root)?;
        let head = git_text(&canonical_root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
        let common_dir = fs::canonicalize(git_text(
            &canonical_root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?)?;
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
        let index =
            bounded_output(git_command(&canonical_root).args(["ls-files", "--stage", "-z"]))?;
        if !index.status.success() {
            return Err(WorkspaceError::NotRepository);
        }
        let has_submodules = index
            .stdout
            .split(|b| *b == 0)
            .any(|entry| entry.starts_with(b"160000 "));
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
