//! Private bounded request observations; none of these types deserialize into authority.
use super::{authority::*, IntegrationProgramPin};
use crate::{CoreError, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io,
    io::Read,
    os::fd::AsFd,
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
    entry: &'static str,
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
        Self::capture_named(parent, "refs", control, budget)
    }
    fn capture_named(
        parent: &Path,
        entry: &'static str,
        control: &IntegrationReadControl,
        budget: &mut TreeBudget,
    ) -> Result<Self> {
        if budget.directories >= MAX_TREE_DIRECTORIES {
            return Err(limited());
        }
        budget.directories += 1;
        let fd = tree_root(parent)?;
        let stamp = tree_stamp(&rustix::fs::fstat(&fd).map_err(|_| changed())?);
        let refs = match rustix::fs::statat(&fd, entry, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(st)
                if rustix::fs::FileType::from_raw_mode(st.st_mode)
                    == rustix::fs::FileType::Directory =>
            {
                Some(if entry == "worktrees" {
                    GitTreeObservation::capture_admin(&parent.join(entry), control, budget)?
                } else {
                    GitTreeObservation::capture(&parent.join(entry), control, budget)?
                })
            }
            Err(rustix::io::Errno::NOENT) => None,
            _ => return Err(changed()),
        };
        let observed = Self {
            parent: parent.into(),
            entry,
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
                    rustix::fs::statat(&self.fd, self.entry, rustix::fs::AtFlags::SYMLINK_NOFOLLOW),
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
        Self::capture_inner(root, control, budget, false)
    }
    fn capture_admin(
        root: &Path,
        control: &IntegrationReadControl,
        budget: &mut TreeBudget,
    ) -> Result<Self> {
        Self::capture_inner(root, control, budget, true)
    }
    fn capture_inner(
        root: &Path,
        control: &IntegrationReadControl,
        budget: &mut TreeBudget,
        admin: bool,
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
                if !admin && i == 0 && name.to_bytes() == b".git" {
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
                if !(matches!(kind, FileType::RegularFile | FileType::Directory)
                    || (!admin && kind == FileType::Symlink))
                    || (kind != FileType::Directory && st.st_nlink != 1)
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

// Darwin mount ABI flags from the already locked libc 0.2.191 source. The
// production calls use rustix, so no additional dependency/unsafe FFI is needed.
#[cfg(any(target_os = "macos", test))]
// MNT_RDONLY | MNT_LOCAL | MNT_ROOTFS | MNT_SNAPSHOT.
const APPLE_ROOT_FLAGS: u32 = 0x0000_0001 | 0x0000_1000 | 0x0000_4000 | 0x4000_0000;
#[cfg(any(target_os = "macos", test))]
// MNT_NOEXEC | MNT_IGNORE_OWNERSHIP.
const APPLE_FORBIDDEN_FLAGS: u32 = 0x0000_0004 | 0x0020_0000;
#[cfg(any(target_os = "macos", test))]
const APPLE_VFS_READONLY: u64 = 1;

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct AppleSystemVolume {
    device: u64,
    fsid: u64,
    flags: u32,
    flags_ext: u32,
    vfs_flags: u64,
    owner: u32,
    fs_type: u32,
    fs_subtype: u32,
    filesystem: Vec<u8>,
    mountpoint: Vec<u8>,
    source: Vec<u8>,
    readonly: bool,
}
#[cfg(any(target_os = "macos", test))]
impl AppleSystemVolume {
    fn protected_root_snapshot(&self) -> bool {
        self.readonly
            && self.vfs_flags & APPLE_VFS_READONLY != 0
            && self.flags & APPLE_ROOT_FLAGS == APPLE_ROOT_FLAGS
            && self.flags & APPLE_FORBIDDEN_FLAGS == 0
            && self.owner == 0
            && self.filesystem.as_slice() == b"apfs"
            && self.mountpoint.as_slice() == b"/"
            && self.source.strip_prefix(b"/dev/disk").is_some_and(|tail| {
                !tail.is_empty()
                    && tail[0].is_ascii_digit()
                    && tail.iter().all(|b| b.is_ascii_digit() || *b == b's')
            })
    }
}
#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct AppleFileIdentity {
    device: u64,
    inode: u64,
    uid: u32,
    mode: u32,
    links: u64,
    flags: u32,
}
#[cfg(any(target_os = "macos", test))]
fn apple_git_allowed(path: &Path, file: &AppleFileIdentity, volume: &AppleSystemVolume) -> bool {
    path.as_os_str() == std::ffi::OsStr::new(agentbaton_workspaces::MACOS_SYSTEM_GIT)
        && file.mode & 0o170000 == 0o100000
        && file.uid == 0
        && file.mode & 0o022 == 0
        && file.mode & 0o111 != 0
        && file.links > 0
        && file.device == volume.device
        && volume.protected_root_snapshot()
}
#[cfg(any(target_os = "macos", test))]
fn apple_git_observation_matches(
    path: &Path,
    expected_file: &AppleFileIdentity,
    expected_volume: &AppleSystemVolume,
    file: &AppleFileIdentity,
    volume: &AppleSystemVolume,
) -> bool {
    file == expected_file && volume == expected_volume && apple_git_allowed(path, file, volume)
}
#[cfg(any(target_os = "macos", test))]
fn apple_ancestor_allowed(
    path: &Path,
    file: &AppleFileIdentity,
    volume: &AppleSystemVolume,
    expected_volume: &AppleSystemVolume,
) -> bool {
    ["/", "/usr", "/usr/bin"]
        .iter()
        .any(|name| path.as_os_str() == std::ffi::OsStr::new(*name))
        && file.mode & 0o170000 == 0o040000
        && file.uid == 0
        && file.mode & 0o022 == 0
        && file.links > 0
        && file.device == volume.device
        && volume == expected_volume
        && volume.protected_root_snapshot()
}
#[cfg(target_os = "macos")]
fn apple_file_identity(file: &File) -> Result<AppleFileIdentity> {
    let stat = rustix::fs::fstat(file).map_err(|_| changed())?;
    Ok(AppleFileIdentity {
        device: stat.st_dev as u64,
        inode: stat.st_ino,
        uid: stat.st_uid,
        mode: u32::from(stat.st_mode),
        links: u64::from(stat.st_nlink),
        flags: stat.st_flags,
    })
}
#[cfg(target_os = "macos")]
fn apple_volume(file: &File) -> Result<AppleSystemVolume> {
    fn bounded_string<const N: usize>(value: &[std::ffi::c_char; N]) -> Result<Vec<u8>> {
        let end = value.iter().position(|b| *b == 0).ok_or_else(changed)?;
        Ok(value[..end].iter().map(|b| *b as u8).collect())
    }
    let stat = rustix::fs::fstatfs(file).map_err(|_| changed())?;
    let vfs = rustix::fs::fstatvfs(file).map_err(|_| changed())?;
    Ok(AppleSystemVolume {
        device: file.metadata().map_err(|_| changed())?.dev(),
        fsid: vfs.f_fsid,
        flags: stat.f_flags,
        flags_ext: stat.f_flags_ext,
        vfs_flags: vfs.f_flag.bits(),
        owner: stat.f_owner,
        fs_type: stat.f_type,
        fs_subtype: stat.f_fssubtype,
        filesystem: bounded_string(&stat.f_fstypename)?,
        mountpoint: bounded_string(&stat.f_mntonname)?,
        source: bounded_string(&stat.f_mntfromname)?,
        readonly: vfs.f_flag.contains(rustix::fs::StatVfsMountFlags::RDONLY),
    })
}
#[cfg(target_os = "macos")]
struct AppleAncestorPin {
    path: PathBuf,
    file: File,
    stamp: TreeStamp,
    identity: AppleFileIdentity,
}
#[cfg(target_os = "macos")]
struct AppleSystemGitProof {
    file: AppleFileIdentity,
    volume: AppleSystemVolume,
    ancestors: Vec<AppleAncestorPin>,
}
#[cfg(target_os = "macos")]
impl AppleSystemGitProof {
    fn capture(file: &File) -> Result<Self> {
        let path = Path::new(agentbaton_workspaces::MACOS_SYSTEM_GIT);
        let identity = apple_file_identity(file)?;
        let volume = apple_volume(file)?;
        if !apple_git_allowed(path, &identity, &volume) {
            return Err(changed());
        }
        let mut ancestors = vec![];
        for path in ["/", "/usr", "/usr/bin"] {
            let path = Path::new(path);
            let file = tree_root(path)?;
            let identity = apple_file_identity(&file)?;
            if !apple_ancestor_allowed(path, &identity, &apple_volume(&file)?, &volume) {
                return Err(changed());
            }
            let stamp = tree_stamp(&rustix::fs::fstat(&file).map_err(|_| changed())?);
            ancestors.push(AppleAncestorPin {
                path: path.into(),
                file,
                stamp,
                identity,
            });
        }
        Ok(Self {
            file: identity,
            volume,
            ancestors,
        })
    }
    fn recheck(&self, path: &Path, file: &File) -> Result<()> {
        if path.canonicalize().map_err(|_| changed())?.as_os_str() != path.as_os_str()
            || !apple_git_observation_matches(
                path,
                &self.file,
                &self.volume,
                &apple_file_identity(file)?,
                &apple_volume(file)?,
            )
        {
            return Err(changed());
        }
        for ancestor in &self.ancestors {
            let current = tree_root(&ancestor.path)?;
            for file in [&ancestor.file, &current] {
                let identity = apple_file_identity(file)?;
                if tree_stamp(&rustix::fs::fstat(file).map_err(|_| changed())?) != ancestor.stamp
                    || identity != ancestor.identity
                    || !apple_ancestor_allowed(
                        &ancestor.path,
                        &identity,
                        &apple_volume(file)?,
                        &self.volume,
                    )
                {
                    return Err(changed());
                }
            }
        }
        // Reopen the named leaf without following symlinks as well as checking
        // the held descriptor. A mount/volume change cannot inherit the proof.
        let parent = tree_root(path.parent().ok_or_else(changed)?)?;
        let current: File = rustix::fs::openat(
            &parent,
            path.file_name().ok_or_else(changed)?,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| changed())?
        .into();
        if tree_stamp(&rustix::fs::fstat(&current).map_err(|_| changed())?)
            != tree_stamp(&rustix::fs::fstat(file).map_err(|_| changed())?)
            || !apple_git_observation_matches(
                path,
                &self.file,
                &self.volume,
                &apple_file_identity(&current)?,
                &apple_volume(&current)?,
            )
        {
            return Err(changed());
        }
        Ok(())
    }
}

/// Holds full observed metadata excluding atime, including the original descriptor.
pub(super) struct LeafPin {
    path: PathBuf,
    file: File,
    stamp: TreeStamp,
    #[cfg(target_os = "macos")]
    apple_system_git: Option<AppleSystemGitProof>,
}
impl LeafPin {
    pub(super) fn file_pin(&self) -> Result<agentbaton_workspaces::data_workspace::FilePin> {
        self.recheck()?;
        let m = self.file.metadata().map_err(|_| changed())?;
        Ok(agentbaton_workspaces::data_workspace::FilePin {
            device: m.dev(),
            inode: m.ino(),
            bytes: m.len(),
            uid: m.uid(),
            gid: m.gid(),
            mode: m.mode(),
            mtime: m.mtime(),
            mtime_nsec: m.mtime_nsec(),
            ctime: m.ctime(),
            ctime_nsec: m.ctime_nsec(),
        })
    }
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
            #[cfg(target_os = "macos")]
            apple_system_git: None,
        };
        pin.recheck()?;
        Ok(pin)
    }
    #[cfg(target_os = "macos")]
    fn capture_apple_system_git() -> Result<Self> {
        let path = Path::new(agentbaton_workspaces::MACOS_SYSTEM_GIT);
        if path.canonicalize().map_err(|_| changed())?.as_os_str() != path.as_os_str() {
            return Err(changed());
        }
        let parent = tree_root(path.parent().ok_or_else(changed)?)?;
        let file: File = rustix::fs::openat(
            &parent,
            path.file_name().ok_or_else(changed)?,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| changed())?
        .into();
        let proof = AppleSystemGitProof::capture(&file)?;
        let stamp = tree_stamp(&rustix::fs::fstat(&file).map_err(|_| changed())?);
        let pin = Self {
            path: path.into(),
            file,
            stamp,
            apple_system_git: Some(proof),
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
        #[cfg(target_os = "macos")]
        if let Some(proof) = &self.apple_system_git {
            proof.recheck(&self.path, &self.file)?;
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
        Self::capture_inner(roots, control, false)
    }
    pub(super) fn capture_with_worktrees(
        roots: &[PathBuf],
        control: &IntegrationReadControl,
    ) -> Result<Self> {
        Self::capture_inner(roots, control, true)
    }
    fn capture_inner(
        roots: &[PathBuf],
        control: &IntegrationReadControl,
        include_worktrees: bool,
    ) -> Result<Self> {
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
            let common_dir = match fs::symlink_metadata(gd.join("commondir")) {
                Ok(_) => {
                    let pointer = LeafPin::capture(&gd.join("commondir"))?;
                    let mut bytes = Vec::new();
                    pointer
                        .file
                        .try_clone()
                        .map_err(|_| changed())?
                        .take(4097)
                        .read_to_end(&mut bytes)
                        .map_err(|_| changed())?;
                    pointer.recheck()?;
                    let text = std::str::from_utf8(&bytes)
                        .map_err(|_| changed())?
                        .trim_end_matches(['\n', '\r']);
                    if bytes.len() > 4096 || text.is_empty() || text.chars().any(char::is_control) {
                        return Err(changed());
                    }
                    gd.join(text).canonicalize().map_err(|_| changed())?
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => gd.clone(),
                Err(_) => return Err(changed()),
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
                if include_worktrees {
                    result.refs.push(GitRefObservation::capture_named(
                        parent,
                        "worktrees",
                        control,
                        &mut budget,
                    )?);
                }
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
    // The API has no caller-selected Git executable. macOS uses its fixed OS
    // entry point, never a failed PATH candidate followed by a silent fallback.
    #[cfg(target_os = "macos")]
    let pin = LeafPin::capture_apple_system_git()?;
    #[cfg(not(target_os = "macos"))]
    let pin = {
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
        LeafPin::capture(&selected.ok_or_else(changed)?)?
    };
    control.check()?;
    let path = pin.path.clone();
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
struct ControlledGitChild {
    child: std::process::Child,
    reaped: bool,
    signal_attempted: bool,
}
impl ControlledGitChild {
    fn finished(&self) -> io::Result<bool> {
        let raw = i32::try_from(self.child.id())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        let pid = rustix::process::Pid::from_raw(raw)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        rustix::process::waitid(
            rustix::process::WaitId::Pid(pid),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        )
        .map(|status| status.is_some())
        .map_err(io::Error::from)
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
                if let Some(pid) = rustix::process::Pid::from_raw(pid) {
                    let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
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
impl<T: Read + AsFd> ControlledCapture<T> {
    fn new(pipe: T) -> io::Result<Self> {
        let flags = rustix::fs::fcntl_getfl(&pipe).map_err(io::Error::from)?;
        rustix::fs::fcntl_setfl(&pipe, flags | rustix::fs::OFlags::NONBLOCK)
            .map_err(io::Error::from)?;
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
/// Only fixed internal Git argv reach here. Capture has no detached readers;
/// the original deadline covers leader and inherited pipe completion.
pub(super) fn git(
    root: &Path,
    program: &IntegrationProgramPin,
    args: &[&std::ffi::OsStr],
    control: &IntegrationReadControl,
) -> Result<Vec<u8>> {
    let (status, bytes) = git_status(root, program, args, control)?;
    if !status.success() {
        return Err(changed());
    }
    Ok(bytes)
}
// Exit status is used only by fixed callers that must distinguish symbolic-ref
// exit 1 (direct ref) from actual Git errors. All ownership and bounds are shared.
pub(super) fn git_status(
    root: &Path,
    program: &IntegrationProgramPin,
    args: &[&std::ffi::OsStr],
    control: &IntegrationReadControl,
) -> Result<(std::process::ExitStatus, Vec<u8>)> {
    use std::process::{Command, Stdio};
    control.check()?;
    let mut command = Command::new(&program.canonical_path);
    agentbaton_workspaces::configure_local_git_environment(&mut command);
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
            "-c",
            "core.ignoreStat=false",
            "-c",
            "core.trustctime=true",
            "-c",
            "core.checkStat=default",
        ])
        .arg("-C")
        .arg(root)
        .args(args);
    use std::os::unix::process::CommandExt;
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|_| changed())?;
    let mut owned = ControlledGitChild {
        child,
        reaped: false,
        signal_attempted: false,
    };
    #[cfg(test)]
    controlled_pipe_hook::apply(owned.child.id());
    let mut stdout = ControlledCapture::new(owned.child.stdout.take().ok_or_else(changed)?)
        .map_err(|_| changed())?;
    let mut stderr = ControlledCapture::new(owned.child.stderr.take().ok_or_else(changed)?)
        .map_err(|_| changed())?;
    let deadline = control
        .deadline
        .min(Instant::now() + Duration::from_secs(15));
    let mut failure = None;
    loop {
        let progress = stdout.pump() | stderr.pump();
        if stdout.failed || stderr.failed {
            failure = Some(changed());
            break;
        }
        if stdout.truncated || stderr.truncated {
            failure = Some(limited());
            break;
        }
        let finished = match owned.finished() {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => false,
            Err(_) => {
                failure = Some(CoreError::new(
                    "OUTCOME_UNKNOWN",
                    "Owned integration Git leader could not be observed",
                ));
                break;
            }
        };
        if finished && stdout.eof && stderr.eof {
            break;
        }
        if let Err(error) = control.check() {
            failure = Some(error);
            break;
        }
        if Instant::now() >= deadline {
            failure = Some(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Owned integration Git command exceeded its original bounded deadline",
            ));
            break;
        }
        if !progress {
            std::thread::sleep(
                Duration::from_millis(2).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    }
    owned.signal_own_group();
    let status = owned.reap().map_err(|_| {
        CoreError::new(
            "OUTCOME_UNKNOWN",
            "Owned integration Git command could not be reaped",
        )
    })?;
    while stdout.pump() {}
    while stderr.pump() {}
    if let Some(error) = failure {
        return Err(error);
    }
    if !stdout.eof || !stderr.eof || stdout.failed || stderr.failed {
        return Err(CoreError::new(
            "OUTCOME_UNKNOWN",
            "Integration Git streams have no complete bounded IO receipt",
        ));
    }
    if stdout.truncated || stderr.truncated {
        return Err(limited());
    }
    let out = stdout.bytes;
    control.check()?;
    Ok((status, out))
}

pub(super) fn directory_identity(path: &Path) -> Result<(u64, u64)> {
    let file = tree_root(path)?;
    let m = file.metadata().map_err(|_| changed())?;
    Ok((m.dev(), m.ino()))
}

/// Equivalent Git-only inspection guards with the original request deadline.
pub(super) fn inspect_git(
    root: &Path,
    program: &IntegrationProgramPin,
    control: &IntegrationReadControl,
) -> Result<agentbaton_workspaces::WorkspaceIdentity> {
    use std::ffi::OsStr;
    control.check()?;
    let text = root.to_str().ok_or_else(changed)?;
    if !root.is_absolute()
        || text.len() > 4096
        || text.chars().any(char::is_control)
        || root.canonicalize().map_err(|_| changed())? != root
    {
        return Err(changed());
    }
    let fd = tree_root(root)?;
    let m = fd.metadata().map_err(|_| changed())?;
    let run = |args: &[&str]| {
        let args = args.iter().map(OsStr::new).collect::<Vec<_>>();
        git(root, program, &args, control)
    };
    let config = run(&["config", "--no-includes", "--null", "--list"])?;
    if !config.is_empty() && config.last() != Some(&0) {
        return Err(changed());
    }
    for record in config.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let mut item = record.splitn(2, |b| *b == b'\n');
        let key = item.next().ok_or_else(changed)?;
        let value = item.next();
        if key == b"include.path"
            || (key.starts_with(b"includeif.") && key.ends_with(b".path"))
            || key == b"extensions.partialclone"
            || (key.starts_with(b"remote.") && key.ends_with(b".promisor"))
            || key.starts_with(b"filter.")
        {
            return Err(CoreError::new("PERMISSION_DENIED","Integration repository contains unsupported config indirection, filters or external object sources"));
        }
        if key == b"core.sparsecheckout"
            && !value.is_some_and(|v| {
                [b"false".as_slice(), b"0", b"off", b"no"]
                    .iter()
                    .any(|s| v.eq_ignore_ascii_case(s))
            })
        {
            return Err(changed());
        }
    }
    let metadata = run(&[
        "rev-parse",
        "--path-format=absolute",
        "--show-toplevel",
        "--git-common-dir",
        "--verify",
        "HEAD^{commit}",
    ])?;
    let fields = std::str::from_utf8(&metadata)
        .map_err(|_| changed())?
        .lines()
        .collect::<Vec<_>>();
    if fields.len() != 3
        || !super::reads::oid(fields[2])
        || Path::new(fields[0]).canonicalize().map_err(|_| changed())? != root
    {
        return Err(changed());
    }
    let common_dir = Path::new(fields[1]).canonicalize().map_err(|_| changed())?;
    let entries = run(&["ls-files", "--cached", "--stage", "-v", "-z"])?;
    let mut gitlink = false;
    for entry in entries.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        if entry.len() < 3 || entry[1] != b' ' || entry[0].is_ascii_lowercase() || entry[0] == b'S'
        {
            return Err(changed());
        }
        gitlink |= entry[2..].starts_with(b"160000 ");
    }
    let status = run(&[
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
        "--ignore-submodules=all",
    ])?;
    control.check()?;
    let now = tree_root(root)?.metadata().map_err(|_| changed())?;
    if (m.dev(), m.ino()) != (now.dev(), now.ino()) {
        return Err(changed());
    }
    Ok(agentbaton_workspaces::WorkspaceIdentity {
        canonical_root: root.into(),
        device: m.dev(),
        inode: m.ino(),
        kind: agentbaton_workspaces::WorkspaceKind::Git {
            common_dir,
            head: fields[2].into(),
            dirty: gitlink || !status.is_empty(),
        },
    })
}

#[cfg(test)]
#[path = "integration_system_git_tests.rs"]
mod system_git_policy_tests;

#[cfg(test)]
mod controlled_git_tests {
    use super::*;
    use std::{ffi::OsStr, os::unix::fs::PermissionsExt};
    fn fixture(body: &str) -> (tempfile::TempDir, IntegrationProgramPin) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let file = root.join("controlled-git");
        fs::write(&file, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o700)).unwrap();
        let m = file.metadata().unwrap();
        let bytes = fs::read(&file).unwrap();
        let program = IntegrationProgramPin {
            canonical_path: file,
            device: m.dev().to_string(),
            inode: m.ino().to_string(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes: m.len(),
            uid: m.uid(),
            mode: m.mode(),
            links: m.nlink().to_string(),
        };
        (dir, program)
    }
    #[test]
    fn expired_request_does_not_spawn_a_git_process() {
        let (dir, program) = fixture("for arg; do last=\"$arg\"; done; printf started > \"$last\"");
        let marker = dir.path().join("marker");
        let control = IntegrationReadControl {
            cancel: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now(),
        };
        assert_eq!(
            git(dir.path(), &program, &[marker.as_os_str()], &control)
                .unwrap_err()
                .code,
            "DEADLINE_EXCEEDED"
        );
        assert!(!marker.exists());
    }
    #[test]
    fn request_cancel_does_not_spawn_a_git_process() {
        let (dir, program) = fixture("for arg; do last=\"$arg\"; done; printf started > \"$last\"");
        let marker = dir.path().join("marker");
        let control = IntegrationReadControl {
            cancel: Arc::new(AtomicBool::new(true)),
            deadline: Instant::now() + Duration::from_secs(5),
        };
        assert_eq!(
            git(dir.path(), &program, &[marker.as_os_str()], &control)
                .unwrap_err()
                .code,
            "CANCEL_REQUESTED"
        );
        assert!(!marker.exists());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn original_request_deadline_kills_and_reaps_the_owned_git_child() {
        let (dir, program) = fixture(
            "for arg; do last=\"$arg\"; done; printf '%s' \"$$\" > \"$last\"; exec /bin/sleep 30",
        );
        let marker = dir.path().join("pid");
        let start = Instant::now();
        let control = IntegrationReadControl {
            cancel: Arc::new(AtomicBool::new(false)),
            deadline: start + Duration::from_millis(200),
        };
        assert_eq!(
            git(dir.path(), &program, &[marker.as_os_str()], &control)
                .unwrap_err()
                .code,
            "DEADLINE_EXCEEDED"
        );
        assert!(start.elapsed() < Duration::from_secs(3));
        let pid: std::num::NonZeroU32 = fs::read_to_string(marker).unwrap().parse().unwrap();
        assert!(
            !Path::new("/proc").join(pid.to_string()).exists(),
            "owned child must be actually reaped before the helper returns"
        );
    }
    #[test]
    fn git_stdout_has_an_exact_raw_byte_cap() {
        let (dir, program) = fixture("printf '%4194305s' ''");
        let control = IntegrationReadControl {
            cancel: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(5),
        };
        assert_eq!(
            git(dir.path(), &program, &[OsStr::new("bounded")], &control)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
    }
}

#[cfg(test)]
mod controlled_pipe_hook {
    use std::cell::RefCell;
    type Hook = Box<dyn FnOnce(u32)>;
    thread_local! { static HOOK:RefCell<Option<Hook>>=RefCell::new(None); }
    pub(super) struct Guard;
    pub(super) fn install(hook: impl FnOnce(u32) + 'static) -> Guard {
        HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
        Guard
    }
    pub(super) fn apply(pid: u32) {
        if let Some(hook) = HOOK.with(|slot| slot.borrow_mut().take()) {
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
mod controlled_inherited_pipe_test {
    use super::*;
    use std::sync::mpsc;
    #[test]
    fn exited_leader_does_not_hide_original_deadline_behind_an_inherited_writer() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        use std::os::unix::fs::PermissionsExt;
        let executable = root.join("owned-git-fixture");
        fs::write(&executable, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let (writer_tx, writer_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let started = Instant::now();
        let owner = std::thread::spawn(move || {
            let _hook = controlled_pipe_hook::install(move |pid| {
                let writer = File::options()
                    .write(true)
                    .open(format!("/proc/{pid}/fd/1"));
                writer_tx.send(writer).unwrap();
                release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            });
            let program = IntegrationProgramPin {
                canonical_path: executable,
                device: String::new(),
                inode: String::new(),
                sha256: String::new(),
                bytes: 0,
                uid: 0,
                mode: 0,
                links: String::new(),
            };
            let control = IntegrationReadControl {
                cancel: Arc::new(AtomicBool::new(false)),
                deadline: started + Duration::from_secs(1),
            };
            git(
                &root,
                &program,
                &[std::ffi::OsStr::new("-c"), std::ffi::OsStr::new("exit 0")],
                &control,
            )
        });
        let held = writer_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        release_tx.send(()).unwrap();
        let result = owner.join().unwrap();
        drop(held);
        assert_eq!(result.unwrap_err().code, "DEADLINE_EXCEEDED");
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}

#[cfg(test)]
mod target_worktree_pin_tests {
    use super::*;
    #[test]
    fn target_preview_seals_other_worktree_heads_and_missing_registry_entries() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let control = IntegrationReadControl {
            cancel: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(3),
        };
        let absent = GitRefObservation::capture_named(
            &root,
            "worktrees",
            &control,
            &mut TreeBudget::default(),
        )
        .unwrap();
        fs::create_dir(root.join("worktrees")).unwrap();
        fs::create_dir(root.join("worktrees/other")).unwrap();
        fs::write(
            root.join("worktrees/other/HEAD"),
            b"ref: refs/heads/original\n",
        )
        .unwrap();
        assert!(absent.recheck().is_err());
        let present = GitRefObservation::capture_named(
            &root,
            "worktrees",
            &control,
            &mut TreeBudget::default(),
        )
        .unwrap();
        fs::write(root.join("worktrees/other/HEAD"), b"ref: refs/heads/x\n").unwrap();
        assert!(present.recheck().is_err());
        fs::create_dir(root.join("worktrees/.git")).unwrap();
        fs::write(
            root.join("worktrees/.git/HEAD"),
            b"ref: refs/heads/admin-entry\n",
        )
        .unwrap();
        let admin_marker = GitRefObservation::capture_named(
            &root,
            "worktrees",
            &control,
            &mut TreeBudget::default(),
        )
        .unwrap();
        fs::write(root.join("worktrees/.git/HEAD"), b"ref: refs/heads/x\n").unwrap();
        assert!(
            admin_marker.recheck().is_err(),
            "admin directories do not inherit the user-tree .git skip"
        );
        fs::remove_file(root.join("worktrees/other/HEAD")).unwrap();
        fs::write(root.join("external-head"), b"ref: refs/heads/unobserved\n").unwrap();
        std::os::unix::fs::symlink(
            root.join("external-head"),
            root.join("worktrees/other/HEAD"),
        )
        .unwrap();
        assert!(GitRefObservation::capture_named(
            &root,
            "worktrees",
            &control,
            &mut TreeBudget::default()
        )
        .is_err());
    }
}
