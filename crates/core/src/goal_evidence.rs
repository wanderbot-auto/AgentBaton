//! Graph-origin artifact evidence. Serializable views never grant authority;
//! only a private prepared worker may create a newly captured observation.
use crate::{CoreError, Result};
use agentbaton_workspaces::{
    data_workspace::FilePin, validation::ArtifactRequirement, WorkspaceIdentity, WorkspaceKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub const GRAPH_CONTRACT_VERSION: u32 = 1;
pub const MAX_GRAPH_REVIEW_BYTES: usize = 1_048_576;
pub const MAX_GRAPH_IO_BYTES: u64 = 16_777_216;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GraphArtifactReference {
    pub candidate_id: String,
    pub artifact_id: String,
    pub contract_id: String,
    pub producer_node_id: String,
    pub producer_attempt_id: String,
    pub sha256: String,
    pub bytes: u64,
    pub relative_path: PathBuf,
    pub root: WorkspaceIdentity,
    pub file_pin: FilePin,
    pub writer_epoch: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GraphInputBinding {
    pub artifact_id: String,
    pub contract_id: String,
    pub sha256: String,
    pub candidate: Option<GraphArtifactReference>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GraphCandidateReceipt {
    pub id: String,
    pub graph_contract_version: u32,
    pub run_id: String,
    pub scope_id: String,
    pub plan_revision: u64,
    pub plan_digest: String,
    pub node_id: String,
    pub attempt_id: String,
    pub session_id: String,
    pub task_id: String,
    pub spec_revision: u64,
    pub input_revision: String,
    pub workspace_id: String,
    pub capability_digest: String,
    pub consumed_inputs: Vec<GraphInputBinding>,
    pub artifacts: Vec<GraphArtifactReference>,
    pub stop_receipt_sequence: u64,
    pub capture_job_id: String,
    pub capture_receipt_sequence: u64,
    pub created_at: String,
    pub source: String,
    pub integrity: String,
    pub task_acceptance: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GraphValidationReceipt {
    pub id: String,
    pub graph_contract_version: u32,
    pub run_id: String,
    pub plan_revision: u64,
    pub plan_digest: String,
    pub node_id: String,
    pub attempt_id: String,
    pub rule_id: String,
    pub definition_digest: String,
    pub candidate_refs: Vec<GraphArtifactReference>,
    pub input_bindings: Vec<GraphInputBinding>,
    pub state: String,
    pub passed: bool,
    pub scope: String,
    pub correctness: String,
    pub task_acceptance: String,
    pub result_digest: String,
    pub details_ref: String,
    pub stop_confirmed: bool,
    pub io_completion: String,
    pub actual_runtime_ms: Option<u64>,
    pub created_at: String,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GraphOwnerDecision {
    pub id: String,
    pub graph_contract_version: u32,
    pub approval_id: String,
    pub run_id: String,
    pub plan_revision: u64,
    pub plan_digest: String,
    pub node_id: String,
    pub rule_id: String,
    pub criterion_digest: String,
    pub candidate_refs: Vec<GraphArtifactReference>,
    pub validation_ids: Vec<String>,
    pub decision: String,
    pub created_at: String,
    pub source: String,
}

/// Constructed only by the authoritative actor after exact owned-stop proof.
/// It is deliberately not Deserialize or public wire authority.
pub(crate) struct PreparedCandidateCapture {
    pub(crate) root: WorkspaceIdentity,
    pub(crate) inputs: Vec<ArtifactRequirement>,
    pub(crate) outputs: Vec<(String, ArtifactRequirement)>,
    pub(crate) cancelled: Arc<AtomicBool>,
    pub(crate) deadline: Instant,
}
pub(crate) struct CapturedFile {
    pub(crate) artifact: ArtifactRequirement,
    pub(crate) sha256: String,
    pub(crate) bytes: u64,
    pub(crate) pin: FilePin,
}
pub(crate) struct CapturedCandidate {
    pub(crate) root: WorkspaceIdentity,
    pub(crate) inputs: Vec<CapturedFile>,
    pub(crate) outputs: Vec<(String, CapturedFile)>,
    pub(crate) elapsed: Duration,
}
fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Candidate root, file identity, bytes or version changed",
    )
}
fn limited() -> CoreError {
    CoreError::new(
        "RESOURCE_LIMIT",
        "Graph evidence exceeds existing bounded artifact limits",
    )
}
fn pin(m: &std::fs::Metadata) -> FilePin {
    FilePin {
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
impl PreparedCandidateCapture {
    fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(CoreError::new(
                "CANCEL_REQUESTED",
                "Graph evidence cancellation was requested",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(CoreError::new(
                "TIMEOUT",
                "Graph evidence worker exceeded its approved time reservation",
            ));
        }
        Ok(())
    }
    fn root_fd(&self) -> Result<File> {
        use rustix::fs::{open, openat, Mode, OFlags};
        if !self.root.canonical_root.is_absolute() {
            return Err(changed());
        }
        let flags = OFlags::RDONLY
            | OFlags::DIRECTORY
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK
            | OFlags::CLOEXEC;
        let mut fd: File = open("/", flags, Mode::empty())
            .map_err(|_| changed())?
            .into();
        for part in self.root.canonical_root.components() {
            self.check()?;
            match part {
                Component::RootDir => {}
                Component::Normal(name) => {
                    fd = openat(&fd, name, flags, Mode::empty())
                        .map_err(|_| changed())?
                        .into()
                }
                _ => return Err(changed()),
            }
        }
        let m = fd.metadata().map_err(|_| changed())?;
        if !m.is_dir() || (m.dev(), m.ino()) != (self.root.device, self.root.inode) {
            return Err(changed());
        }
        if matches!(self.root.kind, WorkspaceKind::Directory)
            && (m.mode() & 0o7777 != 0o700 || m.uid() != rustix::process::geteuid().as_raw())
        {
            return Err(changed());
        }
        Ok(fd)
    }
    fn leaf(&self, root: &File, path: &Path) -> Result<File> {
        use rustix::fs::{openat, Mode, OFlags};
        if path.as_os_str().is_empty() || path.as_os_str().len() > 4096 {
            return Err(changed());
        }
        let parts: Vec<_> = path.components().collect();
        let mut fd = root.try_clone().map_err(|_| changed())?;
        for (i, part) in parts.iter().enumerate() {
            self.check()?;
            let Component::Normal(name) = part else {
                return Err(changed());
            };
            let mut flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
            if i + 1 < parts.len() {
                flags |= OFlags::DIRECTORY;
            }
            fd = openat(&fd, *name, flags, Mode::empty())
                .map_err(|_| changed())?
                .into();
        }
        Ok(fd)
    }
    fn capture(
        &self,
        root: &File,
        artifact: &ArtifactRequirement,
        remaining: &mut u64,
    ) -> Result<CapturedFile> {
        self.check()?;
        let file = self.leaf(root, &artifact.relative_path)?;
        let before = file.metadata().map_err(|_| changed())?;
        if !before.is_file() || before.nlink() != 1 {
            return Err(changed());
        }
        if before.len() > artifact.max_bytes || before.len() > *remaining {
            return Err(limited());
        }
        let before_pin = pin(&before);
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut reader = &file;
        let mut chunk = [0u8; 65536];
        loop {
            self.check()?;
            let count = reader.read(&mut chunk).map_err(|_| changed())?;
            if count == 0 {
                break;
            }
            bytes = bytes.checked_add(count as u64).ok_or_else(limited)?;
            if bytes > artifact.max_bytes || bytes > *remaining {
                return Err(limited());
            }
            hash.update(&chunk[..count]);
        }
        self.check()?;
        let held = file.metadata().map_err(|_| changed())?;
        let current = self
            .leaf(root, &artifact.relative_path)?
            .metadata()
            .map_err(|_| changed())?;
        if before_pin != pin(&held)
            || before_pin != pin(&current)
            || held.nlink() != 1
            || current.nlink() != 1
            || bytes != held.len()
        {
            return Err(changed());
        }
        let sha256 = format!("{:x}", hash.finalize());
        if artifact
            .expected_sha256
            .as_ref()
            .is_some_and(|expected| expected != &sha256)
        {
            return Err(changed());
        }
        *remaining = remaining.checked_sub(bytes).ok_or_else(limited)?;
        Ok(CapturedFile {
            artifact: artifact.clone(),
            sha256,
            bytes,
            pin: before_pin,
        })
    }
    pub(crate) fn execute(self) -> Result<CapturedCandidate> {
        self.check()?;
        if self.inputs.len() > 64 || self.outputs.len() > 64 {
            return Err(limited());
        }
        let start = Instant::now();
        let root = self.root_fd()?;
        let mut remaining = MAX_GRAPH_IO_BYTES;
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for input in &self.inputs {
            inputs.push(self.capture(&root, input, &mut remaining)?);
        }
        for (contract, output) in &self.outputs {
            outputs.push((
                contract.clone(),
                self.capture(&root, output, &mut remaining)?,
            ));
        }
        // Recheck original paths/pins after every read and the current root path;
        // a byte-identical replacement is not the original producer artifact.
        for observed in inputs.iter().chain(outputs.iter().map(|(_, file)| file)) {
            self.check()?;
            let current = self
                .leaf(&root, &observed.artifact.relative_path)?
                .metadata()
                .map_err(|_| changed())?;
            if current.nlink() != 1 || pin(&current) != observed.pin {
                return Err(changed());
            }
        }
        let _current_root = self.root_fd()?;
        if matches!(self.root.kind, WorkspaceKind::Git { .. })
            && !crate::policy::live_workspace_matches(&self.root.canonical_root, &self.root)
                .unwrap_or(false)
        {
            return Err(changed());
        }
        self.check()?;
        Ok(CapturedCandidate {
            root: self.root,
            inputs,
            outputs,
            elapsed: start.elapsed(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    fn fixture() -> (tempfile::TempDir, PreparedCandidateCapture) {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().canonicalize().unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(root.join("input"), b"fixed input").unwrap();
        std::fs::write(root.join("report"), b"actual report").unwrap();
        let m = std::fs::metadata(&root).unwrap();
        let plan = PreparedCandidateCapture {
            root: WorkspaceIdentity {
                canonical_root: root,
                device: m.dev(),
                inode: m.ino(),
                kind: WorkspaceKind::Directory,
            },
            inputs: vec![ArtifactRequirement {
                artifact_id: "input".into(),
                relative_path: "input".into(),
                max_bytes: 1024,
                expected_sha256: Some(format!("{:x}", Sha256::digest(b"fixed input"))),
            }],
            outputs: vec![(
                "output:report".into(),
                ArtifactRequirement {
                    artifact_id: "report".into(),
                    relative_path: "report".into(),
                    max_bytes: 1024,
                    expected_sha256: None,
                },
            )],
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(5),
        };
        (d, plan)
    }
    #[test]
    fn real_pinned_worker_hashes_input_and_output_without_inventing_acceptance() {
        let (_d, plan) = fixture();
        let result = std::thread::spawn(move || plan.execute())
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(result.inputs[0].bytes, 11);
        assert_eq!(result.outputs[0].1.bytes, 13);
        assert_eq!(
            result.outputs[0].1.sha256,
            format!("{:x}", Sha256::digest(b"actual report"))
        );
        assert_eq!(result.outputs[0].1.pin.bytes, 13);
        assert_eq!(
            result.root.canonical_root,
            std::fs::canonicalize(&result.root.canonical_root).unwrap()
        );
        assert!(result.elapsed < Duration::from_secs(5));
    }
    #[test]
    fn candidate_worker_refuses_root_replacement_links_fifo_input_drift_and_caps() {
        for mode in [
            "root",
            "symlink",
            "hardlink",
            "fifo",
            "input",
            "total_cap",
            "traversal",
        ] {
            let (d, mut plan) = fixture();
            let root = plan.root.canonical_root.clone();
            match mode {
                "root" => {
                    let old = d.path().with_extension("held-root");
                    std::fs::rename(&root, &old).unwrap();
                    std::fs::create_dir(&root).unwrap();
                    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                        .unwrap();
                    std::fs::write(root.join("report"), b"foreign").unwrap();
                    let result = plan.execute();
                    assert_eq!(result.err().unwrap().code, "INPUT_CHANGED");
                    std::fs::remove_dir_all(old).unwrap();
                    continue;
                }
                "symlink" => {
                    std::fs::remove_file(root.join("report")).unwrap();
                    symlink("input", root.join("report")).unwrap();
                }
                "hardlink" => {
                    std::fs::remove_file(root.join("report")).unwrap();
                    std::fs::hard_link(root.join("input"), root.join("report")).unwrap();
                }
                "fifo" => {
                    std::fs::remove_file(root.join("report")).unwrap();
                    assert!(std::process::Command::new("/usr/bin/mkfifo")
                        .args(["-m", "600"])
                        .arg(root.join("report"))
                        .status()
                        .unwrap()
                        .success());
                }
                "input" => std::fs::write(root.join("input"), b"changed").unwrap(),
                "total_cap" => {
                    File::options()
                        .write(true)
                        .open(root.join("report"))
                        .unwrap()
                        .set_len(MAX_GRAPH_IO_BYTES + 1)
                        .unwrap();
                    plan.outputs[0].1.max_bytes = 67_108_864;
                }
                _ => plan.outputs[0].1.relative_path = "../report".into(),
            }
            let expected = if mode == "total_cap" {
                "RESOURCE_LIMIT"
            } else {
                "INPUT_CHANGED"
            };
            assert_eq!(
                std::thread::spawn(move || plan.execute())
                    .join()
                    .unwrap()
                    .err()
                    .unwrap()
                    .code,
                expected,
                "{mode}"
            );
        }
    }
    #[test]
    fn real_joined_worker_distinguishes_cancel_request_and_deadline_without_zero_metrics() {
        for cancelled in [true, false] {
            let (_d, mut plan) = fixture();
            if cancelled {
                plan.cancelled.store(true, Ordering::Release);
            } else {
                plan.deadline = Instant::now();
            }
            let result = std::thread::spawn(move || plan.execute()).join().unwrap();
            assert_eq!(
                result.err().unwrap().code,
                if cancelled {
                    "CANCEL_REQUESTED"
                } else {
                    "TIMEOUT"
                }
            );
        }
    }
}
