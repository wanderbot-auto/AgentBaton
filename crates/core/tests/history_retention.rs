use agentbaton_core::{
    conversations::{Conversation, ConversationSupervisor},
    conversations_history::retention::*,
    registry::Project,
    sessions::{ManagedSession, Supervisor},
    storage::{Store, Task, TaskState},
};
use agentbaton_workspaces::{WorkspaceIdentity, WorkspaceKind};
use rusqlite::{params, Connection};
use std::{
    fs,
    num::NonZeroU64,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

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
fn store_snapshot_keeps_active_unknown_and_stopped_turns_in_their_own_project() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().canonicalize().unwrap();
    let path = root.join("history.sqlite3");
    let wal = root.join("history.sqlite3-wal");
    let runtime = root.join("runtime");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let mut store = Store::open(&path).unwrap();
    let inspector = Connection::open(&path).unwrap();
    let executable = generated_child(&root);
    let connection = store
        .register_connection("Generated", "custom_acp", &executable, &[], "connection")
        .unwrap();
    let mut sessions = Supervisor::new(&runtime);
    let mut conversations = ConversationSupervisor::new(&runtime);
    sessions.probe(&store, &connection.id).unwrap();
    await_state(|| {
        sessions.tick(&mut store).unwrap();
        store.get_connection(&connection.id).unwrap().probe_state == "ready"
    });

    let (first, first_workspace, first_grant) =
        setup_project(&mut store, &root, "first", &connection.id);
    let (second, second_workspace, second_grant) =
        setup_project(&mut store, &root, "second", &connection.id);
    let chat = store
        .create_conversation(
            &first.id,
            "generated conversation",
            &first_workspace,
            &connection.id,
            &first_grant,
            "conversation",
        )
        .unwrap();
    conversations.launch(&mut store, &chat.id).unwrap();
    await_state(|| {
        conversations.tick(&mut store).unwrap();
        store.get_conversation(&chat.id).unwrap().state == "ready"
    });
    let chat = store.get_conversation(&chat.id).unwrap();
    let secret_body = "generated private conversation body";
    let (_, queued) = store
        .send_conversation(&chat.id, chat.revision, secret_body, "message")
        .unwrap();
    assert!(queued);
    conversations.send(&chat.id, secret_body).unwrap();
    let chat_turn: String = inspector
        .query_row(
            "SELECT id FROM conversation_turns WHERE conversation_id=?1",
            [&chat.id],
            |row| row.get(0),
        )
        .unwrap();

    let task = store
        .create_registered(&second.id, "generated task", "task")
        .unwrap();
    let session = store
        .create_session(
            &task.id,
            &second_workspace,
            &connection.id,
            &second_grant,
            task.revision,
            "session",
        )
        .unwrap();
    sessions.launch(&mut store, &session.id).unwrap();
    await_state(|| {
        sessions.tick(&mut store).unwrap();
        store.get_session(&session.id).unwrap().state == "ready"
    });
    let session = store.get_session(&session.id).unwrap();
    let (_, queued) = store
        .send_session(
            &session.id,
            "generated session body",
            session.revision,
            "turn",
        )
        .unwrap();
    assert!(queued);
    sessions
        .send(&session.id, "generated session body")
        .unwrap();
    let session_turn: String = inspector
        .query_row(
            "SELECT id FROM session_turns WHERE session_id=?1",
            [&session.id],
            |row| row.get(0),
        )
        .unwrap();

    let evaluate = |store: &Store, project: &Project| {
        let before = store.update_database_boundary().unwrap();
        let main_bytes = fs::read(&path).unwrap();
        let wal_bytes = fs::read(&wal).unwrap();
        assert!(!wal_bytes.is_empty());
        let result = store.preview_project_history_retention(
            &project.id,
            HistoryRetentionPolicy::default(),
            NOW,
        );
        assert_eq!(store.update_database_boundary().unwrap(), before);
        assert_eq!(fs::read(&path).unwrap(), main_bytes);
        assert_eq!(fs::read(&wal).unwrap(), wal_bytes);
        result
    };
    let first_preview = evaluate(&store, &first).unwrap();
    let second_preview = evaluate(&store, &second).unwrap();
    assert_eq!(first_preview.plan.entries.len(), 1);
    assert_eq!(second_preview.plan.entries.len(), 1);
    assert_keep(
        &first_preview.plan,
        &chat_turn,
        HistoryKeepReason::ActiveUse,
    );
    assert_keep(
        &second_preview.plan,
        &session_turn,
        HistoryKeepReason::ActiveUse,
    );
    assert_eq!(first_preview.plan.entries[0].kind, HistoryRecordKind::Body);
    assert_eq!(second_preview.plan.entries[0].kind, HistoryRecordKind::Body);
    let serialized = serde_json::to_string(&first_preview).unwrap();
    for private in [secret_body, &second.id, &session_turn] {
        assert!(!serialized.contains(private));
    }
    assert_eq!(
        first_preview.gaps,
        [
            HistorySnapshotGap::RecordPins,
            HistorySnapshotGap::BodyCaptureTimestamp,
            HistorySnapshotGap::DerivedSourceInventory,
            HistorySnapshotGap::MixedJournalAndUnclassifiedArtifacts,
            HistorySnapshotGap::ProjectRetentionPolicy,
        ],
    );
    // Store created both owners, turns and message journal rows. Only the
    // turn IDs appear in the preview even when the journal also contains text.
    let event_count: i64 = inspector
        .query_row(
            "SELECT count(*) FROM events
             WHERE json_extract(document,'$.aggregate_id') IN (?1,?2)",
            params![chat.id, session.id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(event_count > 2);

    await_state(|| {
        conversations.tick(&mut store).unwrap();
        store
            .get_conversation(&chat.id)
            .unwrap()
            .turn_state
            .as_deref()
            == Some("completed")
    });
    await_state(|| {
        sessions.tick(&mut store).unwrap();
        store
            .get_session(&session.id)
            .unwrap()
            .turn_state
            .as_deref()
            == Some("completed")
    });
    conversations.shutdown(&mut store).unwrap();
    sessions.shutdown(&mut store).unwrap();
    assert!(!conversations.has_workers());
    assert!(!sessions.has_workers());
    assert_eq!(store.get_conversation(&chat.id).unwrap().state, "stopped");
    let stopped = evaluate(&store, &first).unwrap();
    assert_keep(
        &stopped.plan,
        &chat_turn,
        HistoryKeepReason::RetentionMetadataUnknown,
    );

    // Inject controlled faults into the generated current Store after the
    // actual children have stopped. Preview must fail closed for each case.
    inspector
        .execute(
            "UPDATE conversation_dispatches SET state='outcome_unknown' WHERE conversation_id=?1",
            [&chat.id],
        )
        .unwrap();
    let unknown = evaluate(&store, &first).unwrap();
    assert_keep(
        &unknown.plan,
        &chat_turn,
        HistoryKeepReason::ActivityUnknown,
    );
    inspector
        .execute(
            "UPDATE conversation_dispatches SET state='stopped' WHERE conversation_id=?1",
            [&chat.id],
        )
        .unwrap();
    inspector
        .execute(
            "UPDATE conversations
             SET document=json_set(document,'$.turn_state','outcome_unknown') WHERE id=?1",
            [&chat.id],
        )
        .unwrap();
    let unknown_owner = evaluate(&store, &first).unwrap();
    assert_keep(
        &unknown_owner.plan,
        &chat_turn,
        HistoryKeepReason::ActivityUnknown,
    );
    inspector
        .execute(
            "UPDATE conversation_turns SET document=json_set(document,'$.extra','unclassified')
             WHERE id=?1",
            [&chat_turn],
        )
        .unwrap();
    let unclassified = evaluate(&store, &first).unwrap();
    assert_eq!(
        unclassified.plan.entries[0].kind,
        HistoryRecordKind::Unclassified
    );
    assert_keep(
        &unclassified.plan,
        &chat_turn,
        HistoryKeepReason::ProtectedKind,
    );

    inspector
        .execute(
            "UPDATE tasks SET document=json_set(document,'$.project_id',?1) WHERE id=?2",
            params![first.id, task.id],
        )
        .unwrap();
    assert_eq!(evaluate(&store, &second).unwrap_err().code, "INPUT_CHANGED");
    assert_eq!(evaluate(&store, &first).unwrap(), unclassified);
    assert_eq!(
        store
            .preview_project_history_retention("missing", HistoryRetentionPolicy::default(), NOW)
            .unwrap_err()
            .code,
        "NOT_FOUND",
    );
}

fn setup_project(
    store: &mut Store,
    root: &Path,
    name: &str,
    connection_id: &str,
) -> (Project, String, String) {
    let source = root.join(name);
    let managed = root.join(format!("{name}-managed"));
    for path in [&source, &managed] {
        fs::create_dir(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    git(&source, &["init", "--initial-branch=main"]);
    fs::write(source.join("input"), b"generated source").unwrap();
    git(&source, &["add", "input"]);
    git(&source, &["commit", "-m", "Fixture"]);
    let project = store.import_project(&source, name, name).unwrap();
    let head = git(&source, &["rev-parse", "HEAD"]);
    let workspace = store
        .create_workspace(&project.id, project.revision, &head, &managed, name)
        .unwrap();
    let grant = store
        .create_grant(
            &project.id,
            &workspace.id,
            connection_id,
            2,
            1,
            60_000,
            true,
            name,
        )
        .unwrap();
    (project, workspace.id, grant.id)
}

fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name.as_bytes().starts_with(b"GIT_") {
            command.env_remove(name);
        }
    }
    let result = command
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Retention fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}

fn generated_child(root: &Path) -> PathBuf {
    let python = Command::new("python3")
        .args([
            "-c",
            "import os,sys; print(os.path.realpath(sys.executable))",
        ])
        .output()
        .unwrap();
    assert!(python.status.success());
    let script = root.join("generated-acp");
    let body = r#"import json,sys
if sys.argv[1:]==['--version']:
 print('Generated retention fixture 1.0'); sys.exit(0)
for line in sys.stdin:
 request=json.loads(line)
 if request['method']=='initialize':
  result={'protocolVersion':1,'agentCapabilities':{}}
 elif request['method']=='session/new':
  result={'sessionId':'generated-session'}
 elif request['method']=='session/prompt':
  result={'stopReason':'end_turn'}
 else:
  sys.exit(3)
 print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result}),flush=True)
"#;
    fs::write(
        &script,
        format!(
            "#!{}\n{body}",
            String::from_utf8_lossy(&python.stdout).trim()
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    script
}

fn await_state(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(12);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "generated process did not settle"
        );
        thread::sleep(Duration::from_millis(5));
    }
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
