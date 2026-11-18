use agentbaton_remote_host::cache_retention::{
    preview_cache_retention, ActiveReference, AttemptState, CacheEntry, DetailKey, DetailRetention,
    EntryKind, EntryPreview, HistoryReason, LocalHistoryConfirmation, Pin, PreviewInput,
    PrimaryHistory, ProjectRetention, RemoteCache, RetainReason,
};
use agentbaton_remote_host::protocol::{Binding, Event, Job, Snapshot};

const NOW: i64 = 1_000;
const TTL: i64 = 100;
const FENCE: i64 = 3;
const A: DetailKey<'static> = DetailKey {
    id: "alpha",
    sequence: 1,
    sha256: [1; 32],
};
const B: DetailKey<'static> = DetailKey {
    id: "beta",
    sequence: 2,
    sha256: [2; 32],
};
const C: DetailKey<'static> = DetailKey {
    id: "gamma",
    sequence: 2,
    sha256: [3; 32],
};

fn binding() -> Binding {
    Binding {
        host: "remote-host".into(),
        project: "project".into(),
        connection: "named-connection".into(),
        attempt: "attempt".into(),
        input_revision: "input-revision".into(),
        configuration: "config".into(),
        identity_ref: "identity".into(),
        mapping: "mapping".into(),
        recipient: "remote-host".into(),
        sensitivity: "internal".into(),
    }
}

fn event(sequence: i64) -> Event {
    Event {
        sequence,
        kind: "synthetic_detail".into(),
        job: None,
    }
}

fn snapshot(binding: &Binding) -> Snapshot {
    Snapshot {
        binding: binding.clone(),
        fence: FENCE,
        // A final page can omit already ingested events. The fixture confirms
        // its full prefix separately through the primary history input.
        events: vec![event(2), event(3)],
        cursor: 3,
        more: false,
        jobs: vec![],
        cancel_requested: false,
        confirmed_stopped: false,
        paused: false,
        revoked: false,
        used_operations: 0,
        used_bytes: 0,
        history_owner: "controller_local_primary".into(),
        cache_policy: "remote_pinned_until_explicit_project_retention_integration".into(),
    }
}

fn entry<'a>(binding: &'a Binding, detail: DetailKey<'a>) -> CacheEntry<'a> {
    CacheEntry {
        detail,
        binding,
        fence: FENCE,
        cached_at_ms: NOW - TTL - 1,
        kind: EntryKind::TemporaryDetail,
        pin: Pin::Unpinned,
        active_reference: ActiveReference::Absent,
    }
}

fn preview_input<'a>(
    binding: &'a Binding,
    snapshot: &'a Snapshot,
    details: &'a [DetailKey<'a>],
) -> PreviewInput<'a> {
    PreviewInput {
        original_binding: binding,
        snapshot: Some(snapshot),
        local_history: Some(LocalHistoryConfirmation {
            binding,
            fence: FENCE,
            committed_through: 3,
            verified_at_ms: NOW - 10,
            valid_until_ms: NOW + 10,
            details,
        }),
        project_retention: Some(ProjectRetention {
            project: &binding.project,
            details: DetailRetention::ExpireAfterMs(TTL),
        }),
        attempt_state: AttemptState::Settled,
        fence: FENCE,
        now_ms: NOW,
    }
}

fn one<'a>(input: PreviewInput<'a>, entry: CacheEntry<'a>) -> EntryPreview<'a> {
    let previews = preview_cache_retention(input, &[entry]);
    assert_eq!(previews.len(), 1);
    previews[0]
}

fn assert_history_hold(input: PreviewInput<'_>, entry: CacheEntry<'_>, reason: HistoryReason) {
    let result = one(input, entry);
    assert_eq!(result.primary_history, PrimaryHistory::Unconfirmed(reason));
    assert_eq!(
        result.remote_cache,
        RemoteCache::Retained(RetainReason::HistoryUnconfirmed)
    );
}

