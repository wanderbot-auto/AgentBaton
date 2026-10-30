//! Host-selected accepted files to raw Git objects. No acceptance, integration,
//! ref, checkout, remote or original-index authority is granted by this module.
use crate::{
    git_command, is_oid, open_root_without_symlinks,
    validation::{sha256, valid_id, BoundArtifact},
    WorkspaceError, WorkspaceIdentity, WorkspaceKind,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

pub const MAX_SELECTED_FILES: usize = 256;
pub const MAX_SELECTED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 4096;
static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

/// Private host capability, not an Owner/TaskAccepted verifier. The host must
/// authenticate the accepted receipt and independent integration policy first.
/// There is deliberately no Deserialize/Clone implementation.
pub struct AuthorizedSourceCapture {
    root: PathBuf,
    root_fd: File,
    git_dir: PathBuf,
    git_fd: File,
    common_dir: PathBuf,
    common_fd: File,
    base: String,
    selected: Vec<BoundArtifact>,
    selection_digest: String,
    selection_ref: String,
    approval_ref: String,
    commit_time: u64,
    deadline: Instant,
    host_deadline: Option<Instant>,
    host_epoch: Option<crate::adoption::PreparationEpoch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedSourceDigest {
    pub artifact_id: String,
    pub relative_path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub mode: String,
}
/// Observed object result, never an acceptance, Grant or adoption receipt.
/// No Deserialize path can reconstruct a trusted capture result.
pub struct CapturedSourceCommit {
    commit: String,
    tree: String,
    base: String,
    selection_digest: String,
    selected: Vec<SelectedSourceDigest>,
}
impl CapturedSourceCommit {
    pub fn commit(&self) -> &str {
        &self.commit
    }
    pub fn tree(&self) -> &str {
        &self.tree
    }
    pub fn base(&self) -> &str {
        &self.base
    }
    pub fn selection_digest(&self) -> &str {
        &self.selection_digest
    }
    pub fn selected(&self) -> &[SelectedSourceDigest] {
        &self.selected
    }
}
fn same_identity(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}
fn same_stamp(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    same_identity(a, b)
        && a.len() == b.len()
        && a.nlink() == b.nlink()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
fn safe_path(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    if text.is_empty()
        || !text.is_ascii()
        || text.len() > MAX_PATH_BYTES
        || text.chars().any(char::is_control)
    {
        return false;
    }
    if path.components().any(|c| !matches!(c, Component::Normal(n) if !n.as_encoded_bytes().eq_ignore_ascii_case(b".git"))) { return false; }
    // Components must not normalize aliases (double separators or dot segments).
    path.components().collect::<PathBuf>().as_os_str() == path.as_os_str()
}
fn oid(bytes: Vec<u8>) -> Result<String, WorkspaceError> {
    let s = String::from_utf8(bytes).map_err(|_| WorkspaceError::InvalidPath)?;
    let s = s.trim_end_matches('\n');
    if !is_oid(s) {
        return Err(WorkspaceError::NotRepository);
    }
    Ok(s.into())
}
fn check_deadline(deadline: Instant) -> Result<(), WorkspaceError> {
    if Instant::now() >= deadline {
        return Err(WorkspaceError::ResourceLimit);
    }
    Ok(())
}
fn check_capture_control(
    deadline: Instant,
    control: Option<&crate::adoption::AdoptionControl<'_>>,
) -> Result<(), WorkspaceError> {
    match control {
        Some(control) => control.narrowed(deadline).check(),
        None => check_deadline(deadline),
    }
}
#[cfg(test)]
fn text_output(deadline: Instant, command: &mut Command) -> Result<Vec<u8>, WorkspaceError> {
    text_output_controlled(deadline, command, None)
}
fn text_output_controlled(
    deadline: Instant,
    command: &mut Command,
    control: Option<&crate::adoption::AdoptionControl<'_>>,
) -> Result<Vec<u8>, WorkspaceError> {
    check_capture_control(deadline, control)?;
    let narrowed = control.map(|c| c.narrowed(deadline));
    let output = crate::output_with_control(command, narrowed.as_ref())?;
    check_capture_control(deadline, control)?;
    if !output.status.success() {
        return Err(WorkspaceError::Git("source capture rejected".into()));
    }
    Ok(output.stdout)
}
fn git_path(
    deadline: Instant,
    root: &Path,
    arg: &str,
    control: Option<&crate::adoption::AdoptionControl<'_>>,
) -> Result<PathBuf, WorkspaceError> {
    let raw = text_output_controlled(
        deadline,
        git_command(root).args(["rev-parse", "--path-format=absolute", arg]),
        control,
    )?;
    let text = String::from_utf8(raw).map_err(|_| WorkspaceError::InvalidPath)?;
    let text = text.strip_suffix('\n').ok_or(WorkspaceError::InvalidPath)?;
    if text.len() > MAX_PATH_BYTES || text.chars().any(char::is_control) {
        return Err(WorkspaceError::InvalidPath);
    }
    let p = PathBuf::from(text);
    if fs::canonicalize(&p)? != p {
        return Err(WorkspaceError::InvalidPath);
    }
    Ok(p)
}
fn no_alternates(
    deadline: Instant,
    common: &Path,
    control: Option<&crate::adoption::AdoptionControl<'_>>,
) -> Result<(), WorkspaceError> {
    // No external loose-object directories, pack links or alternate stores.
    // The scan is metadata-only and globally bounded, never silently truncated.
    let objects = common.join("objects");
    let _objects = open_root_without_symlinks(&objects)?;
    let mut count = 0usize;
    for entry in fs::read_dir(&objects)? {
        check_capture_control(deadline, control)?;
        count += 1;
        if count > 65_536 {
            return Err(WorkspaceError::ResourceLimit);
        }
        let entry = entry?;
        let name = entry.file_name();
        let bytes = name.as_encoded_bytes();
        if bytes != b"info"
            && bytes != b"pack"
            && !(bytes.len() == 2 && bytes.iter().all(u8::is_ascii_hexdigit))
        {
            return Err(WorkspaceError::UnsafeRepository);
        }
        let _dir = open_root_without_symlinks(&entry.path())?;
        for child in fs::read_dir(entry.path())? {
            check_capture_control(deadline, control)?;
            count += 1;
            if count > 65_536 {
                return Err(WorkspaceError::ResourceLimit);
            }
            let child = child?;
            if bytes == b"info"
                && matches!(
                    child.file_name().as_encoded_bytes(),
                    b"alternates" | b"http-alternates"
                )
            {
                return Err(WorkspaceError::UnsafeRepository);
            }
            let meta = fs::symlink_metadata(child.path())?;
            if !meta.is_file() || meta.nlink() != 1 {
                return Err(WorkspaceError::UnsafeRepository);
            }
        }
    }
    Ok(())
}

fn audit(
    deadline: Instant,
    command: &mut Command,
    control: Option<&crate::adoption::AdoptionControl<'_>>,
) -> Result<(), WorkspaceError> {
    let raw = text_output_controlled(
        deadline,
        command.args(["config", "--no-includes", "--null", "--list"]),
        control,
    )?;
    if !raw.is_empty() && raw.last() != Some(&0) {
        return Err(WorkspaceError::UnsafeRepository);
    }
    for entry in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let key = entry.split(|b| *b == b'\n').next().unwrap_or_default();
        if key == b"include.path"
            || (key.starts_with(b"includeif.") && key.ends_with(b".path"))
            || key.starts_with(b"filter.")
            || key.starts_with(b"merge.")
            || key == b"extensions.partialclone"
            || (key.starts_with(b"remote.") && key.ends_with(b".promisor"))
        {
            return Err(WorkspaceError::UnsafeRepository);
        }
    }
    Ok(())
}
impl AuthorizedSourceCapture {
    pub fn from_verified_selection(
        workspace: &WorkspaceIdentity,
        expected_base: String,
        selected: Vec<BoundArtifact>,
        commit_time_unix: u64,
        selection_ref: String,
        approval_ref: String,
    ) -> Result<Self, WorkspaceError> {
        Self::from_verified_selection_inner(
            workspace,
            expected_base,
            selected,
            commit_time_unix,
            selection_ref,
            approval_ref,
            None,
        )
    }
    pub fn from_verified_selection_controlled(
        workspace: &WorkspaceIdentity,
        expected_base: String,
        selected: Vec<BoundArtifact>,
        commit_time_unix: u64,
        selection_ref: String,
        approval_ref: String,
        control: &crate::adoption::AdoptionControl<'_>,
    ) -> Result<Self, WorkspaceError> {
        Self::from_verified_selection_inner(
            workspace,
            expected_base,
            selected,
            commit_time_unix,
            selection_ref,
            approval_ref,
            Some(control),
        )
    }
    fn from_verified_selection_inner(
        workspace: &WorkspaceIdentity,
        expected_base: String,
        selected: Vec<BoundArtifact>,
        commit_time_unix: u64,
        selection_ref: String,
        approval_ref: String,
        control: Option<&crate::adoption::AdoptionControl<'_>>,
    ) -> Result<Self, WorkspaceError> {
        // The same deadline includes host construction, queued handoff and all
        // stages; consuming the capability never refreshes it.
        let deadline = (Instant::now() + Duration::from_secs(15)).min(control.map_or_else(
            || Instant::now() + Duration::from_secs(15),
            |c| c.deadline(),
        ));
        check_capture_control(deadline, control)?;
        if selected.is_empty() || selected.len() > MAX_SELECTED_FILES {
            return Err(WorkspaceError::TooLarge);
        }
        if !valid_id(&selection_ref)
            || !valid_id(&approval_ref)
            || !is_oid(&expected_base)
            || commit_time_unix > u32::MAX as u64
        {
            return Err(WorkspaceError::InvalidPath);
        }
        let WorkspaceKind::Git {
            common_dir, head, ..
        } = &workspace.kind
        else {
            return Err(WorkspaceError::NotRepository);
        };
        if head != &expected_base {
            return Err(WorkspaceError::StaleHead);
        }
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        // Reject count/path/cap errors before reading any selected file.
        for a in &selected {
            check_capture_control(deadline, control)?;
            let v = a.contract_view();
            if v.canonical_root != workspace.canonical_root
                || (v.root_device, v.root_inode) != (workspace.device, workspace.inode)
                || !safe_path(&v.requirement.relative_path)
                || !valid_id(&v.requirement.artifact_id)
                || v.requirement.max_bytes == 0
                || v.requirement.max_bytes > MAX_SELECTED_BYTES
                || !v
                    .requirement
                    .expected_sha256
                    .as_deref()
                    .is_some_and(crate::validation::valid_hash)
                || !ids.insert(v.requirement.artifact_id)
                || !paths.insert(
                    v.requirement
                        .relative_path
                        .to_string_lossy()
                        .to_ascii_lowercase(),
                )
            {
                return Err(WorkspaceError::InvalidPath);
            }
        }
        for path in &paths {
            for parent in Path::new(path)
                .ancestors()
                .skip(1)
                .filter(|p| !p.as_os_str().is_empty())
            {
                if paths.contains(parent.to_str().ok_or(WorkspaceError::InvalidPath)?) {
                    return Err(WorkspaceError::InvalidPath);
                }
            }
        }
        let root_fd = open_root_without_symlinks(&workspace.canonical_root)?;
        let root_meta = root_fd.metadata()?;
        if (root_meta.dev(), root_meta.ino()) != (workspace.device, workspace.inode) {
            return Err(WorkspaceError::InvalidPath);
        }
        audit(
            deadline,
            &mut git_command(&workspace.canonical_root),
            control,
        )?;
        let git_dir = git_path(
            deadline,
            &workspace.canonical_root,
            "--absolute-git-dir",
            control,
        )?;
        if git_path(
            deadline,
            &workspace.canonical_root,
            "--git-common-dir",
            control,
        )? != *common_dir
        {
            return Err(WorkspaceError::InvalidPath);
        }
        let git_fd = open_root_without_symlinks(&git_dir)?;
        let common_fd = open_root_without_symlinks(common_dir)?;
        no_alternates(deadline, common_dir, control)?;
        let mut result = Self {
            root: workspace.canonical_root.clone(),
            root_fd,
            git_dir,
            git_fd,
            common_dir: common_dir.clone(),
            common_fd,
            base: expected_base,
            selected,
            selection_digest: String::new(),
            selection_ref,
            approval_ref,
            commit_time: commit_time_unix,
            deadline,
            host_deadline: control.map(|c| c.deadline()),
            host_epoch: control.map(crate::adoption::AdoptionControl::epoch),
        };
        result.selection_digest = result.current_selection_digest(control)?;
        result.recheck(control)?;
        Ok(result)
    }
    fn current_selection_digest(
        &self,
        control: Option<&crate::adoption::AdoptionControl<'_>>,
    ) -> Result<String, WorkspaceError> {
        check_capture_control(self.deadline, control)?;
        let views: Vec<_> = self
            .selected
            .iter()
            .map(BoundArtifact::contract_view)
            .collect();
        let common = self.common_fd.metadata()?;
        let git = self.git_fd.metadata()?;
        let binding = serde_json::to_vec(&(
            "agentbaton.source-capture.v1",
            &self.selection_ref,
            &self.approval_ref,
            &self.base,
            self.commit_time,
            &views,
            &self.git_dir,
            (git.dev(), git.ino()),
            &self.common_dir,
            (common.dev(), common.ino()),
        ))
        .map_err(|_| WorkspaceError::InvalidPath)?;
        controlled_hash(&binding, self.deadline, control)
    }
    fn command(&self) -> Command {
        let mut c = git_command(&self.root);
        c.arg("--git-dir").arg(&self.git_dir).args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "i18n.commitEncoding=UTF-8",
        ]);
        c
    }
    fn recheck(
        &self,
        control: Option<&crate::adoption::AdoptionControl<'_>>,
    ) -> Result<(), WorkspaceError> {
        let deadline = self.deadline;
        check_capture_control(deadline, control)?;
        if self.current_selection_digest(control)? != self.selection_digest {
            return Err(WorkspaceError::InvalidPath);
        }
        for (p, fd) in [
            (&self.root, &self.root_fd),
            (&self.git_dir, &self.git_fd),
            (&self.common_dir, &self.common_fd),
        ] {
            let current = open_root_without_symlinks(p)?.metadata()?;
            if !same_identity(&current, &fd.metadata()?) {
                return Err(WorkspaceError::InvalidPath);
            }
        }
        audit(deadline, &mut self.command(), control)?;
        no_alternates(deadline, &self.common_dir, control)?;
        if git_path(deadline, &self.root, "--show-toplevel", control)? != self.root
            || git_path(deadline, &self.root, "--absolute-git-dir", control)? != self.git_dir
            || git_path(deadline, &self.root, "--git-common-dir", control)? != self.common_dir
        {
            return Err(WorkspaceError::InvalidPath);
        }
        if oid(text_output_controlled(
            deadline,
            self.command()
                .args(["rev-parse", "--verify", "HEAD^{commit}"]),
            control,
        )?)? != self.base
        {
            return Err(WorkspaceError::StaleHead);
        }
        Ok(())
    }
}
fn check_tree_paths(
    capture: &AuthorizedSourceCapture,
    tree: &str,
    control: Option<&crate::adoption::AdoptionControl<'_>>,
) -> Result<(), WorkspaceError> {
    let deadline = capture.deadline;
    if !is_oid(tree) {
        return Err(WorkspaceError::UnsafeRepository);
    }
    let raw = text_output_controlled(
        deadline,
        capture.command().args(["ls-tree", "-r", "-t", "-z", tree]),
        control,
    )?;
    if !raw.is_empty() && raw.last() != Some(&0) {
        return Err(WorkspaceError::UnsafeRepository);
    }
    let mut count = 0usize;
    let mut paths = BTreeSet::new();
    let mut trees = Vec::new();
    let selected: Vec<_> = capture
        .selected
        .iter()
        .map(|a| a.contract_view().requirement.relative_path)
        .collect();
    let mut rows = raw.split(|b| *b == 0);
    while let Some(entry) = rows.next() {
        check_capture_control(deadline, control)?;
        if entry.is_empty() {
            if rows.next().is_some() {
                return Err(WorkspaceError::UnsafeRepository);
            }
            break;
        }
        count += 1;
        if count > 65_536 {
            return Err(WorkspaceError::ResourceLimit);
        }
        let tab = entry
            .iter()
            .position(|b| *b == b'\t')
            .filter(|n| *n <= 128)
            .ok_or(WorkspaceError::UnsafeRepository)?;
        let header =
            std::str::from_utf8(&entry[..tab]).map_err(|_| WorkspaceError::UnsafeRepository)?;
        let fields: Vec<_> = header.split(' ').collect();
        if fields.len() != 3
            || !is_oid(fields[2])
            || fields[2].len() != capture.base.len()
            || !matches!(
                (fields[0], fields[1]),
                ("040000", "tree")
                    | ("100644" | "100755" | "120000", "blob")
                    | ("160000", "commit")
            )
        {
            return Err(WorkspaceError::UnsafeRepository);
        }
        let text =
            std::str::from_utf8(&entry[tab + 1..]).map_err(|_| WorkspaceError::InvalidPath)?;
        // This slice supports ASCII paths only. Reject the whole base when a
        // Unicode entry could alias an ASCII selection on a normalizing host.
        if !safe_path(Path::new(text)) {
            return Err(WorkspaceError::InvalidPath);
        }
        if !paths.insert(text.to_owned()) {
            return Err(WorkspaceError::UnsafeRepository);
        }
        if fields[0] == "040000" {
            trees.push(text.to_owned());
        }
        for path in &selected {
            let wanted = path.to_str().ok_or(WorkspaceError::InvalidPath)?;
            let base_parts: Vec<_> = text.split('/').collect();
            let wanted_parts: Vec<_> = wanted.split('/').collect();
            for (base, wanted) in base_parts.iter().zip(&wanted_parts) {
                if base.eq_ignore_ascii_case(wanted) && base != wanted {
                    return Err(WorkspaceError::InvalidPath);
                }
                if base != wanted {
                    break;
                }
            }
        }
    }
    // Explicit tree entries are observable source data even when a cached Git
    // index temporarily preserves their OIDs. Never rely on cache-tree state to
    // detect an empty subtree that subsequent index updates could discard.
    for tree in trees {
        check_capture_control(deadline, control)?;
        let prefix = tree + "/";
        if !paths
            .range(prefix.clone()..)
            .next()
            .is_some_and(|path| path.starts_with(&prefix))
        {
            return Err(WorkspaceError::UnsafeRepository);
        }
    }
    Ok(())
}

