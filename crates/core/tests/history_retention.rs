use agentbaton_core::{
    conversations::Conversation,
    conversations_history::retention::*,
    registry::Project,
    sessions::ManagedSession,
    storage::{Task, TaskState},
};
use agentbaton_workspaces::{WorkspaceIdentity, WorkspaceKind};
use std::num::NonZeroU64;

const DAY_MS: i64 = 86_400_000;
const NOW: i64 = 1_800_000_000_000;

fn project(id: &str) -> Project {
    Project {
        id: id.into(),
        name: "private project name".into(),
        identity: WorkspaceIdentity {
            canonical_root: "/private/synthetic/source-path".into(),
            device: 1,
            inode: 2,
            kind: WorkspaceKind::Directory,
        },
        revision: 1,
        archived: false,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn task(project: &Project, id: &str, state: TaskState) -> Task {
    Task {
        id: id.into(),
        project_id: project.id.clone(),
        goal: "private task goal".into(),
        spec_revision: 1,
        state,
        revision: 1,
        acceptance_ref: None,
        acceptance_sequence: None,
        code_workspace_binding: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn conversation(project: &Project, state: &str, turn_state: Option<&str>) -> Conversation {
    Conversation {
        id: "conversation".into(),
        project_id: project.id.clone(),
        title: "private conversation title".into(),
        workspace_id: "workspace".into(),
        connection_id: "connection".into(),
        grant_id: "grant".into(),
        state: state.into(),
        revision: 1,
        native_session_id: None,
        created_at: String::new(),
        updated_at: String::new(),
        last_error: None,
        turn_state: turn_state.map(str::to_owned),
        runtime_snapshot: None,
        attempt_id: "attempt".into(),
        fencing_token: 1,
        git_common_device: 1,
        git_common_inode: 1,
    }
}

fn session(task: &Task, id: &str, state: &str, turn_state: Option<&str>) -> ManagedSession {
    ManagedSession {
        id: id.into(),
        task_id: task.id.clone(),
        project_id: task.project_id.clone(),
        workspace_id: "workspace".into(),
        connection_id: "connection".into(),
        grant_id: "grant".into(),
        state: state.into(),
        revision: 1,
        native_session_id: None,
        created_at: String::new(),
        updated_at: String::new(),
        last_error: None,
        turn_state: turn_state.map(str::to_owned),
        attempt_id: "attempt".into(),
        intent_id: "intent".into(),
        fencing_token: 1,
        runtime_snapshot: None,
    }
}

fn body<'a>(
    id: &'a str,
    source: HistorySource<'a>,
    captured_at_ms: Option<i64>,
) -> HistoryRecord<'a> {
    HistoryRecord {
        id,
        kind: HistoryRecordKind::Body,
        source,
        captured_at_ms,
        pinned: false,
        active_use: HistoryUse::Inactive,
    }
}

fn old_body<'a>(id: &'a str, source: HistorySource<'a>) -> HistoryRecord<'a> {
    body(id, source, Some(NOW - 100 * DAY_MS))
}

fn decision<'a>(plan: &'a HistoryCleanupPlan, id: &str) -> &'a HistoryCleanupDecision {
    &plan
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .unwrap()
        .decision
}

fn assert_keep(plan: &HistoryCleanupPlan, id: &str, reason: HistoryKeepReason) {
    assert_eq!(decision(plan, id), &HistoryCleanupDecision::Keep { reason });
}

fn assert_expire(plan: &HistoryCleanupPlan, id: &str, invalidations: Vec<HistoryInvalidation>) {
    assert_eq!(
        decision(plan, id),
        &HistoryCleanupDecision::ExpireDetail {
            gap: HistoryGapReason::DetailExpired,
            invalidations,
        },
    );
}

