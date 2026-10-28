//! Private IO for an already-authorized graph validator. The actor authenticates
//! the capability, definition and producer subject before constructing this job.
//! This module neither grants permissions nor records acceptance or joined-stop.
use crate::{CoreError, Result};
use agentbaton_workspaces::{
    command_validation::{
        run_approved_command_with_deadline, ApprovedCommandRule, CommandMode, CommandRun,
        CommandSettings,
    },
    data_workspace::FilePin,
    deterministic::{validate_json_table, ApprovedJsonTableRule, JsonTableContract},
    validation::{
        validate_artifacts, AnalysisReproducibility, ApprovedRule, ArtifactContractView,
        ArtifactRequirement, BoundArtifact, ValidationSubject,
    },
    WorkspaceIdentity, WorkspaceKind,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Seek, SeekFrom},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

// Same existing graph review/IO bounds; these are not new policy permissions.
const MAX_IO_BYTES: u64 = 16_777_216;
const MAX_ARTIFACTS: usize = 64;

/// Native actor handoff only. In particular, this is deliberately not Deserialize.
pub(crate) struct PreparedGoalValidator {
    pub(crate) approval_id: String,
    pub(crate) definition_revision: String,
    pub(crate) subject: ValidationSubject,
    pub(crate) workspace: WorkspaceIdentity,
    pub(crate) inputs: Vec<ArtifactRequirement>,
    pub(crate) outputs: Vec<ArtifactRequirement>,
    pub(crate) candidate_pins: Vec<(ArtifactRequirement, FilePin)>,
    pub(crate) definition: GoalValidatorDefinition,
}

pub(crate) enum GoalValidatorDefinition {
    Integrity {
        analysis: Option<AnalysisReproducibility>,
    },
    Data {
        table: JsonTableContract,
    },
    Command {
        program_root: PathBuf,
        program_device: u64,
        program_inode: u64,
        program: ArtifactRequirement,
        /// Deferred approved stdin script, bound to the same managed workspace.
        /// The actor supplies NativeExecutable as the settings.mode placeholder.
        script: Option<ArtifactRequirement>,
        settings: CommandSettings,
    },
}

pub(crate) struct GoalValidatorObservation {
    /// Original library evidence plus a separate `goal_io` observation namespace.
    /// The library's `passed` and digest remain its original, narrower claims.
    pub(crate) view: Value,
    pub(crate) passed: bool,
    pub(crate) elapsed: Duration,
    pub(crate) scope: &'static str,
    pub(crate) correctness: &'static str,
    /// Always retained after command execution, including cancellation, stale
    /// artifacts, late completion and an unreaped original direct child.
    pub(crate) command: Option<CommandRun>,
}

fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Goal validator root, file identity or bytes changed",
    )
}
fn limited() -> CoreError {
    CoreError::new(
        "RESOURCE_LIMIT",
        "Goal validator exceeds bounded graph artifact IO",
    )
}
fn invalid() -> CoreError {
    CoreError::new(
        "INVALID_REQUEST",
        "Goal validator requires an exact normalized artifact contract",
    )
}
fn validation_error(_: agentbaton_workspaces::validation::ValidationError) -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Goal validator library rejected the fixed artifact contract",
    )
}

struct Guard<'a> {
    cancel: &'a AtomicBool,
    deadline: Instant,
}
impl Guard<'_> {
    fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Acquire) {
            return Err(CoreError::new(
                "CANCEL_REQUESTED",
                "Goal validator cancellation was observed",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Goal validator exceeded its runtime reservation",
            ));
        }
        Ok(())
    }
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