struct ExpectedTreeEntry {
    path: PathBuf,
    mode: String,
    blob: String,
    base_mode: String,
    base_blob: String,
}
impl AuthorizedSourceCapture {
    fn verify_tree(
        &self,
        tree: &str,
        expected: &[ExpectedTreeEntry],
        control: Option<&crate::adoption::AdoptionControl<'_>>,
    ) -> Result<(), WorkspaceError> {
        if !is_oid(tree) || expected.is_empty() || expected.len() > MAX_SELECTED_FILES {
            return Err(WorkspaceError::InvalidPath);
        }
        check_tree_paths(self, tree, control)?;
        let raw = text_output_controlled(
            self.deadline,
            self.command().args([
                "diff-tree",
                "-r",
                "--raw",
                "--no-abbrev",
                "-z",
                "--no-renames",
                "--no-ext-diff",
                "--ignore-submodules=none",
                self.base.as_str(),
                tree,
            ]),
            control,
        )?;
        if !raw.is_empty() && raw.last() != Some(&0) {
            return Err(WorkspaceError::InvalidPath);
        }
        let mut seen = BTreeSet::new();
        let mut rows = raw.split(|b| *b == 0);
        while let Some(header) = rows.next() {
            check_capture_control(self.deadline, control)?;
            if header.is_empty() {
                if rows.next().is_some() {
                    return Err(WorkspaceError::InvalidPath);
                }
                break;
            }
            if seen.len() >= MAX_SELECTED_FILES || header.len() > 170 {
                return Err(WorkspaceError::InvalidPath);
            }
            let path = rows
                .next()
                .filter(|p| !p.is_empty() && p.len() <= MAX_PATH_BYTES)
                .ok_or(WorkspaceError::InvalidPath)?;
            let entry = expected
                .iter()
                .find(|e| e.path.as_os_str().as_encoded_bytes() == path)
                .ok_or(WorkspaceError::InvalidPath)?;
            if !seen.insert(path.to_vec()) {
                return Err(WorkspaceError::InvalidPath);
            }
            let h = std::str::from_utf8(header).map_err(|_| WorkspaceError::InvalidPath)?;
            let fields: Vec<_> = h.split(' ').collect();
            let status = if entry.base_mode == "000000" {
                "A"
            } else {
                "M"
            };
            if fields.len() != 5
                || fields[0] != format!(":{}", entry.base_mode)
                || fields[1] != entry.mode
                || fields[2] != entry.base_blob
                || fields[3] != entry.blob
                || fields[4] != status
                || (entry.base_mode == entry.mode && entry.base_blob == entry.blob)
            {
                return Err(WorkspaceError::InvalidPath);
            }
        }
        for entry in expected {
            check_capture_control(self.deadline, control)?;
            if !safe_path(&entry.path)
                || !matches!(entry.mode.as_str(), "100644" | "100755")
                || !is_oid(&entry.blob)
            {
                return Err(WorkspaceError::InvalidPath);
            }
            let changed = entry.base_mode != entry.mode || entry.base_blob != entry.blob;
            if changed != seen.contains(entry.path.as_os_str().as_encoded_bytes()) {
                return Err(WorkspaceError::InvalidPath);
            }
        }
        Ok(())
    }
}

