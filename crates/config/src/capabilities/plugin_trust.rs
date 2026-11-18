//! Static import and enablement review for selected local plugin files.
//!
//! Source selection, compatibility observations and approvals are supplied by
//! trusted callers. This module does not discover, install, or run extensions.
use super::{
    read_source_bytes, sha256, valid_ref, validate_descriptor, Compatibility, ProviderRevision,
    SourceDescriptor, SourceKind, SourceRoot,
};
use crate::{ConfigError, ErrorCode};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::Metadata,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
};

const SCHEMA_VERSION: u32 = 1;
const MAX_COMPATIBILITY: usize = 64;

type ImportResult<T> = Result<T, ImportError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportError {
    InvalidSelection,
    SourceRead(ErrorCode),
    SourceChanged,
    InvalidManifest,
    ExecutableManifest,
    UnsupportedSchema,
    SourceMismatch,
    InvalidVersion,
    LicenseMismatch,
    InvalidCompatibility,
    InvalidPermissions,
    ContradictoryDeclaration,
    EntrypointMismatch,
    EmptyEntrypoint,
}

impl From<ConfigError> for ImportError {
    fn from(error: ConfigError) -> Self {
        Self::SourceRead(error.code)
    }
}

/// The registry explicitly selects the manifest and any single code file. The
/// license identifier is an expected declaration supplied by that registry.
#[derive(Clone, Copy)]
pub struct ApprovedPluginSource<'a> {
    pub descriptor: &'a SourceDescriptor,
    pub manifest: &'a Path,
    pub license_ref: &'a str,
    pub entrypoint: Option<ApprovedEntrypoint<'a>>,
}

#[derive(Clone, Copy)]
pub struct ApprovedEntrypoint<'a> {
    pub reference: &'a str,
    pub relative: &'a Path,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    Execute,
    ReadWorkspace,
    WriteWorkspace,
    Network,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginTrust {
    Declarative,
    Code,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    plugin_id: String,
    source_ref: String,
    version: String,
    license_ref: String,
    kind: PluginTrust,
    compatible_with: Vec<ProviderRevision>,
    permissions: Vec<PluginPermission>,
    entrypoint_ref: Option<String>,
}

/// Only bounded metadata and digests are exposed. Local paths and file bytes
/// are held outside the preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginPreview {
    pub plugin_id: String,
    pub version: String,
    pub license_ref: String,
    pub provider: ProviderRevision,
    pub manifest_digest: String,
    pub entrypoint_digest: Option<String>,
    pub review_digest: String,
    pub trust: PluginTrust,
    pub declared_compatibility: Compatibility,
    pub requested_permissions: BTreeSet<PluginPermission>,
}

pub struct PluginImport {
    descriptor: SourceDescriptor,
    manifest: PathBuf,
    entrypoint: Option<(String, PathBuf)>,
    preview: PluginPreview,
}

