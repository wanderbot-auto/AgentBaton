//! Six fixed Owner parameters; Task CAS and the stable write key live in the envelope.
use crate::{GitWorkspacePin, TaskGitWorkspacePreparation};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PrepareTaskGit {
    task_id: String,
    expected_spec_revision: u64,
    expected_project_revision: u64,
    host_id: String,
    expected_source_pin: GitWorkspacePin,
    preparation_key: String,
}
impl PrepareTaskGit {
    pub(super) fn with_task_revision(
        self,
        expected_task_revision: u64,
    ) -> TaskGitWorkspacePreparation {
        TaskGitWorkspacePreparation {
            task_id: self.task_id,
            expected_task_revision,
            expected_spec_revision: self.expected_spec_revision,
            expected_project_revision: self.expected_project_revision,
            host_id: self.host_id,
            expected_source_pin: self.expected_source_pin,
            preparation_key: self.preparation_key,
        }
    }
}