/// Owns exactly this fixed cat-file child; even setup/unwind paths attempt
/// kill+wait, and normal verification explicitly joins both bounded readers.
struct BlobChild {
    child: Child,
    reaped: bool,
}
impl BlobChild {
    fn stop(&mut self) -> std::io::Result<std::process::ExitStatus> {
        let _ = self.child.kill();
        let result = self.child.wait();
        self.reaped = result.is_ok();
        result
    }
}
impl Drop for BlobChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.stop();
        }
    }
}
impl AuthorizedSourceCapture {
    // This narrow verifier streams actual Git object bytes, rather than trusting
    // a post-Git reread of the mutable scratch pathname. No 64MiB output Vec or
    // alteration of the existing 4MiB bounded_output helper is needed.
    fn verify_blob(
        &self,
        blob: &str,
        expected_sha: &str,
        expected_bytes: u64,
        control: Option<&crate::adoption::AdoptionControl<'_>>,
    ) -> Result<(), WorkspaceError> {
        if let Some(control) = control {
            return self.verify_blob_controlled(blob, expected_sha, expected_bytes, control);
        }
        if !is_oid(blob)
            || !crate::validation::valid_hash(expected_sha)
            || expected_bytes > MAX_SELECTED_BYTES
        {
            return Err(WorkspaceError::InvalidPath);
        }
        check_capture_control(self.deadline, control)?;
        let end = self.deadline.min(Instant::now() + Duration::from_secs(15));
        let child = self
            .command()
            .args(["cat-file", "blob", blob])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut owned = BlobChild {
            child,
            reaped: false,
        };
        let stdout = owned.child.stdout.take().expect("fixed blob stdout pipe");
        let stderr = owned.child.stderr.take().expect("fixed blob stderr pipe");
        let out = std::thread::Builder::new()
            .name("source-blob-sha".into())
            .spawn(move || -> std::io::Result<(u64, String)> {
                let mut reader = stdout.take(expected_bytes + 1);
                let mut hash = Sha256::new();
                let mut total = 0u64;
                let mut chunk = [0u8; 65_536];
                loop {
                    let n = reader.read(&mut chunk)?;
                    if n == 0 {
                        break;
                    }
                    total += n as u64;
                    hash.update(&chunk[..n]);
                }
                Ok((total, format!("{:x}", hash.finalize())))
            })?;
        let err = match std::thread::Builder::new()
            .name("source-blob-stderr".into())
            .spawn(move || -> std::io::Result<u64> {
                let mut reader = stderr.take(4 * 1024 * 1024 + 1);
                let mut total = 0u64;
                let mut chunk = [0u8; 65_536];
                loop {
                    let n = reader.read(&mut chunk)?;
                    if n == 0 {
                        break;
                    }
                    total += n as u64;
                }
                Ok(total)
            }) {
            Ok(reader) => reader,
            Err(error) => {
                let _ = owned.stop();
                let _ = out.join();
                return Err(error.into());
            }
        };
        let mut timed_out = false;
        let status = loop {
            match owned.child.try_wait() {
                Ok(Some(status)) => {
                    owned.reaped = true;
                    break Ok(status);
                }
                Ok(None) => {
                    if Instant::now() >= end {
                        timed_out = true;
                        break owned.stop();
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    let _ = owned.stop();
                    break Err(error);
                }
            }
        };
        // Collect both joins before propagating either error/panic.
        let out = out.join();
        let err = err.join();
        let output = out.map_err(|_| WorkspaceError::ResourceLimit)??;
        let stderr_bytes = err.map_err(|_| WorkspaceError::ResourceLimit)??;
        let status = status?;
        if timed_out || !owned.reaped || Instant::now() >= end {
            return Err(WorkspaceError::ResourceLimit);
        }
        if !status.success()
            || stderr_bytes > 4 * 1024 * 1024
            || output.0 != expected_bytes
            || output.1 != expected_sha
        {
            return Err(WorkspaceError::InvalidPath);
        }
        Ok(())
    }
}

fn controlled_hash(
    bytes: &[u8],
    deadline: Instant,
    control: Option<&crate::adoption::AdoptionControl<'_>>,
) -> Result<String, WorkspaceError> {
    if control.is_none() {
        return Ok(sha256(bytes));
    }
    let mut hash = Sha256::new();
    for chunk in bytes.chunks(65_536) {
        check_capture_control(deadline, control)?;
        hash.update(chunk);
    }
    check_capture_control(deadline, control)?;
    Ok(format!("{:x}", hash.finalize()))
}
fn read_controlled_bound(
    artifact: &BoundArtifact,
    deadline: Instant,
    control: Option<&crate::adoption::AdoptionControl<'_>>,
) -> Result<Vec<u8>, WorkspaceError> {
    if control.is_none() {
        return artifact.read();
    }
    use std::os::fd::{AsRawFd, FromRawFd};
    let view = artifact.contract_view();
    check_capture_control(deadline, control)?;
    if !safe_path(&view.requirement.relative_path)
        || view.requirement.max_bytes == 0
        || view.requirement.max_bytes > MAX_SELECTED_BYTES
    {
        return Err(WorkspaceError::InvalidPath);
    }
    let root = open_root_without_symlinks(&view.canonical_root)?;
    let root_meta = root.metadata()?;
    if (root_meta.dev(), root_meta.ino()) != (view.root_device, view.root_inode) {
        return Err(WorkspaceError::InvalidPath);
    }
    let mut directory = root.try_clone()?;
    let parts = view
        .requirement
        .relative_path
        .components()
        .collect::<Vec<_>>();
    for (i, part) in parts.iter().enumerate() {
        check_capture_control(deadline, control)?;
        let Component::Normal(name) = part else {
            return Err(WorkspaceError::InvalidPath);
        };
        let name = std::ffi::CString::new(name.as_encoded_bytes())
            .map_err(|_| WorkspaceError::InvalidPath)?;
        let last = i + 1 == parts.len();
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if last { 0 } else { libc::O_DIRECTORY };
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        check_capture_control(deadline, control)?;
        if !last {
            directory = file;
            continue;
        }
        let before = file.metadata()?;
        if !before.is_file() || before.nlink() != 1 {
            return Err(WorkspaceError::NotRegularFile);
        }
        if before.len() > view.requirement.max_bytes {
            return Err(WorkspaceError::TooLarge);
        }
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 65_536];
        loop {
            check_capture_control(deadline, control)?;
            let n = file.read(&mut chunk)?;
            check_capture_control(deadline, control)?;
            if n == 0 {
                break;
            }
            if (bytes.len() as u64)
                .checked_add(n as u64)
                .is_none_or(|n| n > view.requirement.max_bytes)
            {
                return Err(WorkspaceError::TooLarge);
            }
            bytes.extend_from_slice(&chunk[..n]);
        }
        let after = file.metadata()?;
        let current_fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if current_fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let current = unsafe { File::from_raw_fd(current_fd) }.metadata()?;
        let current_root = open_root_without_symlinks(&view.canonical_root)?.metadata()?;
        if !same_stamp(&before, &after)
            || !same_stamp(&before, &current)
            || (current_root.dev(), current_root.ino()) != (view.root_device, view.root_inode)
        {
            return Err(WorkspaceError::InvalidPath);
        }
        check_capture_control(deadline, control)?;
        return Ok(bytes);
    }
    Err(WorkspaceError::InvalidPath)
}
impl AuthorizedSourceCapture {
    fn verify_blob_controlled(
        &self,
        blob: &str,
        expected_sha: &str,
        expected_bytes: u64,
        control: &crate::adoption::AdoptionControl<'_>,
    ) -> Result<(), WorkspaceError> {
        use std::os::{fd::AsRawFd, unix::process::CommandExt};
        if !is_oid(blob)
            || !crate::validation::valid_hash(expected_sha)
            || expected_bytes > MAX_SELECTED_BYTES
        {
            return Err(WorkspaceError::InvalidPath);
        }
        let control = control.narrowed(self.deadline);
        control.check()?;
        control.note_git_attempt();
        let child = self
            .command()
            .args(["cat-file", "blob", blob])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()?;
        let mut owned = crate::ControlledGitChild {
            child,
            reaped: false,
            signal_attempted: false,
        };
        let mut stdout = owned
            .child
            .stdout
            .take()
            .ok_or(WorkspaceError::ResourceLimit)?;
        let flags = unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            || unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) }
                < 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut stderr = crate::ControlledCapture::new(
            owned
                .child
                .stderr
                .take()
                .ok_or(WorkspaceError::ResourceLimit)?,
        )?;
        let mut hash = Sha256::new();
        let mut total = 0u64;
        let mut eof = false;
        let mut failure = None;
        loop {
            if let Err(error) = control.check() {
                failure = Some(error);
                break;
            }
            let mut progress = stderr.pump();
            if !eof {
                let mut chunk = [0u8; 65_536];
                let maximum = usize::try_from((expected_bytes + 1).saturating_sub(total))
                    .map_err(|_| WorkspaceError::ResourceLimit)?
                    .min(chunk.len());
                if maximum == 0 {
                    failure = Some(WorkspaceError::InvalidPath);
                    break;
                }
                match stdout.read(&mut chunk[..maximum]) {
                    Ok(0) => {
                        eof = true;
                        progress = true;
                    }
                    Ok(n) => {
                        total += n as u64;
                        hash.update(&chunk[..n]);
                        progress = true;
                        if total > expected_bytes {
                            failure = Some(WorkspaceError::InvalidPath);
                            break;
                        }
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) =>
                    {
                        ()
                    }
                    Err(error) => {
                        failure = Some(error.into());
                        break;
                    }
                }
            }
            if stderr.failed || stderr.truncated {
                failure = Some(WorkspaceError::ResourceLimit);
                break;
            }
            let finished = match owned.finished() {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => false,
                Err(error) => {
                    failure = Some(error.into());
                    break;
                }
            };
            if finished && eof && stderr.eof {
                break;
            }
            if !progress {
                std::thread::sleep(
                    Duration::from_millis(2)
                        .min(control.deadline().saturating_duration_since(Instant::now())),
                );
            }
        }
        owned.signal_own_group();
        let status = owned.reap()?;
        // No reader threads or renewed cleanup clock. Missing EOF stays unknown.
        while stderr.pump() {}
        control.note_git_observation(
            eof && stderr.eof && !stderr.failed && !stderr.truncated,
            Some(total),
            Some(stderr.bytes.len() as u64),
            failure.is_some(),
        );
        if let Some(error) = failure {
            return Err(error);
        }
        control.check()?;
        if !owned.reaped || !eof || !stderr.eof || stderr.failed || stderr.truncated {
            return Err(WorkspaceError::ResourceLimit);
        }
        if !status.success()
            || total != expected_bytes
            || format!("{:x}", hash.finalize()) != expected_sha
        {
            return Err(WorkspaceError::InvalidPath);
        }
        Ok(())
    }
}

