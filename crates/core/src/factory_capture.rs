//! Worker-only bounded selected-byte observations; never Task/Git acceptance.
use super::*;
use crate::{data_workspaces::DataArtifactPins, Result};
use agentbaton_workspaces::validation::{
    self, ApprovedRule, BoundArtifact, CheckState, ValidationSubject, VersionBinding,
};

pub(crate) struct FactoryCapturePlan {
    scope: FactoryScope,
    task_id: String,
    attempt_id: String,
    spec_revision: u64,
    input_ids: Vec<String>,
    output_ids: Vec<String>,
}
pub(crate) struct FactoryCapture {
    pub(crate) artifacts: Vec<FactoryArtifactReference>,
    pub(crate) observed_bytes: Option<u64>,
    pub(crate) inputs_current: bool,
    pub(crate) outputs_complete: bool,
    pins: DataArtifactPins,
}
impl FactoryCapture {
    pub(crate) fn recheck(&self) -> Result<()> {
        self.pins.verify(false)
    }
}
impl FactoryCapturePlan {
    pub(crate) fn new(
        scope: FactoryScope,
        task_id: String,
        attempt_id: String,
        spec_revision: u64,
        input_ids: Vec<String>,
        output_ids: Vec<String>,
    ) -> Self {
        Self {
            scope,
            task_id,
            attempt_id,
            spec_revision,
            input_ids,
            output_ids,
        }
    }
    pub(crate) fn capture(self) -> Result<FactoryCapture> {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for original in self
            .scope
            .bindings
            .inputs
            .iter()
            .filter(|a| self.input_ids.contains(&a.contract.requirement.artifact_id))
        {
            let mut contract = original.contract.clone();
            contract.requirement.expected_sha256 = Some(original.sha256.clone());
            inputs.push(
                BoundArtifact::from_verified_contract(&contract)
                    .map_err(|_| super::store::changed())?,
            );
        }
        for contract in self
            .scope
            .bindings
            .outputs
            .iter()
            .filter(|a| self.output_ids.contains(&a.requirement.artifact_id))
        {
            outputs.push(
                BoundArtifact::from_verified_contract(contract)
                    .map_err(|_| super::store::changed())?,
            );
        }
        let mut contracts = inputs
            .iter()
            .chain(&outputs)
            .map(BoundArtifact::contract_view)
            .collect::<Vec<_>>();
        if contracts.len() > 32 {
            return Err(super::store::limited());
        }
        contracts.sort_by(|a, b| a.requirement.artifact_id.cmp(&b.requirement.artifact_id));
        let pins = DataArtifactPins::capture(&contracts)?;
        let workspace = &self.scope.bindings.workspaces[0];
        let subject = ValidationSubject {
            task_id: self.task_id,
            attempt_id: self.attempt_id.clone(),
            spec_revision: self.spec_revision,
            input_revision: workspace.base_revision.clone(),
            version: VersionBinding::WorkspaceArtifacts {
                workspace_id: workspace.id.clone(),
                base_revision: workspace.base_revision.clone(),
            },
        };
        // This private scope explicitly approves these exact paths/hash bounds.
        // The library is used only to observe safe bytes, not to accept a Task.
        let approval = ApprovedRule::from_verified_policy(
            self.scope.approval_id.clone(),
            format!("{}:{}", self.scope.id, self.scope.revision),
            subject,
            &inputs,
            &outputs,
            None,
        )
        .map_err(|_| super::store::changed())?;
        let evidence = validation::validate_artifacts(&approval, inputs, outputs, None)
            .map_err(|_| super::store::changed())?;
        let view = evidence.view();
        pins.verify(false)?;
        let inputs_current = view.inputs.iter().all(|a| a.state == CheckState::Passed);
        let mut artifacts = Vec::new();
        let mut observed = Some(0u64);
        for a in &view.outputs {
            match a.state {
                CheckState::Passed => {
                    let bytes = a.size.ok_or_else(super::store::changed)?;
                    observed = observed.and_then(|n| n.checked_add(bytes));
                    artifacts.push(FactoryArtifactReference {
                        artifact_id: a.artifact_id.clone(),
                        attempt_id: self.attempt_id.clone(),
                        relative_path: a.relative_path.clone(),
                        sha256: a
                            .observed_sha256
                            .clone()
                            .ok_or_else(super::store::changed)?,
                        bytes,
                    });
                }
                CheckState::Missing => {}
                _ => observed = None,
            }
        }
        Ok(FactoryCapture {
            artifacts,
            observed_bytes: observed,
            inputs_current,
            outputs_complete: view.outputs.iter().all(|a| a.state == CheckState::Passed),
            pins,
        })
    }
}
