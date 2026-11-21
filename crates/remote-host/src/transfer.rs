//! Pure transfer planning and whole-plan integrity checks over in-memory content.
//!
//! The caller resolves the current project approval and classifies inputs before
//! planning. It must revalidate authority and binding before any actual transfer.
//! Registration, route selection and an execution lease are distinct from data approval.

use crate::protocol::{digest, identifier, Binding};
use std::collections::BTreeMap;

/// The known levels follow the product artifact sensitivity vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sensitivity {
    Public,
    Internal,
    Sensitive,
    Unknown,
}

impl Sensitivity {
    fn from_binding(value: &str) -> Result<Self, PlanError> {
        match value {
            "public" => Ok(Self::Public),
            "internal" => Ok(Self::Internal),
            "sensitive" => Ok(Self::Sensitive),
            _ => Err(PlanError::UnknownSensitivity),
        }
    }
}

/// Classification comes from the caller's verified data source.
/// Metadata and digest verification do not classify arbitrary bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataKind {
    ProjectInput,
    Secret,
    AccountMaterial,
    Unknown,
}

/// Logical identity and immutable content metadata. IDs and versions use the
/// protocol identifier syntax, and SHA-256 is represented in lowercase hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataItem<'a> {
    pub id: &'a str,
    pub version: &'a str,
    pub size_bytes: u64,
    pub sha256: &'a str,
    pub sensitivity: Sensitivity,
    pub kind: DataKind,
}

#[derive(Debug, Clone, Copy)]
pub struct TransferRequest<'a> {
    pub binding: &'a Binding,
    pub current_input_revision: &'a str,
    pub necessary_items: &'a [DataItem<'a>],
}

/// A projection of a current, applicable project data approval verified by the
/// caller. The approval applies to exactly one binding and one resource scope.
#[derive(Debug, Clone, Copy)]
pub struct ProjectDataApproval<'a> {
    pub approval_ref: &'a str,
    pub policy_revision: u64,
    pub binding: &'a Binding,
    /// `None` means the approved resource scope could not be determined.
    pub scope: Option<ApprovedScope<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ApprovedScope<'a> {
    pub scope_ref: &'a str,
    pub items: &'a [DataItem<'a>],
    pub max_total_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRequired {
    MissingProjectApproval,
    NewRecipient,
    DataScopeExpansion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanError {
    ApprovalRequired(ApprovalRequired),
    InvalidBinding,
    ProjectMismatch,
    InputRevisionMismatch,
    RecipientIsNotHost,
    BindingMismatch,
    InvalidApprovalReference,
    UnknownApprovedScope,
    InvalidScopeReference,
    InvalidItemId,
    InvalidItemVersion,
    InvalidItemDigest,
    UnknownSensitivity,
    SensitivityExceedsBinding,
    ProhibitedDataKind,
    UnknownDataKind,
    DuplicateRequestedId,
    DuplicateApprovedId,
    ItemVersionMismatch,
    ItemSizeMismatch,
    ItemDigestMismatch,
    ItemSensitivityMismatch,
    SizeOverflow,
    ByteBudgetExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovalContext<'a> {
    pub approval_ref: &'a str,
    pub policy_revision: u64,
    pub scope_ref: &'a str,
}

/// An immutable plan produced after the entire request passes validation.
#[derive(Debug, PartialEq, Eq)]
pub struct TransferPlan<'a> {
    binding: &'a Binding,
    approval: ApprovalContext<'a>,
    entries: Vec<DataItem<'a>>,
    total_bytes: u64,
}

#[derive(Clone, Copy)]
pub struct Content<'a> {
    pub id: &'a str,
    pub version: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentError {
    DuplicateId,
    MissingEntry,
    UnexpectedEntry,
    VersionMismatch,
    SizeMismatch,
    DigestMismatch,
}

