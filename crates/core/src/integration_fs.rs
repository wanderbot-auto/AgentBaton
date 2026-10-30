//! Private bounded request observations; none of these types deserialize into authority.
use super::{authority::*, IntegrationProgramPin};
use crate::{CoreError, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub(super) struct IntegrationReadControl {
    pub(super) cancel: Arc<AtomicBool>,
    pub(super) deadline: Instant,
}
impl IntegrationReadControl {
    pub(super) fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Acquire) {
            return Err(CoreError::new(
                "CANCEL_REQUESTED",
                "Owned integration observation cancelled",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Integration observation exceeded its original deadline",
            ));
        }
        Ok(())
    }
}
// This seals the complete observed work tree, not just selected inputs. A
// directory stamp detects entry creation/removal, while each nofollow leaf stat
// detects edits outside the selected set. Captured BEFORE the worker's full Git
// clean/base check; rechecked after it and at the Actor boundary. This is an
// observation, never continuous OS immutability. Ignored entries are included
// conservatively. Large/deep trees fail explicitly rather than being truncated.
const MAX_TREE_ENTRIES: usize = 8192;
const MAX_TREE_DIRECTORIES: usize = 256;
const MAX_TREE_NAME_BYTES: usize = 1_048_576;
#[derive(PartialEq, Eq)]
struct TreeStamp([i128; 11]);
fn tree_stamp(s: &rustix::fs::Stat) -> TreeStamp {
    TreeStamp([
        s.st_dev as i128,
        s.st_ino as i128,
        s.st_mode as i128,
        s.st_uid as i128,
        s.st_gid as i128,
        s.st_nlink as i128,
        s.st_size as i128,
        s.st_mtime as i128,
        s.st_mtime_nsec as i128,
        s.st_ctime as i128,
        s.st_ctime_nsec as i128,
    ])
}
struct TreeDirectory {
    fd: File,
    stamp: TreeStamp,
    depth: usize,
}
struct TreeEntry {
    directory: usize,
    name: std::ffi::OsString,
    stamp: TreeStamp,
}
struct GitRefObservation {
    parent: PathBuf,
    fd: File,
    stamp: TreeStamp,
    refs: Option<GitTreeObservation>,
}
impl GitRefObservation {
    fn capture(
        parent: &Path,
        control: &IntegrationReadControl,
        budget: &mut TreeBudget,
    ) -> Result<Self> {
        if budget.directories >= MAX_TREE_DIRECTORIES {
            return Err(limited());
        }
        budget.directories += 1;
        let fd = tree_root(parent)?;
        let stamp = tree_stamp(&rustix::fs::fstat(&fd).map_err(|_| changed())?);
        let refs = match rustix::fs::statat(&fd, "refs", rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(st)
                if rustix::fs::FileType::from_raw_mode(st.st_mode)
                    == rustix::fs::FileType::Directory =>
            {
                Some(GitTreeObservation::capture(
                    &parent.join("refs"),
                    control,
                    budget,
                )?)
            }
            Err(rustix::io::Errno::NOENT) => None,
            _ => return Err(changed()),
        };
        let observed = Self {
            parent: parent.into(),
            fd,
            stamp,
            refs,
        };
        observed.recheck()?;
        Ok(observed)
    }
    fn recheck(&self) -> Result<()> {
        if tree_stamp(&rustix::fs::fstat(tree_root(&self.parent)?).map_err(|_| changed())?)
            != self.stamp
            || tree_stamp(&rustix::fs::fstat(&self.fd).map_err(|_| changed())?) != self.stamp
        {
            return Err(changed());
        }
        match &self.refs {
            Some(tree) => tree.recheck()?,
            None => {
                if !matches!(
                    rustix::fs::statat(&self.fd, "refs", rustix::fs::AtFlags::SYMLINK_NOFOLLOW),
                    Err(rustix::io::Errno::NOENT)
                ) {
                    return Err(changed());
                }
            }
        }
        Ok(())
    }
}
struct GitTreeObservation {
    root: PathBuf,
    directories: Vec<TreeDirectory>,
    entries: Vec<TreeEntry>,
}
#[derive(Default)]
struct TreeBudget {
    entries: usize,
    directories: usize,
    names: usize,
}
fn tree_root(path: &Path) -> Result<File> {
    use rustix::fs::{open, openat, Mode, OFlags};
    let flags =
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let mut fd: File = open("/", flags, Mode::empty())
        .map_err(|_| changed())?
        .into();
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(changed());
    }
    for part in path.components() {
        match part {
            std::path::Component::RootDir => {}
            std::path::Component::Normal(name) => {
                fd = openat(&fd, name, flags, Mode::empty())
                    .map_err(|_| changed())?
                    .into();
            }
            _ => return Err(changed()),
        }
    }
    Ok(fd)
}
impl GitTreeObservation {
    fn capture(
        root: &Path,
        control: &IntegrationReadControl,
        budget: &mut TreeBudget,
    ) -> Result<Self> {
        use rustix::fs::{fstat, openat, statat, AtFlags, Dir, FileType, Mode, OFlags};
        use std::os::unix::ffi::OsStrExt;
        control.check()?;
        if budget.directories >= MAX_TREE_DIRECTORIES {
            return Err(limited());
        }
        let fd = tree_root(root)?;
        let root_stat = fstat(&fd).map_err(|_| changed())?;
        budget.directories += 1;
        let mut observation = Self {
            root: root.into(),
            directories: vec![TreeDirectory {
                fd,
                stamp: tree_stamp(&root_stat),
                depth: 0,
            }],
            entries: vec![],
        };
        let mut i = 0;
        while i < observation.directories.len() {
            control.check()?;
            let mut stream =
                Dir::read_from(&observation.directories[i].fd).map_err(|_| changed())?;
            while let Some(entry) = stream.read() {
                control.check()?;
                let entry = entry.map_err(|_| changed())?;
                let name = entry.file_name();
                if name.to_bytes() == b"." || name.to_bytes() == b".." {
                    continue;
                }
                // .git is separately pinned by capture_pins, including its
                // worktree pointer, HEAD/index/config and common metadata.
                if i == 0 && name.to_bytes() == b".git" {
                    continue;
                }
                if budget.entries >= MAX_TREE_ENTRIES
                    || budget
                        .names
                        .checked_add(name.to_bytes().len())
                        .is_none_or(|n| n > MAX_TREE_NAME_BYTES)
                {
                    return Err(limited());
                }
                budget.entries += 1;
                budget.names += name.to_bytes().len();
                let name = std::ffi::OsStr::from_bytes(name.to_bytes()).to_os_string();
                let st = statat(
                    &observation.directories[i].fd,
                    &name,
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(|_| changed())?;
                let kind = FileType::from_raw_mode(st.st_mode);
                if !matches!(
                    kind,
                    FileType::RegularFile | FileType::Directory | FileType::Symlink
                ) || (kind != FileType::Directory && st.st_nlink != 1)
                {
                    return Err(changed());
                }
                if kind == FileType::Directory {
                    if budget.directories >= MAX_TREE_DIRECTORIES
                        || observation.directories[i].depth >= 64
                    {
                        return Err(limited());
                    }
                    let fd: File = openat(
                        &observation.directories[i].fd,
                        &name,
                        OFlags::RDONLY
                            | OFlags::DIRECTORY
                            | OFlags::NOFOLLOW
                            | OFlags::NONBLOCK
                            | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| changed())?
                    .into();
                    if tree_stamp(&fstat(&fd).map_err(|_| changed())?) != tree_stamp(&st) {
                        return Err(changed());
                    }
                    budget.directories += 1;
                    let depth = observation.directories[i].depth + 1;
                    observation.directories.push(TreeDirectory {
                        fd,
                        stamp: tree_stamp(&st),
                        depth,
                    });
                }
                observation.entries.push(TreeEntry {
                    directory: i,
                    name,
                    stamp: tree_stamp(&st),
                });
            }
            i += 1;
        }
        observation.recheck()?;
        Ok(observation)
    }
    fn recheck(&self) -> Result<()> {
        use rustix::fs::{fstat, statat, AtFlags};
        let current = tree_root(&self.root)?;
        if tree_stamp(&fstat(current).map_err(|_| changed())?) != self.directories[0].stamp {
            return Err(changed());
        }
        for directory in &self.directories {
            if tree_stamp(&fstat(&directory.fd).map_err(|_| changed())?) != directory.stamp {
                return Err(changed());
            }
        }
        for entry in &self.entries {
            let st = statat(
                &self.directories[entry.directory].fd,
                &entry.name,
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| changed())?;
            if tree_stamp(&st) != entry.stamp {
                return Err(changed());
            }
        }
        // Recheck parent directories after leaf observations to catch changes
        // during this bounded traversal. Kernel stalls remain non-interruptible.
        for directory in &self.directories {
            if tree_stamp(&fstat(&directory.fd).map_err(|_| changed())?) != directory.stamp {
                return Err(changed());
            }
        }
        Ok(())
    }
}
fn git_directory(root: &Path) -> Result<PathBuf> {
    use std::io::Read;
    let path = root.join(".git");
    let m = fs::symlink_metadata(&path).map_err(|_| changed())?;
    if m.is_dir() {
        return path.canonicalize().map_err(|_| changed());
    }
    if !m.is_file() || m.nlink() != 1 {
        return Err(changed());
    }
    let mut text = String::new();
    OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(&path)
        .map_err(|_| changed())?
        .take(4097)
        .read_to_string(&mut text)
        .map_err(|_| changed())?;
    let target = text
        .strip_prefix("gitdir: ")
        .ok_or_else(changed)?
        .trim_end_matches(['\n', '\r']);
    if text.len() > 4096 || target.is_empty() || target.contains(['\0', '\n', '\r']) {
        return Err(changed());
    }
    root.join(target).canonicalize().map_err(|_| changed())
}
fn check_head_reference(root: &Path) -> Result<()> {
    use std::io::Read;
    let mut text = String::new();
    OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(root.join("HEAD"))
        .map_err(|_| changed())?
        .take(4097)
        .read_to_string(&mut text)
        .map_err(|_| changed())?;
    let text = text.trim_end_matches(['\n', '\r']);
    if text.len() > 4096 {
        return Err(changed());
    }
    if let Some(reference) = text.strip_prefix("ref: refs/") {
        if reference.is_empty()
            || reference.contains(['\0', '\n', '\r'])
            || Path::new(reference)
                .components()
                .any(|p| !matches!(p, std::path::Component::Normal(_)))
        {
            return Err(changed());
        }
    } else if !matches!(text.len(), 40 | 64) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(changed());
    }
    Ok(())
}