#[test]
fn default_90_day_boundary_is_inclusive_and_future_or_unknown_dates_remain() {
    let project = project("project");
    let owner = HistorySource::Project(&project);
    let boundary = NOW - 90 * DAY_MS;
    let records = [
        body("future", owner, Some(NOW + 1)),
        body("older", owner, Some(boundary - 1)),
        body("at-cutoff", owner, Some(boundary)),
        body("newer", owner, Some(boundary + 1)),
        body("now", owner, Some(NOW)),
        body("no-time", owner, None),
    ];
    let plan = plan_project_history_cleanup(
        &project.id,
        HistoryRetentionPolicy::default(),
        NOW,
        &records,
        &[],
    )
    .unwrap();
    assert_eq!(DEFAULT_DETAIL_RETENTION_DAYS, 90);
    assert_eq!(plan.expiry_cutoff_ms, Some(boundary));
    assert_eq!(plan.evaluated_at_ms, NOW);
    assert_eq!(plan.policy.body_capture, BodyCapture::Enabled);
    assert_expire(&plan, "older", vec![]);
    assert_expire(&plan, "at-cutoff", vec![]);
    assert_keep(&plan, "newer", HistoryKeepReason::WithinRetention);
    assert_keep(&plan, "now", HistoryKeepReason::WithinRetention);
    assert_keep(&plan, "future", HistoryKeepReason::FutureTimestamp);
    assert_keep(&plan, "no-time", HistoryKeepReason::MissingTimestamp);
}

#[test]
fn override_permanence_and_capture_have_independent_meanings() {
    let project = project("project");
    let owner = HistorySource::Project(&project);
    let records = [
        body("middle", owner, Some(NOW - 45 * DAY_MS)),
        body("recent", owner, Some(NOW - 10 * DAY_MS)),
        old_body("old", owner),
    ];
    let policy = HistoryRetentionPolicy {
        details: DetailRetention::Days(NonZeroU64::new(30).unwrap()),
        body_capture: BodyCapture::Enabled,
    };
    let short = plan_project_history_cleanup(&project.id, policy, NOW, &records, &[]).unwrap();
    assert_eq!(short.expiry_cutoff_ms, Some(NOW - 30 * DAY_MS));
    assert_expire(&short, "middle", vec![]);
    assert_expire(&short, "old", vec![]);
    assert_keep(&short, "recent", HistoryKeepReason::WithinRetention);
    let disabled = HistoryRetentionPolicy {
        body_capture: BodyCapture::Disabled,
        ..policy
    };
    let mut same = short;
    same.policy = disabled;
    assert_eq!(
        plan_project_history_cleanup(&project.id, disabled, NOW, &records, &[]).unwrap(),
        same
    );

    let forever = HistoryRetentionPolicy {
        details: DetailRetention::Permanent,
        ..disabled
    };
    let plan = plan_project_history_cleanup(&project.id, forever, NOW, &records, &[]).unwrap();
    assert_eq!(plan.expiry_cutoff_ms, None);
    assert_eq!(plan.policy.body_capture, BodyCapture::Disabled);
    for record in records {
        assert_keep(&plan, record.id, HistoryKeepReason::PermanentRetention);
    }
}

#[test]
fn keeps_pins_active_references_and_non_detail_categories() {
    let project = project("project");
    let source = HistorySource::Project(&project);
    let mut records = [
        ("source-file", HistoryRecordKind::SourceFile),
        ("adopted-result", HistoryRecordKind::AdoptedResult),
        ("independent-report", HistoryRecordKind::StandaloneReport),
        ("summary", HistoryRecordKind::Summary),
        ("acceptance", HistoryRecordKind::AcceptanceRecord),
        ("existing-gap", HistoryRecordKind::GapMarker),
        ("unclassified", HistoryRecordKind::Unclassified),
    ]
    .map(|(id, kind)| HistoryRecord {
        kind,
        ..old_body(id, source)
    })
    .to_vec();
    let protected_count = records.len();
    records.extend([
        HistoryRecord {
            pinned: true,
            kind: HistoryRecordKind::Log,
            ..old_body("pinned-log", source)
        },
        HistoryRecord {
            active_use: HistoryUse::Active,
            ..old_body("needed-by-active-work", source)
        },
        HistoryRecord {
            active_use: HistoryUse::Unknown,
            ..old_body("use-unknown", source)
        },
    ]);
    let plan = plan_project_history_cleanup(
        &project.id,
        HistoryRetentionPolicy::default(),
        NOW,
        &records,
        &[],
    )
    .unwrap();
    for record in &records[..protected_count] {
        assert_keep(&plan, record.id, HistoryKeepReason::ProtectedKind);
    }
    assert_keep(&plan, "pinned-log", HistoryKeepReason::Pinned);
    assert_keep(&plan, "needed-by-active-work", HistoryKeepReason::ActiveUse);
    assert_keep(&plan, "use-unknown", HistoryKeepReason::ActivityUnknown);
}

