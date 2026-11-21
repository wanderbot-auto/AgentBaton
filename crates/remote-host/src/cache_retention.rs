//! Read-only preview of retention for temporary remote log details.
//!
//! The caller verifies the attempt, current project policy, remote snapshot,
//! and local durable history before building this input. A reconciliation
//! cursor covers event metadata. The main history must separately confirm each
//! cached detail. Any later cleanup needs its own current evidence and authority.

use crate::protocol::{identifier, Binding, Job, Snapshot, MAX_EVENTS, MAX_JOBS};
use std::collections::{BTreeMap, BTreeSet};

// The v1 journal in Ledger::ingest and the companion store uses this ceiling.
const MAX_EVENT_SEQUENCE: i64 = 10_000;

/// Identity of one immutable detail within its attempt binding and fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetailKey<'a> {
    pub id: &'a str,
    pub sequence: i64,
    pub sha256: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    TemporaryDetail,
    Protected,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pin {
    Pinned,
    Unpinned,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveReference {
    Present,
    Absent,
    Unknown,
}

/// Caller-verified cache inventory and its effective pin/reference observations.
#[derive(Debug, Clone, Copy)]
pub struct CacheEntry<'a> {
    pub detail: DetailKey<'a>,
    pub binding: &'a Binding,
    pub fence: i64,
    pub cached_at_ms: i64,
    pub kind: EntryKind,
    pub pin: Pin,
    pub active_reference: ActiveReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailRetention {
    Forever,
    ExpireAfterMs(i64),
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectRetention<'a> {
    pub project: &'a str,
    pub details: DetailRetention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptState {
    Settled,
    Active,
    OutcomeUnknown,
}

/// A fresh confirmation from the local primary history after durable commit.
/// The cursor is a contiguous prefix. Each detail confirms the exact bytes
/// retained for that event. Remote claims and a transport ACK supply no proof here.
#[derive(Debug, Clone, Copy)]
pub struct LocalHistoryConfirmation<'a> {
    pub binding: &'a Binding,
    pub fence: i64,
    pub committed_through: i64,
    pub verified_at_ms: i64,
    pub valid_until_ms: i64,
    pub details: &'a [DetailKey<'a>],
}

