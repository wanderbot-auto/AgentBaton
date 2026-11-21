//! Project history retention preview over metadata from one Core read snapshot.
//!
//! The caller supplies verified record classifications, owner records, pins,
//! active references, and source-specific derived references from that snapshot.
//! A preview provides no authorization to apply a cleanup. Any later operation
//! needs to confirm the preview against current metadata before changing history.

use super::bounded_id;
use crate::{
    conversations::Conversation,
    registry::Project,
    sessions::ManagedSession,
    storage::{Task, TaskState},
    CoreError, Result,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
};

#[path = "retention_snapshot.rs"]
mod retention_snapshot;
pub use retention_snapshot::{HistorySnapshotGap, ProjectHistoryRetentionPreview};

pub const DEFAULT_DETAIL_RETENTION_DAYS: u64 = 90;
const MILLIS_PER_DAY: i64 = 86_400_000;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailRetention {
    #[default]
    Default,
    Days(NonZeroU64),
    Permanent,
}

/// Applies to new body capture. Previously stored details still use the TTL.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyCapture {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HistoryRetentionPolicy {
    pub details: DetailRetention,
    pub body_capture: BodyCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryRecordKind {
    Body,
    Log,
    SourceFile,
    AdoptedResult,
    StandaloneReport,
    Summary,
    AcceptanceRecord,
    GapMarker,
    Unclassified,
}

