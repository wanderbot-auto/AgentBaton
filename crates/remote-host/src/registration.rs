//! Pure registration metadata validation. No file, network, process or authority operation.
//! A syntactically valid endpoint/reference is not a verified Host or permission to contact it.
use crate::protocol::{identifier, require, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const REGISTRATION_VERSION: u32 = 1;
pub const MAX_REGISTRATION_BYTES: usize = 16 * 1024;
pub const MAX_DIRECTORY_MAPPINGS: usize = 8;
pub const MAX_REFERENCE_BYTES: usize = 256;

/// Exactly the legacy transport predicates and error order, without metadata policy or IO.
/// This deliberately permits `local` as a syntactic identifier; registration semantics do not.
pub fn validate_ssh_endpoint_syntax(
    expected_host_identity: &str,
    host: &str,
    user: &str,
    port: u16,
    host_key_alias: &str,
) -> Result<()> {
    require(identifier(expected_host_identity), "ssh_host_identity")?;
    require(
        !host.is_empty()
            && host.len() <= 253
            && !host.starts_with('-')
            && host
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b".-".contains(&c)),
        "ssh_host",
    )?;
    require(
        identifier(user) && identifier(host_key_alias) && port > 0,
        "ssh_identity",
    )
}

/// Exactly the legacy fixed-command path predicate. Does not inspect any filesystem.
/// Duplicate/trailing separators and `/` remain valid syntax here, as before.
pub fn ssh_path_syntax(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 512
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
        && !path.split('/').any(|p| p == ".." || p == ".")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SshEndpointMetadata {
    pub expected_host_identity: String,
    pub address: String,
    pub user: String,
    pub port: u16,
    pub host_key_alias: String,
}
impl SshEndpointMetadata {
    pub fn validate_syntax(&self) -> Result<()> {
        validate_ssh_endpoint_syntax(
            &self.expected_host_identity,
            &self.address,
            &self.user,
            self.port,
            &self.host_key_alias,
        )
    }
}

/// Claimed platform only; no observation or compatibility proof is produced here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeclaredPlatform {
    #[serde(rename = "macos")]
    MacOs,
    #[serde(rename = "linux")]
    Linux,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredConfigurationOwnership {
    BatonManaged,
    ExternalReadOnly,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationReference {
    pub reference: String,
    pub declared_ownership: DeclaredConfigurationOwnership,
}
/// Expected digest of a provisioned known_hosts FILE, not a remote key fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct KnownHostsReference {
    pub reference: String,
    pub file_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DeclaredDirectoryMapping {
    pub id: String,
    pub remote_root: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostRegistration {
    pub schema_version: u32,
    pub name: String,
    pub declared_platform: DeclaredPlatform,
    pub endpoint: SshEndpointMetadata,
    pub known_hosts: KnownHostsReference,
    pub transport_identity_ref: String,
    pub configuration: ConfigurationReference,
    pub directory_mappings: Vec<DeclaredDirectoryMapping>,
}

fn opaque_reference(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= MAX_REFERENCE_BYTES
        && !matches!(reference, "." | "..")
        && reference
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
impl HostRegistration {
    /// Checks complete metadata semantics, not a transport, credential, path or Grant.
    /// Fields stay unmodified: consumers must validate again before persisting changed data.
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema_version == REGISTRATION_VERSION,
            "registration_version",
        )?;
        require(
            !self.name.is_empty()
                && self.name.len() <= 256
                && self.name.trim() == self.name
                && !self.name.chars().any(char::is_control),
            "registration_name",
        )?;
        self.endpoint.validate_syntax()?;
        require(
            !self
                .endpoint
                .expected_host_identity
                .eq_ignore_ascii_case("local"),
            "registration_reserved_host",
        )?;
        require(
            opaque_reference(&self.known_hosts.reference)
                && opaque_reference(&self.transport_identity_ref)
                && opaque_reference(&self.configuration.reference),
            "registration_reference",
        )?;
        require(
            self.known_hosts.file_sha256.len() == 64
                && self
                    .known_hosts
                    .file_sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "registration_digest",
        )?;
        require(
            self.directory_mappings.len() <= MAX_DIRECTORY_MAPPINGS,
            "registration_mappings",
        )?;
        let mut seen = BTreeSet::new();
        for mapping in &self.directory_mappings {
            require(
                identifier(&mapping.id) && seen.insert(&mapping.id),
                "registration_mapping_id",
            )?;
            // A declared remote namespace, never canonicalized against the controller's host.
            require(
                ssh_path_syntax(&mapping.remote_root)
                    && mapping.remote_root != "/"
                    && !mapping.remote_root.ends_with('/')
                    && !mapping.remote_root.contains("//"),
                "registration_remote_root",
            )?;
        }
        require(
            serde_json::to_vec(self)?.len() <= MAX_REGISTRATION_BYTES,
            "registration_size",
        )
    }
}

/// Bound the actual input before JSON allocation, then apply all registration semantics.
/// Direct serde deserialization alone is structural only; it does not call `validate`.
pub fn decode_registration(bytes: &[u8]) -> Result<HostRegistration> {
    require(bytes.len() <= MAX_REGISTRATION_BYTES, "registration_size")?;
    let registration: HostRegistration = serde_json::from_slice(bytes)?;
    registration.validate()?;
    Ok(registration)
}