#[test]
fn only_eligible_old_details_enter_preview_in_sequence_then_logical_id_order() {
    let binding = binding();
    let snapshot = snapshot(&binding);
    let input = preview_input(&binding, &snapshot, &[C, A, B]);
    let recent = CacheEntry {
        cached_at_ms: NOW,
        ..entry(&binding, B)
    };
    let entries = [entry(&binding, C), recent, entry(&binding, A)];
    let results = preview_cache_retention(input, &entries);
    let ids: Vec<_> = results
        .iter()
        .map(|result| (result.sequence, result.id))
        .collect();
    assert_eq!(
        ids,
        [(1, Some("alpha")), (2, Some("beta")), (2, Some("gamma"))]
    );
    assert!(results
        .iter()
        .all(|result| result.primary_history == PrimaryHistory::Confirmed));
    assert_eq!(results[0].remote_cache, RemoteCache::CleanupCandidate);
    assert_eq!(
        results[1].remote_cache,
        RemoteCache::Retained(RetainReason::WithinTtl)
    );
    assert_eq!(results[2].remote_cache, RemoteCache::CleanupCandidate);
    let mut reshuffled = entries;
    reshuffled.reverse();
    assert_eq!(results, preview_cache_retention(input, &reshuffled));
    assert!(preview_cache_retention(input, &[]).is_empty());
}

#[test]
fn cursors_and_remote_history_claims_require_exact_primary_detail_confirmation() {
    let binding = binding();
    let mut snapshot = snapshot(&binding);
    snapshot.history_owner = "local_body_is_already_durable".into();
    snapshot.cache_policy = "delete_all_remote_detail".into();
    let input = preview_input(&binding, &snapshot, &[]);
    let entry = entry(&binding, B);
    assert_history_hold(
        PreviewInput {
            local_history: None,
            ..input
        },
        entry,
        HistoryReason::MissingLocalConfirmation,
    );
    assert_history_hold(input, entry, HistoryReason::DetailNotConfirmed);
    let wrong_hash = DetailKey {
        sha256: [99; 32],
        ..B
    };
    let wrong_sequence = DetailKey { sequence: 1, ..B };
    for durable in [wrong_hash, wrong_sequence] {
        assert_history_hold(
            preview_input(&binding, &snapshot, &[durable]),
            entry,
            HistoryReason::DetailMismatch,
        );
    }
    let later = CacheEntry {
        detail: DetailKey { sequence: 4, ..B },
        ..entry
    };
    assert_history_hold(input, later, HistoryReason::EntryBeyondCommittedCursor);
}

#[test]
fn reconnect_holds_through_more_pages_malformed_pages_and_cursor_disagreement() {
    let binding = binding();
    let original = snapshot(&binding);
    let input = preview_input(&binding, &original, &[A]);
    let entry = entry(&binding, A);
    assert_history_hold(
        PreviewInput {
            snapshot: None,
            ..input
        },
        entry,
        HistoryReason::SnapshotUnavailable,
    );
    let more = Snapshot {
        more: true,
        ..original.clone()
    };
    assert_history_hold(
        preview_input(&binding, &more, &[A]),
        entry,
        HistoryReason::MoreReconciliationPages,
    );
    let malformed = [
        Snapshot {
            events: vec![event(1), event(3)],
            ..original.clone()
        },
        Snapshot {
            events: vec![event(2), event(2), event(3)],
            ..original.clone()
        },
        Snapshot {
            cursor: 2,
            ..original.clone()
        },
        Snapshot {
            events: vec![],
            cursor: -1,
            ..original.clone()
        },
        Snapshot {
            events: vec![event(0), event(1)],
            cursor: 1,
            ..original.clone()
        },
        Snapshot {
            events: vec![event(10_001)],
            cursor: 10_001,
            ..original.clone()
        },
        Snapshot {
            confirmed_stopped: true,
            ..original.clone()
        },
    ];
    for page in malformed {
        assert_history_hold(
            preview_input(&binding, &page, &[A]),
            entry,
            HistoryReason::InvalidSnapshot,
        );
    }
    for cursor in [-1, 0, 2, 4] {
        let not_reconciled = LocalHistoryConfirmation {
            committed_through: cursor,
            ..input.local_history.unwrap()
        };
        assert_history_hold(
            PreviewInput {
                local_history: Some(not_reconciled),
                ..input
            },
            entry,
            HistoryReason::ReconciliationCursorMismatch,
        );
    }
    let empty_final_page = Snapshot {
        events: vec![],
        ..original
    };
    assert_eq!(
        one(preview_input(&binding, &empty_final_page, &[A]), entry).remote_cache,
        RemoteCache::CleanupCandidate
    );
}

