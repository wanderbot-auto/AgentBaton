//! Explicit owner-authorized data copies. This is not recursive backup, an OS
//! sandbox, an atomic directory snapshot, or authority derived from Agent JSON.
use crate::{
    open_root_without_symlinks,
    validation::{sha256, valid_id, ArtifactRequirement},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{CString, OsStr},
    fs::{self, File, Metadata},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path, PathBuf},
};
// st_mode kind bits are represented as u32 by MetadataExt on Linux and Darwin.
const FORMAT: u32 = 1;
const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_TOTAL: u64 = 256 * 1024 * 1024;
const MAX_FILES: u32 = 64;
const MAX_METADATA: u64 = 4 * 1024 * 1024;
const CREATING: &str = ".baton-data-creating.json";
const READY: &str = ".baton-data-ready.json";
const FAILED: &str = ".baton-data-failed.json";

// Private deterministic I/O fault injection; absent from production builds.
#[cfg(test)]
type PublicationHook = Box<dyn Fn(&Path, bool)>;
#[cfg(test)]
thread_local! {
    static PUBLICATION_HOOK: std::cell::RefCell<Option<PublicationHook>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
fn publication_observed(path: &Path, ready_written: bool) {
    PUBLICATION_HOOK.with(|hook| {
        if let Some(hook) = &*hook.borrow() {
            hook(path, ready_written);
        }
    });
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataLimits {
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
    pub max_files: u32,
}
impl Default for DataLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: MAX_FILE,
            max_total_bytes: MAX_TOTAL,
            max_files: MAX_FILES,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryPin {
    pub canonical_root: PathBuf,
    pub device: u64,
    pub inode: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilePin {
    pub device: u64,
    pub inode: u64,
    pub bytes: u64,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub mtime: i64,
    pub mtime_nsec: i64,
    pub ctime: i64,
    pub ctime_nsec: i64,
}
impl FilePin {
    fn from(m: &Metadata) -> Self {
        Self {
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
        }
    }
}
/// A serializable declaration/view, never path authorization by itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataSelectionContract {
    pub format_version: u32,
    pub approval_reference: String,
    pub source: DirectoryPin,
    pub files: Vec<ArtifactRequirement>,
    pub limits: DataLimits,
}
/// Construct only after the host authenticates project/path ownership and the
/// actual file selection. Strings named approval do not authenticate a caller.
/// ```compile_fail
/// use agentbaton_workspaces::data_workspace::ApprovedDataSelection;
/// let grant:ApprovedDataSelection=serde_json::from_str("{}").unwrap();
/// ```
pub struct ApprovedDataSelection {
    contract: DataSelectionContract,
    root: File,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFileObservation {
    pub requirement: ArtifactRequirement,
    pub source_identity: FilePin,
    pub sha256: String,
    pub bytes: u64,
    pub destination_relative_path: PathBuf,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetSnapshotView {
    pub contract: DataSelectionContract,
    pub selection_digest: String,
    pub dataset_revision: String,
    pub files: Vec<SourceFileObservation>,
    pub total_bytes: u64,
    pub source_observation_scope: String,
    pub atomic_directory_freeze: bool,
}
/// Private verified bytes/identity observations, not a deserialized Agent claim.
pub struct VerifiedDatasetSnapshot {
    view: DatasetSnapshotView,
    root: File,
}
impl VerifiedDatasetSnapshot {
    pub fn view(&self) -> DatasetSnapshotView {
        self.view.clone()
    }
    pub fn dataset_revision(&self) -> &str {
        &self.view.dataset_revision
    }
}
/// The destination is selected by the authenticated host, never a renderer path.
pub struct ApprovedManagedRoot {
    pin: DirectoryPin,
    root: File,
}
impl ApprovedManagedRoot {
    pub fn from_verified_host(path: &Path) -> Result<Self, DataWorkspaceError> {
        let root = checked_root(path)?;
        check_private_dir(&root)?;
        let pin = directory_pin(path, &root)?;
        Ok(Self { pin, root })
    }
    pub fn view(&self) -> DirectoryPin {
        self.pin.clone()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataWorkspaceError {
    InvalidSelection,
    UnauthorizedPath,
    UnsafeFile,
    SourceChanged,
    SourceUnavailable,
    ResourceLimit,
    ManagedRootInvalid,
    DestinationConflict,
    DestinationChanged,
    Io,
    MetadataInvalid,
}
impl std::fmt::Display for DataWorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "data workspace rejected: {self:?}")
    }
}
impl std::error::Error for DataWorkspaceError {}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationState {
    Creating,
    Ready,
    Failed,
    OutcomeUnknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopiedDataFile {
    pub input: SourceFileObservation,
    pub destination_identity: FilePin,
}
/// Host-persisted view. Reconstructing a trusted restore token requires current
/// host approval; a disk Ready record or this JSON alone is never authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataWorkspaceContract {
    pub format_version: u32,
    pub creation_id: String,
    pub snapshot: DatasetSnapshotView,
    pub managed_root: DirectoryPin,
    pub destination: DirectoryPin,
    pub files: Vec<CopiedDataFile>,
    pub directories: Vec<DirectoryPin>,
    pub contract_digest: String,
}
/// Only authenticated host persistence can authorize restoration.
/// ```compile_fail
/// use agentbaton_workspaces::data_workspace::ApprovedRestoreContract;
/// let restored: ApprovedRestoreContract = serde_json::from_str("{}").unwrap();
/// ```
pub struct ApprovedRestoreContract {
    view: DataWorkspaceContract,
}
impl ApprovedRestoreContract {
    pub fn from_verified_host(view: DataWorkspaceContract) -> Result<Self, DataWorkspaceError> {
        validate_completed_contract(&view)?;
        Ok(Self { view })
    }
    pub fn view(&self) -> DataWorkspaceContract {
        self.view.clone()
    }
}
/// Only real complete verification creates this token; no Deserialize.
/// ```compile_fail
/// use agentbaton_workspaces::data_workspace::VerifiedDataWorkspace;
/// let ready: VerifiedDataWorkspace = serde_json::from_str("{}").unwrap();
/// ```
pub struct VerifiedDataWorkspace {
    contract: DataWorkspaceContract,
    directory: File,
}
impl VerifiedDataWorkspace {
    pub fn contract_view(&self) -> DataWorkspaceContract {
        self.contract.clone()
    }
    pub fn directory_identity(&self) -> Result<DirectoryPin, DataWorkspaceError> {
        check_directory(&self.directory, &self.contract.destination)?;
        Ok(self.contract.destination.clone())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataCreationView {
    pub creation_id: String,
    pub destination: PathBuf,
    pub state: CreationState,
    pub dataset_revision: String,
    pub selection_digest: String,
    /// Copies observed in this call; incomplete inspection returns zero without
    /// asserting that the retained directory contains no copied/owner files.
    pub copied_files: u32,
    pub error: Option<DataWorkspaceError>,
    pub source_observation_scope: String,
    pub atomic_directory_freeze: bool,
    pub filesystem_sandbox: bool,
}
pub struct DataWorkspaceCreation {
    view: DataCreationView,
    ready: Option<VerifiedDataWorkspace>,
}
impl DataWorkspaceCreation {
    pub fn view(&self) -> DataCreationView {
        self.view.clone()
    }
    pub fn verified(&self) -> Option<&VerifiedDataWorkspace> {
        self.ready.as_ref()
    }
    pub fn into_verified(self) -> Option<VerifiedDataWorkspace> {
        self.ready
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    format_version: u32,
    creation_id: String,
    snapshot: DatasetSnapshotView,
    destination: DirectoryPin,
    managed_root: DirectoryPin,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    creation_id: String,
    selection_digest: String,
    dataset_revision: String,
    error: DataWorkspaceError,
}
fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, DataWorkspaceError> {
    let bytes = serde_json::to_vec(value).map_err(|_| DataWorkspaceError::MetadataInvalid)?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err(DataWorkspaceError::ResourceLimit);
    }
    Ok(bytes)
}
fn digest<T: Serialize>(value: &T) -> Result<String, DataWorkspaceError> {
    Ok(sha256(&json_bytes(value)?))
}
fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn auth_component(name: &OsStr) -> bool {
    let text = name.to_string_lossy().to_ascii_lowercase();
    matches!(
        text.as_str(),
        ".git"
            | ".ssh"
            | ".aws"
            | ".azure"
            | ".kube"
            | ".gnupg"
            | ".codex"
            | ".claude"
            | ".qoder"
            | ".config"
            | ".claude.json"
            | ".netrc"
            | "auth.json"
            | "credentials.json"
            | ".credentials.json"
    )
}
fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().collect::<PathBuf>().as_os_str() == path.as_os_str()
        && path
            .to_str()
            .is_some_and(|v| v.len() <= 4096 && !v.chars().any(char::is_control))
        && path.components().count() <= 16
        && path
            .components()
            .all(|p| matches!(p,Component::Normal(name) if !auth_component(name)))
}
fn canonical_shape(path: &Path) -> bool {
    path.is_absolute()
        && path
            .to_str()
            .is_some_and(|p| p.len() <= 4096 && !p.chars().any(char::is_control))
        && path
            .components()
            .all(|p| matches!(p, Component::RootDir | Component::Normal(_)))
        && path.components().collect::<PathBuf>().as_os_str() == path.as_os_str()
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate_selection(c: &DataSelectionContract) -> Result<(), DataWorkspaceError> {
    if !canonical_shape(&c.source.canonical_root)
        || c.source.canonical_root == Path::new("/")
        || c.format_version != FORMAT
        || !valid_id(&c.approval_reference)
        || c.files.is_empty()
        || c.files.len() > c.limits.max_files as usize
        || c.limits.max_files == 0
        || c.limits.max_files > MAX_FILES
        || c.limits.max_file_bytes == 0
        || c.limits.max_file_bytes > MAX_FILE
        || c.limits.max_total_bytes == 0
        || c.limits.max_total_bytes > MAX_TOTAL
    {
        return Err(DataWorkspaceError::InvalidSelection);
    }
    let mut ids = BTreeSet::new();
    let mut paths = vec![];
    let mut component_names = BTreeMap::new();
    for r in &c.files {
        if !valid_id(&r.artifact_id)
            || !ids.insert(r.artifact_id.clone())
            || !safe_relative(&r.relative_path)
            || r.max_bytes == 0
            || r.max_bytes > c.limits.max_file_bytes
            || r.expected_sha256.as_ref().is_some_and(|s| !valid_hash(s))
        {
            return Err(DataWorkspaceError::InvalidSelection);
        }
        let mut prefix = PathBuf::new();
        for part in r.relative_path.components() {
            prefix.push(part);
            let lower = prefix.to_str().unwrap().to_lowercase();
            if component_names
                .insert(lower, prefix.clone())
                .is_some_and(|old| old != prefix)
            {
                return Err(DataWorkspaceError::InvalidSelection);
            }
        }
        paths.push(r.relative_path.to_str().unwrap().to_lowercase());
    }
    // Compare every pair: sorting adjacent entries misses a, a-b, a/c.
    if paths.iter().enumerate().any(|(i, a)| {
        paths.iter().skip(i + 1).any(|b| {
            a == b || a.starts_with(&(b.clone() + "/")) || b.starts_with(&(a.clone() + "/"))
        })
    }) {
        return Err(DataWorkspaceError::InvalidSelection);
    }
    Ok(())
}
fn checked_root(path: &Path) -> Result<File, DataWorkspaceError> {
    if path.to_str().is_none()
        || !path.is_absolute()
        || fs::canonicalize(path)
            .map_err(|_| DataWorkspaceError::SourceUnavailable)?
            .as_os_str()
            != path.as_os_str()
    {
        return Err(DataWorkspaceError::ManagedRootInvalid);
    }
    open_root_without_symlinks(path).map_err(|_| DataWorkspaceError::ManagedRootInvalid)
}
fn directory_pin(path: &Path, file: &File) -> Result<DirectoryPin, DataWorkspaceError> {
    let m = file.metadata().map_err(|_| DataWorkspaceError::Io)?;
    if !m.is_dir() {
        return Err(DataWorkspaceError::ManagedRootInvalid);
    }
    Ok(DirectoryPin {
        canonical_root: path.into(),
        device: m.dev(),
        inode: m.ino(),
    })
}
fn check_directory(file: &File, pin: &DirectoryPin) -> Result<(), DataWorkspaceError> {
    let m = file.metadata().map_err(|_| DataWorkspaceError::Io)?;
    let current = checked_root(&pin.canonical_root)?;
    let now = current.metadata().map_err(|_| DataWorkspaceError::Io)?;
    if !m.is_dir()
        || (m.dev(), m.ino()) != (pin.device, pin.inode)
        || (now.dev(), now.ino()) != (pin.device, pin.inode)
    {
        return Err(DataWorkspaceError::DestinationChanged);
    }
    Ok(())
}
fn check_private_dir(file: &File) -> Result<(), DataWorkspaceError> {
    let m = file.metadata().map_err(|_| DataWorkspaceError::Io)?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o7777 != 0o700 {
        return Err(DataWorkspaceError::ManagedRootInvalid);
    }
    Ok(())
}
fn name(value: &OsStr) -> Result<CString, DataWorkspaceError> {
    CString::new(value.as_bytes()).map_err(|_| DataWorkspaceError::InvalidSelection)
}
fn open_child(parent: &File, value: &OsStr, directory: bool) -> Result<File, DataWorkspaceError> {
    let value = name(value)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            value.as_ptr(),
            libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK
                | if directory { libc::O_DIRECTORY } else { 0 },
        )
    };
    if fd < 0 {
        return Err(DataWorkspaceError::SourceUnavailable);
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn open_relative(root: &File, path: &Path) -> Result<File, DataWorkspaceError> {
    if !safe_relative(path) {
        return Err(DataWorkspaceError::UnauthorizedPath);
    }
    let mut directory = root.try_clone().map_err(|_| DataWorkspaceError::Io)?;
    let parts: Vec<_> = path.components().collect();
    for (i, part) in parts.iter().enumerate() {
        let Component::Normal(component) = part else {
            return Err(DataWorkspaceError::InvalidSelection);
        };
        directory = open_child(&directory, component, i + 1 < parts.len())?;
    }
    Ok(directory)
}
fn read_file(mut file: File, max: u64) -> Result<(Vec<u8>, FilePin), DataWorkspaceError> {
    let before = file.metadata().map_err(|_| DataWorkspaceError::Io)?;
    if !before.is_file() || before.nlink() != 1 {
        return Err(DataWorkspaceError::UnsafeFile);
    }
    if before.len() > max {
        return Err(DataWorkspaceError::ResourceLimit);
    }
    let stamp = FilePin::from(&before);
    let mut bytes = vec![];
    Read::by_ref(&mut file)
        .take(
            max.checked_add(1)
                .ok_or(DataWorkspaceError::ResourceLimit)?,
        )
        .read_to_end(&mut bytes)
        .map_err(|_| DataWorkspaceError::Io)?;
    let after = file.metadata().map_err(|_| DataWorkspaceError::Io)?;
    if bytes.len() as u64 > max {
        return Err(DataWorkspaceError::ResourceLimit);
    }
    if after.nlink() != 1 || FilePin::from(&after) != stamp {
        return Err(DataWorkspaceError::SourceChanged);
    }
    Ok((bytes, stamp))
}
impl ApprovedDataSelection {
    pub fn from_verified_host(
        root: &Path,
        approval_reference: String,
        files: Vec<ArtifactRequirement>,
        limits: DataLimits,
    ) -> Result<Self, DataWorkspaceError> {
        // Validate bounds and names before sorting, root I/O or metadata cloning.
        if !canonical_shape(root) {
            return Err(DataWorkspaceError::InvalidSelection);
        }
        let mut contract = DataSelectionContract {
            format_version: FORMAT,
            approval_reference,
            source: DirectoryPin {
                canonical_root: root.into(),
                device: 0,
                inode: 0,
            },
            files,
            limits,
        };
        validate_selection(&contract)?;
        if root
            .components()
            .any(|p| matches!(p, Component::Normal(name) if auth_component(name)))
            || std::env::var_os("HOME")
                .and_then(|p| fs::canonicalize(p).ok())
                .is_some_and(|p| p == root)
        {
            return Err(DataWorkspaceError::UnauthorizedPath);
        }
        contract
            .files
            .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        let fd = checked_root(root)?;
        contract.source = directory_pin(root, &fd)?;
        Ok(Self { contract, root: fd })
    }
    pub fn contract_view(&self) -> DataSelectionContract {
        self.contract.clone()
    }
}
fn dataset_revision(files: &[SourceFileObservation]) -> Result<String, DataWorkspaceError> {
    let content: Vec<_> = files
        .iter()
        .map(|f| {
            (
                &f.requirement.artifact_id,
                &f.requirement.relative_path,
                &f.destination_relative_path,
                f.bytes,
                &f.sha256,
            )
        })
        .collect();
    digest(&(FORMAT, "explicit_selected_file_bytes", content))
}
fn observe(
    root: &File,
    requirement: &ArtifactRequirement,
    max: u64,
) -> Result<(Vec<u8>, FilePin, String), DataWorkspaceError> {
    let (bytes, pin) = read_file(
        open_relative(root, &requirement.relative_path)?,
        max.min(requirement.max_bytes),
    )?;
    let hash = sha256(&bytes);
    if requirement
        .expected_sha256
        .as_ref()
        .is_some_and(|s| s != &hash)
    {
        return Err(DataWorkspaceError::SourceChanged);
    }
    let current = open_relative(root, &requirement.relative_path)?
        .metadata()
        .map_err(|_| DataWorkspaceError::Io)?;
    if current.nlink() != 1 || FilePin::from(&current) != pin {
        return Err(DataWorkspaceError::SourceChanged);
    }
    Ok((bytes, pin, hash))
}
pub fn verify_dataset(
    selection: &ApprovedDataSelection,
) -> Result<VerifiedDatasetSnapshot, DataWorkspaceError> {
    check_directory(&selection.root, &selection.contract.source)
        .map_err(|_| DataWorkspaceError::SourceChanged)?;
    let mut files = vec![];
    let mut total = 0u64;
    let mut identities = BTreeSet::new();
    for r in &selection.contract.files {
        let remaining = selection
            .contract
            .limits
            .max_total_bytes
            .checked_sub(total)
            .ok_or(DataWorkspaceError::ResourceLimit)?;
        let (bytes, pin, hash) = observe(&selection.root, r, remaining)?;
        if !identities.insert((pin.device, pin.inode)) {
            return Err(DataWorkspaceError::InvalidSelection);
        }
        total = total
            .checked_add(bytes.len() as u64)
            .ok_or(DataWorkspaceError::ResourceLimit)?;
        files.push(SourceFileObservation {
            requirement: r.clone(),
            source_identity: pin,
            sha256: hash,
            bytes: bytes.len() as u64,
            destination_relative_path: Path::new("inputs").join(&r.relative_path),
        });
    }
    for item in &files {
        let (_, pin, hash) = observe(
            &selection.root,
            &item.requirement,
            item.requirement.max_bytes,
        )?;
        if pin != item.source_identity || hash != item.sha256 {
            return Err(DataWorkspaceError::SourceChanged);
        }
    }
    check_directory(&selection.root, &selection.contract.source)
        .map_err(|_| DataWorkspaceError::SourceChanged)?;
    let view = DatasetSnapshotView {
        contract: selection.contract.clone(),
        selection_digest: digest(&selection.contract)?,
        dataset_revision: dataset_revision(&files)?,
        files,
        total_bytes: total,
        source_observation_scope: "selected_files_before_after_only".into(),
        atomic_directory_freeze: false,
    };
    Ok(VerifiedDatasetSnapshot {
        view,
        root: selection
            .root
            .try_clone()
            .map_err(|_| DataWorkspaceError::Io)?,
    })
}

fn validate_snapshot(v: &DatasetSnapshotView) -> Result<(), DataWorkspaceError> {
    validate_selection(&v.contract)?;
    if v.files.len() != v.contract.files.len()
        || !valid_hash(&v.selection_digest)
        || !valid_hash(&v.dataset_revision)
        || v.selection_digest != digest(&v.contract)?
        || v.atomic_directory_freeze
        || v.source_observation_scope != "selected_files_before_after_only"
    {
        return Err(DataWorkspaceError::MetadataInvalid);
    }
    let mut total = 0u64;
    let mut pins = BTreeSet::new();
    for (r, f) in v.contract.files.iter().zip(&v.files) {
        if r != &f.requirement
            || f.destination_relative_path != Path::new("inputs").join(&r.relative_path)
            || f.bytes != f.source_identity.bytes
            || f.bytes > r.max_bytes
            || f.source_identity.mode & 0o170000 != 0o100000
            || !valid_hash(&f.sha256)
            || r.expected_sha256.as_ref().is_some_and(|h| h != &f.sha256)
            || !pins.insert((f.source_identity.device, f.source_identity.inode))
        {
            return Err(DataWorkspaceError::MetadataInvalid);
        }
        total = total
            .checked_add(f.bytes)
            .ok_or(DataWorkspaceError::ResourceLimit)?;
    }
    if total != v.total_bytes
        || total > v.contract.limits.max_total_bytes
        || v.dataset_revision != dataset_revision(&v.files)?
    {
        return Err(DataWorkspaceError::MetadataInvalid);
    }
    Ok(())
}
fn completed_digest(c: &DataWorkspaceContract) -> Result<String, DataWorkspaceError> {
    let mut view = c.clone();
    view.contract_digest.clear();
    digest(&view)
}
fn validate_completed_contract(c: &DataWorkspaceContract) -> Result<(), DataWorkspaceError> {
    validate_snapshot(&c.snapshot)?;
    if c.format_version != FORMAT
        || !canonical_shape(&c.managed_root.canonical_root)
        || !canonical_shape(&c.destination.canonical_root)
        || c.managed_root
            .canonical_root
            .starts_with(&c.snapshot.contract.source.canonical_root)
        || (c.managed_root.device, c.managed_root.inode)
            == (
                c.snapshot.contract.source.device,
                c.snapshot.contract.source.inode,
            )
        || !safe_id(&c.creation_id)
        || c.destination.canonical_root
            != c.managed_root
                .canonical_root
                .join(format!("data-{}", c.creation_id))
        || c.files.len() != c.snapshot.files.len()
        || c.directories.len() > MAX_FILES as usize * 16
        || c.directories
            .iter()
            .any(|pin| !canonical_shape(&pin.canonical_root))
        || !valid_hash(&c.contract_digest)
    {
        return Err(DataWorkspaceError::MetadataInvalid);
    }
    for (f, expected) in c.files.iter().zip(&c.snapshot.files) {
        if &f.input != expected
            || f.destination_identity.bytes != expected.bytes
            || f.destination_identity.mode & 0o170000 != 0o100000
            || f.destination_identity.mode & 0o7777 != 0o600
        {
            return Err(DataWorkspaceError::MetadataInvalid);
        }
    }
    let mut expected = BTreeSet::new();
    for f in &c.files {
        let mut path = f.input.destination_relative_path.parent();
        while let Some(p) = path.filter(|p| !p.as_os_str().is_empty()) {
            expected.insert(c.destination.canonical_root.join(p));
            path = p.parent();
        }
    }
    let actual: BTreeSet<_> = c
        .directories
        .iter()
        .map(|d| d.canonical_root.clone())
        .collect();
    if actual != expected
        || actual.len() != c.directories.len()
        || c.contract_digest != completed_digest(c)?
    {
        return Err(DataWorkspaceError::MetadataInvalid);
    }
    Ok(())
}
fn ensure_outside_source(
    snapshot: &VerifiedDatasetSnapshot,
    managed: &ApprovedManagedRoot,
) -> Result<(), DataWorkspaceError> {
    let source = &snapshot.view.contract.source;
    if managed
        .pin
        .canonical_root
        .starts_with(&source.canonical_root)
        || (managed.pin.device, managed.pin.inode) == (source.device, source.inode)
    {
        return Err(DataWorkspaceError::ManagedRootInvalid);
    }
    let mut cursor = managed
        .root
        .try_clone()
        .map_err(|_| DataWorkspaceError::Io)?;
    for _ in 0..512 {
        let current = cursor.metadata().map_err(|_| DataWorkspaceError::Io)?;
        if (current.dev(), current.ino()) == (source.device, source.inode) {
            return Err(DataWorkspaceError::ManagedRootInvalid);
        }
        let parent = open_child(&cursor, OsStr::new(".."), true)?;
        let p = parent.metadata().map_err(|_| DataWorkspaceError::Io)?;
        if (p.dev(), p.ino()) == (current.dev(), current.ino()) {
            return Ok(());
        }
        cursor = parent;
    }
    Err(DataWorkspaceError::ManagedRootInvalid)
}
fn reserve_directory(parent: &File, child: &OsStr) -> Result<(), DataWorkspaceError> {
    let n = name(child)?;
    if unsafe { libc::mkdirat(parent.as_raw_fd(), n.as_ptr(), 0o700) } < 0 {
        return Err(
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EEXIST) {
                DataWorkspaceError::DestinationConflict
            } else {
                DataWorkspaceError::Io
            },
        );
    }
    Ok(())
}
fn create_directory(parent: &File, child: &OsStr) -> Result<File, DataWorkspaceError> {
    reserve_directory(parent, child)?;
    let file =
        open_child(parent, child, true).map_err(|_| DataWorkspaceError::DestinationChanged)?;
    check_private_dir(&file)?;
    Ok(file)
}
fn write_new(parent: &File, child: &OsStr, bytes: &[u8]) -> Result<FilePin, DataWorkspaceError> {
    let n = name(child)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            n.as_ptr(),
            libc::O_RDWR
                | libc::O_CREAT
                | libc::O_EXCL
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EEXIST) {
                DataWorkspaceError::DestinationConflict
            } else {
                DataWorkspaceError::Io
            },
        );
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let before = file.metadata().map_err(|_| DataWorkspaceError::Io)?;
    if !before.is_file() || before.nlink() != 1 || before.uid() != unsafe { libc::geteuid() } {
        return Err(DataWorkspaceError::UnsafeFile);
    }
    if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } < 0 {
        return Err(DataWorkspaceError::Io);
    }
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| DataWorkspaceError::Io)?;
    let after = file.metadata().map_err(|_| DataWorkspaceError::Io)?;
    if after.nlink() != 1 || after.mode() & 0o7777 != 0o600 || after.len() != bytes.len() as u64 {
        return Err(DataWorkspaceError::DestinationChanged);
    }
    Ok(FilePin::from(&after))
}
fn verify_destination_location(
    managed: &ApprovedManagedRoot,
    directory: &File,
    destination: &DirectoryPin,
) -> Result<(), DataWorkspaceError> {
    check_directory(&managed.root, &managed.pin)?;
    check_private_dir(&managed.root)?;
    check_directory(directory, destination)?;
    check_private_dir(directory)
}
fn output_parent(
    directory: &File,
    root_path: &Path,
    path: &Path,
    known: &mut BTreeMap<PathBuf, DirectoryPin>,
) -> Result<(File, CString), DataWorkspaceError> {
    let mut cursor = directory.try_clone().map_err(|_| DataWorkspaceError::Io)?;
    let mut relative = PathBuf::new();
    let parts: Vec<_> = path.components().collect();
    for part in &parts[..parts.len() - 1] {
        let Component::Normal(component) = part else {
            return Err(DataWorkspaceError::InvalidSelection);
        };
        relative.push(component);
        if let Some(pin) = known.get(&relative) {
            let opened = open_child(&cursor, component, true)
                .map_err(|_| DataWorkspaceError::DestinationChanged)?;
            check_directory(&opened, pin)?;
            check_private_dir(&opened)?;
            cursor = opened;
        } else {
            let child = create_directory(&cursor, component)?;
            known.insert(
                relative.clone(),
                directory_pin(&root_path.join(&relative), &child)?,
            );
            cursor = child;
        }
    }
    let Component::Normal(last) = parts.last().ok_or(DataWorkspaceError::InvalidSelection)? else {
        return Err(DataWorkspaceError::InvalidSelection);
    };
    Ok((cursor, name(last)?))
}
fn initial_view(
    snapshot: &DatasetSnapshotView,
    id: &str,
    destination: PathBuf,
) -> DataCreationView {
    DataCreationView {
        creation_id: id.into(),
        destination,
        state: CreationState::Creating,
        dataset_revision: snapshot.dataset_revision.clone(),
        selection_digest: snapshot.selection_digest.clone(),
        copied_files: 0,
        error: None,
        source_observation_scope: snapshot.source_observation_scope.clone(),
        atomic_directory_freeze: false,
        filesystem_sandbox: false,
    }
}
/// Copies only the already verified file list. Errors before reservation return
/// Err; every later failure returns a retained, non-Ready creation observation.
/// Existing destinations are never adopted, overwritten, resumed or deleted.
pub fn create_data_workspace(
    snapshot: &VerifiedDatasetSnapshot,
    managed: &ApprovedManagedRoot,
    creation_id: &str,
) -> Result<DataWorkspaceCreation, DataWorkspaceError> {
    if !safe_id(creation_id) {
        return Err(DataWorkspaceError::InvalidSelection);
    }
    validate_snapshot(&snapshot.view)?;
    check_directory(&snapshot.root, &snapshot.view.contract.source)
        .map_err(|_| DataWorkspaceError::SourceChanged)?;
    check_directory(&managed.root, &managed.pin)?;
    check_private_dir(&managed.root)?;
    ensure_outside_source(snapshot, managed)?;
    let base = format!("data-{creation_id}");
    let destination_path = managed.pin.canonical_root.join(&base);
    let mut view = initial_view(&snapshot.view, creation_id, destination_path.clone());
    // Once mkdir succeeds, no subsequent error may imply that nothing happened.
    reserve_directory(&managed.root, OsStr::new(&base))?;
    let opened = (|| {
        let directory = open_child(&managed.root, OsStr::new(&base), true)?;
        check_private_dir(&directory)?;
        let pin = directory_pin(&destination_path, &directory)?;
        verify_destination_location(managed, &directory, &pin)?;
        Ok::<_, DataWorkspaceError>((directory, pin))
    })();
    let (directory, destination) = match opened {
        Ok(pair) => pair,
        Err(error) => {
            view.state = CreationState::OutcomeUnknown;
            view.error = Some(error);
            return Ok(DataWorkspaceCreation { view, ready: None });
        }
    };
    let intent = Intent {
        format_version: FORMAT,
        creation_id: creation_id.into(),
        snapshot: snapshot.view.clone(),
        destination: destination.clone(),
        managed_root: managed.pin.clone(),
    };
    let result = (|| -> Result<DataWorkspaceContract, DataWorkspaceError> {
        verify_destination_location(managed, &directory, &destination)?;
        write_new(&directory, OsStr::new(CREATING), &json_bytes(&intent)?)?;
        #[cfg(test)]
        publication_observed(&destination.canonical_root, false);
        directory.sync_all().map_err(|_| DataWorkspaceError::Io)?;
        let mut known = BTreeMap::new();
        let mut copied = vec![];
        for input in &snapshot.view.files {
            check_directory(&snapshot.root, &snapshot.view.contract.source)
                .map_err(|_| DataWorkspaceError::SourceChanged)?;
            verify_destination_location(managed, &directory, &destination)?;
            let (bytes, pin, hash) = observe(
                &snapshot.root,
                &input.requirement,
                input.requirement.max_bytes,
            )?;
            if pin != input.source_identity
                || hash != input.sha256
                || bytes.len() as u64 != input.bytes
            {
                return Err(DataWorkspaceError::SourceChanged);
            }
            let (parent, leaf) = output_parent(
                &directory,
                &destination.canonical_root,
                &input.destination_relative_path,
                &mut known,
            )?;
            let stamp = write_new(&parent, OsStr::from_bytes(leaf.as_bytes()), &bytes)?;
            drop(bytes);
            parent.sync_all().map_err(|_| DataWorkspaceError::Io)?;
            copied.push(CopiedDataFile {
                input: input.clone(),
                destination_identity: stamp,
            });
            view.copied_files += 1;
        }
        // This second source pass is an explicit observation, never proof that the
        // files simultaneously existed or remained immutable during the operation.
        for input in &snapshot.view.files {
            let (_, pin, hash) = observe(
                &snapshot.root,
                &input.requirement,
                input.requirement.max_bytes,
            )?;
            if pin != input.source_identity || hash != input.sha256 {
                return Err(DataWorkspaceError::SourceChanged);
            }
        }
        check_directory(&snapshot.root, &snapshot.view.contract.source)
            .map_err(|_| DataWorkspaceError::SourceChanged)?;
        let mut contract = DataWorkspaceContract {
            format_version: FORMAT,
            creation_id: creation_id.into(),
            snapshot: snapshot.view.clone(),
            managed_root: managed.pin.clone(),
            destination: destination.clone(),
            files: copied,
            directories: known.into_values().collect(),
            contract_digest: String::new(),
        };
        contract.contract_digest = completed_digest(&contract)?;
        validate_completed_contract(&contract)?;
        verify_copy_bytes(&directory, &contract)?;
        verify_destination_location(managed, &directory, &destination)?;
        directory.sync_all().map_err(|_| DataWorkspaceError::Io)?;
        verify_intent(&directory, &contract)?;
        write_new(&directory, OsStr::new(READY), &json_bytes(&contract)?)?;
        #[cfg(test)]
        publication_observed(&destination.canonical_root, true);
        directory.sync_all().map_err(|_| DataWorkspaceError::Io)?;
        managed
            .root
            .sync_all()
            .map_err(|_| DataWorkspaceError::Io)?;
        verify_publication(&directory, &contract)?;
        verify_destination_location(managed, &directory, &destination)?;
        Ok(contract)
    })();
    match result {
        Ok(contract) => {
            view.state = CreationState::Ready;
            Ok(DataWorkspaceCreation {
                view,
                ready: Some(VerifiedDataWorkspace {
                    contract,
                    directory,
                }),
            })
        }
        Err(error) => {
            view.error = Some(error);
            let failure = Failure {
                creation_id: creation_id.into(),
                selection_digest: snapshot.view.selection_digest.clone(),
                dataset_revision: snapshot.view.dataset_revision.clone(),
                error,
            };
            let retained = verify_destination_location(managed, &directory, &destination).is_ok()
                && json_bytes(&failure)
                    .and_then(|b| write_new(&directory, OsStr::new(FAILED), &b))
                    .is_ok()
                && directory.sync_all().is_ok();
            view.state = if retained {
                CreationState::Failed
            } else {
                CreationState::OutcomeUnknown
            };
            Ok(DataWorkspaceCreation { view, ready: None })
        }
    }
}
fn verify_copy_bytes(
    directory: &File,
    c: &DataWorkspaceContract,
) -> Result<(), DataWorkspaceError> {
    for pin in &c.directories {
        let relative = pin
            .canonical_root
            .strip_prefix(&c.destination.canonical_root)
            .map_err(|_| DataWorkspaceError::MetadataInvalid)?;
        let subdirectory = open_relative(directory, relative)?;
        check_directory(&subdirectory, pin)?;
        check_private_dir(&subdirectory)?;
    }
    for copied in &c.files {
        let (bytes, stamp) = read_file(
            open_relative(directory, &copied.input.destination_relative_path)?,
            copied.input.requirement.max_bytes,
        )?;
        let reopened = open_relative(directory, &copied.input.destination_relative_path)?;
        let current = reopened.metadata().map_err(|_| DataWorkspaceError::Io)?;
        if current.nlink() != 1
            || FilePin::from(&current) != stamp
            || stamp != copied.destination_identity
            || stamp.mode & 0o7777 != 0o600
            || stamp.uid != unsafe { libc::geteuid() }
            || bytes.len() as u64 != copied.input.bytes
            || sha256(&bytes) != copied.input.sha256
        {
            return Err(DataWorkspaceError::DestinationChanged);
        }
    }
    Ok(())
}
fn read_metadata<T: for<'a> Deserialize<'a>>(
    directory: &File,
    name: &str,
) -> Result<T, DataWorkspaceError> {
    let (bytes, pin) = read_file(
        open_child(directory, OsStr::new(name), false)?,
        MAX_METADATA,
    )?;
    let current = open_child(directory, OsStr::new(name), false)?
        .metadata()
        .map_err(|_| DataWorkspaceError::Io)?;
    if current.nlink() != 1
        || FilePin::from(&current) != pin
        || pin.mode & 0o7777 != 0o600
        || pin.uid != unsafe { libc::geteuid() }
    {
        return Err(DataWorkspaceError::MetadataInvalid);
    }
    serde_json::from_slice(&bytes).map_err(|_| DataWorkspaceError::MetadataInvalid)
}
fn verify_intent(
    directory: &File,
    contract: &DataWorkspaceContract,
) -> Result<(), DataWorkspaceError> {
    let intent: Intent = read_metadata(directory, CREATING)?;
    if intent.format_version != FORMAT
        || intent.creation_id != contract.creation_id
        || intent.snapshot != contract.snapshot
        || intent.destination != contract.destination
        || intent.managed_root != contract.managed_root
    {
        return Err(DataWorkspaceError::MetadataInvalid);
    }
    Ok(())
}
fn verify_publication(
    directory: &File,
    contract: &DataWorkspaceContract,
) -> Result<(), DataWorkspaceError> {
    verify_intent(directory, contract)?;
    let ready: DataWorkspaceContract = read_metadata(directory, READY)?;
    if &ready != contract {
        return Err(DataWorkspaceError::MetadataInvalid);
    }
    Ok(())
}
/// Restores only from an independently authenticated persisted host contract.
/// No original source path is reopened: an unchanged Ready copy survives source
/// movement/deletion/modification. Disk labels alone cannot create this token.
pub fn restore_data_workspace(
    expected: &ApprovedRestoreContract,
    managed: &ApprovedManagedRoot,
) -> Result<VerifiedDataWorkspace, DataWorkspaceError> {
    let contract = &expected.view;
    validate_completed_contract(contract)?;
    if contract.managed_root != managed.pin {
        return Err(DataWorkspaceError::ManagedRootInvalid);
    }
    check_directory(&managed.root, &managed.pin)?;
    check_private_dir(&managed.root)?;
    let directory = open_child(
        &managed.root,
        OsStr::new(&format!("data-{}", contract.creation_id)),
        true,
    )
    .map_err(|_| DataWorkspaceError::DestinationChanged)?;
    verify_destination_location(managed, &directory, &contract.destination)?;
    verify_publication(&directory, contract)?;
    verify_copy_bytes(&directory, contract)?;
    verify_destination_location(managed, &directory, &contract.destination)?;
    Ok(VerifiedDataWorkspace {
        contract: contract.clone(),
        directory,
    })
}
/// Inspect retained incomplete work without retry/cleanup. A Ready label here
/// deliberately remains OutcomeUnknown until restore_data_workspace receives a
/// separate authenticated complete contract and verifies every destination byte.
pub fn inspect_incomplete_creation(
    snapshot: &VerifiedDatasetSnapshot,
    managed: &ApprovedManagedRoot,
    creation_id: &str,
) -> Result<DataCreationView, DataWorkspaceError> {
    if !safe_id(creation_id) {
        return Err(DataWorkspaceError::InvalidSelection);
    }
    check_directory(&managed.root, &managed.pin)?;
    check_private_dir(&managed.root)?;
    let path = managed
        .pin
        .canonical_root
        .join(format!("data-{creation_id}"));
    let directory = open_child(
        &managed.root,
        OsStr::new(&format!("data-{creation_id}")),
        true,
    )?;
    check_private_dir(&directory)?;
    let pin = directory_pin(&path, &directory)?;
    check_directory(&directory, &pin)?;
    let intent: Intent = read_metadata(&directory, CREATING)?;
    if intent.format_version != FORMAT
        || intent.creation_id != creation_id
        || intent.snapshot != snapshot.view
        || intent.destination != pin
        || intent.managed_root != managed.pin
    {
        return Err(DataWorkspaceError::MetadataInvalid);
    }
    let mut view = initial_view(&snapshot.view, creation_id, path);
    view.state = CreationState::OutcomeUnknown;
    if let Ok(failure) = read_metadata::<Failure>(&directory, FAILED) {
        if failure.creation_id == creation_id
            && failure.selection_digest == snapshot.view.selection_digest
            && failure.dataset_revision == snapshot.view.dataset_revision
        {
            view.state = CreationState::Failed;
            view.error = Some(failure.error);
        }
    }
    Ok(view)
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn marker_fault(ready_written: bool) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("source");
        let managed = root.join("managed");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(source.join("dataset"), b"fixed input").unwrap();
        let selected = ApprovedDataSelection::from_verified_host(
            &source,
            "host-approved".into(),
            vec![ArtifactRequirement {
                artifact_id: "dataset".into(),
                relative_path: "dataset".into(),
                max_bytes: 100,
                expected_sha256: None,
            }],
            DataLimits::default(),
        )
        .unwrap();
        let snapshot = verify_dataset(&selected).unwrap();
        let host_root = ApprovedManagedRoot::from_verified_host(&managed).unwrap();
        PUBLICATION_HOOK.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move |path, stage| {
                if stage == ready_written {
                    fs::write(
                        path.join(if ready_written { READY } else { CREATING }),
                        b"{}",
                    )
                    .unwrap();
                }
            }))
        });
        let result = create_data_workspace(&snapshot, &host_root, "marker-fault");
        PUBLICATION_HOOK.with(|hook| *hook.borrow_mut() = None);
        let creation = result.unwrap();
        assert!(
            creation.verified().is_none(),
            "modified publication metadata must never yield a Ready token"
        );
        assert_ne!(creation.view().state, CreationState::Ready);
        assert_eq!(
            creation.view().error,
            Some(DataWorkspaceError::MetadataInvalid)
        );
        assert_eq!(fs::read(source.join("dataset")).unwrap(), b"fixed input");
        assert!(managed.join("data-marker-fault").exists());
    }
    #[test]
    fn creating_marker_drift_cannot_publish_a_verified_token() {
        marker_fault(false);
    }
    #[test]
    fn ready_marker_drift_cannot_publish_a_verified_token() {
        marker_fault(true);
    }
}