/// Keep the original descriptors alive across every library read. A byte-identical
/// path replacement cannot become evidence for the producer's original file.
struct Root {
    path: PathBuf,
    device: u64,
    inode: u64,
    fd: File,
}
impl Root {
    fn open(path: &Path, device: u64, inode: u64, guard: &Guard<'_>) -> Result<Self> {
        use rustix::fs::{open, openat, Mode, OFlags};
        guard.check()?;
        if !path.is_absolute() || path.as_os_str().len() > 4096 {
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
        for part in path.components() {
            guard.check()?;
            match part {
                Component::RootDir => (),
                Component::Normal(name) => {
                    fd = openat(&fd, name, flags, Mode::empty())
                        .map_err(|_| changed())?
                        .into();
                }
                _ => return Err(changed()),
            }
        }
        let m = fd.metadata().map_err(|_| changed())?;
        guard.check()?;
        if !m.is_dir() || (m.dev(), m.ino()) != (device, inode) {
            return Err(changed());
        }
        Ok(Self {
            path: path.into(),
            device,
            inode,
            fd,
        })
    }
    fn leaf(&self, path: &Path, guard: &Guard<'_>) -> Result<File> {
        use rustix::fs::{openat, Mode, OFlags};
        if path.as_os_str().is_empty() || path.as_os_str().len() > 4096 {
            return Err(changed());
        }
        let parts: Vec<_> = path.components().collect();
        let mut fd = self.fd.try_clone().map_err(|_| changed())?;
        for (i, part) in parts.iter().enumerate() {
            guard.check()?;
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
        guard.check()?;
        Ok(fd)
    }
    fn verify(&self, guard: &Guard<'_>) -> Result<()> {
        let _current = Self::open(&self.path, self.device, self.inode, guard)?;
        let m = self.fd.metadata().map_err(|_| changed())?;
        if !m.is_dir() || (m.dev(), m.ino()) != (self.device, self.inode) {
            return Err(changed());
        }
        guard.check()
    }
}

struct HeldArtifact {
    root: Root,
    file: File,
    requirement: ArtifactRequirement,
    pin: FilePin,
    requested_max_bytes: u64,
}
impl HeldArtifact {
    fn read_hash(&mut self, guard: &Guard<'_>) -> Result<()> {
        guard.check()?;
        let before = self.file.metadata().map_err(|_| changed())?;
        if !before.is_file() || before.nlink() != 1 || pin(&before) != self.pin {
            return Err(changed());
        }
        self.file.seek(SeekFrom::Start(0)).map_err(|_| changed())?;
        let mut hasher = Sha256::new();
        let mut bytes = 0u64;
        let mut chunk = [0u8; 65_536];
        loop {
            guard.check()?;
            let count = self.file.read(&mut chunk).map_err(|_| changed())?;
            guard.check()?;
            if count == 0 {
                break;
            }
            bytes = bytes.checked_add(count as u64).ok_or_else(limited)?;
            if bytes > self.requirement.max_bytes || bytes > self.pin.bytes {
                return Err(limited());
            }
            hasher.update(&chunk[..count]);
        }
        let after = self.file.metadata().map_err(|_| changed())?;
        let current = self
            .root
            .leaf(&self.requirement.relative_path, guard)?
            .metadata()
            .map_err(|_| changed())?;
        let observed_hash = format!("{:x}", hasher.finalize());
        if after.nlink() != 1
            || current.nlink() != 1
            || pin(&after) != self.pin
            || pin(&current) != self.pin
            || bytes != self.pin.bytes
            || self.requirement.expected_sha256.as_deref() != Some(observed_hash.as_str())
        {
            return Err(changed());
        }
        self.root.verify(guard)?;
        guard.check()
    }
}

struct ArtifactIo {
    files: Vec<HeldArtifact>,
    artifact_bytes: u64,
    locations: BTreeSet<PathBuf>,
    ids: BTreeSet<String>,
}
impl ArtifactIo {
    fn new() -> Self {
        Self {
            files: Vec::new(),
            artifact_bytes: 0,
            locations: BTreeSet::new(),
            ids: BTreeSet::new(),
        }
    }
    fn bind(
        &mut self,
        mut view: ArtifactContractView,
        expected_pin: Option<&FilePin>,
        guard: &Guard<'_>,
    ) -> Result<BoundArtifact> {
        guard.check()?;
        if self.files.len() >= MAX_ARTIFACTS {
            return Err(limited());
        }
        let requirement = &view.requirement;
        if requirement.max_bytes == 0
            || requirement.expected_sha256.is_none()
            || !self
                .locations
                .insert(view.canonical_root.join(&requirement.relative_path))
            || !self.ids.insert(requirement.artifact_id.clone())
        {
            return Err(invalid());
        }
        let root = Root::open(
            &view.canonical_root,
            view.root_device,
            view.root_inode,
            guard,
        )?;
        let file = root.leaf(&requirement.relative_path, guard)?;
        let m = file.metadata().map_err(|_| changed())?;
        if !m.is_file() || m.nlink() != 1 {
            return Err(changed());
        }
        if m.len() > requirement.max_bytes {
            return Err(limited());
        }
        self.artifact_bytes = self
            .artifact_bytes
            .checked_add(m.len())
            .ok_or_else(limited)?;
        if self.artifact_bytes > MAX_IO_BYTES {
            return Err(limited());
        }
        let observed_pin = pin(&m);
        if expected_pin.is_some_and(|expected| *expected != observed_pin) {
            return Err(changed());
        }
        let requested_max_bytes = requirement.max_bytes;
        // Narrow only this private execution binding after its FD size and pin
        // have been observed. Growing files cannot force an unbounded library
        // read; the owner-approved upper ceiling and digest remain unchanged.
        view.requirement.max_bytes = m.len().max(1);
        let mut held = HeldArtifact {
            root,
            file,
            requirement: view.requirement.clone(),
            pin: observed_pin,
            requested_max_bytes,
        };
        held.read_hash(guard)?;
        // Reconstruction is from the actor's authenticated definition, never a
        // wire contract. Our descriptor checks additionally forbid parent links.
        let bound = BoundArtifact::from_verified_contract(&view).map_err(validation_error)?;
        guard.check()?;
        self.files.push(held);
        Ok(bound)
    }
    fn verify(&mut self, workspace: &WorkspaceIdentity, guard: &Guard<'_>) -> Result<()> {
        guard.check()?;
        for file in &mut self.files {
            file.read_hash(guard)?;
        }
        let root = Root::open(
            &workspace.canonical_root,
            workspace.device,
            workspace.inode,
            guard,
        )?;
        let m = root.fd.metadata().map_err(|_| changed())?;
        if matches!(workspace.kind, WorkspaceKind::Directory)
            && (m.mode() & 0o7777 != 0o700 || m.uid() != rustix::process::geteuid().as_raw())
        {
            return Err(changed());
        }
        if matches!(workspace.kind, WorkspaceKind::Git { .. })
            && !crate::policy::live_workspace_matches(&workspace.canonical_root, workspace)
                .unwrap_or(false)
        {
            return Err(changed());
        }
        // Recheck all paths again after the final read, without treating a
        // prior regular-file check as authority for a later replacement.
        for file in &self.files {
            guard.check()?;
            let m = file
                .root
                .leaf(&file.requirement.relative_path, guard)?
                .metadata()
                .map_err(|_| changed())?;
            if m.nlink() != 1 || pin(&m) != file.pin {
                return Err(changed());
            }
            file.root.verify(guard)?;
        }
        root.verify(guard)?;
        guard.check()
    }
}

fn artifact_view(
    workspace: &WorkspaceIdentity,
    requirement: ArtifactRequirement,
) -> ArtifactContractView {
    ArtifactContractView {
        canonical_root: workspace.canonical_root.clone(),
        root_device: workspace.device,
        root_inode: workspace.inode,
        requirement,
    }
}

fn candidate_pin<'a>(
    requirement: &ArtifactRequirement,
    pins: &'a [(ArtifactRequirement, FilePin)],
) -> Result<Option<&'a FilePin>> {
    let mut matches = pins.iter().filter(|(artifact, _)| artifact == requirement);
    let found = matches.next().map(|(_, pin)| pin);
    if matches.next().is_some() {
        return Err(invalid());
    }
    Ok(found)
}