struct Scratch {
    root: PathBuf,
    identity: fs::Metadata,
}
impl Scratch {
    fn create(capture: &AuthorizedSourceCapture) -> Result<Self, WorkspaceError> {
        let parent = fs::canonicalize(std::env::temp_dir())?;
        let _parent = open_root_without_symlinks(&parent)?;
        if parent.starts_with(&capture.root) || parent.starts_with(&capture.common_dir) {
            return Err(WorkspaceError::InvalidPath);
        }
        for _ in 0..16 {
            let name = format!(
                "agentbaton-source-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| WorkspaceError::InvalidPath)?
                    .as_nanos(),
                NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed)
            );
            let root = parent.join(name);
            match fs::DirBuilder::new().mode(0o700).create(&root) {
                Ok(()) => {
                    let identity = open_root_without_symlinks(&root)?.metadata()?;
                    return Ok(Self { root, identity });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }
        Err(WorkspaceError::ResourceLimit)
    }
    fn check(&self) -> Result<(), WorkspaceError> {
        if !same_identity(
            &open_root_without_symlinks(&self.root)?.metadata()?,
            &self.identity,
        ) {
            return Err(WorkspaceError::InvalidPath);
        }
        Ok(())
    }
}

/// Build only raw objects; failures may leave unreferenced objects and a private
/// scratch directory for host reconciliation. Never recursively removes files.
pub fn capture_source_commit(
    capture: AuthorizedSourceCapture,
) -> Result<CapturedSourceCommit, WorkspaceError> {
    capture_source_commit_inner(capture, None)
}
pub fn capture_source_commit_controlled(
    capture: AuthorizedSourceCapture,
    control: &crate::adoption::AdoptionControl<'_>,
) -> Result<CapturedSourceCommit, WorkspaceError> {
    capture_source_commit_inner(capture, Some(control))
}
fn capture_source_commit_inner(
    capture: AuthorizedSourceCapture,
    control: Option<&crate::adoption::AdoptionControl<'_>>,
) -> Result<CapturedSourceCommit, WorkspaceError> {
    match control {
        Some(control)
            if capture
                .host_deadline
                .is_some_and(|original| control.deadline() <= original)
                && capture
                    .host_epoch
                    .as_ref()
                    .is_some_and(|epoch| control.matches_epoch(epoch)) => {}
        None if capture.host_deadline.is_none() && capture.host_epoch.is_none() => {}
        _ => return Err(WorkspaceError::InvalidPath),
    }
    let deadline = capture.deadline;
    capture.recheck(control)?;
    check_tree_paths(&capture, &capture.base, control)?;
    check_capture_control(deadline, control)?;
    if let Some(control) = control {
        control.note_effects();
    }
    let scratch = Scratch::create(&capture)?;
    check_capture_control(deadline, control)?;
    let index = scratch.root.join("index");
    let mut selected = Vec::with_capacity(capture.selected.len());
    let mut total = 0u64;
    let mut blobs = Vec::new();
    for (i, a) in capture.selected.iter().enumerate() {
        check_capture_control(deadline, control)?;
        let v = a.contract_view();
        let remaining = MAX_SELECTED_BYTES
            .checked_sub(total)
            .ok_or(WorkspaceError::TooLarge)?;
        let mut narrowed = v.clone();
        narrowed.requirement.max_bytes = narrowed.requirement.max_bytes.min(remaining.max(1));
        let selected_bound = BoundArtifact::from_verified_contract(&narrowed)
            .map_err(|_| WorkspaceError::InvalidPath)?;
        let bytes = read_controlled_bound(&selected_bound, deadline, control)?;
        check_capture_control(deadline, control)?;
        total = total
            .checked_add(bytes.len() as u64)
            .ok_or(WorkspaceError::TooLarge)?;
        if total > MAX_SELECTED_BYTES {
            return Err(WorkspaceError::TooLarge);
        }
        let digest = controlled_hash(&bytes, deadline, control)?;
        if Some(digest.as_str()) != v.requirement.expected_sha256.as_deref() {
            return Err(WorkspaceError::InvalidPath);
        }
        check_capture_control(deadline, control)?;
        scratch.check()?;
        let raw = scratch.root.join(format!("blob-{i}"));
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&raw)?;
        if control.is_some() {
            for chunk in bytes.chunks(65_536) {
                check_capture_control(deadline, control)?;
                f.write_all(chunk)?;
                check_capture_control(deadline, control)?;
            }
        } else {
            f.write_all(&bytes)?;
        }
        check_capture_control(deadline, control)?;
        f.sync_all()?;
        check_capture_control(deadline, control)?;
        capture.recheck(control)?;
        let before = f.metadata()?;
        let blob = oid(text_output_controlled(
            deadline,
            capture
                .command()
                .args(["hash-object", "-w", "--no-filters", "--"])
                .arg(&raw),
            control,
        )?)?;
        capture.verify_blob(&blob, &digest, bytes.len() as u64, control)?;
        let raw_check = crate::validation::BoundArtifact::from_approved_root(
            &scratch.root,
            crate::validation::ArtifactRequirement {
                artifact_id: "private-raw".into(),
                relative_path: PathBuf::from(format!("blob-{i}")),
                max_bytes: MAX_SELECTED_BYTES,
                expected_sha256: Some(digest.clone()),
            },
        )
        .map_err(|_| WorkspaceError::InvalidPath)?;
        let raw_bytes = read_controlled_bound(&raw_check, deadline, control)?;
        check_capture_control(deadline, control)?;
        let after = fs::symlink_metadata(&raw)?;
        if !same_stamp(&before, &after)
            || after.nlink() != 1
            || after.mode() & 0o777 != 0o600
            || controlled_hash(&raw_bytes, deadline, control)? != digest
        {
            return Err(WorkspaceError::InvalidPath);
        }
        blobs.push((v, blob));
        selected.push(SelectedSourceDigest {
            artifact_id: a.contract_view().requirement.artifact_id,
            relative_path: a.contract_view().requirement.relative_path,
            sha256: digest,
            bytes: bytes.len() as u64,
            mode: String::new(),
        });
    }
    scratch.check()?;
    text_output_controlled(
        deadline,
        capture
            .command()
            .env("GIT_INDEX_FILE", &index)
            .args(["read-tree", capture.base.as_str()]),
        control,
    )?;
    // An index cannot represent every possible raw Git tree (for example an
    // explicit empty subtree). Refuse instead of silently dropping base data.
    let original_tree = oid(text_output_controlled(
        deadline,
        capture.command().args([
            "rev-parse",
            "--verify",
            &format!("{}^{{tree}}", capture.base),
        ]),
        control,
    )?)?;
    let roundtrip_tree = oid(text_output_controlled(
        deadline,
        capture
            .command()
            .env("GIT_INDEX_FILE", &index)
            .args(["write-tree"]),
        control,
    )?)?;
    if original_tree != roundtrip_tree {
        return Err(WorkspaceError::UnsafeRepository);
    }
    let mut expected_tree = Vec::with_capacity(selected.len());
    for ((v, blob), s) in blobs.iter().zip(selected.iter_mut()) {
        let path = &v.requirement.relative_path;
        let listing = text_output_controlled(
            deadline,
            capture
                .command()
                .args([
                    "--literal-pathspecs",
                    "ls-tree",
                    "-z",
                    capture.base.as_str(),
                    "--",
                ])
                .arg(path),
            control,
        )?;
        let (mode, base_mode, base_blob) = if listing.is_empty() {
            ("100644", "000000", "0".repeat(blob.len()))
        } else {
            let mut rows = listing.split(|b| *b == 0).filter(|r| !r.is_empty());
            let row = rows.next().ok_or(WorkspaceError::InvalidPath)?;
            if rows.next().is_some() {
                return Err(WorkspaceError::InvalidPath);
            }
            let tab = row
                .iter()
                .position(|b| *b == b'\t')
                .ok_or(WorkspaceError::InvalidPath)?;
            let header = &row[..tab];
            let found = &row[tab + 1..];
            if found != path.as_os_str().as_encoded_bytes() {
                return Err(WorkspaceError::InvalidPath);
            }
            let fields: Vec<_> = std::str::from_utf8(header)
                .map_err(|_| WorkspaceError::InvalidPath)?
                .split(' ')
                .collect();
            if fields.len() != 3
                || fields[1] != "blob"
                || !is_oid(fields[2])
                || fields[2].len() != blob.len()
            {
                return Err(WorkspaceError::NotRegularFile);
            }
            match fields[0] {
                "100644" => ("100644", "100644", fields[2].to_string()),
                "100755" => ("100755", "100755", fields[2].to_string()),
                _ => return Err(WorkspaceError::NotRegularFile),
            }
        };
        // --add cannot overwrite a parent tree/symlink/gitlink. Missing nested
        // paths below non-tree entries must fail, not remove those entries.
        for parent in path
            .ancestors()
            .skip(1)
            .filter(|p| !p.as_os_str().is_empty())
        {
            let entry = text_output_controlled(
                deadline,
                capture
                    .command()
                    .args([
                        "--literal-pathspecs",
                        "ls-tree",
                        "-z",
                        capture.base.as_str(),
                        "--",
                    ])
                    .arg(parent),
                control,
            )?;
            if !entry.is_empty() && !entry.starts_with(b"040000 tree ") {
                return Err(WorkspaceError::InvalidPath);
            }
        }
        scratch.check()?;
        text_output_controlled(
            deadline,
            capture
                .command()
                .env("GIT_INDEX_FILE", &index)
                .args(["update-index", "--add", "--cacheinfo", mode, blob.as_str()])
                .arg(path),
            control,
        )?;
        s.mode = mode.into();
        expected_tree.push(ExpectedTreeEntry {
            path: path.clone(),
            mode: mode.into(),
            blob: blob.clone(),
            base_mode: base_mode.into(),
            base_blob,
        });
    }
    let tree = oid(text_output_controlled(
        deadline,
        capture
            .command()
            .env("GIT_INDEX_FILE", &index)
            .args(["write-tree"]),
        control,
    )?)?;
    // Re-read all approved bytes after object creation. A prior successful read
    // or serialized contract is not fresh evidence after replacement/drift.
    for (a, s) in capture.selected.iter().zip(&selected) {
        check_capture_control(deadline, control)?;
        let bytes = read_controlled_bound(a, deadline, control)?;
        check_capture_control(deadline, control)?;
        if bytes.len() as u64 != s.bytes || controlled_hash(&bytes, deadline, control)? != s.sha256
        {
            return Err(WorkspaceError::InvalidPath);
        }
    }
    capture.recheck(control)?;
    capture.verify_tree(&tree, &expected_tree, control)?;
    let date = format!("@{} +0000", capture.commit_time);
    let commit = oid(text_output_controlled(
        deadline,
        capture
            .command()
            .env("GIT_AUTHOR_NAME", "AgentBaton")
            .env("GIT_AUTHOR_EMAIL", "development@agentbaton.invalid")
            .env("GIT_COMMITTER_NAME", "AgentBaton")
            .env("GIT_COMMITTER_EMAIL", "development@agentbaton.invalid")
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .args([
                "commit-tree",
                tree.as_str(),
                "-p",
                capture.base.as_str(),
                "-m",
                "AgentBaton accepted selected source",
            ]),
        control,
    )?)?;
    capture.recheck(control)?;
    let actual_tree = oid(text_output_controlled(
        deadline,
        capture
            .command()
            .args(["rev-parse", "--verify", &format!("{commit}^{{tree}}")]),
        control,
    )?)?;
    if actual_tree != tree {
        return Err(WorkspaceError::InvalidPath);
    }
    capture.verify_tree(&tree, &expected_tree, control)?;
    for (a, s) in capture.selected.iter().zip(&selected) {
        check_capture_control(deadline, control)?;
        let bytes = read_controlled_bound(a, deadline, control)?;
        check_capture_control(deadline, control)?;
        if bytes.len() as u64 != s.bytes || controlled_hash(&bytes, deadline, control)? != s.sha256
        {
            return Err(WorkspaceError::InvalidPath);
        }
    }
    for ((_, blob), selected) in blobs.iter().zip(&selected) {
        capture.verify_blob(blob, &selected.sha256, selected.bytes, control)?;
    }
    capture.verify_tree(&tree, &expected_tree, control)?;
    capture.recheck(control)?;
    scratch.check()?;
    check_capture_control(deadline, control)?;
    // No cleanup on failure, and no recursive cleanup even on success. The host
    // may retain/reconcile this private scratch; it contains no authority token.
    Ok(CapturedSourceCommit {
        commit,
        tree,
        base: capture.base,
        selection_digest: capture.selection_digest,
        selected,
    })
}

