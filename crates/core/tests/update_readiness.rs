//! Real Core state, private SQLite files and a generated ACP child. Approval,
//! actor coverage and the release verifier are supplied as explicit fixtures.
use agentbaton_core::{
    background_status::BackgroundStatus,
    execution::{
        update_readiness::{
            ApprovedDevelopmentUpdate, BackgroundAvailability, DevelopmentUpdate, UpdateBackground,
            UpdateBackupProof, UpdateBlocker as Blocker, UpdateDatabaseBoundary,
            UpdateEvidenceFile, UpdateMigrationProof, UpdateReadinessRequest, UpdateRecoveryProof,
            UpdateRuntimeObservation, UpdateWorkSource as Source, UpdateWorkState as Work,
        },
        ExecutionBinding, IntentState,
    },
    sessions::Supervisor,
    storage::{Store, TaskState},
};
use agentbaton_providers::installation::{executable_digest, BinaryDigest};
use rusqlite::{Connection, OpenFlags, MAIN_DB};
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use std::os::unix::fs::MetadataExt;
use std::{
    fs,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

struct Fixture {
    private: tempfile::TempDir,
    root: PathBuf,
    database: PathBuf,
    store: Store,
    boundary: UpdateDatabaseBoundary,
    instance: String,
    candidate: DevelopmentUpdate,
    artifact: PathBuf,
    backup: PathBuf,
    backup_digest: BinaryDigest,
    migrated: PathBuf,
    migrated_digest: BinaryDigest,
    restored: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let private = tempfile::tempdir().unwrap();
        let root = private.path().canonicalize().unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let database = root.join("core.sqlite3");
        let mut store = Store::open(&database).unwrap();
        store
            .create("fixture", "Unscheduled local draft", "baseline")
            .unwrap();
        let boundary = store.update_database_boundary().unwrap();
        let artifact = root.join("fixture-unsigned-package");
        fs::write(&artifact, b"synthetic development package; not installable").unwrap();
        let candidate = DevelopmentUpdate {
            version: "0.1.0-dev-fixture".into(),
            os: "macos".into(),
            arch: "aarch64".into(),
            commit_sha: "c".repeat(40),
            artifact_sha256: executable_digest(&artifact).unwrap(),
            target_schema: boundary.schema,
        };
        let backup = root.join("backup ? percent %.sqlite3");
        let migrated = root.join("migrated.sqlite3");
        let restored = root.join("restored.sqlite3");
        let (backup_digest, migrated_digest) =
            rehearse_copies(&database, &boundary, &backup, &migrated, &restored);
        Self {
            private,
            root,
            database,
            store,
            boundary,
            instance: Uuid::new_v4().to_string(),
            candidate,
            artifact,
            backup,
            backup_digest,
            migrated,
            migrated_digest,
            restored,
        }
    }

    fn refresh_database_proofs(&mut self) {
        for copy in [&self.backup, &self.migrated, &self.restored] {
            fs::remove_file(copy).unwrap();
        }
        self.boundary = self.store.update_database_boundary().unwrap();
        (self.backup_digest, self.migrated_digest) = rehearse_copies(
            &self.database,
            &self.boundary,
            &self.backup,
            &self.migrated,
            &self.restored,
        );
    }

    fn proof(&self) -> Evidence<'_> {
        Evidence {
            approval: ApprovedDevelopmentUpdate {
                approval_ref: "approved-local-fixture",
                candidate: &self.candidate,
            },
            backup: UpdateBackupProof {
                receipt_ref: "sqlite-backup-fixture",
                database: &self.boundary,
                backup: UpdateEvidenceFile {
                    path: &self.backup,
                    sha256: &self.backup_digest,
                },
            },
            migration: UpdateMigrationProof {
                rehearsal_ref: "same-schema-core-reopen",
                candidate: &self.candidate,
                input_backup_sha256: &self.backup_digest,
                source_schema: self.boundary.schema,
                result: UpdateEvidenceFile {
                    path: &self.migrated,
                    sha256: &self.migrated_digest,
                },
            },
            recovery: UpdateRecoveryProof {
                rehearsal_ref: "copy-backup-for-restore",
                migration_rehearsal_ref: "same-schema-core-reopen",
                candidate: &self.candidate,
                restored: UpdateEvidenceFile {
                    path: &self.restored,
                    sha256: &self.backup_digest,
                },
            },
        }
    }

    fn background(
        &self,
        database: &UpdateDatabaseBoundary,
        owned_provider_workers: bool,
    ) -> BackgroundStatus {
        BackgroundStatus {
            instance_id: self.instance.clone(),
            started_at: "2026-11-19T00:00:00Z".into(),
            api_version: agentbaton_core::ipc::API_VERSION,
            database_schema_version: database.schema,
            sequence: database.sequence,
            owned_provider_workers,
            store_recovery: "completed_on_start".into(),
        }
    }

    fn request<'a>(
        &'a self,
        evidence: &'a Evidence<'a>,
        runtime: Option<&'a UpdateRuntimeObservation<'a>>,
    ) -> UpdateReadinessRequest<'a> {
        UpdateReadinessRequest {
            candidate: &self.candidate,
            approval: Some(&evidence.approval),
            artifact_path: Some(&self.artifact),
            runtime,
            backup: Some(&evidence.backup),
            migration: Some(&evidence.migration),
            recovery: Some(&evidence.recovery),
        }
    }
}