#[test]
fn task_conversation_and_session_activity_come_from_core_owners() {
    let project = project("project");
    let draft = task(&project, "draft-task", TaskState::Draft);
    let running = task(&project, "running-task", TaskState::Running);
    let cancel_requested = task(
        &project,
        "cancel-requested-task",
        TaskState::CancelRequested,
    );
    let unknown = task(&project, "unknown-task", TaskState::OutcomeUnknown);
    let accepted = task(&project, "accepted-task", TaskState::Accepted);
    let cancelled = task(&project, "cancelled-task", TaskState::Cancelled);
    let ready = conversation(&project, "ready", None);
    let settled = Conversation {
        id: "settled-conversation".into(),
        ..conversation(&project, "stopped", Some("completed"))
    };
    let uncertain = Conversation {
        id: "uncertain-conversation".into(),
        ..conversation(&project, "stopped", Some("outcome_unknown"))
    };
    let live_session = session(&cancelled, "live-session", "running", Some("running"));
    let used_session = session(&running, "used-session", "stopped", Some("completed"));
    let stopped_session = session(&accepted, "ended-session", "stopped", Some("completed"));
    let records = [
        old_body("draft", HistorySource::Task(&draft)),
        old_body("running", HistorySource::Task(&running)),
        old_body("cancel-requested", HistorySource::Task(&cancel_requested)),
        old_body("unknown", HistorySource::Task(&unknown)),
        old_body("accepted", HistorySource::Task(&accepted)),
        old_body("cancelled", HistorySource::Task(&cancelled)),
        old_body("ready-conversation", HistorySource::Conversation(&ready)),
        old_body(
            "settled-conversation",
            HistorySource::Conversation(&settled),
        ),
        old_body(
            "uncertain-conversation",
            HistorySource::Conversation(&uncertain),
        ),
        old_body(
            "running-session",
            HistorySource::Session {
                session: &live_session,
                task: &cancelled,
            },
        ),
        old_body(
            "used-session",
            HistorySource::Session {
                session: &used_session,
                task: &running,
            },
        ),
        HistoryRecord {
            kind: HistoryRecordKind::Log,
            ..old_body(
                "stopped-log",
                HistorySource::Session {
                    session: &stopped_session,
                    task: &accepted,
                },
            )
        },
    ];
    let plan = plan_project_history_cleanup(
        &project.id,
        HistoryRetentionPolicy::default(),
        NOW,
        &records,
        &[],
    )
    .unwrap();
    for id in [
        "draft",
        "running",
        "cancel-requested",
        "unknown",
        "ready-conversation",
        "uncertain-conversation",
        "running-session",
        "used-session",
    ] {
        assert_keep(&plan, id, HistoryKeepReason::ActiveUse);
    }
    for id in [
        "accepted",
        "cancelled",
        "settled-conversation",
        "stopped-log",
    ] {
        assert_expire(&plan, id, vec![]);
    }
}

