//! AgentBaton's single business authority, shared by every local client.
pub mod execution;
pub mod ipc;
pub mod policy;
pub mod registry;
pub mod sessions;
pub mod storage;

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