fn rehearse_copies(
    database: &Path,
    boundary: &UpdateDatabaseBoundary,
    backup: &Path,
    migrated: &Path,
    restored: &Path,
) -> (BinaryDigest, BinaryDigest) {
    let reader = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    reader.backup(MAIN_DB, backup, None).unwrap();
    drop(reader);
    fs::set_permissions(backup, fs::Permissions::from_mode(0o600)).unwrap();
    fs::copy(backup, migrated).unwrap();
    // Rehearse an actual isolated, same-schema Core reopen on a copy.
    let target = Store::open(migrated).unwrap();
    assert_eq!(target.sequence().unwrap(), boundary.sequence);
    drop(target);
    fs::copy(backup, restored).unwrap();
    (
        executable_digest(backup).unwrap(),
        executable_digest(migrated).unwrap(),
    )
}

struct Evidence<'a> {
    approval: ApprovedDevelopmentUpdate<'a>,
    backup: UpdateBackupProof<'a>,
    migration: UpdateMigrationProof<'a>,
    recovery: UpdateRecoveryProof<'a>,
}

fn available<'a>(
    database: &'a UpdateDatabaseBoundary,
    background: &'a BackgroundStatus,
) -> UpdateRuntimeObservation<'a> {
    UpdateRuntimeObservation {
        database,
        background: UpdateBackground::Available(background),
        other_local_work: Work::Idle,
        remote_work: Work::Idle,
    }
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn idle_available_and_confirmed_stopped_are_distinct_readiness_states() {
    let f = Fixture::new();
    let proof = f.proof();
    let background = f.background(&f.boundary, false);
    let runtime = available(&f.boundary, &background);
    let baseline_files = directory_evidence(&f.root);
    let loaded = f
        .store
        .assess_update_readiness(&f.request(&proof, Some(&runtime)));
    assert_eq!(loaded.background, BackgroundAvailability::Available);
    assert!(loaded.blockers.is_empty(), "{:?}", loaded.blockers);
    assert!(loaded.ready.is_some());

    let stopped = UpdateRuntimeObservation {
        database: &f.boundary,
        background: UpdateBackground::ConfirmedStopped {
            last_instance_id: &f.instance,
            joined_stop_ref: "fixture-supervisor-joined",
        },
        other_local_work: Work::Idle,
        remote_work: Work::Idle,
    };
    let stopped_result = f
        .store
        .assess_update_readiness(&f.request(&proof, Some(&stopped)));
    assert_eq!(
        stopped_result.background,
        BackgroundAvailability::ConfirmedStopped
    );
    assert!(stopped_result.ready.is_some());
    let transition = f.store.recheck_update_readiness(
        loaded.ready.as_ref().unwrap(),
        &f.request(&proof, Some(&stopped)),
    );
    assert_eq!(transition.blockers, vec![Blocker::ReadinessChanged]);
    assert!(transition.ready.is_none());
    assert_eq!(f.store.update_database_boundary().unwrap(), f.boundary);
    assert_eq!(directory_evidence(&f.root), baseline_files);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn directory_evidence(path: &Path) -> Vec<(PathBuf, BinaryDigest)> {
    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let digest = executable_digest(&path).unwrap();
            (path, digest)
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

#[test]
fn missing_and_unknown_authority_never_looks_like_idle() {
    let f = Fixture::new();
    let request = UpdateReadinessRequest {
        candidate: &f.candidate,
        approval: None,
        artifact_path: None,
        runtime: None,
        backup: None,
        migration: None,
        recovery: None,
    };
    let result = f.store.assess_update_readiness(&request);
    assert_eq!(result.background, BackgroundAvailability::Unknown);
    for reason in [
        Blocker::RuntimeEvidenceMissing,
        Blocker::ApprovalMissing,
        Blocker::ArtifactMissing,
        Blocker::BackupMissing,
        Blocker::MigrationProofMissing,
        Blocker::RecoveryProofMissing,
    ] {
        assert!(result.blockers.contains(&reason), "{reason:?}");
    }
    assert!(result.ready.is_none());
    let proof = f.proof();
    let background = f.background(&f.boundary, false);
    let unknown = UpdateRuntimeObservation {
        remote_work: Work::OutcomeUnknown,
        ..available(&f.boundary, &background)
    };
    let assessment = f
        .store
        .assess_update_readiness(&f.request(&proof, Some(&unknown)));
    assert!(assessment
        .blockers
        .contains(&Blocker::Work(Source::RemoteWorkers, Work::OutcomeUnknown)));
    assert!(assessment.ready.is_none());
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn approved_artifact_migration_and_exact_restoration_are_separate_checks() {
    let f = Fixture::new();
    let background = f.background(&f.boundary, false);
    let runtime = available(&f.boundary, &background);
    let wrong_candidate = DevelopmentUpdate {
        version: "other-target".into(),
        ..f.candidate.clone()
    };
    let mut evidence = f.proof();
    evidence.approval.candidate = &wrong_candidate;
    assert_eq!(
        f.store
            .assess_update_readiness(&f.request(&evidence, Some(&runtime)))
            .blockers,
        vec![Blocker::ApprovalMismatch]
    );
    evidence.approval.candidate = &f.candidate;
    evidence.migration.candidate = &wrong_candidate;
    assert_eq!(
        f.store
            .assess_update_readiness(&f.request(&evidence, Some(&runtime)))
            .blockers,
        vec![Blocker::MigrationProofMismatch]
    );
    evidence.migration.candidate = &f.candidate;
    evidence.migration.source_schema = f.boundary.schema - 1;
    assert_eq!(
        f.store
            .assess_update_readiness(&f.request(&evidence, Some(&runtime)))
            .blockers,
        vec![Blocker::MigrationProofMismatch]
    );
    evidence.migration.source_schema = f.boundary.schema;
    evidence.recovery.restored.path = &f.backup;
    assert_eq!(
        f.store
            .assess_update_readiness(&f.request(&evidence, Some(&runtime)))
            .blockers,
        vec![Blocker::RecoveryProofMismatch]
    );
    evidence.recovery.restored.path = &f.restored;
    fs::write(&f.artifact, b"substituted artifact bytes").unwrap();
    assert_eq!(
        f.store
            .assess_update_readiness(&f.request(&evidence, Some(&runtime)))
            .blockers,
        vec![Blocker::ArtifactMismatch]
    );
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn backup_proof_requires_same_store_private_copy_and_standalone_sqlite() {
    let f = Fixture::new();
    let other = Store::open(&f.root.join("other.sqlite3")).unwrap();
    let other_boundary = other.update_database_boundary().unwrap();
    let background = f.background(&f.boundary, false);
    let runtime = available(&f.boundary, &background);
    let mut evidence = f.proof();
    evidence.backup.database = &other_boundary;
    let blocked = |evidence: &Evidence<'_>| {
        let decision = f
            .store
            .assess_update_readiness(&f.request(evidence, Some(&runtime)));
        assert_eq!(decision.blockers, vec![Blocker::BackupMismatch]);
        assert!(decision.ready.is_none());
    };
    blocked(&evidence);
    evidence.backup.database = &f.boundary;
    let sidecar = f.root.join(format!(
        "{}-wal",
        f.backup.file_name().unwrap().to_str().unwrap()
    ));
    fs::write(&sidecar, b"uncheckpointed-sidecar").unwrap();
    blocked(&evidence);
    fs::remove_file(sidecar).unwrap();
    fs::write(&f.backup, b"not SQLite even when its digest matches").unwrap();
    let wrong_bytes = executable_digest(&f.backup).unwrap();
    evidence.backup.backup.sha256 = &wrong_bytes;
    blocked(&evidence);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn update_recheck_rejects_new_actor_and_durable_drift_without_a_new_event() {
    let f = Fixture::new();
    let proof = f.proof();
    let background = f.background(&f.boundary, false);
    let runtime = available(&f.boundary, &background);
    let stamped = f
        .store
        .assess_update_readiness(&f.request(&proof, Some(&runtime)))
        .ready
        .unwrap();
    let changed = BackgroundStatus {
        instance_id: Uuid::new_v4().to_string(),
        ..background.clone()
    };
    let next = available(&f.boundary, &changed);
    let different_actor = f
        .store
        .recheck_update_readiness(&stamped, &f.request(&proof, Some(&next)));
    assert_eq!(different_actor.blockers, vec![Blocker::ReadinessChanged]);
    let second = Connection::open(&f.database).unwrap();
    second
        .execute_batch("CREATE TABLE fixture_drift_without_event(id INTEGER);")
        .unwrap();
    assert_eq!(f.store.sequence().unwrap(), f.boundary.sequence);
    let stale = f
        .store
        .recheck_update_readiness(&stamped, &f.request(&proof, Some(&runtime)));
    assert!(stale.blockers.contains(&Blocker::RuntimeObservationStale));
    assert!(stale.ready.is_none());
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn a_replaced_database_path_cannot_be_reanchored_by_the_open_sqlite_connection() {
    let f = Fixture::new();
    let old_inode = fs::metadata(&f.database).unwrap().ino();
    let parked = f.root.join("old-core.sqlite3");
    let replacement = f.root.join("replacement-core.sqlite3");
    fs::copy(&f.backup, &replacement).unwrap();
    fs::rename(&f.database, parked).unwrap();
    fs::rename(replacement, &f.database).unwrap();
    assert_ne!(fs::metadata(&f.database).unwrap().ino(), old_inode);
    assert_eq!(f.store.sequence().unwrap(), f.boundary.sequence);

    // Re-resolving a runtime and backup observation against the old connection
    // must not make the replacement filename its new database boundary.
    if let Ok(incorrect_boundary) = f.store.update_database_boundary() {
        let mut proof = f.proof();
        proof.backup.database = &incorrect_boundary;
        let status = f.background(&incorrect_boundary, false);
        let observed = available(&incorrect_boundary, &status);
        let result = f
            .store
            .assess_update_readiness(&f.request(&proof, Some(&observed)));
        assert!(
            result.ready.is_none(),
            "opened connection certified the replacement database path"
        );
    } else {
        let proof = f.proof();
        let result = f.store.assess_update_readiness(&f.request(&proof, None));
        assert_eq!(result.blockers, vec![Blocker::StoreObservationUnavailable]);
    }
}

fn binding() -> ExecutionBinding {
    ExecutionBinding {
        connection_id: "generated-provider".into(),
        environment_id: "fixture-env".into(),
        identity_ref: "fixture-identity".into(),
        host_id: "local".into(),
        workspace_id: "fixture-work".into(),
        config_revision: "fixture-config-v1".into(),
        input_revision: "fixture-input-v1".into(),
        capability_snapshot_id: "fixture-caps".into(),
        policy_revision: "fixture-policy".into(),
    }
}

#[test]
fn real_store_restart_preserves_unknown_dispatch_as_blocker() {
    let mut f = Fixture::new();
    let task = f
        .store
        .create("fixture", "Send then lose the executor", "dispatch-task")
        .unwrap();
    f.store.configure_budget(true, "fixture", 2, 1).unwrap();
    f.store.configure_budget(false, &task.id, 2, 1).unwrap();
    let dispatch = f
        .store
        .prepare_attempt(&task.id, task.revision, binding(), "prepare")
        .unwrap();
    f.store
        .mark_dispatch_started(&dispatch.id, "fixture-executor")
        .unwrap();
    let replacement = Store::open(&f.root.join("replacement.sqlite3")).unwrap();
    drop(std::mem::replace(&mut f.store, replacement));
    f.store = Store::open(&f.database).unwrap();
    assert_eq!(
        f.store.intent(&dispatch.id).unwrap().state,
        IntentState::OutcomeUnknown
    );
    assert_eq!(
        f.store.get(&task.id).unwrap().state,
        TaskState::OutcomeUnknown
    );
    let current = f.store.update_database_boundary().unwrap();
    let background = f.background(&current, false);
    let refs = f.proof();
    let result = f
        .store
        .assess_update_readiness(&f.request(&refs, Some(&available(&current, &background))));
    assert!(result
        .blockers
        .contains(&Blocker::Work(Source::Dispatch, Work::OutcomeUnknown)));
    assert!(result
        .blockers
        .contains(&Blocker::Work(Source::Tasks, Work::OutcomeUnknown)));
    assert_eq!(f.store.update_database_boundary().unwrap(), current);
    assert!(result.ready.is_none());
}

#[test]
fn goal_graph_factory_and_command_durable_states_are_checked_independently() {
    let f = Fixture::new();
    let database = Connection::open(&f.database).unwrap();
    database.execute_batch(
        "PRAGMA foreign_keys=OFF;
         INSERT INTO goal_runs(id,scope_id,project_id,task_id,revision,state,document)
            VALUES('goal','scope','project','task',1,'queued','{}');
         INSERT INTO goal_graph_jobs VALUES('job','goal','node',1,'cancel_requested',NULL,'workspace','{}');
         INSERT INTO factory_runs VALUES('factory','factory-scope','root','project',1,'outcome_unknown','{}');
         INSERT INTO factory_attempts VALUES('session','attempt','factory-scope','factory','task','root','{}','reserved','{}');
         INSERT INTO command_validation_runs VALUES('command','policy','task','workspace','running','{}','idem','{}');
         INSERT INTO events(document) VALUES(json_object(
              'kind','session.turn_ended','payload',json_object('turn_state','outcome_unknown')
         ));"
    ).unwrap();
    let current = f.store.update_database_boundary().unwrap();
    let background = f.background(&current, false);
    let proof = f.proof();
    let result = f
        .store
        .assess_update_readiness(&f.request(&proof, Some(&available(&current, &background))));
    for (source, work) in [
        (Source::Goal, Work::Active),
        (Source::Graph, Work::CancellationPending),
        (Source::Factory, Work::OutcomeUnknown),
        (Source::Command, Work::Active),
        (Source::Session, Work::OutcomeUnknown),
    ] {
        assert!(result.blockers.contains(&Blocker::Work(source, work)));
    }
    assert!(result.blockers.contains(&Blocker::StoreFencesUnresolved));
    assert!(result.ready.is_none());
    database
        .execute(
            "UPDATE goal_graph_jobs SET state='completed' WHERE id='job'",
            [],
        )
        .unwrap();
    let current = f.store.update_database_boundary().unwrap();
    let background = f.background(&current, false);
    let result = f
        .store
        .assess_update_readiness(&f.request(&proof, Some(&available(&current, &background))));
    assert!(!result
        .blockers
        .iter()
        .any(|blocker| matches!(blocker, Blocker::Work(Source::Graph, _))));
}

#[test]
fn real_managed_work_blocks_until_the_completed_turn_and_stop_are_settled() {
    let mut f = Fixture::new();
    let source = f.root.join("source");
    let managed = f.root.join("managed");
    let runtime = f.root.join("runtime");
    for path in [&source, &managed, &runtime] {
        fs::create_dir(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    git(&source, &["init", "--initial-branch=main"]);
    fs::write(source.join("input"), b"fixture").unwrap();
    git(&source, &["add", "input"]);
    git(&source, &["commit", "-m", "Generated"]);
    let project = f
        .store
        .import_project(&source, "Generated", "project")
        .unwrap();
    let head = git(&source, &["rev-parse", "HEAD"]);
    let workspace = f
        .store
        .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
        .unwrap();
    let task = f
        .store
        .create_registered(&project.id, "Generated local child", "task")
        .unwrap();
    let child = write_child(&f.root);
    let connection = f
        .store
        .register_connection("Generated", "custom_acp", &child, &[], "conn")
        .unwrap();
    let mut supervisor = Supervisor::new(&runtime);
    supervisor.probe(&f.store, &connection.id).unwrap();
    wait_until(|| {
        supervisor.tick(&mut f.store).unwrap();
        f.store.get_connection(&connection.id).unwrap().probe_state == "ready"
    });
    let grant = f
        .store
        .create_grant(
            &project.id,
            &workspace.id,
            &connection.id,
            2,
            1,
            60_000,
            true,
            "grant",
        )
        .unwrap();
    let session = f
        .store
        .create_session(
            &task.id,
            &workspace.id,
            &connection.id,
            &grant.id,
            task.revision,
            "session",
        )
        .unwrap();
    supervisor.launch(&mut f.store, &session.id).unwrap();
    wait_until(|| {
        supervisor.tick(&mut f.store).unwrap();
        f.store.get_session(&session.id).unwrap().state == "ready"
    });
    assert!(supervisor.has_workers());
    {
        let database = f.store.update_database_boundary().unwrap();
        let background = f.background(&database, supervisor.has_workers());
        let proof = f.proof();
        let result = f
            .store
            .assess_update_readiness(&f.request(&proof, Some(&available(&database, &background))));
        assert!(result
            .blockers
            .contains(&Blocker::Work(Source::Session, Work::Active)));
        assert!(result.ready.is_none());
        assert!(supervisor.has_workers());
        assert_eq!(f.store.update_database_boundary().unwrap(), database);
    }
    let session = f.store.get_session(&session.id).unwrap();
    let prompt = "Finish a harmless local fixture turn";
    let (_, queued) = f
        .store
        .send_session(&session.id, prompt, session.revision, "turn")
        .unwrap();
    assert!(queued);
    supervisor.send(&session.id, prompt).unwrap();
    wait_until(|| {
        supervisor.tick(&mut f.store).unwrap();
        f.store
            .get_session(&session.id)
            .unwrap()
            .turn_state
            .as_deref()
            == Some("completed")
    });
    let session = f.store.get_session(&session.id).unwrap();
    f.store
        .cancel_session(&session.id, session.revision, "cancel", true)
        .unwrap();
    {
        let database = f.store.update_database_boundary().unwrap();
        let background = f.background(&database, true);
        let proof = f.proof();
        let result = f
            .store
            .assess_update_readiness(&f.request(&proof, Some(&available(&database, &background))));
        assert!(result
            .blockers
            .contains(&Blocker::Work(Source::Session, Work::CancellationPending)));
        assert!(result.ready.is_none());
        assert_eq!(
            f.store.get_session(&session.id).unwrap().state,
            "cancel_requested"
        );
    }
    // Fixture cleanup belongs to the test. The readiness API never stops it.
    supervisor.shutdown(&mut f.store).unwrap();
    assert_eq!(f.store.get_session(&session.id).unwrap().state, "stopped");
    assert_eq!(
        f.store.intent(&session.intent_id).unwrap().state,
        IntentState::Stopped
    );
    assert!(!supervisor.has_workers());
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        f.refresh_database_proofs();
        let proof = f.proof();
        let observed = UpdateRuntimeObservation {
            database: &f.boundary,
            background: UpdateBackground::ConfirmedStopped {
                last_instance_id: &f.instance,
                joined_stop_ref: &session.intent_id,
            },
            other_local_work: Work::Idle,
            remote_work: Work::Idle,
        };
        let result = f
            .store
            .assess_update_readiness(&f.request(&proof, Some(&observed)));
        assert!(result.blockers.is_empty(), "{:?}", result.blockers);
        assert!(result.ready.is_some());
    }
    assert!(f.private.path().exists());
}

fn write_child(root: &Path) -> PathBuf {
    let python = Command::new("python3")
        .args([
            "-c",
            "import os,sys; print(os.path.realpath(sys.executable))",
        ])
        .output()
        .unwrap();
    assert!(python.status.success());
    let script = root.join("generated-child");
    let body = r#"import json,sys
if sys.argv[1:]==['--version']:
 print('Generated fixture 1.0'); sys.exit(0)
for line in sys.stdin:
 r=json.loads(line)
 if r['method']=='initialize':
  result={'protocolVersion':1,'agentCapabilities':{}}
 elif r['method']=='session/new':
  result={'sessionId':'generated-session'}
 elif r['method']=='session/prompt':
  assert r['params']['sessionId']=='generated-session'
  result={'stopReason':'end_turn'}
 else:
  sys.exit(3)
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':result}),flush=True)
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

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(12);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "generated process did not become ready"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn git(root: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name.as_bytes().starts_with(b"GIT_") {
            cmd.env_remove(name);
        }
    }
    let result = cmd
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Readiness fixture",
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