#[cfg(test)]
mod deadline_tests {
    use super::*;

    #[test]
    fn expired_private_capture_rejects_before_any_scratch_or_object_write() {
        // Real generated Git setup, then a test-local expired deadline. No
        // production knob or sleep can renew/shorten a host capability.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("--no-optional-locks")
                .arg("-C")
                .arg(&root)
                .args(args)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap();
            assert!(output.status.success());
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        };
        git(&["init", "-b", "source"]);
        git(&["config", "user.name", "Generated fixture"]);
        git(&["config", "user.email", "fixture@example.invalid"]);
        fs::write(root.join("file"), b"base\n").unwrap();
        git(&["add", "file"]);
        git(&["commit", "-m", "base"]);
        let workspace = crate::inspect(&root).unwrap();
        let base = git(&["rev-parse", "HEAD"]);
        let artifact = BoundArtifact::from_approved_root(
            &root,
            crate::validation::ArtifactRequirement {
                artifact_id: "file".into(),
                relative_path: "file".into(),
                max_bytes: 32,
                expected_sha256: Some(sha256(b"base\n")),
            },
        )
        .unwrap();
        let mut capture = AuthorizedSourceCapture::from_verified_selection(
            &workspace,
            base.clone(),
            vec![artifact],
            1_700_000_000,
            "selection".into(),
            "owner-policy".into(),
        )
        .unwrap();
        let index = fs::read(root.join(".git/index")).unwrap();
        let refs = git(&["show-ref"]);
        let objects = git(&["count-objects", "-v"]);
        capture.deadline = Instant::now() - Duration::from_millis(1);
        assert!(matches!(
            capture_source_commit(capture),
            Err(WorkspaceError::ResourceLimit)
        ));
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), index);
        assert_eq!(git(&["show-ref"]), refs);
        assert_eq!(git(&["count-objects", "-v"]), objects);
        assert_eq!(git(&["rev-parse", "HEAD"]), base);
        assert_eq!(fs::read(root.join("file")).unwrap(), b"base\n");
    }

    fn test_git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout)
            .unwrap()
            .trim_end_matches('\n')
            .to_owned()
    }
    fn verifier_fixture() -> (tempfile::TempDir, AuthorizedSourceCapture) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        test_git(&root, &["init", "-b", "source"]);
        test_git(&root, &["config", "user.name", "Generated fixture"]);
        test_git(&root, &["config", "user.email", "fixture@example.invalid"]);
        fs::write(root.join("file"), b"base\n").unwrap();
        test_git(&root, &["add", "file"]);
        test_git(&root, &["commit", "-m", "base"]);
        let workspace = crate::inspect(&root).unwrap();
        let base = test_git(&root, &["rev-parse", "HEAD"]);
        let item = BoundArtifact::from_approved_root(
            &root,
            crate::validation::ArtifactRequirement {
                artifact_id: "file".into(),
                relative_path: "file".into(),
                max_bytes: 128,
                expected_sha256: Some(sha256(b"base\n")),
            },
        )
        .unwrap();
        let capture = AuthorizedSourceCapture::from_verified_selection(
            &workspace,
            base,
            vec![item],
            1_700_000_000,
            "selection".into(),
            "owner-policy".into(),
        )
        .unwrap();
        (temp, capture)
    }
    #[test]
    fn actual_wrong_blob_is_rejected_after_mutable_same_inode_raw_bytes_are_restored() {
        let (_temp, capture) = verifier_fixture();
        let raw = capture.root.join("generated-raw");
        fs::write(&raw, b"approved\n").unwrap();
        let before = fs::metadata(&raw).unwrap();
        let good = test_git(
            &capture.root,
            &["hash-object", "-w", "--no-filters", raw.to_str().unwrap()],
        );
        capture
            .verify_blob(&good, &sha256(b"approved\n"), 9, None)
            .unwrap();
        fs::write(&raw, b"tampered\n").unwrap();
        let wrong = test_git(
            &capture.root,
            &["hash-object", "-w", "--no-filters", raw.to_str().unwrap()],
        );
        fs::write(&raw, b"approved\n").unwrap();
        let after = fs::metadata(&raw).unwrap();
        assert!(same_identity(&before, &after));
        assert_eq!(before.mode(), after.mode());
        assert_eq!(before.nlink(), after.nlink());
        assert_eq!(sha256(&fs::read(&raw).unwrap()), sha256(b"approved\n"));
        assert_ne!(good, wrong);
        assert!(capture
            .verify_blob(&wrong, &sha256(b"approved\n"), 9, None)
            .is_err());
        assert_eq!(
            test_git(&capture.root, &["rev-parse", "HEAD"]),
            capture.base
        );
    }
    #[test]
    fn actual_blob_stream_rejects_oversize_short_hash_and_expired_deadline() {
        let (_temp, mut capture) = verifier_fixture();
        let raw = capture.root.join("generated-raw");
        fs::write(&raw, b"approved\n").unwrap();
        let good = test_git(
            &capture.root,
            &["hash-object", "-w", "--no-filters", raw.to_str().unwrap()],
        );
        let hash = sha256(b"approved\n");
        capture.verify_blob(&good, &hash, 9, None).unwrap();
        assert!(capture.verify_blob(&good, &hash, 8, None).is_err());
        assert!(capture.verify_blob(&good, &hash, 10, None).is_err());
        assert!(capture
            .verify_blob(&good, &sha256(b"foreign"), 9, None)
            .is_err());
        assert!(capture
            .verify_blob(&good, &hash, MAX_SELECTED_BYTES + 1, None)
            .is_err());
        assert!(capture
            .verify_blob("-not-an-object", &hash, 9, None)
            .is_err());
        capture.deadline = Instant::now() - Duration::from_millis(1);
        assert!(matches!(
            capture.verify_blob(&good, &hash, 9, None),
            Err(WorkspaceError::ResourceLimit)
        ));
    }
    #[test]
    fn private_index_foreign_omitted_mode_changed_and_deleted_entries_are_rejected() {
        let (_temp, capture) = verifier_fixture();
        let raw = capture.root.join("generated-raw");
        fs::write(&raw, b"approved\n").unwrap();
        let blob = test_git(
            &capture.root,
            &["hash-object", "-w", "--no-filters", raw.to_str().unwrap()],
        );
        let old = test_git(
            &capture.root,
            &["rev-parse", &format!("{}:file", capture.base)],
        );
        let expected = [ExpectedTreeEntry {
            path: "file".into(),
            mode: "100644".into(),
            blob: blob.clone(),
            base_mode: "100644".into(),
            base_blob: old.clone(),
        }];
        let private = tempfile::tempdir().unwrap();
        let index = private.path().join("private-index");
        let original_index = fs::read(capture.root.join(".git/index")).unwrap();
        let refs = test_git(&capture.root, &["show-ref"]);
        text_output(
            capture.deadline,
            capture
                .command()
                .env("GIT_INDEX_FILE", &index)
                .args(["read-tree", capture.base.as_str()]),
        )
        .unwrap();
        let tree = oid(text_output(
            capture.deadline,
            capture
                .command()
                .env("GIT_INDEX_FILE", &index)
                .args(["write-tree"]),
        )
        .unwrap())
        .unwrap();
        assert!(capture.verify_tree(&tree, &expected, None).is_err()); // omitted selected change
        text_output(
            capture.deadline,
            capture.command().env("GIT_INDEX_FILE", &index).args([
                "update-index",
                "--add",
                "--cacheinfo",
                "100644",
                blob.as_str(),
                "file",
            ]),
        )
        .unwrap();
        let tree = oid(text_output(
            capture.deadline,
            capture
                .command()
                .env("GIT_INDEX_FILE", &index)
                .args(["write-tree"]),
        )
        .unwrap())
        .unwrap();
        capture.verify_tree(&tree, &expected, None).unwrap();
        text_output(
            capture.deadline,
            capture.command().env("GIT_INDEX_FILE", &index).args([
                "update-index",
                "--add",
                "--cacheinfo",
                "100644",
                blob.as_str(),
                "foreign-unselected",
            ]),
        )
        .unwrap();
        let tree = oid(text_output(
            capture.deadline,
            capture
                .command()
                .env("GIT_INDEX_FILE", &index)
                .args(["write-tree"]),
        )
        .unwrap())
        .unwrap();
        assert!(capture.verify_tree(&tree, &expected, None).is_err());
        text_output(
            capture.deadline,
            capture.command().env("GIT_INDEX_FILE", &index).args([
                "update-index",
                "--force-remove",
                "foreign-unselected",
            ]),
        )
        .unwrap();
        text_output(
            capture.deadline,
            capture.command().env("GIT_INDEX_FILE", &index).args([
                "update-index",
                "--cacheinfo",
                "100755",
                blob.as_str(),
                "file",
            ]),
        )
        .unwrap();
        let tree = oid(text_output(
            capture.deadline,
            capture
                .command()
                .env("GIT_INDEX_FILE", &index)
                .args(["write-tree"]),
        )
        .unwrap())
        .unwrap();
        assert!(capture.verify_tree(&tree, &expected, None).is_err());
        text_output(
            capture.deadline,
            capture.command().env("GIT_INDEX_FILE", &index).args([
                "update-index",
                "--force-remove",
                "file",
            ]),
        )
        .unwrap();
        let tree = oid(text_output(
            capture.deadline,
            capture
                .command()
                .env("GIT_INDEX_FILE", &index)
                .args(["write-tree"]),
        )
        .unwrap())
        .unwrap();
        assert!(capture.verify_tree(&tree, &expected, None).is_err());
        // A structurally extra empty tree has no recursive leaf diff. Its
        // selected file is otherwise exact, so only full tree audit rejects it.
        let empty = test_git(&capture.root, &["mktree"]);
        let mut raw_tree = Vec::new();
        for (header, object) in [
            (b"40000 empty\0".as_slice(), empty.as_str()),
            (b"100644 file\0".as_slice(), blob.as_str()),
        ] {
            raw_tree.extend_from_slice(header);
            for pair in object.as_bytes().as_chunks::<2>().0 {
                raw_tree.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
            }
        }
        fs::write(capture.root.join("generated-empty-tree"), raw_tree).unwrap();
        let extra_empty = test_git(
            &capture.root,
            &[
                "hash-object",
                "-w",
                "--no-filters",
                "-t",
                "tree",
                "generated-empty-tree",
            ],
        );
        assert!(matches!(
            capture.verify_tree(&extra_empty, &expected, None),
            Err(WorkspaceError::UnsafeRepository)
        ));
        assert_eq!(
            fs::read(capture.root.join(".git/index")).unwrap(),
            original_index
        );
        assert_eq!(test_git(&capture.root, &["show-ref"]), refs);
        assert_eq!(fs::read(capture.root.join("file")).unwrap(), b"base\n");
    }

    #[test]
    fn configured_submodule_ignore_cannot_hide_foreign_private_index_gitlink_change() {
        // This locks the fixed verifier contract; it is not a claim that an old
        // diff-tree plumbing invocation actually obeyed porcelain config.
        let (_temp, previous) = verifier_fixture();
        let root = previous.root.clone();
        test_git(
            &root,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                "160000",
                previous.base.as_str(),
                "unselected-submodule",
            ],
        );
        test_git(&root, &["commit", "-m", "base with unselected gitlink"]);
        let base = test_git(&root, &["rev-parse", "HEAD"]);
        test_git(&root, &["config", "diff.ignoreSubmodules", "all"]);
        let workspace = crate::inspect(&root).unwrap();
        let item = BoundArtifact::from_approved_root(
            &root,
            crate::validation::ArtifactRequirement {
                artifact_id: "file".into(),
                relative_path: "file".into(),
                max_bytes: 128,
                expected_sha256: Some(sha256(b"base\n")),
            },
        )
        .unwrap();
        let capture = AuthorizedSourceCapture::from_verified_selection(
            &workspace,
            base.clone(),
            vec![item],
            1_700_000_000,
            "selection".into(),
            "owner-policy".into(),
        )
        .unwrap();
        let raw = root.join("generated-raw");
        fs::write(&raw, b"approved\n").unwrap();
        let blob = test_git(
            &root,
            &["hash-object", "-w", "--no-filters", raw.to_str().unwrap()],
        );
        let old = test_git(&root, &["rev-parse", &format!("{base}:file")]);
        let expected = [ExpectedTreeEntry {
            path: "file".into(),
            mode: "100644".into(),
            blob: blob.clone(),
            base_mode: "100644".into(),
            base_blob: old,
        }];
        let private = tempfile::tempdir().unwrap();
        let index = private.path().join("private-index");
        let original_index = fs::read(root.join(".git/index")).unwrap();
        let refs = test_git(&root, &["show-ref"]);
        text_output(
            capture.deadline,
            capture
                .command()
                .env("GIT_INDEX_FILE", &index)
                .args(["read-tree", base.as_str()]),
        )
        .unwrap();
        text_output(
            capture.deadline,
            capture.command().env("GIT_INDEX_FILE", &index).args([
                "update-index",
                "--cacheinfo",
                "100644",
                blob.as_str(),
                "file",
            ]),
        )
        .unwrap();
        let preserved = oid(text_output(
            capture.deadline,
            capture
                .command()
                .env("GIT_INDEX_FILE", &index)
                .args(["write-tree"]),
        )
        .unwrap())
        .unwrap();
        capture.verify_tree(&preserved, &expected, None).unwrap();
        assert_eq!(
            test_git(
                &root,
                &["rev-parse", &format!("{preserved}:unselected-submodule")]
            ),
            previous.base
        );
        text_output(
            capture.deadline,
            capture.command().env("GIT_INDEX_FILE", &index).args([
                "update-index",
                "--cacheinfo",
                "160000",
                base.as_str(),
                "unselected-submodule",
            ]),
        )
        .unwrap();
        let tampered = oid(text_output(
            capture.deadline,
            capture
                .command()
                .env("GIT_INDEX_FILE", &index)
                .args(["write-tree"]),
        )
        .unwrap())
        .unwrap();
        assert!(capture.verify_tree(&tampered, &expected, None).is_err());
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), original_index);
        assert_eq!(test_git(&root, &["show-ref"]), refs);
        assert_eq!(test_git(&root, &["config", "diff.ignoreSubmodules"]), "all");
    }
}
