//! Explicit protocol routing over owned transports. A route is not a permission grant.
use crate::codex::CodexProcess;
use crate::connections::ProtocolRoute;
use crate::transport::{
    ApprovedInstallation, Diagnostics, ProviderEvent, ProviderProcess, TransportFailure,
};
use crate::{ProtocolError, TurnState};
use serde_json::Value;
use std::time::Duration;
pub enum ManagedProviderProcess {
    Acp(ProviderProcess),
    Codex(CodexProcess),
}
impl ManagedProviderProcess {
    pub fn start(
        route: ProtocolRoute,
        installation: ApprovedInstallation,
        timeout: Duration,
    ) -> Result<Self, TransportFailure> {
        match route {
            ProtocolRoute::CodexAppServer => {
                CodexProcess::start(installation, timeout).map(Self::Codex)
            }
            ProtocolRoute::ClaudeAgentAcp => {
                ProviderProcess::start_claude(installation, timeout).map(Self::Acp)
            }
            ProtocolRoute::CustomAcp
            | ProtocolRoute::QoderNativeAcp
            | ProtocolRoute::CodexAcpAdapter => {
                ProviderProcess::start(installation, timeout).map(Self::Acp)
            }
        }
    }
    pub fn configure_session_configuration(
        &mut self,
        configuration: &Value,
    ) -> Result<(), TransportFailure> {
        match self {
            Self::Acp(p) => p.configure_session_configuration(configuration),
            Self::Codex(_) => Err(TransportFailure::protocol(
                ProtocolError::CapabilityUnsupported,
            )),
        }
    }
    pub fn configure_factory_session_configuration(
        &mut self,
        configuration: &Value,
        approved: &crate::ApprovedFactoryMcp,
    ) -> Result<(), TransportFailure> {
        match self {
            Self::Acp(p) => p.configure_factory_session_configuration(configuration, approved),
            Self::Codex(_) => Err(TransportFailure::protocol(
                ProtocolError::CapabilityUnsupported,
            )),
        }
    }
    pub fn new_session(&mut self, binding: &str) -> Result<String, TransportFailure> {
        match self {
            Self::Acp(p) => p.new_session(binding),
            Self::Codex(p) => p.new_session(binding),
        }
    }
    pub fn begin_prompt(&mut self, id: &str, text: &str) -> Result<(), TransportFailure> {
        match self {
            Self::Acp(p) => p.begin_prompt(id, text),
            Self::Codex(p) => p.begin_prompt(id, text),
        }
    }
    pub fn poll_prompt(
        &mut self,
        id: &str,
        quantum: Duration,
    ) -> Result<Option<TurnState>, TransportFailure> {
        match self {
            Self::Acp(p) => p.poll_prompt(id, quantum),
            Self::Codex(p) => p.poll_prompt(id, quantum),
        }
    }
    pub fn wait_prompt(&mut self, id: &str) -> Result<TurnState, TransportFailure> {
        match self {
            Self::Acp(p) => p.wait_prompt(id),
            Self::Codex(p) => p.wait_prompt(id),
        }
    }
    pub fn cancel(&mut self, id: &str) -> Result<(), TransportFailure> {
        match self {
            Self::Acp(p) => p.cancel(id),
            Self::Codex(p) => p.cancel(id),
        }
    }
    pub fn set_io_timeout(&mut self, timeout: Duration) -> Result<(), TransportFailure> {
        match self {
            Self::Acp(p) => p.set_io_timeout(timeout),
            Self::Codex(p) => p.set_io_timeout(timeout),
        }
    }
    pub fn terminate(&mut self) {
        match self {
            Self::Acp(p) => p.terminate(),
            Self::Codex(p) => p.terminate(),
        }
    }
    pub fn diagnostics(&self) -> &Diagnostics {
        match self {
            Self::Acp(p) => p.diagnostics(),
            Self::Codex(p) => p.diagnostics(),
        }
    }
    pub fn process_id(&self) -> u32 {
        match self {
            Self::Acp(p) => p.process_id(),
            Self::Codex(p) => p.process_id(),
        }
    }
    pub fn take_updates(&mut self) -> Vec<ProviderEvent> {
        match self {
            Self::Acp(p) => p.take_updates(),
            Self::Codex(p) => p.take_updates(),
        }
    }
    pub fn take_evidence(&mut self) -> Vec<Value> {
        match self {
            Self::Acp(p) => p.take_evidence(),
            Self::Codex(p) => p.take_evidence(),
        }
    }
    pub fn turn_state(&self, id: &str) -> Result<&TurnState, ProtocolError> {
        match self {
            Self::Acp(p) => p.turn_state(id),
            Self::Codex(p) => p.turn_state(id),
        }
    }
}