fn all_pins_bound(prepared: &PreparedGoalValidator) -> bool {
    prepared.candidate_pins.iter().all(|(artifact, _)| {
        prepared.inputs.iter().chain(&prepared.outputs).any(|bound| bound == artifact)
            || matches!(&prepared.definition, GoalValidatorDefinition::Command { script: Some(script), .. } if script == artifact)
    })
}

fn finish_view(mut view: Value, status: &Result<()>, io: &ArtifactIo) -> Value {
    // Additive context outside the library's digest domain. Never rewrite the
    // actual library result, even when the graph IO observation disqualifies it.
    view["goal_io"] = match status {
        Ok(()) => {
            json!({"state":"observed", "artifact_stability":"before_after_fd_pin_and_sha256", "task_acceptance":"not_established"})
        }
        Err(error) => {
            json!({"state":"observation_rejected", "error_code":error.code, "artifact_stability":"not_established", "task_acceptance":"not_established"})
        }
    };
    view["goal_io"]["artifact_bytes"] = json!(io.artifact_bytes);
    view["goal_io"]["execution_read_bounds"] = json!(io
        .files
        .iter()
        .map(|file| json!({
            "artifact_id":file.requirement.artifact_id,
            "requested_max_bytes":file.requested_max_bytes,
            "effective_max_bytes":file.requirement.max_bytes
        }))
        .collect::<Vec<_>>());
    view
}

