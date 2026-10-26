//! Named connection and immutable execution bindings. These types distinguish
//! program, configuration, identity and host. Shape/reference checks never replace
//! trusted approval lookup, native authentication or runtime enforcement.
use crate::installation::{BinaryDigest, VersionEvidence, VersionProbeState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    Codex,
    Qoder,
    ClaudeCode,
    CustomAcp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolRoute {
    CodexAppServer,
    CodexAcpAdapter,
    QoderNativeAcp,
    ClaudeAgentAcp,
    CustomAcp,
}
impl ProtocolRoute {
    pub fn provider(self) -> ProviderKind {
        match self {
            Self::CodexAppServer | Self::CodexAcpAdapter => ProviderKind::Codex,
            Self::QoderNativeAcp => ProviderKind::Qoder,
            Self::ClaudeAgentAcp => ProviderKind::ClaudeCode,
            Self::CustomAcp => ProviderKind::CustomAcp,
        }
    }
    /// Documented launch shape only, not a compatibility or authorization claim.
    /// Custom argv must be explicitly supplied; native App Server is not ACP.
    pub fn documented_acp_arguments(self) -> Option<&'static [&'static str]> {
        match self {
            Self::CodexAcpAdapter | Self::ClaudeAgentAcp => Some(&[]),
            Self::QoderNativeAcp => Some(&["--acp"]),
            Self::CodexAppServer | Self::CustomAcp => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationSource {
    BatonManaged,
    ExternalExecutable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationRecord {
    pub id: String,
    pub provider: ProviderKind,
    pub host_id: String,
    pub source: InstallationSource,
    pub approval_reference: String,
    pub canonical_executable: PathBuf,
    pub binary_sha256: BinaryDigest,
    pub version_probe: VersionEvidence,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConfigurationSource {
    BatonManaged {
        config_root: PathBuf,
        state_root: PathBuf,
    },
    /// Only immutable references; there is deliberately no external write-back
    /// option and no copied raw/native credential directory.
    NativeConnected { config_source_refs: Vec<String> },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEnvironment {
    pub id: String,
    pub host_id: String,
    pub configuration: ConfigurationSource,
    pub config_snapshot_sha256: BinaryDigest,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityReference {
    pub id: String,
    pub provider: ProviderKind,
    pub host_id: String,
    /// Opaque reference resolved by the native secret store, never a token.
    pub native_handle_ref: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportState {
    Supported,
    Unsupported,
    Unknown,
    Untested,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    Documentation,
    VersionProbe,
    ProtocolNegotiation,
    FixtureTest,
    RealProviderTest,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityEvidence {
    pub capability: String,
    pub state: SupportState,
    pub source: EvidenceSource,
    pub evidence_ref: String,
    pub provider_version: String,
    pub observed_at_unix_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportManifest {
    pub id: String,
    pub installation_id: String,
    pub provider: ProviderKind,
    pub provider_version: String,
    pub capabilities: Vec<CapabilityEvidence>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityFallback {
    Deny,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedConnection {
    pub id: String,
    pub label: String,
    pub revision: u64,
    pub provider: ProviderKind,
    pub protocol_route: ProtocolRoute,
    pub installation_id: String,
    pub environment_id: String,
    pub identity_ref: String,
    pub host_id: String,
    pub support_manifest_ref: String,
    pub identity_fallback: IdentityFallback,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Interactive,
    Batch,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionBinding {
    pub task_id: String,
    pub attempt_id: String,
    pub input_revision: String,
    pub connection_id: String,
    pub connection_revision: u64,
    pub provider: ProviderKind,
    pub protocol_route: ProtocolRoute,
    pub installation_id: String,
    pub executable_sha256: BinaryDigest,
    pub provider_version: String,
    pub environment_id: String,
    pub configuration: ConfigurationSource,
    pub config_snapshot_sha256: BinaryDigest,
    pub identity_ref: String,
    pub host_id: String,
    pub support_manifest_ref: String,
    pub execution_mode: ExecutionMode,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingError {
    InvalidIdentifier,
    InvalidConfiguration,
    InstallationUnverified,
    ReferenceMismatch,
    ProviderRouteMismatch,
    InvalidEvidence,
    CapabilityUnavailable,
}
impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "execution binding: {self:?}")
    }
}
impl std::error::Error for BindingError {}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
}
fn text_reference(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}
impl SupportManifest {
    pub fn validate(&self) -> Result<(), BindingError> {
        if !identifier(&self.id)
            || !identifier(&self.installation_id)
            || !text_reference(&self.provider_version)
            || self.capabilities.len() > 128
        {
            return Err(BindingError::InvalidEvidence);
        }
        let mut seen = BTreeSet::new();
        for item in &self.capabilities {
            if !identifier(&item.capability)
                || !seen.insert(&item.capability)
                || !text_reference(&item.evidence_ref)
                || item.provider_version != self.provider_version
                || item.observed_at_unix_ms == 0
                || (item.state == SupportState::Supported
                    && matches!(
                        item.source,
                        EvidenceSource::Documentation | EvidenceSource::VersionProbe
                    ))
            {
                return Err(BindingError::InvalidEvidence);
            }
        }
        Ok(())
    }
    /// Only actual runtime negotiation/real Provider evidence may satisfy a live
    /// execution prerequisite. Fixture success cannot silently become live support.
    pub fn require_runtime_capability(&self, capability: &str) -> Result<(), BindingError> {
        self.validate()?;
        if self.capabilities.iter().any(|item| {
            item.capability == capability
                && item.state == SupportState::Supported
                && matches!(
                    item.source,
                    EvidenceSource::ProtocolNegotiation | EvidenceSource::RealProviderTest
                )
        }) {
            Ok(())
        } else {
            Err(BindingError::CapabilityUnavailable)
        }
    }
}
impl NamedConnection {
    #[allow(clippy::too_many_arguments)]
    pub fn bind_execution(
        &self,
        task_id: &str,
        attempt_id: &str,
        input_revision: &str,
        mode: ExecutionMode,
        installation: &InstallationRecord,
        environment: &RuntimeEnvironment,
        identity: &IdentityReference,
        support: &SupportManifest,
        required_runtime_capabilities: &[&str],
    ) -> Result<ExecutionBinding, BindingError> {
        for value in [
            &self.id,
            &self.installation_id,
            &self.environment_id,
            &self.identity_ref,
            &self.host_id,
            &self.support_manifest_ref,
            &installation.id,
            &installation.host_id,
            &environment.id,
            &environment.host_id,
            &identity.id,
            &identity.host_id,
            task_id,
            attempt_id,
        ] {
            if !identifier(value) {
                return Err(BindingError::InvalidIdentifier);
            }
        }
        if !text_reference(&self.label)
            || !text_reference(input_revision)
            || self.revision == 0
            || !text_reference(&identity.native_handle_ref)
        {
            return Err(BindingError::InvalidIdentifier);
        }
        if self.protocol_route.provider() != self.provider {
            return Err(BindingError::ProviderRouteMismatch);
        }
        if self.installation_id != installation.id
            || self.environment_id != environment.id
            || self.identity_ref != identity.id
            || self.support_manifest_ref != support.id
            || support.installation_id != installation.id
            || self.host_id != installation.host_id
            || self.host_id != environment.host_id
            || self.host_id != identity.host_id
            || self.provider != installation.provider
            || self.provider != identity.provider
            || self.provider != support.provider
        {
            return Err(BindingError::ReferenceMismatch);
        }
        if !text_reference(&installation.approval_reference)
            || !installation.canonical_executable.is_absolute()
            || installation.version_probe.state != VersionProbeState::Succeeded
            || installation.version_probe.exit_code != Some(0)
            || installation.version_probe.executable_sha256 != installation.binary_sha256
            || installation.version_probe.version_line.as_deref()
                != Some(support.provider_version.as_str())
            || installation.version_probe.observed_at_unix_ms == 0
            || !installation.version_probe.process_reaped
        {
            return Err(BindingError::InstallationUnverified);
        }
        match &environment.configuration {
            ConfigurationSource::BatonManaged {
                config_root,
                state_root,
            } if !config_root.is_absolute() || !state_root.is_absolute() => {
                return Err(BindingError::InvalidConfiguration)
            }
            ConfigurationSource::NativeConnected { config_source_refs }
                if config_source_refs.is_empty()
                    || config_source_refs.len() > 32
                    || config_source_refs.iter().any(|v| !text_reference(v)) =>
            {
                return Err(BindingError::InvalidConfiguration)
            }
            _ => {}
        }
        support.validate()?;
        for capability in required_runtime_capabilities {
            support.require_runtime_capability(capability)?;
        }
        Ok(ExecutionBinding {
            task_id: task_id.into(),
            attempt_id: attempt_id.into(),
            input_revision: input_revision.into(),
            connection_id: self.id.clone(),
            connection_revision: self.revision,
            provider: self.provider,
            protocol_route: self.protocol_route,
            installation_id: installation.id.clone(),
            executable_sha256: installation.binary_sha256.clone(),
            provider_version: support.provider_version.clone(),
            environment_id: environment.id.clone(),
            configuration: environment.configuration.clone(),
            config_snapshot_sha256: environment.config_snapshot_sha256.clone(),
            identity_ref: identity.id.clone(),
            host_id: self.host_id.clone(),
            support_manifest_ref: support.id.clone(),
            execution_mode: mode,
        })
    }
}
