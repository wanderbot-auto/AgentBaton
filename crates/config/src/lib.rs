//! Durable managed-configuration changes. This is a filesystem primitive, not an
//! authorization authority. The daemon must validate the owner, grants, scope and
//! content classification before constructing a managed root or accepting edits.
//! Native configurations remain read-only; this crate never imports credentials.
pub mod capabilities;

use fs2::FileExt;
use rustix::fs::{self as rfs, AtFlags, FileType, Mode, OFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::Mutex,
};
use uuid::Uuid;

const MAX_BYTES: usize = 1_048_576;
const MAX_FILES: usize = 64;
const RESERVED: &str = ".baton-";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    BatonManaged,
    NativeConnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    InvalidPath,
    PermissionDenied,
    Conflict,
    ResourceLimit,
    Io,
    OutcomeUnknown,
}

#[derive(Debug)]
pub struct ConfigError {
    pub code: ErrorCode,
    pub transaction_id: Option<String>,
}
impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Configuration operation failed: {:?}", self.code)
    }
}
impl std::error::Error for ConfigError {}
type Result<T> = std::result::Result<T, ConfigError>;
fn error(code: ErrorCode) -> ConfigError {
    ConfigError {
        code,
        transaction_id: None,
    }
}
fn io_error<T>(result: std::result::Result<T, impl std::fmt::Debug>) -> Result<T> {
    result.map_err(|_| error(ErrorCode::Io))
}
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Caller-approved, non-secret replacement bytes. No value is logged or exposed
/// in previews. Secret material must stay in the native credential mechanism.
pub struct Edit {
    relative_path: PathBuf,
    expected_sha256: Option<String>,
    replacement: Vec<u8>,
}
impl Edit {
    /// `None` explicitly expects a missing file. Hashes refer to exact original
    /// bytes; this API does not parse/reformat away unknown fields or comments.
    pub fn nonsecret_replacement(
        path: impl Into<PathBuf>,
        expected_sha256: Option<String>,
        replacement: Vec<u8>,
    ) -> Self {
        Self {
            relative_path: path.into(),
            expected_sha256,
            replacement,
        }
    }
}
struct PlannedFile {
    path: PathBuf,
    baseline: Option<Vec<u8>>,
    replacement: Vec<u8>,
}
pub struct ChangeSet {
    root_identity: (u64, u64),
    id: String,
    files: Vec<PlannedFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Preview {
    pub relative_path: PathBuf,
    pub before_sha256: Option<String>,
    pub after_sha256: String,
    pub before_bytes: usize,
    pub after_bytes: usize,
    /// Runtime-specific syntax validation/reload must be performed by adapters.
    pub reload_required: bool,
}
impl ChangeSet {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn preview(&self) -> Vec<Preview> {
        self.files
            .iter()
            .map(|file| Preview {
                relative_path: file.path.clone(),
                before_sha256: file.baseline.as_deref().map(sha256),
                after_sha256: sha256(&file.replacement),
                before_bytes: file.baseline.as_ref().map_or(0, Vec::len),
                after_bytes: file.replacement.len(),
                reload_required: true,
            })
            .collect()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Applying,
    Applied,
    PartialFailure,
    RollingBack,
    RolledBack,
    Conflict,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileState {
    Pending,
    Written,
    NotApplied,
    RolledBack,
    Conflict,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileReport {
    pub relative_path: PathBuf,
    pub before_sha256: Option<String>,
    pub after_sha256: String,
    pub state: FileState,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub root_device: u64,
    pub root_inode: u64,
    pub schema_version: u32,
    pub transaction_id: String,
    pub phase: Phase,
    pub files: Vec<FileReport>,
}

/// An approved root pinned by a directory descriptor. Opening it does not confer
/// permission: only a trusted daemon-side ownership registry should call `open`.
pub struct ManagedRoot {
    root_identity: (u64, u64),
    root: File,
    journals: File,
    lock: File,
    serial: Mutex<()>,
}
impl ManagedRoot {
    pub fn open(path: &Path, ownership: Ownership) -> Result<Self> {
        if ownership != Ownership::BatonManaged {
            return Err(error(ErrorCode::PermissionDenied));
        }
        // Require an explicit canonical path. This also rejects caller '..' and
        // symlink aliases; system /var aliases can be canonicalized by the host.
        if !path.is_absolute() || io_error(path.canonicalize())? != path {
            return Err(error(ErrorCode::InvalidPath));
        }
        let root = open_dir_at(rfs::CWD, path)?;
        let lock = open_private(&root, Path::new(".baton-config.lock"), true)?;
        match rfs::mkdirat(&root, ".baton-transactions", Mode::from_raw_mode(0o700)) {
            Ok(()) => io_error(root.sync_all())?,
            Err(rustix::io::Errno::EXIST) => {}
            Err(_) => return Err(error(ErrorCode::Io)),
        }
        let journals = open_dir_at(&root, Path::new(".baton-transactions"))?;
        let metadata = io_error(root.metadata())?;
        Ok(Self {
            root_identity: (metadata.dev(), metadata.ino()),
            root,
            journals,
            lock,
            serial: Mutex::new(()),
        })
    }
    pub fn plan(&self, edits: Vec<Edit>) -> Result<ChangeSet> {
        if edits.is_empty() || edits.len() > MAX_FILES {
            return Err(error(ErrorCode::ResourceLimit));
        }
        let mut paths = HashSet::new();
        let mut files = Vec::new();
        for edit in edits {
            validate_relative(&edit.relative_path)?;
            if !paths.insert(edit.relative_path.clone()) {
                return Err(error(ErrorCode::InvalidPath));
            }
            if edit.replacement.len() > MAX_BYTES {
                return Err(error(ErrorCode::ResourceLimit));
            }
            let baseline = self.read(&edit.relative_path)?;
            if baseline.as_deref().map(sha256) != edit.expected_sha256 {
                return Err(error(ErrorCode::Conflict));
            }
            files.push(PlannedFile {
                path: edit.relative_path,
                baseline,
                replacement: edit.replacement,
            });
        }
        Ok(ChangeSet {
            root_identity: self.root_identity,
            id: Uuid::new_v4().to_string(),
            files,
        })
    }
    pub fn apply(&self, change: ChangeSet) -> Result<Report> {
        self.apply_observed(change, |_| {})
    }
    fn apply_observed(
        &self,
        change: ChangeSet,
        mut after_write: impl FnMut(usize),
    ) -> Result<Report> {
        let _serial = self.serial.lock().map_err(|_| error(ErrorCode::Conflict))?;
        let _guard = LockGuard::acquire(&self.lock)?;
        if change.root_identity != self.root_identity {
            return Err(error(ErrorCode::Conflict));
        }
        // Check every baseline before persisting intent or making the first write.
        for file in &change.files {
            if self.read(&file.path)? != file.baseline {
                return Err(error(ErrorCode::Conflict));
            }
        }
        let transaction = self.create_transaction(&change.id)?;
        for (index, file) in change.files.iter().enumerate() {
            if let Some(bytes) = &file.baseline {
                write_new(&transaction, &format!("backup-{index}.bin"), bytes)?;
            }
        }
        let mut report = Report {
            root_device: self.root_identity.0,
            root_inode: self.root_identity.1,
            schema_version: 1,
            transaction_id: change.id,
            phase: Phase::Applying,
            files: change
                .files
                .iter()
                .map(|file| FileReport {
                    relative_path: file.path.clone(),
                    before_sha256: file.baseline.as_deref().map(sha256),
                    after_sha256: sha256(&file.replacement),
                    state: FileState::Pending,
                })
                .collect(),
        };
        save_report(&transaction, &report)?;
        for (index, file) in change.files.iter().enumerate() {
            let result = self.replace(
                &file.path,
                file.baseline.as_deref().map(sha256).as_deref(),
                &file.replacement,
            );
            if result.is_err() {
                report.files[index].state = FileState::Conflict;
                report.phase = Phase::PartialFailure;
                save_report(&transaction, &report)
                    .map_err(|_| outcome_unknown(&report.transaction_id))?;
                return Ok(report);
            }
            report.files[index].state = FileState::Written;
            save_report(&transaction, &report)
                .map_err(|_| outcome_unknown(&report.transaction_id))?;
            after_write(index);
        }
        report.phase = Phase::Applied;
        save_report(&transaction, &report).map_err(|_| outcome_unknown(&report.transaction_id))?;
        Ok(report)
    }
    /// Reconcile persisted intent against exact disk hashes. Never blindly replay
    /// an uncertain write after a crash, and never infer external reload success.
    pub fn recover(&self, transaction_id: &str) -> Result<Report> {
        let _serial = self.serial.lock().map_err(|_| error(ErrorCode::Conflict))?;
        let _guard = LockGuard::acquire(&self.lock)?;
        let transaction = self.transaction(transaction_id)?;
        let mut report = load_report(&transaction, transaction_id)?;
        if (report.root_device, report.root_inode) != self.root_identity {
            return Err(error(ErrorCode::Conflict));
        }
        self.reconcile(&mut report)?;
        save_report(&transaction, &report)?;
        Ok(report)
    }
    pub fn rollback(&self, transaction_id: &str) -> Result<Report> {
        let _serial = self.serial.lock().map_err(|_| error(ErrorCode::Conflict))?;
        let _guard = LockGuard::acquire(&self.lock)?;
        let transaction = self.transaction(transaction_id)?;
        let mut report = load_report(&transaction, transaction_id)?;
        if (report.root_device, report.root_inode) != self.root_identity {
            return Err(error(ErrorCode::Conflict));
        }
        self.reconcile(&mut report)?;
        report.phase = Phase::RollingBack;
        save_report(&transaction, &report)?;
        for index in (0..report.files.len()).rev() {
            let file = &report.files[index];
            if matches!(file.state, FileState::NotApplied | FileState::RolledBack) {
                continue;
            }
            if file.state == FileState::Conflict {
                report.phase = Phase::Conflict;
                save_report(&transaction, &report)?;
                return Ok(report);
            }
            let baseline = if let Some(expected) = &file.before_sha256 {
                let bytes = read_at(&transaction, Path::new(&format!("backup-{index}.bin")))?
                    .ok_or_else(|| error(ErrorCode::Io))?;
                if sha256(&bytes) != *expected {
                    return Err(error(ErrorCode::Conflict));
                }
                Some(bytes)
            } else {
                None
            };
            let result = match baseline {
                Some(bytes) => self.replace(&file.relative_path, Some(&file.after_sha256), &bytes),
                None => self.remove_if_hash(&file.relative_path, &file.after_sha256),
            };
            if result.is_err() {
                report.files[index].state = FileState::Conflict;
                report.phase = Phase::Conflict;
                save_report(&transaction, &report).map_err(|_| outcome_unknown(transaction_id))?;
                return Ok(report);
            }
            report.files[index].state = FileState::RolledBack;
            save_report(&transaction, &report).map_err(|_| outcome_unknown(transaction_id))?;
        }
        report.phase = Phase::RolledBack;
        save_report(&transaction, &report).map_err(|_| outcome_unknown(transaction_id))?;
        Ok(report)
    }
    fn reconcile(&self, report: &mut Report) -> Result<()> {
        let rolling_back = matches!(report.phase, Phase::RollingBack | Phase::RolledBack);
        for file in &mut report.files {
            let current = self.read(&file.relative_path)?.as_deref().map(sha256);
            file.state = if current.as_deref() == Some(&file.after_sha256) {
                FileState::Written
            } else if current == file.before_sha256 {
                if rolling_back {
                    FileState::RolledBack
                } else {
                    FileState::NotApplied
                }
            } else {
                FileState::Conflict
            };
        }
        if report
            .files
            .iter()
            .any(|file| file.state == FileState::Conflict)
        {
            report.phase = Phase::Conflict;
        } else if report
            .files
            .iter()
            .all(|file| file.state == FileState::Written)
        {
            report.phase = Phase::Applied;
        } else if report
            .files
            .iter()
            .all(|file| matches!(file.state, FileState::RolledBack | FileState::NotApplied))
        {
            report.phase = if rolling_back {
                Phase::RolledBack
            } else {
                Phase::PartialFailure
            };
        } else {
            report.phase = if rolling_back {
                Phase::RollingBack
            } else {
                Phase::PartialFailure
            };
        }
        Ok(())
    }
    fn read(&self, relative: &Path) -> Result<Option<Vec<u8>>> {
        let (parent, name) = self.parent(relative)?;
        read_at(&parent, &name)
    }
    fn parent(&self, relative: &Path) -> Result<(File, PathBuf)> {
        validate_relative(relative)?;
        let parts: Vec<_> = relative.components().collect();
        let mut directory = io_error(self.root.try_clone())?;
        for part in &parts[..parts.len() - 1] {
            directory = open_dir_at(&directory, Path::new(part.as_os_str()))?;
        }
        Ok((directory, PathBuf::from(parts[parts.len() - 1].as_os_str())))
    }
    fn replace(
        &self,
        relative: &Path,
        expected_hash: Option<&str>,
        replacement: &[u8],
    ) -> Result<()> {
        let (parent, name) = self.parent(relative)?;
        let temporary = format!(".baton-write-{}", Uuid::new_v4());
        write_new(&parent, &temporary, replacement)?;
        let result = (|| {
            if read_at(&parent, &name)?.as_deref().map(sha256).as_deref() != expected_hash {
                return Err(error(ErrorCode::Conflict));
            }
            io_error(rfs::renameat(&parent, temporary.as_str(), &parent, &name))?;
            io_error(parent.sync_all())
        })();
        if result.is_err() {
            let _ = rfs::unlinkat(&parent, temporary.as_str(), AtFlags::empty());
        }
        result
    }
    fn remove_if_hash(&self, relative: &Path, expected_hash: &str) -> Result<()> {
        let (parent, name) = self.parent(relative)?;
        if read_at(&parent, &name)?.as_deref().map(sha256).as_deref() != Some(expected_hash) {
            return Err(error(ErrorCode::Conflict));
        }
        io_error(rfs::unlinkat(&parent, &name, AtFlags::empty()))?;
        io_error(parent.sync_all())
    }
    fn create_transaction(&self, id: &str) -> Result<File> {
        io_error(rfs::mkdirat(&self.journals, id, Mode::from_raw_mode(0o700)))?;
        io_error(self.journals.sync_all())?;
        open_dir_at(&self.journals, Path::new(id))
    }
    fn transaction(&self, id: &str) -> Result<File> {
        if Uuid::parse_str(id).is_err() || id.contains('/') {
            return Err(error(ErrorCode::InvalidPath));
        }
        open_dir_at(&self.journals, Path::new(id))
    }
}
fn outcome_unknown(id: &str) -> ConfigError {
    ConfigError {
        code: ErrorCode::OutcomeUnknown,
        transaction_id: Some(id.into()),
    }
}
struct LockGuard<'a>(&'a File);
impl<'a> LockGuard<'a> {
    fn acquire(file: &'a File) -> Result<Self> {
        FileExt::try_lock_exclusive(file).map_err(|_| error(ErrorCode::Conflict))?;
        Ok(Self(file))
    }
}
impl Drop for LockGuard<'_> {
    fn drop(&mut self) {
        let _ = FileExt::unlock(self.0);
    }
}
fn validate_relative(path: &Path) -> Result<()> {
    let parts: Vec<_> = path.components().collect();
    if parts.is_empty() || parts.len() > 32 || path.as_os_str().len() > 4096
        || parts.iter().any(|part| !matches!(part, Component::Normal(name) if !name.to_string_lossy().starts_with(RESERVED))) {
        return Err(error(ErrorCode::InvalidPath));
    }
    Ok(())
}
fn open_dir_at(dir: impl std::os::fd::AsFd, path: &Path) -> Result<File> {
    let fd = rfs::openat(
        dir,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| error(ErrorCode::PermissionDenied))?;
    let stat = io_error(rfs::fstat(&fd))?;
    if stat.st_uid != rustix::process::geteuid().as_raw() || stat.st_mode & 0o077 != 0 {
        return Err(error(ErrorCode::PermissionDenied));
    }
    Ok(File::from(fd))
}
fn open_private(dir: &File, path: &Path, create: bool) -> Result<File> {
    let mut flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
    if create {
        flags |= OFlags::CREATE;
    }
    let fd = rfs::openat(dir, path, flags, Mode::from_raw_mode(0o600))
        .map_err(|_| error(ErrorCode::PermissionDenied))?;
    let stat = io_error(rfs::fstat(&fd))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_mode & 0o077 != 0
        || stat.st_nlink != 1
    {
        return Err(error(ErrorCode::PermissionDenied));
    }
    Ok(File::from(fd))
}
fn read_at(dir: &File, path: &Path) -> Result<Option<Vec<u8>>> {
    match rfs::statat(dir, path, AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(_) => return Err(error(ErrorCode::Io)),
        Ok(stat) if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile => {
            return Err(error(ErrorCode::PermissionDenied))
        }
        Ok(_) => {}
    }
    let file = open_private(dir, path, false)?;
    let mut bytes = Vec::new();
    io_error(file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes))?;
    if bytes.len() > MAX_BYTES {
        return Err(error(ErrorCode::ResourceLimit));
    }
    Ok(Some(bytes))
}
fn write_new(dir: &File, path: &str, bytes: &[u8]) -> Result<()> {
    let fd = io_error(rfs::openat(
        dir,
        path,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    ))?;
    let mut file = File::from(fd);
    io_error(file.write_all(bytes))?;
    io_error(file.sync_all())?;
    io_error(dir.sync_all())
}
fn save_report(dir: &File, report: &Report) -> Result<()> {
    let raw = io_error(serde_json::to_vec(report))?;
    let temporary = format!("manifest-{}.tmp", Uuid::new_v4());
    write_new(dir, &temporary, &raw)?;
    io_error(rfs::renameat(dir, temporary.as_str(), dir, "manifest.json"))?;
    io_error(dir.sync_all())
}
fn load_report(dir: &File, expected_id: &str) -> Result<Report> {
    let bytes = read_at(dir, Path::new("manifest.json"))?.ok_or_else(|| error(ErrorCode::Io))?;
    let report: Report = io_error(serde_json::from_slice(&bytes))?;
    if report.schema_version != 1
        || report.transaction_id != expected_id
        || report.files.is_empty()
        || report.files.len() > MAX_FILES
    {
        return Err(error(ErrorCode::Conflict));
    }
    let mut seen = HashSet::new();
    for file in &report.files {
        validate_relative(&file.relative_path)?;
        if !seen.insert(&file.relative_path)
            || !valid_hash(&file.after_sha256)
            || file
                .before_sha256
                .as_ref()
                .is_some_and(|hash| !valid_hash(hash))
        {
            return Err(error(ErrorCode::Conflict));
        }
    }
    Ok(report)
}
fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod failure_tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};
    #[test]
    fn partial_failure_journal_survives_and_preserves_external_edit() {
        let temp = tempfile::tempdir().unwrap();
        let root_path = temp.path().canonicalize().unwrap();
        fs::set_permissions(&root_path, fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["a", "b"] {
            fs::write(root_path.join(name), b"old").unwrap();
            fs::set_permissions(root_path.join(name), fs::Permissions::from_mode(0o600)).unwrap();
        }
        let root = ManagedRoot::open(&root_path, Ownership::BatonManaged).unwrap();
        let change = root
            .plan(
                ["a", "b"]
                    .iter()
                    .map(|name| {
                        Edit::nonsecret_replacement(*name, Some(sha256(b"old")), b"new".to_vec())
                    })
                    .collect(),
            )
            .unwrap();
        let report = root
            .apply_observed(change, |index| {
                if index == 0 {
                    fs::write(root_path.join("b"), b"external").unwrap();
                }
            })
            .unwrap();
        assert_eq!(report.phase, Phase::PartialFailure);
        assert_eq!(report.files[0].state, FileState::Written);
        assert_eq!(report.files[1].state, FileState::Conflict);
        drop(root);
        let root = ManagedRoot::open(&root_path, Ownership::BatonManaged).unwrap();
        let recovered = root.recover(&report.transaction_id).unwrap();
        assert_eq!(recovered.phase, Phase::Conflict);
        assert_eq!(fs::read(root_path.join("b")).unwrap(), b"external");
        assert_eq!(fs::read(root_path.join("a")).unwrap(), b"new");
    }
}