#[test]
fn each_identity_boundary_must_match_the_exact_original_binding_and_fence() {
    let binding = binding();
    let original = snapshot(&binding);
    let input = preview_input(&binding, &original, &[A]);
    let entry = entry(&binding, A);
    let changes: [fn(&mut Binding) -> &mut String; 10] = [
        |binding| &mut binding.host,
        |binding| &mut binding.project,
        |binding| &mut binding.connection,
        |binding| &mut binding.attempt,
        |binding| &mut binding.input_revision,
        |binding| &mut binding.configuration,
        |binding| &mut binding.identity_ref,
        |binding| &mut binding.mapping,
        |binding| &mut binding.recipient,
        |binding| &mut binding.sensitivity,
    ];
    for change in changes {
        let mut wrong_binding = binding.clone();
        change(&mut wrong_binding).push_str("-other");
        assert_eq!(
            one(
                input,
                CacheEntry {
                    binding: &wrong_binding,
                    ..entry
                }
            )
            .remote_cache,
            RemoteCache::Retained(RetainReason::EntryBindingMismatch)
        );
        let wrong_snapshot = Snapshot {
            binding: wrong_binding.clone(),
            ..original.clone()
        };
        assert_history_hold(
            preview_input(&binding, &wrong_snapshot, &[A]),
            entry,
            HistoryReason::SnapshotBindingMismatch,
        );
        let wrong_confirmation = LocalHistoryConfirmation {
            binding: &wrong_binding,
            ..input.local_history.unwrap()
        };
        assert_history_hold(
            PreviewInput {
                local_history: Some(wrong_confirmation),
                ..input
            },
            entry,
            HistoryReason::ConfirmationBindingMismatch,
        );
    }
    for fence in [FENCE - 1, FENCE + 1] {
        assert_eq!(
            one(input, CacheEntry { fence, ..entry }).remote_cache,
            RemoteCache::Retained(RetainReason::EntryFenceMismatch)
        );
        let wrong_snapshot = Snapshot {
            fence,
            ..original.clone()
        };
        assert_history_hold(
            preview_input(&binding, &wrong_snapshot, &[A]),
            entry,
            HistoryReason::SnapshotFenceMismatch,
        );
        let wrong_confirmation = LocalHistoryConfirmation {
            fence,
            ..input.local_history.unwrap()
        };
        assert_history_hold(
            PreviewInput {
                local_history: Some(wrong_confirmation),
                ..input
            },
            entry,
            HistoryReason::ConfirmationFenceMismatch,
        );
    }
}

#[test]
fn local_confirmation_must_be_fresh_and_unambiguous() {
    let binding = binding();
    let snapshot = snapshot(&binding);
    let input = preview_input(&binding, &snapshot, &[A]);
    let entry = entry(&binding, A);
    let base = input.local_history.unwrap();
    let times = [
        (
            LocalHistoryConfirmation {
                valid_until_ms: NOW,
                ..base
            },
            HistoryReason::ConfirmationExpired,
        ),
        (
            LocalHistoryConfirmation {
                verified_at_ms: NOW + 1,
                ..base
            },
            HistoryReason::InvalidConfirmationTime,
        ),
        (
            LocalHistoryConfirmation {
                verified_at_ms: -1,
                ..base
            },
            HistoryReason::InvalidConfirmationTime,
        ),
        (
            LocalHistoryConfirmation {
                valid_until_ms: base.verified_at_ms,
                ..base
            },
            HistoryReason::InvalidConfirmationTime,
        ),
    ];
    for (local, reason) in times {
        assert_history_hold(
            PreviewInput {
                local_history: Some(local),
                ..input
            },
            entry,
            reason,
        );
    }
    let just_verified = LocalHistoryConfirmation {
        verified_at_ms: NOW,
        ..base
    };
    assert_eq!(
        one(
            PreviewInput {
                local_history: Some(just_verified),
                ..input
            },
            entry
        )
        .primary_history,
        PrimaryHistory::Confirmed
    );
    for details in [
        [A, A],
        [
            A,
            DetailKey {
                sha256: [5; 32],
                ..A
            },
        ],
        [A, DetailKey { sequence: 0, ..B }],
        [A, DetailKey { sequence: 4, ..B }],
        [
            A,
            DetailKey {
                id: "../remote/invalid",
                ..B
            },
        ],
    ] {
        assert_history_hold(
            preview_input(&binding, &snapshot, &details),
            entry,
            HistoryReason::InvalidDetailConfirmations,
        );
    }
}