#[derive(Debug, Clone, Copy)]
pub struct PreviewInput<'a> {
    pub original_binding: &'a Binding,
    pub fence: i64,
    pub attempt_state: AttemptState,
    /// Current authenticated reconciliation page for this attempt.
    pub snapshot: Option<&'a Snapshot>,
    pub local_history: Option<LocalHistoryConfirmation<'a>>,
    pub project_retention: Option<ProjectRetention<'a>>,
    /// Trusted evaluation time using the same Unix millisecond basis as evidence.
    pub now_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryReason {
    EntryIdentityUnverified,
    SnapshotUnavailable,
    SnapshotBindingMismatch,
    SnapshotFenceMismatch,
    InvalidSnapshot,
    MoreReconciliationPages,
    MissingLocalConfirmation,
    ConfirmationBindingMismatch,
    ConfirmationFenceMismatch,
    InvalidConfirmationTime,
    ConfirmationExpired,
    ReconciliationCursorMismatch,
    InvalidDetailConfirmations,
    EntryBeyondCommittedCursor,
    DetailNotConfirmed,
    DetailMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryHistory {
    Confirmed,
    Unconfirmed(HistoryReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainReason {
    InvalidAttemptBinding,
    InvalidFence,
    InvalidEntryId,
    InvalidSequence,
    DuplicateEntryId,
    EntryBindingMismatch,
    EntryFenceMismatch,
    ProtectedEntryKind,
    UnknownEntryKind,
    InvalidTime,
    Pinned,
    UnknownPin,
    ReferencedByActiveWork,
    UnknownReferences,
    ActiveAttempt,
    UnknownAttemptOutcome,
    UnknownProjectRetention,
    RetentionProjectMismatch,
    PermanentRetention,
    InvalidTtl,
    WithinTtl,
    HistoryUnconfirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteCache {
    CleanupCandidate,
    Retained(RetainReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryPreview<'a> {
    /// A stable logical identifier. Invalid strings are withheld from output.
    pub id: Option<&'a str>,
    pub sequence: i64,
    pub primary_history: PrimaryHistory,
    pub remote_cache: RemoteCache,
}

/// Returns one preview per entry in sequence and logical ID order.
pub fn preview_cache_retention<'a>(
    input: PreviewInput<'a>,
    entries: &[CacheEntry<'a>],
) -> Vec<EntryPreview<'a>> {
    let scope = validate_scope(input);
    let history = scope
        .map_err(|_| HistoryReason::EntryIdentityUnverified)
        .and_then(|()| history_index(input));
    let attempt_hold = attempt_hold(input);
    let project_ttl = project_ttl(input);
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for entry in entries {
        if !seen.insert(entry.detail.id) {
            duplicates.insert(entry.detail.id);
        }
    }
    let mut preview = Vec::with_capacity(entries.len());
    for entry in entries {
        let identity = scope.and_then(|()| check_entry_identity(input, entry, &duplicates));
        let primary_history = if identity.is_ok() {
            entry_history(entry.detail, &history)
        } else {
            PrimaryHistory::Unconfirmed(HistoryReason::EntryIdentityUnverified)
        };
        let eligibility = identity.and_then(|()| {
            check_cleanup_eligibility(
                entry,
                input.now_ms,
                attempt_hold,
                project_ttl,
                primary_history,
            )
        });
        preview.push(EntryPreview {
            id: identifier(entry.detail.id).then_some(entry.detail.id),
            sequence: entry.detail.sequence,
            primary_history,
            remote_cache: match eligibility {
                Ok(()) => RemoteCache::CleanupCandidate,
                Err(reason) => RemoteCache::Retained(reason),
            },
        });
    }
    preview.sort_unstable_by_key(|entry| (entry.sequence, entry.id));
    preview
}

fn validate_scope(input: PreviewInput<'_>) -> Result<(), RetainReason> {
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
    } = input.original_binding;
    if host != recipient
        || ![
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
        ]
        .into_iter()
        .all(|id| identifier(id))
    {
        return Err(RetainReason::InvalidAttemptBinding);
    }
    if !(1..i64::MAX).contains(&input.fence) {
        return Err(RetainReason::InvalidFence);
    }
    Ok(())
}

fn check_entry_identity(
    input: PreviewInput<'_>,
    entry: &CacheEntry<'_>,
    duplicates: &BTreeSet<&str>,
) -> Result<(), RetainReason> {
    use RetainReason::*;
    if !identifier(entry.detail.id) {
        return Err(InvalidEntryId);
    }
    if duplicates.contains(entry.detail.id) {
        return Err(DuplicateEntryId);
    }
    if !(1..=MAX_EVENT_SEQUENCE).contains(&entry.detail.sequence) {
        return Err(InvalidSequence);
    }
    if entry.binding != input.original_binding {
        return Err(EntryBindingMismatch);
    }
    if entry.fence != input.fence {
        return Err(EntryFenceMismatch);
    }
    Ok(())
}

struct HistoryIndex<'a> {
    cursor: i64,
    details: BTreeMap<&'a str, DetailKey<'a>>,
}

fn history_index(input: PreviewInput<'_>) -> Result<HistoryIndex<'_>, HistoryReason> {
    use HistoryReason::*;
    let snapshot = input.snapshot.ok_or(SnapshotUnavailable)?;
    if &snapshot.binding != input.original_binding {
        return Err(SnapshotBindingMismatch);
    }
    if snapshot.fence != input.fence {
        return Err(SnapshotFenceMismatch);
    }
    if !(0..=MAX_EVENT_SEQUENCE).contains(&snapshot.cursor)
        || snapshot.events.len() > MAX_EVENTS
        || snapshot.jobs.len() > MAX_JOBS
        || (snapshot.confirmed_stopped
            && (!snapshot.cancel_requested || !snapshot.jobs.iter().all(child_join_confirmed)))
        || snapshot.events.iter().any(|event| {
            !(1..=MAX_EVENT_SEQUENCE).contains(&event.sequence)
                || event.kind.len() > 64
                || event.job.as_deref().is_some_and(|id| !identifier(id))
        })
        || snapshot
            .events
            .last()
            .is_some_and(|event| event.sequence != snapshot.cursor)
        || snapshot
            .events
            .windows(2)
            .any(|events| events[0].sequence.checked_add(1) != Some(events[1].sequence))
    {
        return Err(InvalidSnapshot);
    }
    if snapshot.more {
        return Err(MoreReconciliationPages);
    }
    let local = input.local_history.ok_or(MissingLocalConfirmation)?;
    if local.binding != input.original_binding {
        return Err(ConfirmationBindingMismatch);
    }
    if local.fence != input.fence {
        return Err(ConfirmationFenceMismatch);
    }
    if local.verified_at_ms < 0
        || input.now_ms < local.verified_at_ms
        || local.valid_until_ms <= local.verified_at_ms
    {
        return Err(InvalidConfirmationTime);
    }
    if input.now_ms >= local.valid_until_ms {
        return Err(ConfirmationExpired);
    }
    if local.committed_through != snapshot.cursor {
        return Err(ReconciliationCursorMismatch);
    }
    let mut details = BTreeMap::new();
    for detail in local.details {
        if !identifier(detail.id)
            || detail.sequence <= 0
            || detail.sequence > local.committed_through
            || details.insert(detail.id, *detail).is_some()
        {
            return Err(InvalidDetailConfirmations);
        }
    }
    Ok(HistoryIndex {
        cursor: local.committed_through,
        details,
    })
}

fn entry_history(
    detail: DetailKey<'_>,
    history: &Result<HistoryIndex<'_>, HistoryReason>,
) -> PrimaryHistory {
    use HistoryReason::*;
    let reason = match history {
        Err(reason) => *reason,
        Ok(confirmed) if detail.sequence > confirmed.cursor => EntryBeyondCommittedCursor,
        Ok(confirmed) => match confirmed.details.get(detail.id) {
            Some(durable) if durable == &detail => return PrimaryHistory::Confirmed,
            Some(_) => DetailMismatch,
            None => DetailNotConfirmed,
        },
    };
    PrimaryHistory::Unconfirmed(reason)
}

fn attempt_hold(input: PreviewInput<'_>) -> Result<(), RetainReason> {
    use RetainReason::*;
    match input.attempt_state {
        AttemptState::Active => return Err(ActiveAttempt),
        AttemptState::OutcomeUnknown => return Err(UnknownAttemptOutcome),
        AttemptState::Settled => {}
    }
    let mut active = false;
    if let Some(snapshot) = input
        .snapshot
        .filter(|s| &s.binding == input.original_binding && s.fence == input.fence)
    {
        active = snapshot.cancel_requested && !snapshot.confirmed_stopped;
        for job in &snapshot.jobs {
            match job.state.as_str() {
                "reserved" | "running" | "effect_intent" => active = true,
                "succeeded"
                | "failed_no_effect"
                | "stopped_without_effect"
                | "spawn_failed_no_effect" => active |= !child_join_confirmed(job),
                _ => return Err(UnknownAttemptOutcome),
            }
        }
    }
    if active {
        return Err(ActiveAttempt);
    }
    Ok(())
}

fn child_join_confirmed(job: &Job) -> bool {
    matches!(
        job.stop_evidence.as_deref(),
        Some("owned_child_wait_exit_0" | "owned_child_wait_nonzero" | "spawn_failed_no_child")
    )
}

fn project_ttl(input: PreviewInput<'_>) -> Result<i64, RetainReason> {
    use RetainReason::*;
    let project = input.project_retention.ok_or(UnknownProjectRetention)?;
    if project.project != input.original_binding.project {
        return Err(RetentionProjectMismatch);
    }
    match project.details {
        DetailRetention::Forever => Err(PermanentRetention),
        DetailRetention::ExpireAfterMs(ttl) if ttl < 0 => Err(InvalidTtl),
        DetailRetention::ExpireAfterMs(ttl) => Ok(ttl),
    }
}

fn check_cleanup_eligibility(
    entry: &CacheEntry<'_>,
    now_ms: i64,
    attempt_hold: Result<(), RetainReason>,
    project_ttl: Result<i64, RetainReason>,
    primary_history: PrimaryHistory,
) -> Result<(), RetainReason> {
    use RetainReason::*;
    match entry.kind {
        EntryKind::Protected => return Err(ProtectedEntryKind),
        EntryKind::Unknown => return Err(UnknownEntryKind),
        EntryKind::TemporaryDetail => {}
    }
    if entry.cached_at_ms < 0 || now_ms < entry.cached_at_ms {
        return Err(InvalidTime);
    }
    match entry.pin {
        Pin::Pinned => return Err(Pinned),
        Pin::Unknown => return Err(UnknownPin),
        Pin::Unpinned => {}
    }
    match entry.active_reference {
        ActiveReference::Present => return Err(ReferencedByActiveWork),
        ActiveReference::Unknown => return Err(UnknownReferences),
        ActiveReference::Absent => {}
    }
    attempt_hold?;
    let ttl = project_ttl?;
    if now_ms - entry.cached_at_ms <= ttl {
        return Err(WithinTtl);
    }
    if primary_history != PrimaryHistory::Confirmed {
        return Err(HistoryUnconfirmed);
    }
    Ok(())
}