impl<'a> TransferPlan<'a> {
    pub fn binding(&self) -> &'a Binding {
        self.binding
    }

    pub fn approval(&self) -> ApprovalContext<'a> {
        self.approval
    }

    /// The exact requested subset, sorted by stable logical ID.
    pub fn entries(&self) -> &[DataItem<'a>] {
        &self.entries
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Verifies the entire planned set and returns no partial result.
    pub fn verify_content(&self, content: &[Content<'_>]) -> Result<(), ContentError> {
        let mut supplied = BTreeMap::new();
        for entry in content {
            if supplied.insert(entry.id, entry).is_some() {
                return Err(ContentError::DuplicateId);
            }
        }
        for item in &self.entries {
            let entry = supplied.remove(item.id).ok_or(ContentError::MissingEntry)?;
            if entry.version != item.version {
                return Err(ContentError::VersionMismatch);
            }
            if u64::try_from(entry.bytes.len()).ok() != Some(item.size_bytes) {
                return Err(ContentError::SizeMismatch);
            }
            if digest(entry.bytes) != item.sha256 {
                return Err(ContentError::DigestMismatch);
            }
        }
        if !supplied.is_empty() {
            return Err(ContentError::UnexpectedEntry);
        }
        Ok(())
    }
}

/// Plans only the necessary items using an independently verified data approval.
pub fn plan_transfer<'a>(
    request: TransferRequest<'a>,
    approval: Option<ProjectDataApproval<'a>>,
) -> Result<TransferPlan<'a>, PlanError> {
    let binding = request.binding;
    let ceiling = validate_binding(binding)?;
    if request.current_input_revision != binding.input_revision {
        return Err(PlanError::InputRevisionMismatch);
    }
    let requested = index_items(request.necessary_items, PlanError::DuplicateRequestedId)?;
    if requested.values().any(|item| item.sensitivity > ceiling) {
        return Err(PlanError::SensitivityExceedsBinding);
    }

    let approval = approval.ok_or(PlanError::ApprovalRequired(
        ApprovalRequired::MissingProjectApproval,
    ))?;
    validate_binding(approval.binding)?;
    if !identifier(approval.approval_ref) || approval.policy_revision == 0 {
        return Err(PlanError::InvalidApprovalReference);
    }
    if binding.project != approval.binding.project {
        return Err(PlanError::ProjectMismatch);
    }
    if binding.input_revision != approval.binding.input_revision {
        return Err(PlanError::InputRevisionMismatch);
    }
    if binding.recipient != approval.binding.recipient {
        return Err(PlanError::ApprovalRequired(ApprovalRequired::NewRecipient));
    }
    if binding != approval.binding {
        return Err(PlanError::BindingMismatch);
    }
    let scope = approval.scope.ok_or(PlanError::UnknownApprovedScope)?;
    if !identifier(scope.scope_ref) {
        return Err(PlanError::InvalidScopeReference);
    }
    let approved = index_items(scope.items, PlanError::DuplicateApprovedId)?;
    let mut entries = Vec::with_capacity(requested.len());
    let mut total_bytes = 0_u64;
    for item in requested.into_values() {
        let approved_item = approved.get(item.id).ok_or(PlanError::ApprovalRequired(
            ApprovalRequired::DataScopeExpansion,
        ))?;
        match_item(item, approved_item)?;
        total_bytes = total_bytes
            .checked_add(item.size_bytes)
            .ok_or(PlanError::SizeOverflow)?;
        entries.push(item);
    }
    if total_bytes > scope.max_total_bytes {
        return Err(PlanError::ByteBudgetExceeded);
    }
    Ok(TransferPlan {
        binding,
        approval: ApprovalContext {
            approval_ref: approval.approval_ref,
            policy_revision: approval.policy_revision,
            scope_ref: scope.scope_ref,
        },
        entries,
        total_bytes,
    })
}

fn validate_binding(binding: &Binding) -> Result<Sensitivity, PlanError> {
    let Binding {
        host,
        project,
        connection,
        attempt,
        input_revision,
        configuration,
        identity_ref,
        mapping,
        recipient,
        sensitivity,
    } = binding;
    let identifiers = [
        host,
        project,
        connection,
        attempt,
        input_revision,
        configuration,
        identity_ref,
        mapping,
        recipient,
    ];
    if !identifiers.into_iter().all(|value| identifier(value)) {
        return Err(PlanError::InvalidBinding);
    }
    if recipient != host {
        return Err(PlanError::RecipientIsNotHost);
    }
    Sensitivity::from_binding(sensitivity)
}

fn index_items<'a>(
    items: &[DataItem<'a>],
    duplicate: PlanError,
) -> Result<BTreeMap<&'a str, DataItem<'a>>, PlanError> {
    let mut index = BTreeMap::new();
    for item in items {
        if !identifier(item.id) {
            return Err(PlanError::InvalidItemId);
        }
        if !identifier(item.version) {
            return Err(PlanError::InvalidItemVersion);
        }
        if item.sha256.len() != 64
            || !item
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(PlanError::InvalidItemDigest);
        }
        if item.sensitivity == Sensitivity::Unknown {
            return Err(PlanError::UnknownSensitivity);
        }
        match item.kind {
            DataKind::ProjectInput => {}
            DataKind::Secret | DataKind::AccountMaterial => {
                return Err(PlanError::ProhibitedDataKind)
            }
            DataKind::Unknown => return Err(PlanError::UnknownDataKind),
        }
        if index.insert(item.id, *item).is_some() {
            return Err(duplicate);
        }
    }
    Ok(index)
}

fn match_item(item: DataItem<'_>, approved: &DataItem<'_>) -> Result<(), PlanError> {
    if item.version != approved.version {
        return Err(PlanError::ItemVersionMismatch);
    }
    if item.size_bytes != approved.size_bytes {
        return Err(PlanError::ItemSizeMismatch);
    }
    if item.sha256 != approved.sha256 {
        return Err(PlanError::ItemDigestMismatch);
    }
    if item.sensitivity != approved.sensitivity {
        return Err(PlanError::ItemSensitivityMismatch);
    }
    Ok(())
}