/// Holds full observed metadata excluding atime, including the original descriptor.
pub(super) struct LeafPin {
    path: PathBuf,
    file: File,
    stamp: TreeStamp,
}
impl LeafPin {
    pub(super) fn capture(path: &Path) -> Result<Self> {
        use rustix::fs::{openat, Mode, OFlags};
        let parent = tree_root(path.parent().ok_or_else(changed)?)?;
        let file: File = openat(
            &parent,
            path.file_name().ok_or_else(changed)?,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| changed())?
        .into();
        let m = file.metadata().map_err(|_| changed())?;
        if !m.is_file() || m.nlink() != 1 {
            return Err(changed());
        }
        let stamp = tree_stamp(&rustix::fs::fstat(&file).map_err(|_| changed())?);
        let pin = Self {
            path: path.into(),
            file,
            stamp,
        };
        pin.recheck()?;
        Ok(pin)
    }
    pub(super) fn recheck(&self) -> Result<()> {
        if tree_stamp(&rustix::fs::fstat(&self.file).map_err(|_| changed())?) != self.stamp {
            return Err(changed());
        }
        let parent = tree_root(self.path.parent().ok_or_else(changed)?)?;
        let stat = rustix::fs::statat(
            &parent,
            self.path.file_name().ok_or_else(changed)?,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| changed())?;
        if tree_stamp(&stat) != self.stamp {
            return Err(changed());
        }
        Ok(())
    }
    pub(super) fn digest(
        &self,
        max: u64,
        control: &IntegrationReadControl,
    ) -> Result<(String, u64)> {
        use std::io::{Seek, SeekFrom};
        control.check()?;
        self.recheck()?;
        if self.file.metadata().map_err(|_| changed())?.len() > max {
            return Err(limited());
        }
        let mut fd = self.file.try_clone().map_err(|_| changed())?;
        fd.seek(SeekFrom::Start(0)).map_err(|_| changed())?;
        let mut sum = Sha256::new();
        let mut n = 0u64;
        let mut buf = [0u8; 65536];
        loop {
            control.check()?;
            let count = fd.read(&mut buf).map_err(|_| changed())?;
            if count == 0 {
                break;
            }
            n = n.checked_add(count as u64).ok_or_else(limited)?;
            if n > max {
                return Err(limited());
            }
            sum.update(&buf[..count]);
        }
        self.recheck()?;
        control.check()?;
        Ok((format!("{:x}", sum.finalize()), n))
    }
}
/// Work trees, all loose refs and fixed metadata share one request-wide resource budget.
pub(super) struct FsObservation {
    trees: Vec<GitTreeObservation>,
    refs: Vec<GitRefObservation>,
    leaves: Vec<LeafPin>,
}
impl FsObservation {
    pub(super) fn capture(roots: &[PathBuf], control: &IntegrationReadControl) -> Result<Self> {
        let mut result = Self {
            trees: vec![],
            refs: vec![],
            leaves: vec![],
        };
        let mut budget = TreeBudget::default();
        let mut seen = std::collections::BTreeSet::new();
        for root in roots {
            control.check()?;
            result
                .trees
                .push(GitTreeObservation::capture(root, control, &mut budget)?);
            let gd = git_directory(root)?;
            check_head_reference(&gd)?;
            let current = agentbaton_workspaces::inspect(root).map_err(|_| changed())?;
            let agentbaton_workspaces::WorkspaceKind::Git { common_dir, .. } = current.kind else {
                return Err(changed());
            };
            if root.join(".git").is_file() {
                result.leaves.push(LeafPin::capture(&root.join(".git"))?);
            }
            for parent in [&gd, &common_dir] {
                if !seen.insert(parent.clone()) {
                    continue;
                }
                result
                    .refs
                    .push(GitRefObservation::capture(parent, control, &mut budget)?);
                for name in [
                    "HEAD",
                    "index",
                    "config",
                    "config.worktree",
                    "packed-refs",
                    "commondir",
                    "shallow",
                ] {
                    let path = parent.join(name);
                    match fs::symlink_metadata(&path) {
                        Ok(_) => result.leaves.push(LeafPin::capture(&path)?),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                        Err(_) => return Err(changed()),
                    }
                }
            }
        }
        result.recheck()?;
        Ok(result)
    }
    pub(super) fn recheck(&self) -> Result<()> {
        for tree in &self.trees {
            tree.recheck()?;
        }
        for refs in &self.refs {
            refs.recheck()?;
        }
        for pin in &self.leaves {
            pin.recheck()?;
        }
        Ok(())
    }
    pub(super) fn add_leaf(&mut self, pin: LeafPin) {
        self.leaves.push(pin);
    }
}
pub(super) fn system_git(
    control: &IntegrationReadControl,
) -> Result<(IntegrationProgramPin, LeafPin)> {
    control.check()?;
    let path = std::env::var_os("PATH").ok_or_else(changed)?;
    let mut selected = None;
    for directory in std::env::split_paths(&path) {
        if !directory.is_absolute() {
            return Err(changed());
        }
        let candidate = directory.join("git");
        match fs::metadata(&candidate) {
            Ok(m) if m.is_file() && m.mode() & 0o111 != 0 => {
                selected = Some(candidate.canonicalize().map_err(|_| changed())?);
                break;
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(changed()),
        }
    }
    let path = selected.ok_or_else(changed)?;
    let pin = LeafPin::capture(&path)?;
    let metadata = pin.file.metadata().map_err(|_| changed())?;
    if metadata.mode() & 0o022 != 0 || metadata.mode() & 0o111 == 0 {
        return Err(changed());
    }
    #[cfg(target_os = "macos")]
    if metadata.uid() != 0 {
        return Err(changed());
    }
    let (sha256, bytes) = pin.digest(268435456, control)?;
    Ok((
        IntegrationProgramPin {
            canonical_path: path,
            device: metadata.dev().to_string(),
            inode: metadata.ino().to_string(),
            sha256,
            bytes,
            uid: metadata.uid(),
            mode: metadata.mode(),
            links: metadata.nlink().to_string(),
        },
        pin,
    ))
}
/// Only fixed internal Git argv reach here. Readers cap both streams and are always joined.
pub(super) fn git(
    root: &Path,
    program: &IntegrationProgramPin,
    args: &[&std::ffi::OsStr],
    control: &IntegrationReadControl,
) -> Result<Vec<u8>> {
    use std::{
        os::unix::ffi::OsStrExt,
        process::{Command, Stdio},
    };
    control.check()?;
    let mut command = Command::new(&program.canonical_path);
    for (key, _) in std::env::vars_os() {
        if key.as_bytes().starts_with(b"GIT_") {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_GRAFT_FILE", "/dev/null")
        .args([
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.file.allow=never",
            "-c",
            "submodule.recurse=false",
        ])
        .arg("-C")
        .arg(root)
        .args(args);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| changed())?;
    let out = child.stdout.take().ok_or_else(changed)?;
    let err = child.stderr.take().ok_or_else(changed)?;
    let read = |r: Box<dyn Read + Send>| {
        let mut bytes = vec![];
        r.take(4_194_305).read_to_end(&mut bytes).map(|_| bytes)
    };
    let stdout = std::thread::spawn(move || read(Box::new(out)));
    let stderr = std::thread::spawn(move || read(Box::new(err)));
    let deadline = control
        .deadline
        .min(Instant::now() + Duration::from_secs(15));
    let mut failure = None;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => (),
            Err(_) => {
                failure = Some(changed());
                break None;
            }
        }
        if let Err(e) = control.check() {
            failure = Some(e);
            break None;
        }
        if Instant::now() >= deadline {
            failure = Some(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Owned integration Git command exceeded its bounded deadline",
            ));
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if status.is_none() {
        let _ = child.kill();
        if child.wait().is_err() {
            failure = Some(CoreError::new(
                "OUTCOME_UNKNOWN",
                "Owned integration Git command could not be reaped",
            ));
        }
    }
    let out = stdout
        .join()
        .map_err(|_| changed())
        .and_then(|r| r.map_err(|_| changed()));
    let err = stderr
        .join()
        .map_err(|_| changed())
        .and_then(|r| r.map_err(|_| changed()));
    if let Some(e) = failure {
        return Err(e);
    }
    let out = out?;
    let err = err?;
    if out.len() > 4_194_304 || err.len() > 4_194_304 {
        return Err(limited());
    }
    if !status.is_some_and(|s| s.success()) {
        return Err(changed());
    }
    control.check()?;
    Ok(out)
}

pub(super) fn directory_identity(path: &Path) -> Result<(u64, u64)> {
    let file = tree_root(path)?;
    let m = file.metadata().map_err(|_| changed())?;
    Ok((m.dev(), m.ino()))
}
