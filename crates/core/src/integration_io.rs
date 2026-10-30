//! Worker-only IO for an independently approved integration. The private
//! context retains real library handles; a persisted JSON pass cannot rebuild it.
use super::{authority::*, fs::*, runtime::IntegrationStageTicket, *};
use crate::{
    goal_validator_io::{GoalValidatorDefinition, GoalValidatorObservation, PreparedGoalValidator},
    Result,
};
use agentbaton_workspaces::{
    adoption::{AuthorizedIntegration, PreparedCandidate},
    data_workspace::FilePin,
    source_capture::{capture_source_commit, AuthorizedSourceCapture, CapturedSourceCommit},
    validation::{
        validate_artifacts, ApprovedRule, ArtifactRequirement, BoundArtifact, ValidationSubject,
    },
    WorkspaceIdentity, WorkspaceKind,
};
use serde_json::Value;
use std::{
    fs,
    path::Path,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

pub(super) struct IntegrationPreparedContext {
    pub(super) policy: IntegrationPolicy,
    pub(super) run_id: String,
    pub(super) captured: CapturedSourceCommit,
    pub(super) control: WorkspaceIdentity,
    pub(super) candidate: PreparedCandidate,
    pub(super) inputs: Vec<ArtifactRequirement>,
    pub(super) outputs: Vec<(ArtifactRequirement, FilePin)>,
    pub(super) validations: Vec<(
        crate::goal_store::GoalValidatorRef,
        GoalValidatorObservation,
    )>,
}
fn identity(pin: &IntegrationGitPin) -> Result<WorkspaceIdentity> {
    Ok(WorkspaceIdentity {
        canonical_root: pin.canonical_root.clone(),
        device: decimal(&pin.device)?,
        inode: decimal(&pin.inode)?,
        kind: WorkspaceKind::Git {
            common_dir: pin.common_dir.clone(),
            head: pin.head.clone(),
            dirty: false,
        },
    })
}
fn decimal(text: &str) -> Result<u64> {
    let value = text.parse::<u64>().map_err(|_| changed())?;
    if value.to_string() != text {
        return Err(changed());
    }
    Ok(value)
}
fn bound(root: &WorkspaceIdentity, requirement: ArtifactRequirement) -> Result<BoundArtifact> {
    BoundArtifact::from_verified_contract(
        &agentbaton_workspaces::validation::ArtifactContractView {
            canonical_root: root.canonical_root.clone(),
            root_device: root.device,
            root_inode: root.inode,
            requirement,
        },
    )
    .map_err(|_| changed())
}
fn held_file(
    root: &WorkspaceIdentity,
    requirement: &ArtifactRequirement,
    control: &IntegrationReadControl,
) -> Result<(LeafPin, String, u64, FilePin)> {
    if requirement.relative_path.as_os_str().is_empty()
        || requirement.relative_path.as_os_str().len() > 4096
        || requirement
            .relative_path
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
        || requirement.max_bytes == 0
        || requirement.max_bytes > 67_108_864
    {
        return Err(invalid());
    }
    if super::fs::directory_identity(&root.canonical_root)? != (root.device, root.inode) {
        return Err(changed());
    }
    let pin = LeafPin::capture(&root.canonical_root.join(&requirement.relative_path))?;
    let (hash, bytes) = pin.digest(requirement.max_bytes, control)?;
    let stamp = pin.file_pin()?;
    Ok((pin, hash, bytes, stamp))
}
fn same_root(a: &WorkspaceIdentity, b: &WorkspaceIdentity) -> bool {
    a.canonical_root == b.canonical_root
        && a.device == b.device
        && a.inode == b.inode
        && matches!((&a.kind,&b.kind),(WorkspaceKind::Git{common_dir:x,head:h,..},WorkspaceKind::Git{common_dir:y,head:j,..}) if x==y&&h==j)
}
fn private_managed_root(root: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let m = fs::symlink_metadata(root).map_err(|_| changed())?;
    if !m.is_dir()
        || m.file_type().is_symlink()
        || m.mode() & 0o7777 != 0o700
        || m.uid() != rustix::process::geteuid().as_raw()
        || root.canonicalize().map_err(|_| changed())? != root
    {
        return Err(changed());
    }
    Ok(())
}
pub(super) fn prepare(
    ticket: &IntegrationStageTicket,
    cancel: Arc<AtomicBool>,
) -> Result<IntegrationPreparedContext> {
    let guard = IntegrationReadControl {
        cancel,
        deadline: ticket.deadline,
    };
    guard.check()?;
    private_managed_root(&ticket.managed_root)?;
    let policy = &ticket.policy;
    let source = identity(&policy.bindings.source)?;
    let mut selected = vec![];
    let mut held = vec![];
    let mut total = 0u64;
    for output in &policy.bindings.outputs {
        let refs = ticket
            .acceptance
            .artifact_refs
            .iter()
            .filter(|a| {
                a.artifact_id == output.artifact.artifact_id
                    && a.relative_path == output.artifact.relative_path
            })
            .collect::<Vec<_>>();
        if refs.len() != 1 || !same_root(&refs[0].root, &source) {
            return Err(changed());
        }
        let accepted = refs[0];
        let mut requirement = output.artifact.clone();
        requirement.expected_sha256 = Some(accepted.sha256.clone());
        let (pin, hash, bytes, stamp) = held_file(&source, &requirement, &guard)?;
        if hash != accepted.sha256 || bytes != accepted.bytes || stamp != accepted.file_pin {
            return Err(changed());
        }
        total = total.checked_add(bytes).ok_or_else(limited)?;
        if total > 67_108_864 || total > ticket.attempt.view.reservation.output_bytes {
            return Err(limited());
        }
        selected.push(bound(&source, requirement)?);
        held.push(pin);
    }
    let commit_time = u64::try_from(ticket.attempt.prepared_at_ms / 1000).map_err(|_| invalid())?;
    let approved = AuthorizedSourceCapture::from_verified_selection(
        &source,
        policy.bindings.source.head.clone(),
        selected,
        commit_time,
        ticket.acceptance.id.clone(),
        policy.approval_id.clone(),
    )
    .map_err(|_| changed())?;
    guard.check()?;
    let captured = capture_source_commit(approved).map_err(|e| match e {
        agentbaton_workspaces::WorkspaceError::ResourceLimit
        | agentbaton_workspaces::WorkspaceError::TooLarge => limited(),
        _ => changed(),
    })?;
    guard.check()?;
    for pin in &held {
        pin.recheck()?;
    }
    let control_name = ticket
        .control
        .root
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(changed)?;
    let control = agentbaton_workspaces::create_isolated(
        &source.canonical_root,
        &ticket.managed_root,
        control_name,
        &policy.bindings.source.head,
    )
    .map_err(|_| changed())?;
    guard.check()?;
    let approved = AuthorizedIntegration::from_verified_grant(
        &control,
        ticket.run.target_ref.clone(),
        ticket.run.expected_target_head.clone(),
        captured.commit().into(),
        policy.approval_id.clone(),
        commit_time,
    )
    .map_err(|_| changed())?;
    let candidate_name = ticket
        .candidate
        .root
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(changed)?;
    let candidate = agentbaton_workspaces::adoption::prepare_candidate(
        approved,
        &ticket.managed_root,
        candidate_name,
    )
    .map_err(|error| match error {
        agentbaton_workspaces::adoption::AdoptionError::StaleHead => CoreError::new(
            "INPUT_CHANGED",
            "Authorized integration target advanced during preparation",
        ),
        agentbaton_workspaces::adoption::AdoptionError::Conflict => CoreError::new(
            "CONFIG_CONFLICT",
            "Accepted selected files conflict with the exact integration target",
        ),
        _ => changed(),
    })?;
    guard.check()?;
    let workspace = candidate.workspace();
    let mut outputs = vec![];
    for output in &policy.bindings.outputs {
        let mut requirement = output.artifact.clone();
        let (_, hash, _, pin) = held_file(workspace, &requirement, &guard)?;
        // Only the independently approved integration may rebind output SHA at
        // this exact same selected path. Original source definitions stay intact.
        requirement.expected_sha256 = Some(hash);
        outputs.push((requirement, pin));
    }
    for input in &policy.bindings.inputs {
        let (_, hash, _, _) = held_file(workspace, input, &guard)?;
        if input.expected_sha256.as_deref() != Some(hash.as_str()) {
            return Err(changed());
        }
    }
    for validator in &policy.bindings.validators {
        if let Some(script) = &validator.script {
            if script.canonical_root != source.canonical_root
                || script.root_device != policy.bindings.source.device
                || script.root_inode != policy.bindings.source.inode
            {
                return Err(changed());
            }
            let (_, hash, _, _) = held_file(workspace, &script.requirement, &guard)?;
            if script.requirement.expected_sha256.as_deref() != Some(hash.as_str()) {
                return Err(changed());
            }
        }
    }
    guard.check()?;
    Ok(IntegrationPreparedContext {
        policy: policy.clone(),
        run_id: ticket.run.id.clone(),
        captured,
        control,
        candidate,
        inputs: policy.bindings.inputs.clone(),
        outputs,
        validations: vec![],
    })
}
impl IntegrationPreparedContext {
    pub(super) fn validation(
        &self,
        reference: &crate::goal_store::GoalValidatorRef,
        attempt_id: &str,
    ) -> Result<PreparedGoalValidator> {
        let original = self
            .policy
            .bindings
            .validators
            .iter()
            .find(|v| &v.reference == reference)
            .ok_or_else(denied)?;
        let definition: Value =
            serde_json::from_str(&original.definition_json).map_err(|_| changed())?;
        if sha(&definition)? != original.definition_digest {
            return Err(changed());
        }
        let file = if reference.kind == crate::goal_store::GoalValidatorKind::Data {
            &definition["file_contract"]
        } else {
            &definition["contract"]["file_contract"]
        };
        let contract: agentbaton_workspaces::validation::ValidationPolicyContractView =
            serde_json::from_value(file.clone()).map_err(|_| changed())?;
        let program_id = original
            .program
            .as_ref()
            .map(|p| p.requirement.artifact_id.as_str());
        let script_id = original
            .script
            .as_ref()
            .map(|p| p.requirement.artifact_id.as_str());
        let mut inputs = vec![];
        for source in &contract.inputs {
            if Some(source.requirement.artifact_id.as_str()) == program_id
                || Some(source.requirement.artifact_id.as_str()) == script_id
            {
                continue;
            }
            let input = self
                .inputs
                .iter()
                .find(|a| {
                    a.artifact_id == source.requirement.artifact_id
                        && a.relative_path == source.requirement.relative_path
                })
                .ok_or_else(changed)?;
            if source
                .requirement
                .expected_sha256
                .as_ref()
                .is_some_and(|sha| Some(sha) != input.expected_sha256.as_ref())
            {
                return Err(changed());
            }
            inputs.push(input.clone());
        }
        let mut outputs = vec![];
        let mut candidate_pins = vec![];
        for template in &contract.outputs {
            let (output, pin) = self
                .outputs
                .iter()
                .find(|(a, _)| {
                    a.artifact_id == template.requirement.artifact_id
                        && a.relative_path == template.requirement.relative_path
                })
                .ok_or_else(changed)?;
            outputs.push(output.clone());
            candidate_pins.push((output.clone(), pin.clone()));
        }
        if outputs.is_empty() {
            return Err(changed());
        }
        let runtime = self
            .policy
            .selection
            .stage_reservations
            .validators
            .iter()
            .find(|v| &v.validator == reference)
            .ok_or_else(changed)?;
        let definition = match reference.kind {
            crate::goal_store::GoalValidatorKind::Integrity => GoalValidatorDefinition::Integrity {
                analysis: contract.analysis,
            },
            crate::goal_store::GoalValidatorKind::Data => GoalValidatorDefinition::Data {
                table: serde_json::from_value(definition["table"].clone())
                    .map_err(|_| changed())?,
            },
            crate::goal_store::GoalValidatorKind::Command => {
                let (settings, script) =
                    crate::command_validation::graph_definition_settings(&definition)?;
                if settings
                    .wall_time_ms
                    .checked_add(settings.cleanup_time_ms)
                    .is_none_or(|v| v > runtime.reservation.runtime_ms)
                    || u64::from(settings.stdout_limit)
                        .checked_add(u64::from(settings.stderr_limit))
                        .is_none_or(|v| v > runtime.reservation.output_bytes)
                {
                    return Err(limited());
                }
                let program = original.program.as_ref().ok_or_else(changed)?;
                GoalValidatorDefinition::Command {
                    program_root: program.canonical_root.clone(),
                    program_device: decimal(&program.root_device)?,
                    program_inode: decimal(&program.root_inode)?,
                    program: program.requirement.clone(),
                    script,
                    settings,
                }
            }
        };
        Ok(PreparedGoalValidator {
            approval_id: self.policy.approval_id.clone(),
            definition_revision: original.definition_digest.clone(),
            subject: ValidationSubject {
                task_id: self.policy.selection.task_id.clone(),
                attempt_id: attempt_id.into(),
                spec_revision: self.policy.selection.expected_spec_revision,
                input_revision: self.policy.bindings.source.head.clone(),
                version: self.candidate.version_binding(),
            },
            workspace: self.candidate.workspace().clone(),
            inputs,
            outputs,
            candidate_pins,
            definition,
        })
    }
    pub(super) fn validation_is_stopped(observation: &GoalValidatorObservation) -> bool {
        observation.command.as_ref().is_none_or(|run| {
            let view = run.view();
            view.cleanup.owned_leader_reaped
                && view.cleanup_completed_within_budget
                && view.exit_observed_within_budget
        })
    }
    pub(super) fn cas(
        &self,
        cancel: Arc<AtomicBool>,
        deadline: Instant,
        attempt_id: &str,
    ) -> Result<agentbaton_workspaces::adoption::ControlledIntegrationResult> {
        let guard = IntegrationReadControl { cancel, deadline };
        guard.check()?;
        if self.validations.len() != self.policy.bindings.validators.len()
            || !self.policy.bindings.validators.iter().all(|required| {
                self.validations.iter().any(|(reference, v)| {
                    *reference == required.reference && v.passed && Self::validation_is_stopped(v)
                })
            })
        {
            return Err(denied());
        }
        for (_, observation) in &self.validations {
            if let Some(command) = &observation.command {
                command.verify_current().map_err(|_| changed())?;
            }
        }
        let workspace = self.candidate.workspace();
        let mut inputs = vec![];
        let mut outputs = vec![];
        for input in &self.inputs {
            inputs.push(bound(workspace, input.clone())?);
        }
        for (output, pin) in &self.outputs {
            let (_, hash, _, current) = held_file(workspace, output, &guard)?;
            if current != *pin || output.expected_sha256.as_deref() != Some(hash.as_str()) {
                return Err(changed());
            }
            outputs.push(bound(workspace, output.clone())?);
        }
        let subject = ValidationSubject {
            task_id: self.policy.selection.task_id.clone(),
            attempt_id: attempt_id.into(),
            spec_revision: self.policy.selection.expected_spec_revision,
            input_revision: self.policy.bindings.source.head.clone(),
            version: self.candidate.version_binding(),
        };
        let rule = ApprovedRule::from_verified_policy(
            self.policy.approval_id.clone(),
            self.policy.snapshot_digest.clone(),
            subject,
            &inputs,
            &outputs,
            None,
        )
        .map_err(|_| changed())?;
        let evidence = validate_artifacts(&rule, inputs, outputs, None).map_err(|_| changed())?;
        let acceptance = evidence.accept(&rule).map_err(|_| changed())?;
        guard.check()?;
        let control =
            agentbaton_workspaces::adoption::AdoptionControl::for_request(&guard.cancel, deadline);
        let receipt = agentbaton_workspaces::adoption::adopt_local_reference_controlled(
            &self.candidate,
            &evidence,
            &acceptance,
            &control,
        )
        .map_err(|_| changed())?;
        Ok(receipt)
    }
}