fn derived<'a>(
    id: &'a str,
    kind: DerivedHistoryKind,
    project: &'a Project,
    source_record_id: &'a str,
) -> DerivedHistoryRecord<'a> {
    DerivedHistoryRecord {
        id,
        kind,
        project_id: &project.id,
        source_record_id,
    }
}

#[test]
fn scoped_invalidation_links_and_gap_markers_are_deterministic() {
    let foreign = project("foreign-project");
    let project = project("target-project");
    let expired = old_body("expired", HistorySource::Project(&project));
    let pinned = HistoryRecord {
        pinned: true,
        ..old_body("pinned", HistorySource::Project(&project))
    };
    let unrelated = old_body("foreign-detail", HistorySource::Project(&foreign));
    let mut records = [unrelated, pinned, expired];
    let mut dependencies = [
        derived(
            "foreign-index",
            DerivedHistoryKind::SearchIndex,
            &foreign,
            unrelated.id,
        ),
        derived(
            "protected-cache",
            DerivedHistoryKind::Cache,
            &project,
            pinned.id,
        ),
        derived(
            "z-index",
            DerivedHistoryKind::SearchIndex,
            &project,
            expired.id,
        ),
        derived(
            "detail-cache",
            DerivedHistoryKind::Cache,
            &project,
            expired.id,
        ),
        derived(
            "a-index",
            DerivedHistoryKind::SearchIndex,
            &project,
            expired.id,
        ),
    ];
    let run = |records: &[HistoryRecord<'_>], dependencies: &[DerivedHistoryRecord<'_>]| {
        plan_project_history_cleanup(
            &project.id,
            HistoryRetentionPolicy::default(),
            NOW,
            records,
            dependencies,
        )
        .unwrap()
    };
    let plan = run(&records, &dependencies);
    assert_eq!(
        plan.entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        ["expired", "pinned"]
    );
    assert_expire(
        &plan,
        expired.id,
        [
            ("a-index", DerivedHistoryKind::SearchIndex),
            ("z-index", DerivedHistoryKind::SearchIndex),
            ("detail-cache", DerivedHistoryKind::Cache),
        ]
        .map(|(id, kind)| HistoryInvalidation {
            id: id.into(),
            kind,
        })
        .to_vec(),
    );
    assert_keep(&plan, pinned.id, HistoryKeepReason::Pinned);
    records.reverse();
    dependencies.reverse();
    assert_eq!(plan, run(&records, &dependencies));
    let metadata = serde_json::to_string(&plan).unwrap();
    assert!(metadata.contains("detail_expired"));
    for private in [
        "foreign-project",
        "foreign-detail",
        "foreign-index",
        "protected-cache",
        "private project name",
        "source-path",
    ] {
        assert!(!metadata.contains(private), "{private}");
    }
}