impl PluginImport {
    pub fn preview(&self) -> &PluginPreview {
        &self.preview
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCompatibility {
    VerifiedCompatible,
    VerifiedIncompatible,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Isolation {
    Process,
    OsSandbox,
}

/// Caller-verified effective permissions and identity of the actual constraints.
/// Process isolation alone establishes no OS sandbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnforcedProfile {
    pub constraint_ref: String,
    pub isolation: Isolation,
    pub effective_permissions: BTreeSet<PluginPermission>,
}

pub struct RuntimeEvidence<'a> {
    pub review_digest: &'a str,
    pub provider: &'a ProviderRevision,
    pub compatibility: RuntimeCompatibility,
    pub enforcement: Option<&'a EnforcedProfile>,
}

/// A current approval resolved by the trusted caller from an independent review.
/// This type has no manifest or deserialization entry point.
pub struct CodeApproval<'a> {
    pub review_digest: &'a str,
    pub enforced_profile: &'a EnforcedProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnableBlock {
    SourceInvalid(ImportError),
    SourceChanged,
    CompatibilityUndeclared,
    DeclaredIncompatible,
    RuntimeEvidenceMismatch,
    RuntimeCompatibilityUnknown,
    RuntimeIncompatible,
    CodeReviewRequired,
    EnforcementUnknown,
    EnforcementMismatch,
    ApprovalMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnableDecision {
    Eligible(PluginTrust),
    Blocked(EnableBlock),
}

impl SourceRoot {
    pub fn import_plugin(&self, source: ApprovedPluginSource<'_>) -> ImportResult<PluginImport> {
        verify_selection(source)?;
        let manifest = self.read_fixed(source.manifest)?;
        if manifest.stamp.mode & 0o111 != 0 {
            return Err(ImportError::ExecutableManifest);
        }
        let declared: Manifest =
            serde_json::from_slice(&manifest.bytes).map_err(|_| ImportError::InvalidManifest)?;
        let (declared_compatibility, requested_permissions) = verify_manifest(&declared, source)?;
        let entry = match (declared.entrypoint_ref.as_deref(), source.entrypoint) {
            (None, None) => None,
            (Some(reference), Some(approved)) if reference == approved.reference => {
                let pinned = self.read_fixed(approved.relative)?;
                if pinned.bytes.is_empty() {
                    return Err(ImportError::EmptyEntrypoint);
                }
                Some(pinned)
            }
            _ => return Err(ImportError::EntrypointMismatch),
        };
        if self.source_stamp(source.manifest)? != manifest.stamp {
            return Err(ImportError::SourceChanged);
        }
        if let Some((selection, pinned)) = source.entrypoint.zip(entry.as_ref()) {
            if self.source_stamp(selection.relative)? != pinned.stamp {
                return Err(ImportError::SourceChanged);
            }
        }
        let manifest_digest = sha256(&manifest.bytes);
        let entrypoint_digest = entry.as_ref().map(|file| sha256(&file.bytes));
        let trust = if entry.is_some() {
            PluginTrust::Code
        } else {
            PluginTrust::Declarative
        };
        let reviewed = serde_json::to_vec(&(
            SCHEMA_VERSION,
            self.identity,
            source.descriptor,
            source.manifest.as_os_str().as_bytes(),
            manifest.stamp,
            &manifest_digest,
            source
                .entrypoint
                .map(|file| (file.reference, file.relative.as_os_str().as_bytes())),
            entry.map(|file| file.stamp),
            &entrypoint_digest,
            trust,
            &requested_permissions,
        ))
        .map_err(|_| ImportError::InvalidManifest)?;
        Ok(PluginImport {
            descriptor: source.descriptor.clone(),
            manifest: source.manifest.to_owned(),
            entrypoint: source
                .entrypoint
                .map(|file| (file.reference.into(), file.relative.into())),
            preview: PluginPreview {
                plugin_id: declared.plugin_id,
                version: declared.version,
                license_ref: declared.license_ref,
                provider: source.descriptor.provider.clone(),
                manifest_digest,
                entrypoint_digest,
                review_digest: sha256(&reviewed),
                trust,
                declared_compatibility,
                requested_permissions,
            },
        })
    }

    /// Reopens exactly the reviewed source and returns a decision without running code.
    pub fn check_plugin_enablement(
        &self,
        imported: &PluginImport,
        runtime: RuntimeEvidence<'_>,
        approval: Option<CodeApproval<'_>>,
    ) -> EnableDecision {
        let fresh = self.import_plugin(ApprovedPluginSource {
            descriptor: &imported.descriptor,
            manifest: &imported.manifest,
            license_ref: &imported.preview.license_ref,
            entrypoint: imported.entrypoint.as_ref().map(|(reference, relative)| {
                ApprovedEntrypoint {
                    reference,
                    relative,
                }
            }),
        });
        match fresh {
            Err(error) => EnableDecision::Blocked(EnableBlock::SourceInvalid(error)),
            Ok(fresh) if fresh.preview != imported.preview => {
                EnableDecision::Blocked(EnableBlock::SourceChanged)
            }
            Ok(fresh) => match check_facts(&fresh.preview, runtime, approval) {
                Ok(()) => EnableDecision::Eligible(fresh.preview.trust),
                Err(reason) => EnableDecision::Blocked(reason),
            },
        }
    }

    fn source_stamp(&self, relative: &Path) -> ImportResult<FileStamp> {
        let file = self.open_selected(relative)?;
        Ok(FileStamp::read(
            &file
                .metadata()
                .map_err(|_| ImportError::SourceRead(ErrorCode::Io))?,
        ))
    }