impl PreparedGoalValidator {
    pub(crate) fn execute(
        self,
        cancel: &AtomicBool,
        deadline: Instant,
    ) -> Result<GoalValidatorObservation> {
        let start = Instant::now();
        let guard = Guard { cancel, deadline };
        guard.check()?;
        if self
            .inputs
            .len()
            .checked_add(self.outputs.len())
            .ok_or_else(limited)?
            > MAX_ARTIFACTS
            || self.candidate_pins.len() > MAX_ARTIFACTS
        {
            return Err(limited());
        }
        if !all_pins_bound(&self) {
            return Err(invalid());
        }
        // Outputs belong to an actual captured producer. A fresh hash alone
        // must never replace the producer's original file-identity witness.
        for output in &self.outputs {
            if candidate_pin(output, &self.candidate_pins)?.is_none() {
                return Err(invalid());
            }
        }
        let mut io = ArtifactIo::new();
        let mut bind = |requirement: &ArtifactRequirement| -> Result<BoundArtifact> {
            let expected = candidate_pin(requirement, &self.candidate_pins)?;
            io.bind(
                artifact_view(&self.workspace, requirement.clone()),
                expected,
                &guard,
            )
        };
        let inputs = self
            .inputs
            .iter()
            .map(&mut bind)
            .collect::<Result<Vec<_>>>()?;
        let outputs = self
            .outputs
            .iter()
            .map(&mut bind)
            .collect::<Result<Vec<_>>>()?;
        io.verify(&self.workspace, &guard)?;

        let (view, library_passed, scope, correctness, command) = match self.definition {
            GoalValidatorDefinition::Integrity { analysis } => {
                let rule = ApprovedRule::from_verified_policy(
                    self.approval_id,
                    self.definition_revision,
                    self.subject,
                    &inputs,
                    &outputs,
                    analysis.clone(),
                )
                .map_err(validation_error)?;
                guard.check()?;
                let evidence = validate_artifacts(&rule, inputs, outputs, analysis)
                    .map_err(validation_error)?;
                let view = evidence.view();
                (
                    json!(view),
                    view.passed,
                    "artifact_integrity_only",
                    "not_established",
                    None,
                )
            }
            GoalValidatorDefinition::Data { table } => {
                let rule = ApprovedJsonTableRule::from_verified_policy(
                    self.approval_id,
                    self.definition_revision,
                    self.subject,
                    inputs,
                    outputs,
                    table,
                )
                .map_err(validation_error)?;
                guard.check()?;
                let evidence = validate_json_table(&rule).map_err(validation_error)?;
                let view = evidence.view();
                (
                    json!(view),
                    view.passed,
                    "data_contract_only",
                    "approved_schema_ranges_metadata_only",
                    None,
                )
            }
            GoalValidatorDefinition::Command {
                program_root,
                program_device,
                program_inode,
                program,
                script,
                mut settings,
            } => {
                if !matches!(settings.mode, CommandMode::NativeExecutable) {
                    return Err(invalid());
                }
                let program = io.bind(
                    ArtifactContractView {
                        canonical_root: program_root,
                        root_device: program_device,
                        root_inode: program_inode,
                        requirement: program,
                    },
                    None,
                    &guard,
                )?;
                if let Some(script) = script {
                    let expected = candidate_pin(&script, &self.candidate_pins)?;
                    let script =
                        io.bind(artifact_view(&self.workspace, script), expected, &guard)?;
                    settings.mode = CommandMode::PosixShStdin { script };
                }
                let budget = Duration::from_millis(
                    settings
                        .wall_time_ms
                        .checked_add(settings.cleanup_time_ms)
                        .ok_or_else(limited)?,
                );
                let rule = ApprovedCommandRule::from_verified_policy(
                    self.approval_id,
                    self.definition_revision,
                    self.subject,
                    program,
                    self.workspace.clone(),
                    inputs,
                    outputs,
                    settings,
                )
                .map_err(crate::command_validation::command_error)?;
                io.verify(&self.workspace, &guard)?;
                guard.check()?;
                if deadline.saturating_duration_since(Instant::now()) < budget {
                    return Err(CoreError::new("DEADLINE_EXCEEDED", "Goal runtime reservation cannot fit the unchanged approved command and cleanup budgets"));
                }
                let run = run_approved_command_with_deadline(&rule, cancel, deadline)
                    .map_err(crate::command_validation::command_error)?;
                let view = run.view();
                // No fallible early return after this point: the manager must
                // receive ownership even if the original child remains unreaped.
                (
                    json!(view),
                    view.passed,
                    "approved_deterministic_command_only",
                    "approved_fixed_command_observations_only",
                    Some(run),
                )
            }
        };
        let mut status = io.verify(&self.workspace, &guard);
        let mut view = finish_view(view, &status, &io);
        // Serialization is bounded work too. A late cancel/deadline cannot
        // become a successful graph observation after the last IO check.
        if status.is_ok() {
            if let Err(error) = guard.check() {
                status = Err(error);
                view = finish_view(view, &status, &io);
            }
        }
        let passed = library_passed && status.is_ok();
        Ok(GoalValidatorObservation {
            view,
            passed,
            elapsed: start.elapsed(),
            scope,
            correctness,
            command,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentbaton_workspaces::{
        command_validation::RequiredEnforcement,
        deterministic::{Column, JsonLimits, ScalarType},
        validation::{sha256, AnalysisScope, VersionBinding},
    };
    use std::{
        collections::BTreeMap,
        fs,
        os::unix::fs::{symlink, PermissionsExt},
    };

    fn requirement(root: &Path, name: &str) -> ArtifactRequirement {
        ArtifactRequirement {
            artifact_id: name.into(),
            relative_path: name.into(),
            max_bytes: 4096,
            expected_sha256: Some(sha256(&fs::read(root.join(name)).unwrap())),
        }
    }
    fn fixture() -> (tempfile::TempDir, PreparedGoalValidator) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(root.join("input"), b"fixed input").unwrap();
        fs::write(root.join("report"), b"fixed report").unwrap();
        let m = fs::metadata(&root).unwrap();
        let input = requirement(&root, "input");
        let output = requirement(&root, "report");
        let output_pin = pin(&fs::metadata(root.join("report")).unwrap());
        let prepared = PreparedGoalValidator {
            approval_id: "graph-capability".into(),
            definition_revision: "definition-digest".into(),
            subject: ValidationSubject {
                task_id: "task".into(),
                attempt_id: "actual-producer".into(),
                spec_revision: 1,
                input_revision: "input-revision".into(),
                version: VersionBinding::WorkspaceArtifacts {
                    workspace_id: "workspace".into(),
                    base_revision: "revision".into(),
                },
            },
            workspace: WorkspaceIdentity {
                canonical_root: root,
                device: m.dev(),
                inode: m.ino(),
                kind: WorkspaceKind::Directory,
            },
            inputs: vec![input],
            outputs: vec![output.clone()],
            candidate_pins: vec![(output, output_pin)],
            definition: GoalValidatorDefinition::Integrity { analysis: None },
        };
        (directory, prepared)
    }
    fn execute(prepared: PreparedGoalValidator) -> Result<GoalValidatorObservation> {
        prepared.execute(
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(5),
        )
    }
    fn rejection(prepared: PreparedGoalValidator) -> String {
        match execute(prepared) {
            Err(error) => error.code,
            Ok(_) => panic!("unexpected completed observation"),
        }
    }

    #[test]
    fn real_integrity_observation_keeps_actual_subject_and_narrow_scope() {
        let (_directory, prepared) = fixture();
        let observed = execute(prepared).unwrap();
        assert!(observed.passed);
        assert_eq!(observed.scope, "artifact_integrity_only");
        assert_eq!(observed.correctness, "not_established");
        assert_eq!(observed.view["subject"]["attempt_id"], "actual-producer");
        assert_eq!(observed.view["approval_id"], "graph-capability");
        assert_eq!(observed.view["goal_io"]["state"], "observed");
        assert!(observed.command.is_none());
    }

    #[test]
    fn wide_approved_ceilings_are_narrowed_to_real_fd_bytes() {
        let (_directory, mut prepared) = fixture();
        prepared.inputs[0].max_bytes = MAX_IO_BYTES;
        prepared.outputs[0].max_bytes = MAX_IO_BYTES;
        prepared.candidate_pins[0].0.max_bytes = MAX_IO_BYTES;
        let observed = execute(prepared).unwrap();
        assert!(observed.passed);
        assert_eq!(observed.view["goal_io"]["artifact_bytes"], 23);
        assert_eq!(
            observed.view["goal_io"]["execution_read_bounds"][0]["requested_max_bytes"],
            MAX_IO_BYTES
        );
        assert_eq!(
            observed.view["goal_io"]["execution_read_bounds"][0]["effective_max_bytes"],
            11
        );
    }

    #[test]
    fn identical_replacement_digest_drift_and_foreign_pins_refuse() {
        let (_directory, prepared) = fixture();
        let root = &prepared.workspace.canonical_root;
        fs::rename(root.join("report"), root.join("old-report")).unwrap();
        fs::write(root.join("report"), b"fixed report").unwrap();
        assert_eq!(rejection(prepared), "INPUT_CHANGED");
        let (_directory, prepared) = fixture();
        fs::write(
            prepared.workspace.canonical_root.join("input"),
            b"changed input",
        )
        .unwrap();
        assert_eq!(rejection(prepared), "INPUT_CHANGED");
        let (_directory, mut prepared) = fixture();
        prepared.candidate_pins[0].0.max_bytes += 1;
        assert_eq!(rejection(prepared), "INVALID_REQUEST");
    }

    #[test]
    fn producer_output_without_original_pin_cannot_accept_same_hash_replacement() {
        let (_directory, mut prepared) = fixture();
        let root = &prepared.workspace.canonical_root;
        fs::rename(root.join("report"), root.join("held-original")).unwrap();
        fs::write(root.join("report"), b"fixed report").unwrap();
        prepared.candidate_pins.clear();
        assert_eq!(rejection(prepared), "INVALID_REQUEST");
    }

    #[test]
    fn symlink_hardlink_fifo_and_replaced_root_never_become_regular_evidence() {
        for kind in ["symlink", "hardlink", "fifo"] {
            let (_directory, prepared) = fixture();
            let path = prepared.workspace.canonical_root.join("report");
            fs::rename(&path, prepared.workspace.canonical_root.join("original")).unwrap();
            match kind {
                "symlink" => symlink("original", &path).unwrap(),
                "hardlink" => {
                    fs::hard_link(prepared.workspace.canonical_root.join("original"), &path)
                        .unwrap()
                }
                _ => {
                    use std::{ffi::CString, os::unix::ffi::OsStrExt};
                    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
                    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
                }
            }
            assert_eq!(rejection(prepared), "INPUT_CHANGED");
        }
        let (directory, mut prepared) = fixture();
        let holder = tempfile::tempdir().unwrap();
        fs::rename(directory.path(), holder.path().join("original-root")).unwrap();
        fs::create_dir(directory.path()).unwrap();
        prepared.inputs.clear();
        assert_eq!(rejection(prepared), "INPUT_CHANGED");
    }

    #[test]
    fn cancellation_deadline_and_total_actual_bytes_reject_before_validation() {
        let (_directory, prepared) = fixture();
        let result = prepared.execute(
            &AtomicBool::new(true),
            Instant::now() + Duration::from_secs(1),
        );
        assert!(matches!(result, Err(error) if error.code == "CANCEL_REQUESTED"));
        let (_directory, prepared) = fixture();
        let result = prepared.execute(&AtomicBool::new(false), Instant::now());
        assert!(matches!(result, Err(error) if error.code == "DEADLINE_EXCEEDED"));
        let (_directory, mut prepared) = fixture();
        let input = prepared.workspace.canonical_root.join("input");
        File::options()
            .write(true)
            .open(input)
            .unwrap()
            .set_len(MAX_IO_BYTES + 1)
            .unwrap();
        prepared.inputs[0].max_bytes = MAX_IO_BYTES + 1;
        assert_eq!(rejection(prepared), "RESOURCE_LIMIT");
    }

    fn data_fixture(bad_range: bool) -> (tempfile::TempDir, PreparedGoalValidator) {
        let (directory, mut prepared) = fixture();
        let root = &prepared.workspace.canonical_root;
        fs::write(root.join("script"), b"not executed").unwrap();
        fs::write(root.join("environment"), b"pinned metadata").unwrap();
        let analysis = AnalysisReproducibility {
            script_artifact_id: "script".into(),
            environment_artifact_id: "environment".into(),
            units: BTreeMap::from([("x".into(), "m".into())]),
            timezone: "UTC".into(),
            seed: Some(7),
            scope: AnalysisScope::Full,
        };
        let document = json!({"metadata":{"input_revision":"input-revision","dataset_revision":"dataset",
            "input_digests":{"input":prepared.inputs[0].expected_sha256},"table_artifact_id":"report",
            "row_count":1,"dataset_rows":1,"units":analysis.units,"timezone":"UTC","seed":7,
            "scope":{"kind":"full"},"script_sha256":sha256(b"not executed"),"environment_sha256":sha256(b"pinned metadata")},
            "rows":[{"x":if bad_range { 20 } else { 3 }}]});
        fs::write(root.join("report"), serde_json::to_vec(&document).unwrap()).unwrap();
        prepared.subject.version = VersionBinding::Data {
            dataset_revision: "dataset".into(),
        };
        prepared.outputs = ["report", "script", "environment"]
            .iter()
            .map(|name| requirement(root, name))
            .collect();
        prepared.candidate_pins = prepared
            .outputs
            .iter()
            .map(|artifact| {
                (
                    artifact.clone(),
                    pin(&fs::metadata(root.join(&artifact.relative_path)).unwrap()),
                )
            })
            .collect();
        prepared.definition = GoalValidatorDefinition::Data {
            table: JsonTableContract {
                table_artifact_id: "report".into(),
                columns: vec![Column {
                    name: "x".into(),
                    required: true,
                    nullable: false,
                    scalar: ScalarType::Integer { min: 0, max: 10 },
                }],
                dataset_rows: 1,
                analysis,
                limits: JsonLimits::default(),
            },
        };
        (directory, prepared)
    }

    #[test]
    fn actual_json_contract_distinguishes_valid_data_from_hash_valid_out_of_range_rows() {
        for bad_range in [false, true] {
            let (_directory, prepared) = data_fixture(bad_range);
            let observed = execute(prepared).unwrap();
            assert_eq!(observed.passed, !bad_range);
            assert_eq!(observed.scope, "data_contract_only");
            assert_eq!(observed.correctness, "approved_schema_ranges_metadata_only");
            assert_eq!(observed.view["file_integrity"]["passed"], true);
            if bad_range {
                assert_eq!(observed.view["failure"]["code"], "numeric_range");
            }
        }
    }

    fn command_fixture(
        program: &str,
        arguments: Vec<String>,
    ) -> (tempfile::TempDir, PreparedGoalValidator) {
        let (directory, mut prepared) = fixture();
        let program_path = Path::new(program)
            .canonicalize()
            .or_else(|_| {
                Path::new("/bin")
                    .join(Path::new(program).file_name().unwrap())
                    .canonicalize()
            })
            .unwrap();
        let program_root = program_path.parent().unwrap().to_path_buf();
        let m = fs::metadata(&program_root).unwrap();
        let bytes = fs::read(&program_path).unwrap();
        prepared.definition = GoalValidatorDefinition::Command {
            program_root,
            program_device: m.dev(),
            program_inode: m.ino(),
            program: ArtifactRequirement {
                artifact_id: "fixed-program".into(),
                relative_path: program_path.file_name().unwrap().into(),
                max_bytes: MAX_IO_BYTES,
                expected_sha256: Some(sha256(&bytes)),
            },
            settings: CommandSettings {
                mode: CommandMode::NativeExecutable,
                arguments,
                environment: BTreeMap::new(),
                expected_exit_codes: vec![0],
                wall_time_ms: 1000,
                cleanup_time_ms: 500,
                stdout_limit: 1024,
                stderr_limit: 1024,
                require: RequiredEnforcement::default(),
            },
            script: None,
        };
        (directory, prepared)
    }

    #[test]
    fn actual_command_pass_and_timeout_retain_owned_cleanup_observations() {
        for (program, args, should_pass) in [
            ("/usr/bin/true", vec![], true),
            ("/usr/bin/sleep", vec!["2".into()], false),
        ] {
            let (_directory, prepared) = command_fixture(program, args);
            let mut observed = execute(prepared).unwrap();
            assert_eq!(observed.passed, should_pass);
            assert_eq!(observed.scope, "approved_deterministic_command_only");
            let run = observed.command.as_mut().unwrap();
            assert!(run.view().process_started);
            // A false return here is retained ownership, never inferred stop.
            let until = Instant::now() + Duration::from_secs(2);
            while !run.try_reap_owned_leader().unwrap() {
                assert!(Instant::now() < until, "owned child did not reconcile");
                std::thread::yield_now();
            }
            assert!(run.view().cleanup.owned_leader_reaped);
            if !should_pass {
                assert!(!run.view().passed);
            }
        }
    }

    #[test]
    fn post_command_file_failure_keeps_run_and_insufficient_budget_never_spawns() {
        let (_directory, prepared) = command_fixture("/usr/bin/rm", vec!["report".into()]);
        let observed = execute(prepared).unwrap();
        assert!(!observed.passed);
        assert_eq!(observed.view["goal_io"]["error_code"], "INPUT_CHANGED");
        assert!(observed.command.as_ref().unwrap().view().process_started);
        let (_directory, prepared) = command_fixture("/usr/bin/true", vec![]);
        let result = prepared.execute(
            &AtomicBool::new(false),
            Instant::now() + Duration::from_millis(300),
        );
        assert!(matches!(result, Err(error) if error.code == "DEADLINE_EXCEEDED"));
    }

    #[test]
    fn deferred_script_uses_real_pinned_bytes_and_cancel_after_actual_spawn_keeps_child() {
        let (_directory, mut prepared) = command_fixture("/bin/sh", vec![]);
        let root = prepared.workspace.canonical_root.clone();
        fs::write(
            root.join("script"),
            b"printf started > command-started\nwhile :; do :; done\n",
        )
        .unwrap();
        let script = requirement(&root, "script");
        prepared.candidate_pins.push((
            script.clone(),
            pin(&fs::metadata(root.join("script")).unwrap()),
        ));
        let GoalValidatorDefinition::Command {
            script: deferred, ..
        } = &mut prepared.definition
        else {
            unreachable!()
        };
        *deferred = Some(script);
        let cancelled = AtomicBool::new(false);
        let observed = std::thread::scope(|scope| {
            scope.spawn(|| {
                let until = Instant::now() + Duration::from_secs(3);
                while !root.join("command-started").exists() {
                    assert!(
                        Instant::now() < until,
                        "approved script never actually started"
                    );
                    std::thread::yield_now();
                }
                cancelled.store(true, Ordering::Release);
            });
            prepared
                .execute(&cancelled, Instant::now() + Duration::from_secs(5))
                .unwrap()
        });
        assert!(!observed.passed);
        let mut run = observed.command.unwrap();
        assert!(run.view().process_started);
        assert!(run.view().cancellation_observed);
        assert_eq!(run.view().contract.mode, "posix_sh_stdin");
        let until = Instant::now() + Duration::from_secs(2);
        while !run.try_reap_owned_leader().unwrap() {
            assert!(
                Instant::now() < until,
                "cancelled owned child did not reconcile"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn script_cannot_alias_a_command_output_despite_an_approved_producer_pin() {
        let (_directory, mut prepared) = command_fixture("/bin/sh", vec![]);
        let GoalValidatorDefinition::Command { script, .. } = &mut prepared.definition else {
            unreachable!()
        };
        *script = Some(prepared.outputs[0].clone());
        assert_eq!(rejection(prepared), "INVALID_REQUEST");
    }
}