#[test]
fn pin_reference_and_attempt_protection_keeps_even_durably_confirmed_detail() {
    let binding = binding();
    let snapshot = snapshot(&binding);
    let input = preview_input(&binding, &snapshot, &[A]);
    let entry = entry(&binding, A);
    let protection = [
        (
            CacheEntry {
                pin: Pin::Pinned,
                ..entry
            },
            RetainReason::Pinned,
        ),
        (
            CacheEntry {
                pin: Pin::Unknown,
                ..entry
            },
            RetainReason::UnknownPin,
        ),
        (
            CacheEntry {
                active_reference: ActiveReference::Present,
                ..entry
            },
            RetainReason::ReferencedByActiveWork,
        ),
        (
            CacheEntry {
                active_reference: ActiveReference::Unknown,
                ..entry
            },
            RetainReason::UnknownReferences,
        ),
        (
            CacheEntry {
                kind: EntryKind::Protected,
                ..entry
            },
            RetainReason::ProtectedEntryKind,
        ),
        (
            CacheEntry {
                kind: EntryKind::Unknown,
                ..entry
            },
            RetainReason::UnknownEntryKind,
        ),
    ];
    for (protected, reason) in protection {
        let result = one(input, protected);
        assert_eq!(result.primary_history, PrimaryHistory::Confirmed);
        assert_eq!(result.remote_cache, RemoteCache::Retained(reason));
    }
    for (attempt_state, reason) in [
        (AttemptState::Active, RetainReason::ActiveAttempt),
        (
            AttemptState::OutcomeUnknown,
            RetainReason::UnknownAttemptOutcome,
        ),
    ] {
        let result = one(
            PreviewInput {
                attempt_state,
                ..input
            },
            entry,
        );
        assert_eq!(result.primary_history, PrimaryHistory::Confirmed);
        assert_eq!(result.remote_cache, RemoteCache::Retained(reason));
    }
}

#[test]
fn cancel_request_or_worker_stop_without_known_outcome_does_not_settle_the_attempt() {
    let binding = binding();
    let original = snapshot(&binding);
    let entry = entry(&binding, A);
    let pending = Snapshot {
        cancel_requested: true,
        ..original.clone()
    };
    assert_eq!(
        one(preview_input(&binding, &pending, &[A]), entry).remote_cache,
        RemoteCache::Retained(RetainReason::ActiveAttempt)
    );
    for (state, stop_evidence, reason) in [
        ("running", None, RetainReason::ActiveAttempt),
        ("succeeded", None, RetainReason::ActiveAttempt),
        (
            "outcome_unknown",
            Some("owned_child_wait_nonzero"),
            RetainReason::UnknownAttemptOutcome,
        ),
        (
            "future_job_state",
            Some("owned_child_wait_exit_0"),
            RetainReason::UnknownAttemptOutcome,
        ),
    ] {
        let job = Job {
            id: "job".into(),
            name: "synthetic".into(),
            digest: "d".repeat(64),
            state: state.into(),
            stop_evidence: stop_evidence.map(str::to_owned),
        };
        let pending = Snapshot {
            cancel_requested: true,
            confirmed_stopped: job.stop_evidence.is_some(),
            jobs: vec![job],
            ..original.clone()
        };
        assert_eq!(
            one(preview_input(&binding, &pending, &[A]), entry).remote_cache,
            RemoteCache::Retained(reason)
        );
    }
    let joined = Snapshot {
        jobs: vec![Job {
            id: "job".into(),
            name: "synthetic".into(),
            digest: "d".repeat(64),
            state: "succeeded".into(),
            stop_evidence: Some("owned_child_wait_exit_0".into()),
        }],
        ..original
    };
    assert_eq!(
        one(preview_input(&binding, &joined, &[A]), entry).remote_cache,
        RemoteCache::CleanupCandidate
    );
}