#[derive(Debug, Clone, Copy)]
pub enum HistorySource<'a> {
    Project(&'a Project),
    Task(&'a Task),
    Conversation(&'a Conversation),
    Session {
        session: &'a ManagedSession,
        task: &'a Task,
    },
    Unknown,
}

/// References beyond the immediate owner, such as active Goal work that still
/// uses this retained detail. Uncertain references keep the record protected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryUse {
    Inactive,
    Active,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct HistoryRecord<'a> {
    /// Stable ID within the metadata inventory, never a file path.
    pub id: &'a str,
    pub kind: HistoryRecordKind,
    pub source: HistorySource<'a>,
    pub captured_at_ms: Option<i64>,
    pub pinned: bool,
    pub active_use: HistoryUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DerivedHistoryKind {
    SearchIndex,
    Cache,
}

#[derive(Debug, Clone, Copy)]
pub struct DerivedHistoryRecord<'a> {
    /// ID of a source-specific derived entry, not a whole index or cache.
    pub id: &'a str,
    pub kind: DerivedHistoryKind,
    pub project_id: &'a str,
    pub source_record_id: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryInvalidation {
    pub id: String,
    pub kind: DerivedHistoryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryKeepReason {
    ProtectedKind,
    Pinned,
    ActiveUse,
    ActivityUnknown,
    RetentionMetadataUnknown,
    PermanentRetention,
    MissingTimestamp,
    FutureTimestamp,
    WithinRetention,
}

struct InventoryRecord<'a> {
    id: &'a str,
    kind: HistoryRecordKind,
    source: HistorySource<'a>,
    captured_at_ms: Option<i64>,
    pinned: Option<bool>,
    active_use: InventoryUse,
}

enum InventoryUse {
    Additional(HistoryUse),
    Observed(HistoryUse),
}

impl<'a> From<HistoryRecord<'a>> for InventoryRecord<'a> {
    fn from(record: HistoryRecord<'a>) -> Self {
        Self {
            id: record.id,
            kind: record.kind,
            source: record.source,
            captured_at_ms: record.captured_at_ms,
            pinned: Some(record.pinned),
            active_use: InventoryUse::Additional(record.active_use),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryGapReason {
    DetailExpired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum HistoryCleanupDecision {
    Keep {
        reason: HistoryKeepReason,
    },
    ExpireDetail {
        /// A later cleanup must record this gap for the same stable record ID.
        gap: HistoryGapReason,
        invalidations: Vec<HistoryInvalidation>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryCleanupEntry {
    pub id: String,
    pub kind: HistoryRecordKind,
    pub decision: HistoryCleanupDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryCleanupPlan {
    pub project_id: String,
    pub evaluated_at_ms: i64,
    /// Details captured at or before this instant are eligible for expiry.
    pub expiry_cutoff_ms: Option<i64>,
    pub policy: HistoryRetentionPolicy,
    /// Ordered by record ID. Other projects' records are excluded.
    pub entries: Vec<HistoryCleanupEntry>,
}

fn changed(message: &str) -> CoreError {
    CoreError::new("INPUT_CHANGED", message)
}

fn cutoff(retention: DetailRetention, evaluated_at_ms: i64) -> Result<Option<i64>> {
    let days = match retention {
        DetailRetention::Default => DEFAULT_DETAIL_RETENTION_DAYS,
        DetailRetention::Days(days) => days.get(),
        DetailRetention::Permanent => return Ok(None),
    };
    i64::try_from(days)
        .ok()
        .and_then(|days| days.checked_mul(MILLIS_PER_DAY))
        .and_then(|duration| evaluated_at_ms.checked_sub(duration))
        .map(Some)
        .ok_or_else(|| {
            CoreError::new(
                "INVALID_REQUEST",
                "History retention cutoff is out of range",
            )
        })
}

fn task_active(task: &Task) -> bool {
    !matches!(task.state, TaskState::Accepted | TaskState::Cancelled)
}

fn session_active(state: &str, turn_state: Option<&str>) -> bool {
    !matches!(state, "stopped" | "failed")
        || !matches!(
            turn_state,
            None | Some("completed" | "failed" | "cancelled")
        )
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum OwnerKind {
    Project,
    Task,
    Conversation,
    Session,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct OwnerState<'a> {
    project: &'a str,
    task: Option<&'a str>,
    active: bool,
}

type Owners<'a> = BTreeMap<(OwnerKind, &'a str), OwnerState<'a>>;

fn remember_owner<'a>(
    owners: &mut Owners<'a>,
    kind: OwnerKind,
    id: &'a str,
    state: OwnerState<'a>,
) -> Result<()> {
    if !bounded_id(state.project) || !bounded_id(id) {
        return Err(changed(
            "History source identity or project ownership is invalid",
        ));
    }
    if owners
        .insert((kind, id), state)
        .is_some_and(|prior| prior != state)
    {
        return Err(changed(
            "Conflicting history source ownership or active state",
        ));
    }
    Ok(())
}

impl<'a> HistorySource<'a> {
    fn project_and_activity(self, owners: &mut Owners<'a>) -> Result<(&'a str, bool)> {
        let (kind, id, project, parent_task, active) = match self {
            Self::Project(project) => (
                OwnerKind::Project,
                project.id.as_str(),
                project.id.as_str(),
                None,
                false,
            ),
            Self::Task(task) => (
                OwnerKind::Task,
                task.id.as_str(),
                task.project_id.as_str(),
                None,
                task_active(task),
            ),
            Self::Conversation(conversation) => (
                OwnerKind::Conversation,
                conversation.id.as_str(),
                conversation.project_id.as_str(),
                None,
                session_active(&conversation.state, conversation.turn_state.as_deref()),
            ),
            Self::Session { session, task } => {
                if session.task_id != task.id || session.project_id != task.project_id {
                    return Err(changed("History session and task ownership disagree"));
                }
                let (_, task_is_active) = Self::Task(task).project_and_activity(owners)?;
                (
                    OwnerKind::Session,
                    session.id.as_str(),
                    session.project_id.as_str(),
                    Some(task.id.as_str()),
                    task_is_active || session_active(&session.state, session.turn_state.as_deref()),
                )
            }
            Self::Unknown => return Err(changed("History record source is unknown")),
        };
        remember_owner(
            owners,
            kind,
            id,
            OwnerState {
                project,
                task: parent_task,
                active,
            },
        )?;
        Ok((project, active))
    }
}

fn keep_reason(
    record: &InventoryRecord<'_>,
    owner_active: bool,
    derived_inventory_known: bool,
    evaluated_at_ms: i64,
    expiry_cutoff_ms: Option<i64>,
) -> Option<HistoryKeepReason> {
    use HistoryKeepReason::*;
    if !matches!(
        record.kind,
        HistoryRecordKind::Body | HistoryRecordKind::Log
    ) {
        return Some(ProtectedKind);
    }
    if record.pinned == Some(true) {
        return Some(Pinned);
    }
    match record.active_use {
        InventoryUse::Additional(_) if owner_active => return Some(ActiveUse),
        InventoryUse::Additional(HistoryUse::Active)
        | InventoryUse::Observed(HistoryUse::Active) => return Some(ActiveUse),
        InventoryUse::Additional(HistoryUse::Unknown)
        | InventoryUse::Observed(HistoryUse::Unknown) => return Some(ActivityUnknown),
        InventoryUse::Additional(HistoryUse::Inactive)
        | InventoryUse::Observed(HistoryUse::Inactive) => (),
    }
    let Some(expiry_cutoff_ms) = expiry_cutoff_ms else {
        return Some(PermanentRetention);
    };
    if record.pinned.is_none() || !derived_inventory_known {
        return Some(RetentionMetadataUnknown);
    }
    match record.captured_at_ms {
        None => Some(MissingTimestamp),
        Some(time) if time > evaluated_at_ms => Some(FutureTimestamp),
        Some(time) if time > expiry_cutoff_ms => Some(WithinRetention),
        Some(_) => None,
    }
}

/// Compute the preview at an explicit Unix millisecond time using 24-hour days.
/// Record IDs must be unique across the supplied snapshot. Derived IDs are
/// unique within their kind and must link to a known record of the same project.
/// A conflicting or unbound input fails the preview before any plan is returned.
pub fn plan_project_history_cleanup(
    project_id: &str,
    policy: HistoryRetentionPolicy,
    evaluated_at_ms: i64,
    records: &[HistoryRecord<'_>],
    derived: &[DerivedHistoryRecord<'_>],
) -> Result<HistoryCleanupPlan> {
    plan_inventory_cleanup(
        project_id,
        policy,
        evaluated_at_ms,
        records.iter().copied().map(InventoryRecord::from),
        Some(derived),
    )
}

fn plan_inventory_cleanup<'a>(
    project_id: &str,
    policy: HistoryRetentionPolicy,
    evaluated_at_ms: i64,
    records: impl IntoIterator<Item = InventoryRecord<'a>>,
    derived: Option<&[DerivedHistoryRecord<'a>]>,
) -> Result<HistoryCleanupPlan> {
    if !bounded_id(project_id) {
        return Err(CoreError::new(
            "INVALID_REQUEST",
            "Invalid history project ID",
        ));
    }
    let expiry_cutoff_ms = cutoff(policy.details, evaluated_at_ms)?;
    let mut owners = Owners::new();
    let mut sources = BTreeMap::new();
    for record in records {
        let (owner_project, active) = record.source.project_and_activity(&mut owners)?;
        if !bounded_id(record.id) {
            return Err(changed("History record identity is invalid"));
        }
        if sources
            .insert(record.id, (record, owner_project, active))
            .is_some()
        {
            return Err(changed("Duplicate history record identity"));
        }
    }

    let mut seen = BTreeSet::new();
    let mut by_source: BTreeMap<&str, Vec<&DerivedHistoryRecord<'_>>> = BTreeMap::new();
    for item in derived.unwrap_or_default() {
        if !bounded_id(item.id)
            || !bounded_id(item.project_id)
            || !bounded_id(item.source_record_id)
        {
            return Err(changed(
                "Derived history identity or source link is invalid",
            ));
        }
        let Some((_, source_project, _)) = sources.get(item.source_record_id) else {
            return Err(changed("Derived history source record is missing"));
        };
        if *source_project != item.project_id {
            return Err(changed(
                "Derived history source and project ownership disagree",
            ));
        }
        if !seen.insert((item.kind, item.id)) {
            return Err(changed("Duplicate derived history identity"));
        }
        by_source
            .entry(item.source_record_id)
            .or_default()
            .push(item);
    }

    let mut entries = Vec::new();
    for (id, (record, owner_project, active)) in sources {
        if owner_project != project_id {
            continue;
        }
        let decision = if let Some(reason) = keep_reason(
            &record,
            active,
            derived.is_some(),
            evaluated_at_ms,
            expiry_cutoff_ms,
        ) {
            HistoryCleanupDecision::Keep { reason }
        } else {
            let mut linked = by_source.remove(id).unwrap_or_default();
            linked.sort_unstable_by_key(|item| (item.kind, item.id));
            HistoryCleanupDecision::ExpireDetail {
                gap: HistoryGapReason::DetailExpired,
                invalidations: linked
                    .into_iter()
                    .map(|item| HistoryInvalidation {
                        id: item.id.to_owned(),
                        kind: item.kind,
                    })
                    .collect(),
            }
        };
        entries.push(HistoryCleanupEntry {
            id: id.to_owned(),
            kind: record.kind,
            decision,
        });
    }
    Ok(HistoryCleanupPlan {
        project_id: project_id.to_owned(),
        evaluated_at_ms,
        expiry_cutoff_ms,
        policy,
        entries,
    })
}