    fn read_fixed(&self, relative: &Path) -> ImportResult<PinnedFile> {
        let mut file = self.open_selected(relative)?;
        let before = file
            .metadata()
            .map_err(|_| ImportError::SourceRead(ErrorCode::Io))?;
        let stamp = FileStamp::read(&before);
        let bytes = read_source_bytes(&mut file)?;
        let after = file
            .metadata()
            .map_err(|_| ImportError::SourceRead(ErrorCode::Io))?;
        if FileStamp::read(&after) != stamp || u64::try_from(bytes.len()).ok() != Some(before.len())
        {
            return Err(ImportError::SourceChanged);
        }
        Ok(PinnedFile { stamp, bytes })
    }
}

#[derive(PartialEq, Eq, Serialize)]
struct FileStamp {
    dev: u64,
    ino: u64,
    uid: u32,
    mode: u32,
    size: u64,
    links: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl FileStamp {
    fn read(meta: &Metadata) -> Self {
        Self {
            dev: meta.dev(),
            ino: meta.ino(),
            uid: meta.uid(),
            mode: meta.mode(),
            size: meta.len(),
            links: meta.nlink(),
            modified: (meta.mtime(), meta.mtime_nsec()),
            changed: (meta.ctime(), meta.ctime_nsec()),
        }
    }
}

struct PinnedFile {
    stamp: FileStamp,
    bytes: Vec<u8>,
}

fn opaque(value: &str) -> bool {
    valid_ref(value) && !value.contains('/')
}

fn license_id(value: &str) -> bool {
    value.len() <= 80
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-+".contains(&byte))
}

fn version(value: &str) -> bool {
    if value.len() > 29 {
        return false;
    }
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts.into_iter().all(|part| {
            !part.is_empty()
                && part.len() <= 9
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
        })
}

fn private_provider(provider: &ProviderRevision) -> bool {
    opaque(&provider.provider_id) && opaque(&provider.version)
}

fn verify_selection(source: ApprovedPluginSource<'_>) -> ImportResult<()> {
    if validate_descriptor(source.descriptor).is_err()
        || source.descriptor.kind != SourceKind::Plugin
        || !opaque(&source.descriptor.source_id)
        || !opaque(&source.descriptor.locator_ref)
        || !private_provider(&source.descriptor.provider)
        || !license_id(source.license_ref)
        || source
            .entrypoint
            .is_some_and(|file| !opaque(file.reference) || file.relative == source.manifest)
    {
        return Err(ImportError::InvalidSelection);
    }
    Ok(())
}

fn verify_manifest(
    declared: &Manifest,
    source: ApprovedPluginSource<'_>,
) -> ImportResult<(Compatibility, BTreeSet<PluginPermission>)> {
    if declared.schema_version != SCHEMA_VERSION {
        return Err(ImportError::UnsupportedSchema);
    }
    if declared.plugin_id != source.descriptor.source_id
        || declared.source_ref != source.descriptor.locator_ref
    {
        return Err(ImportError::SourceMismatch);
    }
    if !version(&declared.version) {
        return Err(ImportError::InvalidVersion);
    }
    if declared.license_ref != source.license_ref {
        return Err(ImportError::LicenseMismatch);
    }
    let compatibility: BTreeSet<_> = declared
        .compatible_with
        .iter()
        .map(|item| (&item.provider_id, &item.version))
        .collect();
    if declared.compatible_with.len() > MAX_COMPATIBILITY
        || compatibility.len() != declared.compatible_with.len()
        || declared
            .compatible_with
            .iter()
            .any(|item| !private_provider(item))
    {
        return Err(ImportError::InvalidCompatibility);
    }
    let permissions: BTreeSet<_> = declared.permissions.iter().copied().collect();
    if permissions.len() != declared.permissions.len() {
        return Err(ImportError::InvalidPermissions);
    }
    match declared.kind {
        PluginTrust::Declarative if declared.entrypoint_ref.is_none() && permissions.is_empty() => {
        }
        PluginTrust::Code
            if declared.entrypoint_ref.as_deref().is_some_and(opaque)
                && permissions.contains(&PluginPermission::Execute) => {}
        _ => return Err(ImportError::ContradictoryDeclaration),
    }
    let compatibility = if compatibility.is_empty() {
        Compatibility::Unknown
    } else if declared
        .compatible_with
        .contains(&source.descriptor.provider)
    {
        Compatibility::DeclaredCompatible
    } else {
        Compatibility::DeclaredIncompatible
    };
    Ok((compatibility, permissions))
}

fn check_facts(
    preview: &PluginPreview,
    runtime: RuntimeEvidence<'_>,
    approval: Option<CodeApproval<'_>>,
) -> Result<(), EnableBlock> {
    match preview.declared_compatibility {
        Compatibility::Unknown => return Err(EnableBlock::CompatibilityUndeclared),
        Compatibility::DeclaredIncompatible => return Err(EnableBlock::DeclaredIncompatible),
        Compatibility::DeclaredCompatible => {}
    }
    if runtime.review_digest != preview.review_digest || runtime.provider != &preview.provider {
        return Err(EnableBlock::RuntimeEvidenceMismatch);
    }
    match runtime.compatibility {
        RuntimeCompatibility::Unknown => return Err(EnableBlock::RuntimeCompatibilityUnknown),
        RuntimeCompatibility::VerifiedIncompatible => return Err(EnableBlock::RuntimeIncompatible),
        RuntimeCompatibility::VerifiedCompatible => {}
    }
    if preview.trust == PluginTrust::Code {
        let approval = approval.ok_or(EnableBlock::CodeReviewRequired)?;
        let profile = runtime.enforcement.ok_or(EnableBlock::EnforcementUnknown)?;
        if !opaque(&profile.constraint_ref)
            || profile.effective_permissions != preview.requested_permissions
        {
            return Err(EnableBlock::EnforcementMismatch);
        }
        if approval.review_digest != preview.review_digest || approval.enforced_profile != profile {
            return Err(EnableBlock::ApprovalMismatch);
        }
    }
    Ok(())
}