#[test]
fn project_rules_use_the_supplied_ttl_and_exact_project() {
    let binding = binding();
    let snapshot = snapshot(&binding);
    let input = preview_input(&binding, &snapshot, &[A]);
    let entry = entry(&binding, A);
    let base = input.project_retention.unwrap();
    let cases = [
        (None, RetainReason::UnknownProjectRetention),
        (
            Some(ProjectRetention {
                project: "other-project",
                ..base
            }),
            RetainReason::RetentionProjectMismatch,
        ),
        (
            Some(ProjectRetention {
                details: DetailRetention::Forever,
                ..base
            }),
            RetainReason::PermanentRetention,
        ),
        (
            Some(ProjectRetention {
                details: DetailRetention::ExpireAfterMs(-1),
                ..base
            }),
            RetainReason::InvalidTtl,
        ),
        (
            Some(ProjectRetention {
                details: DetailRetention::ExpireAfterMs(TTL + 1),
                ..base
            }),
            RetainReason::WithinTtl,
        ),
        (
            Some(ProjectRetention {
                details: DetailRetention::ExpireAfterMs(i64::MAX),
                ..base
            }),
            RetainReason::WithinTtl,
        ),
    ];
    for (project_retention, reason) in cases {
        let result = one(
            PreviewInput {
                project_retention,
                ..input
            },
            entry,
        );
        assert_eq!(result.primary_history, PrimaryHistory::Confirmed);
        assert_eq!(result.remote_cache, RemoteCache::Retained(reason));
    }
    let immediate = Some(ProjectRetention {
        details: DetailRetention::ExpireAfterMs(0),
        ..base
    });
    let entry_at_clock = CacheEntry {
        cached_at_ms: NOW,
        ..entry
    };
    assert_eq!(
        one(
            PreviewInput {
                project_retention: immediate,
                ..input
            },
            entry_at_clock
        )
        .remote_cache,
        RemoteCache::Retained(RetainReason::WithinTtl)
    );
}

#[test]
fn invalid_times_duplicates_and_nonlogical_identifiers_never_become_candidates() {
    let binding = binding();
    let snapshot = snapshot(&binding);
    let input = preview_input(&binding, &snapshot, &[A]);
    let entry = entry(&binding, A);
    for cached_at_ms in [-1, NOW + 1, i64::MAX] {
        assert_eq!(
            one(
                input,
                CacheEntry {
                    cached_at_ms,
                    ..entry
                }
            )
            .remote_cache,
            RemoteCache::Retained(RetainReason::InvalidTime)
        );
    }
    assert_eq!(
        one(
            PreviewInput {
                now_ms: i64::MIN,
                ..input
            },
            entry
        )
        .remote_cache,
        RemoteCache::Retained(RetainReason::InvalidTime)
    );
    let duplicate = CacheEntry {
        detail: DetailKey { sequence: 2, ..A },
        ..entry
    };
    let b = preview_cache_retention(input, &[duplicate, entry]);
    assert_eq!(b, preview_cache_retention(input, &[entry, duplicate]));
    assert!(b
        .iter()
        .all(|e| e.remote_cache == RemoteCache::Retained(RetainReason::DuplicateEntryId)));
    for sequence in [0, 10_001] {
        let bad_sequence = CacheEntry {
            detail: DetailKey { sequence, ..A },
            ..entry
        };
        assert_eq!(
            one(input, bad_sequence).remote_cache,
            RemoteCache::Retained(RetainReason::InvalidSequence)
        );
    }
    for id in [
        "",
        "/Users/synthetic/cache/body",
        "../nested",
        r"C:\Users\synthetic",
    ] {
        let bad = CacheEntry {
            detail: DetailKey { id, ..A },
            ..entry
        };
        let result = one(input, bad);
        assert_eq!(result.id, None);
        assert_eq!(
            result.remote_cache,
            RemoteCache::Retained(RetainReason::InvalidEntryId)
        );
    }
    for fence in [0, -1, i64::MAX] {
        assert_eq!(
            one(PreviewInput { fence, ..input }, entry).remote_cache,
            RemoteCache::Retained(RetainReason::InvalidFence)
        );
    }
    let different_recipient = Binding {
        recipient: "another-host".into(),
        ..binding.clone()
    };
    assert_eq!(
        one(
            PreviewInput {
                original_binding: &different_recipient,
                ..input
            },
            entry
        )
        .remote_cache,
        RemoteCache::Retained(RetainReason::InvalidAttemptBinding)
    );
}