#[test]
fn rejects_unknown_ownership_conflicting_records_and_ambiguous_derived_sources() {
    let target = project("target-project");
    let foreign = project("foreign-project");
    let first = old_body("first", HistorySource::Project(&target));
    let second = old_body("second", HistorySource::Project(&target));
    let peer = old_body("first", HistorySource::Project(&foreign));
    let unknown = old_body("unbound", HistorySource::Unknown);
    let bad_project = project("");
    let no_owner = old_body("invalid-owner", HistorySource::Project(&bad_project));
    let missing_source = derived("index", DerivedHistoryKind::SearchIndex, &target, "missing");
    let cross_project = derived("index", DerivedHistoryKind::SearchIndex, &foreign, "first");
    let same_a = derived("shared-cache", DerivedHistoryKind::Cache, &target, "first");
    let same_b = derived("shared-cache", DerivedHistoryKind::Cache, &target, "second");
    let accepted = task(&target, "accepted", TaskState::Accepted);
    let mut mismatched = session(&accepted, "wrong-owner-session", "stopped", None);
    mismatched.task_id = "different-task".into();
    let bad_session = old_body(
        "unbound-session",
        HistorySource::Session {
            session: &mismatched,
            task: &accepted,
        },
    );
    let mut swapped_project = session(&accepted, "wrong-project-session", "stopped", None);
    swapped_project.project_id = foreign.id.clone();
    let bad_project_session = old_body(
        "unbound-project-session",
        HistorySource::Session {
            session: &swapped_project,
            task: &accepted,
        },
    );
    let conflicting_task = task(&target, "accepted", TaskState::Running);
    let conflicting_project = task(&foreign, "accepted", TaskState::Accepted);
    let explicit_task = old_body("known-task", HistorySource::Task(&accepted));
    let conflicting_activity = old_body(
        "conflicting-activity",
        HistorySource::Task(&conflicting_task),
    );
    let conflicting_ownership = old_body(
        "conflicting-ownership",
        HistorySource::Task(&conflicting_project),
    );
    let conflicting_session = session(
        &conflicting_task,
        "conflicting-task-session",
        "stopped",
        None,
    );
    let explicit_session = old_body(
        "conflicting-session",
        HistorySource::Session {
            session: &conflicting_session,
            task: &conflicting_task,
        },
    );
    let cases: &[(&[HistoryRecord<'_>], &[DerivedHistoryRecord<'_>], &str)] = &[
        (&[first, unknown], &[], "source is unknown"),
        (&[first, no_owner], &[], "project ownership is invalid"),
        (&[first, first], &[], "Duplicate history record identity"),
        (&[first, peer], &[], "Duplicate history record identity"),
        (&[first], &[missing_source], "source record is missing"),
        (
            &[first],
            &[cross_project],
            "source and project ownership disagree",
        ),
        (
            &[first, second],
            &[same_a, same_b],
            "Duplicate derived history identity",
        ),
        (
            &[first, bad_session],
            &[],
            "session and task ownership disagree",
        ),
        (
            &[first, bad_project_session],
            &[],
            "session and task ownership disagree",
        ),
        (
            &[explicit_task, conflicting_activity],
            &[],
            "Conflicting history source",
        ),
        (
            &[explicit_task, conflicting_ownership],
            &[],
            "Conflicting history source",
        ),
        (
            &[explicit_task, explicit_session],
            &[],
            "Conflicting history source",
        ),
    ];
    for &(records, dependencies, reason) in cases {
        let error = plan_project_history_cleanup(
            &target.id,
            HistoryRetentionPolicy::default(),
            NOW,
            records,
            dependencies,
        )
        .unwrap_err();
        assert_eq!(error.code, "INPUT_CHANGED");
        assert_eq!(error.side_effect_state, "not_started");
        assert!(error.message.contains(reason), "{error}");
    }
}

#[test]
fn invalid_scope_and_unrepresentable_time_refuse_without_clamping() {
    let policy = HistoryRetentionPolicy::default();
    let error = plan_project_history_cleanup("unsafe/path", policy, NOW, &[], &[]).unwrap_err();
    assert_eq!(error.code, "INVALID_REQUEST");
    assert_eq!(error.message, "Invalid history project ID");
    for (days, reference) in [
        (u64::MAX, NOW),
        (i64::MAX as u64, NOW),
        (1, i64::MIN + DAY_MS - 1),
    ] {
        let policy = HistoryRetentionPolicy {
            details: DetailRetention::Days(NonZeroU64::new(days).unwrap()),
            ..policy
        };
        let error =
            plan_project_history_cleanup("project", policy, reference, &[], &[]).unwrap_err();
        assert_eq!(error.code, "INVALID_REQUEST");
        assert_eq!(error.side_effect_state, "not_started");
        assert!(error.message.contains("cutoff is out of range"));
    }
    let oldest = HistoryRetentionPolicy {
        details: DetailRetention::Days(NonZeroU64::new(1).unwrap()),
        ..policy
    };
    assert_eq!(
        plan_project_history_cleanup("project", oldest, i64::MIN + DAY_MS, &[], &[])
            .unwrap()
            .expiry_cutoff_ms,
        Some(i64::MIN),
    );
}
