//! Non-default debug evidence delay, never preview/read/copy authority.
//! No descriptor path is opened. Only a host-validated contract can match it.
use crate::{data_workspaces::data_error, CoreError, Result};
use agentbaton_workspaces::{
    data_workspace::{DataLimits, DataSelectionContract, DataWorkspaceError},
    validation::ArtifactRequirement,
};
use rustix::fs::{openat, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, Metadata},
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
const CONFIG: &str = ".agentbaton-preview-evidence-v1.json";
const READY: &str = ".agentbaton-preview-evidence-ready-v1.json";
const MARKER: &str = "agentbaton_preview_evidence_gate_v1";
const MAX_CONFIG: u64 = 8192;
const MAX_SELECTION_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    canonical_root: PathBuf,
    device: String,
    inode: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    schema_version: u32,
    nonce: String,
    source: Source,
    files: Vec<ArtifactRequirement>,
    limits: DataLimits,
}
#[derive(Serialize)]
struct Ready<'a> {
    schema_version: u32,
    gate: &'static str,
    stage: &'static str,
    nonce: &'a str,
    descriptor_sha256: &'a str,
    preview_id: &'a str,
    project_id: &'a str,
}
pub(super) struct PreviewEvidenceGate {
    descriptor: Descriptor,
    digest: String,
    parent: File,
    path: PathBuf,
    used: AtomicBool,
}
fn refused() -> CoreError {
    CoreError::new(
        "CONFIG_CONFLICT",
        "Private preview evidence fixture is invalid; no evidence delay was enabled",
    )
}
fn valid_decimal(value: &str) -> bool {
    value.parse::<u64>().is_ok_and(|n| n.to_string() == value)
}
fn canonical_shape(path: &Path) -> bool {
    path.to_str().is_some_and(|v| {
        v.starts_with('/')
            && !v.contains('\0')
            && (v == "/"
                || v[1..]
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != ".."))
    })
}
fn valid_descriptor(d: &Descriptor) -> bool {
    d.schema_version == 1
        && (32..=64).contains(&d.nonce.len())
        && d.nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && canonical_shape(&d.source.canonical_root)
        && valid_decimal(&d.source.device)
        && valid_decimal(&d.source.inode)
        && !d.files.is_empty()
        && d.files.len() <= 4
        && d.limits.max_files > 0
        && d.limits.max_files <= 4
        && d.files.len() <= d.limits.max_files as usize
        && d.limits.max_file_bytes > 0
        && d.limits.max_file_bytes <= MAX_SELECTION_BYTES
        && d.limits.max_total_bytes > 0
        && d.limits.max_total_bytes <= MAX_SELECTION_BYTES
        && d.files.iter().all(|f| {
            !f.artifact_id.is_empty()
                && f.artifact_id.len() <= 256
                && f.artifact_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._:-/".contains(&b))
                && f.relative_path.to_str().is_some_and(|v| {
                    !v.is_empty()
                        && !v.starts_with('/')
                        && !v.contains('\0')
                        && v.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
                })
                && f.max_bytes > 0
                && f.max_bytes <= d.limits.max_file_bytes
                && f.expected_sha256.as_ref().is_none_or(|h| {
                    h.len() == 64
                        && h.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
        })
        && d.files
            .windows(2)
            .all(|pair| pair[0].relative_path < pair[1].relative_path)
        && d.files
            .iter()
            .map(|f| &f.artifact_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == d.files.len()
}
fn private_file(m: &Metadata) -> bool {
    m.is_file()
        && m.nlink() == 1
        && m.uid() == rustix::process::geteuid().as_raw()
        && m.mode() & 0o7777 == 0o600
        && m.len() > 0
        && m.len() <= MAX_CONFIG
}
fn check_parent(path: &Path, parent: &File) -> Result<()> {
    let held = parent.metadata().map_err(|_| refused())?;
    let current = std::fs::symlink_metadata(path).map_err(|_| refused())?;
    if !held.is_dir()
        || !current.is_dir()
        || held.uid() != rustix::process::geteuid().as_raw()
        || current.uid() != held.uid()
        || held.mode() & 0o7777 != 0o700
        || current.mode() & 0o7777 != 0o700
        || (held.dev(), held.ino()) != (current.dev(), current.ino())
        || std::fs::canonicalize(path).ok().as_deref() != Some(path)
    {
        return Err(refused());
    }
    Ok(())
}
fn control(cancel: &AtomicBool, deadline: Instant) -> Result<()> {
    // Same precedence and result mapping as the existing CopyControl checkpoint.
    if Instant::now() >= deadline {
        return Err(data_error(DataWorkspaceError::DeadlineExceeded));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(data_error(DataWorkspaceError::CancelRequested));
    }
    Ok(())
}
impl PreviewEvidenceGate {
    /// Called once at worker-set construction. No env/IPC switch or hot reload.
    pub(super) fn load(path: &Path, parent: &File) -> Result<Option<Arc<Self>>> {
        check_parent(path, parent)?;
        let fd = match openat(
            parent,
            CONFIG,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(error) if error == rustix::io::Errno::NOENT => return Ok(None),
            Err(_) => return Err(refused()),
        };
        let mut file: File = fd.into();
        let before = file.metadata().map_err(|_| refused())?;
        if !private_file(&before) {
            return Err(refused());
        }
        let mut raw = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_CONFIG + 1)
            .read_to_end(&mut raw)
            .map_err(|_| refused())?;
        let after = file.metadata().map_err(|_| refused())?;
        if !private_file(&after)
            || raw.len() as u64 != before.len()
            || (
                before.dev(),
                before.ino(),
                before.len(),
                before.mtime(),
                before.mtime_nsec(),
                before.ctime(),
                before.ctime_nsec(),
            ) != (
                after.dev(),
                after.ino(),
                after.len(),
                after.mtime(),
                after.mtime_nsec(),
                after.ctime(),
                after.ctime_nsec(),
            )
        {
            return Err(refused());
        }
        let descriptor: Descriptor = serde_json::from_slice(&raw).map_err(|_| refused())?;
        if !valid_descriptor(&descriptor) {
            return Err(refused());
        }
        check_parent(path, parent)?;
        Ok(Some(Arc::new(Self {
            descriptor,
            digest: agentbaton_workspaces::validation::sha256(&raw),
            parent: parent.try_clone().map_err(|_| refused())?,
            path: path.into(),
            used: AtomicBool::new(false),
        })))
    }
    fn matches(&self, contract: &DataSelectionContract) -> bool {
        let d = &self.descriptor;
        contract.format_version == 1
            && d.source.canonical_root == contract.source.canonical_root
            && d.source.device == contract.source.device.to_string()
            && d.source.inode == contract.source.inode.to_string()
            && d.files == contract.files
            && d.limits == contract.limits
    }
    pub(super) fn wait(
        &self,
        preview_id: &str,
        project_id: &str,
        contract: &DataSelectionContract,
        cancel: &AtomicBool,
        deadline: Instant,
    ) -> Result<()> {
        if !self.matches(contract) {
            return Ok(());
        }
        if self
            .used
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }
        control(cancel, deadline)?;
        check_parent(&self.path, &self.parent)?;
        if preview_id != contract.approval_reference
            || preview_id.is_empty()
            || preview_id.len() > 128
            || project_id.is_empty()
            || project_id.len() > 128
        {
            return Err(refused());
        }
        let ready = Ready {
            schema_version: 1,
            gate: MARKER,
            stage: "before_scan",
            nonce: &self.descriptor.nonce,
            descriptor_sha256: &self.digest,
            preview_id,
            project_id,
        };
        let bytes = serde_json::to_vec(&ready).map_err(|_| refused())?;
        let fd = openat(
            &self.parent,
            READY,
            OFlags::WRONLY
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK
                | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|_| refused())?;
        let mut marker: File = fd.into();
        // A restrictive umask must not create ambiguous handshake permissions.
        use std::os::unix::fs::PermissionsExt;
        marker
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|_| refused())?;
        let info = marker.metadata().map_err(|_| refused())?;
        if !info.is_file()
            || info.nlink() != 1
            || info.uid() != rustix::process::geteuid().as_raw()
            || info.mode() & 0o7777 != 0o600
        {
            return Err(refused());
        }
        marker
            .write_all(&bytes)
            .and_then(|()| marker.sync_all())
            .map_err(|_| refused())?;
        // Never return success for a matching gate. Only original cancellation
        // or original timeout can release this worker, then its real join path.
        loop {
            control(cancel, deadline)?;
            std::thread::sleep(
                Duration::from_millis(2).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    }
}

#[cfg(test)]
#[path = "preview_evidence_gate_tests.rs"]
mod tests;
