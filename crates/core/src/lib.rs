//! AgentBaton's single business authority, shared by every local client.
pub mod command_validation;
pub mod command_workers;
pub mod data_validation;
mod data_workspace_workers;
pub mod data_workspaces;
pub mod execution;
pub mod goal_plan;
pub mod goal_store;
mod goal_validator_io;
pub mod ipc;
pub mod policy;
pub mod registry;
mod routes;
mod runtime_config;
pub use runtime_config::RuntimeBinding;
pub mod sessions;
pub mod storage;
mod task_workspace_binding;
pub mod validation;
pub use task_workspace_binding::{
    GitWorkspacePin, TaskGitWorkspacePreparation, TaskWorkspaceBinding, TaskWorkspaceBindingState,
    TaskWorkspaceBindingView, TaskWorkspaceReservationReceipt,
};
mod workspace_resolution;
pub use workspace_resolution::{
    CodeWorkspaceCandidateInfo, CodeWorkspacePreparation, CodeWorkspacePreparationReason,
    CodeWorkspacePreparationRequest, CodeWorkspacePreparationState,
};

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CoreError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub side_effect_state: String,
    pub details_ref: Option<String>,
}
impl CoreError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
            side_effect_state: "not_started".into(),
            details_ref: None,
        }
    }
    pub fn transport() -> Self {
        Self {
            retryable: true,
            side_effect_state: "unknown".into(),
            ..Self::new(
                "PROVIDER_DISCONNECTED",
                "Local daemon connection failed; reconcile before retrying a write",
            )
        }
    }
}
impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for CoreError {}
pub type Result<T> = std::result::Result<T, CoreError>;

pub mod factory;

pub mod factory_companion;
