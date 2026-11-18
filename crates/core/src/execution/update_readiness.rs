//! Read-only preflight for a specifically approved development update.
//! The caller must resolve evidence in the trusted actor, then stop new
//! admissions and recheck under its exclusive maintenance boundary.
use crate::{
    background_status::BackgroundStatus,
    storage::{db_error, Store},
    CoreError, Result,
};
use agentbaton_providers::installation::{verify_executable, BinaryDigest};
use rusqlite::{Connection, OpenFlags};
use std::{
    ffi::OsString,
    fmt::Write,
    fs,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevelopmentUpdate {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub commit_sha: String,
    pub artifact_sha256: BinaryDigest,
    pub target_schema: u32,
}

/// Resolved by the actor from the owner's live development approval.
/// A record from a manifest, installer or renderer cannot supply authority.
pub struct ApprovedDevelopmentUpdate<'a> {
    pub approval_ref: &'a str,
    pub candidate: &'a DevelopmentUpdate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateDatabaseBoundary {
    path: PathBuf,
    device: u64,
    inode: u64,
    pub schema: u32,
    pub sequence: u64,
    local_changes: u64,
    external_version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateWorkState {
    Idle,
    Active,
    CancellationPending,
    OutcomeUnknown,
}

pub enum UpdateBackground<'a> {
    Available(&'a BackgroundStatus),
    ConfirmedStopped {
        last_instance_id: &'a str,
        joined_stop_ref: &'a str,
    },
    Unavailable,
}

/// Capture under the same trusted actor/Store boundary. Other local work
/// includes pending scheduler admissions, validators, configuration probes,
/// integrations, data/workspace workers and outstanding settlement receipts.
/// The remote value covers authorized executors and outstanding handoffs.
pub struct UpdateRuntimeObservation<'a> {
    pub database: &'a UpdateDatabaseBoundary,
    pub background: UpdateBackground<'a>,
    pub other_local_work: UpdateWorkState,
    pub remote_work: UpdateWorkState,
}

pub struct UpdateEvidenceFile<'a> {
    pub path: &'a Path,
    pub sha256: &'a BinaryDigest,
}

/// The trusted backup verifier binds the bytes to the exact live Store
/// boundary captured with the SQLite backup; the gate inspects the copy.
pub struct UpdateBackupProof<'a> {
    pub receipt_ref: &'a str,
    pub database: &'a UpdateDatabaseBoundary,
    pub backup: UpdateEvidenceFile<'a>,
}

/// Receipt of an isolated successful run of the exact target build against
/// the exact backup. Includes the output database for a read-only check.
pub struct UpdateMigrationProof<'a> {
    pub rehearsal_ref: &'a str,
    pub candidate: &'a DevelopmentUpdate,
    pub input_backup_sha256: &'a BinaryDigest,
    pub source_schema: u32,
    pub result: UpdateEvidenceFile<'a>,
}

/// A successful isolated restore of the standalone pre-update backup,
/// tied to the exact approved rehearsal. No reverse schema migration is assumed.
pub struct UpdateRecoveryProof<'a> {
    pub rehearsal_ref: &'a str,
    pub migration_rehearsal_ref: &'a str,
    pub candidate: &'a DevelopmentUpdate,
    pub restored: UpdateEvidenceFile<'a>,
}

pub struct UpdateReadinessRequest<'a> {
    pub candidate: &'a DevelopmentUpdate,
    pub approval: Option<&'a ApprovedDevelopmentUpdate<'a>>,
    pub artifact_path: Option<&'a Path>,
    pub runtime: Option<&'a UpdateRuntimeObservation<'a>>,
    pub backup: Option<&'a UpdateBackupProof<'a>>,
    pub migration: Option<&'a UpdateMigrationProof<'a>>,
    pub recovery: Option<&'a UpdateRecoveryProof<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundAvailability {
    Available,
    ConfirmedStopped,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateWorkSource {
    Tasks,
    Dispatch,
    Session,
    Conversation,
    Goal,
    Graph,
    Factory,
    Command,
    Integration,
    Workspace,
    LocalWorkers,
    RemoteWorkers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateBlocker {
    RuntimeEvidenceMissing,
    RuntimeObservationStale,
    StoreObservationUnavailable,
    StoreFencesUnresolved,
    Work(UpdateWorkSource, UpdateWorkState),
    CandidateUnsupported,
    ApprovalMissing,
    ApprovalMismatch,
    ArtifactMissing,
    ArtifactMismatch,
    BackupMissing,
    BackupMismatch,
    MigrationProofMissing,
    MigrationProofMismatch,
    RecoveryProofMissing,
    RecoveryProofMismatch,
    ReadinessChanged,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UpdateReadiness {
    pub background: BackgroundAvailability,
    pub blockers: Vec<UpdateBlocker>,
    pub ready: Option<ReadyDevelopmentUpdate>,
}

/// Point-in-time stamp. An installer cannot reuse it after changing the
/// candidate, store, evidence, or background instance.
#[derive(Debug, PartialEq, Eq)]
pub struct ReadyDevelopmentUpdate {
    candidate: DevelopmentUpdate,
    database: UpdateDatabaseBoundary,
    background: BackgroundPin,
    approval_ref: String,
    artifact: FilePin,
    backup: FilePin,
    migrated: FilePin,
    restored: FilePin,
    receipt_refs: [String; 3],
}

#[derive(Debug, PartialEq, Eq)]
struct BackgroundPin {
    availability: BackgroundAvailability,
    instance: String,
    receipt_or_started_at: String,
}

#[derive(Debug, PartialEq, Eq)]
struct FilePin {
    path: PathBuf,
    device: u64,
    inode: u64,
    digest: BinaryDigest,
}

impl Store {
    /// Observes the current open SQLite connection. The counters are local to
    /// this connection; another actor must capture a new boundary.
    pub fn update_database_boundary(&self) -> Result<UpdateDatabaseBoundary> {
        database_boundary(&self.conn)
    }

    /// Does not stop workers, change admission, write SQLite or install anything.
    pub fn assess_update_readiness(&self, request: &UpdateReadinessRequest<'_>) -> UpdateReadiness {
        let mut result = UpdateReadiness {
            background: BackgroundAvailability::Unknown,
            blockers: Vec::new(),
            ready: None,
        };
        let Ok((database, work)) = observe_store(&self.conn) else {
            result
                .blockers
                .push(UpdateBlocker::StoreObservationUnavailable);
            return result;
        };
        result.blockers.extend(work);
        let background = runtime_pin(request.runtime, &database, &mut result);
        let inputs = evidence_inputs(request, &database, &mut result.blockers);
        let ready = (|| {
            let (background, inputs) = (background?, inputs?);
            if !result.blockers.is_empty() {
                return None;
            }
            let verified = match verify_evidence(request.candidate, &database, inputs) {
                Ok(verified) => verified,
                Err(blocker) => {
                    result.blockers.push(blocker);
                    return None;
                }
            };
            // File validation can take time. Reject even an external SQLite
            // change made during it, including writes without event rows.
            if self.update_database_boundary().ok().as_ref() != Some(&database) {
                result.blockers.push(UpdateBlocker::ReadinessChanged);
                return None;
            }
            Some(ReadyDevelopmentUpdate {
                candidate: request.candidate.clone(),
                database,
                background,
                approval_ref: verified.approval_ref.to_owned(),
                artifact: verified.artifact,
                backup: verified.backup,
                migrated: verified.migrated,
                restored: verified.restored,
                receipt_refs: verified.references.map(str::to_owned),
            })
        })();
        result.ready = ready;
        result
    }

    /// Re-run inside the actor's maintenance/quiescence boundary immediately
    /// before any approved switch. This method does not acquire that boundary.
    pub fn recheck_update_readiness(
        &self,
        prior: &ReadyDevelopmentUpdate,
        request: &UpdateReadinessRequest<'_>,
    ) -> UpdateReadiness {
        let mut current = self.assess_update_readiness(request);
        if current.ready.as_ref().is_some_and(|ready| ready != prior) {
            current.ready = None;
            current.blockers.push(UpdateBlocker::ReadinessChanged);
        }
        current
    }
}

fn sequence(conn: &Connection) -> Result<u64> {
    let sequence: i64 = conn
        .query_row("SELECT COALESCE(max(sequence),0) FROM events", [], |r| {
            r.get(0)
        })
        .map_err(db_error)?;
    nonnegative_counter(sequence)
}

fn nonnegative_counter(value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|_| {
        CoreError::new(
            "UPDATE_STATE_UNAVAILABLE",
            "Invalid Core database watermark",
        )
    })
}

fn schema(conn: &Connection) -> Result<u32> {
    conn.pragma_query_value(Some("main"), "user_version", |r| r.get(0))
        .map_err(db_error)
}

fn database_boundary(conn: &Connection) -> Result<UpdateDatabaseBoundary> {
    let (path, device, inode) = live_database_file(conn)?;
    let external_version = data_version(conn)?;
    let observed = UpdateDatabaseBoundary {
        path,
        device,
        inode,
        schema: schema(conn)?,
        sequence: sequence(conn)?,
        local_changes: conn.total_changes(),
        external_version,
    };
    let (path, device, inode) = live_database_file(conn)?;
    if (path, device, inode) != (observed.path.clone(), observed.device, observed.inode)
        || data_version(conn)? != external_version
    {
        return Err(database_unavailable());
    }
    Ok(observed)
}

fn data_version(conn: &Connection) -> Result<u64> {
    nonnegative_counter(
        conn.pragma_query_value(Some("main"), "data_version", |r| r.get(0))
            .map_err(db_error)?,
    )
}

fn database_unavailable() -> CoreError {
    CoreError::new(
        "UPDATE_STATE_UNAVAILABLE",
        "Cannot pin the open Core database",
    )
}

fn live_database_file(conn: &Connection) -> Result<(PathBuf, u64, u64)> {
    if !sqlite_file_still_named(conn) {
        return Err(database_unavailable());
    }
    let original = conn
        .path()
        .filter(|path| !path.is_empty())
        .ok_or_else(database_unavailable)?;
    let path = Path::new(original)
        .canonicalize()
        .map_err(|_| database_unavailable())?;
    let info = fs::symlink_metadata(&path).map_err(|_| database_unavailable())?;
    if path != Path::new(original)
        || !info.is_file()
        || info.nlink() != 1
        || info.uid() != rustix::process::geteuid().as_raw()
        || !sqlite_file_still_named(conn)
    {
        return Err(database_unavailable());
    }
    Ok((path, info.dev(), info.ino()))
}

fn sqlite_file_still_named(conn: &Connection) -> bool {
    let mut moved = -1_i32;
    // SAFETY: the borrowed Connection remains open and is used by its actor
    // thread. HAS_MOVED reads its Unix VFS file identity, taking the address of
    // a live int for its result. It does not replace or write the database.
    let status = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            conn.handle(),
            c"main".as_ptr(),
            rusqlite::ffi::SQLITE_FCNTL_HAS_MOVED,
            (&mut moved as *mut i32).cast(),
        )
    };
    status == rusqlite::ffi::SQLITE_OK && moved == 0
}

struct Lane {
    source: UpdateWorkSource,
    table: &'static str,
    column: &'static str,
    settled: &'static [&'static str],
    active: &'static [&'static str],
}

const fn lane(
    source: UpdateWorkSource,
    table: &'static str,
    settled: &'static [&'static str],
    active: &'static [&'static str],
) -> Lane {
    Lane {
        source,
        table,
        column: "state",
        settled,
        active,
    }
}

fn observe_store(conn: &Connection) -> Result<(UpdateDatabaseBoundary, Vec<UpdateBlocker>)> {
    use UpdateWorkSource as Source;
    use UpdateWorkState as State;
    let tx = conn.unchecked_transaction().map_err(db_error)?;
    let database = database_boundary(&tx)?;
    if database.schema != 19 {
        return Err(CoreError::new(
            "VERSION_UNSUPPORTED",
            "Update readiness requires the schema 19 Core state model",
        ));
    }
    let mut blockers = Vec::new();
    let lanes = [
        Lane {
            source: Source::Tasks,
            table: "tasks",
            column: "json_extract(document,'$.state')",
            settled: &["draft", "cancelled", "accepted"],
            active: &["running"],
        },
        lane(
            Source::Dispatch,
            "dispatch_intents",
            &["stopped"],
            &["prepared", "sent"],
        ),
        lane(
            Source::Session,
            "sessions",
            &["stopped", "failed"],
            &["starting", "ready", "running"],
        ),
        lane(Source::Session, "session_turns", &["ended"], &["pending"]),
        lane(
            Source::Conversation,
            "conversation_dispatches",
            &["stopped"],
            &["prepared", "sent"],
        ),
        lane(
            Source::Conversation,
            "conversations",
            &["stopped", "failed"],
            &["starting", "ready", "running"],
        ),
        lane(
            Source::Conversation,
            "conversation_turns",
            &["completed"],
            &["pending"],
        ),
        Lane {
            source: Source::LocalWorkers,
            table: "connections",
            column: "json_extract(document,'$.probe_state')",
            settled: &["ready", "failed"],
            active: &["pending"],
        },
        lane(
            Source::Goal,
            "goal_runs",
            &["accepted", "cancelled", "blocked", "awaiting_acceptance"],
            &["queued", "running"],
        ),
        lane(
            Source::Goal,
            "goal_run_nodes",
            &[
                "not_started",
                "accepted",
                "cancelled",
                "blocked",
                "candidate",
                "validated",
                "awaiting_owner",
                "failed",
            ],
            &["reserved", "running"],
        ),
        lane(
            Source::Graph,
            "goal_graph_jobs",
            &["finished", "passed", "failed", "cancelled"],
            &["prepared", "running"],
        ),
        lane(
            Source::Factory,
            "factory_runs",
            &["completed", "cancelled", "blocked"],
            &["queued", "starting", "running", "waiting_child", "resuming"],
        ),
        lane(
            Source::Command,
            "command_validation_runs",
            &["command_passed", "command_failed"],
            &["prepared", "running"],
        ),
        lane(
            Source::Integration,
            "integrations",
            &[
                "confirmed_local_ref",
                "blocked",
                "stale_target",
                "failed",
                "cancelled",
            ],
            &[
                "queued",
                "capturing",
                "preparing",
                "validating",
                "cas_prepared",
            ],
        ),
        lane(
            Source::Integration,
            "integration_attempts",
            &["finished"],
            &["prepared", "running"],
        ),
        lane(
            Source::Workspace,
            "managed_workspaces",
            &["ready"],
            &["creating"],
        ),
        lane(
            Source::Workspace,
            "data_workspace_previews",
            &["verified", "failed", "cancelled"],
            &["checking"],
        ),
        lane(
            Source::Workspace,
            "data_workspace_creations",
            &["ready", "failed", "cancelled"],
            &["creating"],
        ),
    ];
    for lane in lanes {
        // Identifiers and expressions are closed, in-process schema constants.
        // Bound unexpected state strings and report them only as unknown.
        let sql = format!(
            "SELECT DISTINCT CASE WHEN length({0})<=64 THEN {0} END FROM {1} LIMIT 32",
            lane.column, lane.table
        );
        let mut query = tx.prepare(&sql).map_err(db_error)?;
        let states = query
            .query_map([], |r| r.get::<_, Option<String>>(0))
            .map_err(db_error)?;
        for state in states {
            let state = state.map_err(db_error)?;
            let text = state.as_deref().unwrap_or("");
            if lane.settled.contains(&text) {
                continue;
            }
            let classification = match text {
                "cancel_requested" | "stopping" => State::CancellationPending,
                active if lane.active.contains(&active) => State::Active,
                _ => State::OutcomeUnknown,
            };
            add_work(&mut blockers, lane.source, classification);
        }
    }
    let unresolved: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspace_leases WHERE holder!='')
          OR EXISTS(SELECT 1 FROM command_workspace_fences)
          OR EXISTS(SELECT 1 FROM integration_ref_fences WHERE state!='released')
          OR EXISTS(SELECT 1 FROM budgets WHERE active!=0)
          OR EXISTS(SELECT 1 FROM attempts a LEFT JOIN dispatch_intents d ON d.attempt_id=a.id WHERE d.id IS NULL)
          OR EXISTS(SELECT 1 FROM tasks WHERE json_type(document,'$.code_workspace_binding')='object'
                     AND COALESCE(json_extract(document,'$.code_workspace_binding.state'),'')!='ready')
          OR EXISTS(SELECT 1 FROM goal_run_nodes WHERE session_id IS NOT NULL
                     AND COALESCE(json_extract(document,'$.stop_confirmed'),0)!=1)",
        [], |row| row.get(0),
    ).map_err(db_error)?;
    if unresolved {
        blockers.push(UpdateBlocker::StoreFencesUnresolved);
    }
    let unknown_session_effects: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE json_extract(document,'$.turn_state')='outcome_unknown')
          OR EXISTS(SELECT 1 FROM events WHERE json_extract(document,'$.kind')='session.turn_ended'
                     AND json_extract(document,'$.payload.turn_state')='outcome_unknown')",
        [], |row| row.get(0),
    ).map_err(db_error)?;
    if unknown_session_effects {
        add_work(&mut blockers, Source::Session, State::OutcomeUnknown);
    }
    if database_boundary(&tx)? != database {
        return Err(database_unavailable());
    }
    tx.commit().map_err(db_error)?;
    Ok((database, blockers))
}

fn add_work(blockers: &mut Vec<UpdateBlocker>, source: UpdateWorkSource, state: UpdateWorkState) {
    let blocker = UpdateBlocker::Work(source, state);
    if state != UpdateWorkState::Idle && !blockers.contains(&blocker) {
        blockers.push(blocker);
    }
}

fn reference(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn runtime_pin(
    runtime: Option<&UpdateRuntimeObservation<'_>>,
    database: &UpdateDatabaseBoundary,
    result: &mut UpdateReadiness,
) -> Option<BackgroundPin> {
    let Some(runtime) = runtime else {
        result.blockers.push(UpdateBlocker::RuntimeEvidenceMissing);
        return None;
    };
    add_work(
        &mut result.blockers,
        UpdateWorkSource::LocalWorkers,
        runtime.other_local_work,
    );
    add_work(
        &mut result.blockers,
        UpdateWorkSource::RemoteWorkers,
        runtime.remote_work,
    );
    let runtime_current = runtime.database == database;
    let (availability, instance, reference) = match &runtime.background {
        UpdateBackground::Available(status) => {
            if status.owned_provider_workers {
                add_work(
                    &mut result.blockers,
                    UpdateWorkSource::LocalWorkers,
                    UpdateWorkState::Active,
                );
            }
            if status.api_version != crate::ipc::API_VERSION
                || status.database_schema_version != database.schema
                || status.sequence != database.sequence
                || status.store_recovery != "completed_on_start"
            {
                result.blockers.push(UpdateBlocker::RuntimeObservationStale);
                return None;
            }
            (
                BackgroundAvailability::Available,
                status.instance_id.as_str(),
                status.started_at.as_str(),
            )
        }
        UpdateBackground::ConfirmedStopped {
            last_instance_id,
            joined_stop_ref,
        } => (
            BackgroundAvailability::ConfirmedStopped,
            *last_instance_id,
            *joined_stop_ref,
        ),
        UpdateBackground::Unavailable => {
            result.blockers.push(UpdateBlocker::RuntimeEvidenceMissing);
            return None;
        }
    };
    if !runtime_current || !self::reference(reference) || Uuid::parse_str(instance).is_err() {
        result.blockers.push(UpdateBlocker::RuntimeObservationStale);
        return None;
    }
    result.background = availability;
    Some(BackgroundPin {
        availability,
        instance: instance.into(),
        receipt_or_started_at: reference.into(),
    })
}

struct EvidenceInputs<'a> {
    approval: &'a ApprovedDevelopmentUpdate<'a>,
    artifact: &'a Path,
    backup: &'a UpdateBackupProof<'a>,
    migration: &'a UpdateMigrationProof<'a>,
    recovery: &'a UpdateRecoveryProof<'a>,
}

fn evidence_inputs<'a>(
    r: &'a UpdateReadinessRequest<'a>,
    database: &UpdateDatabaseBoundary,
    blockers: &mut Vec<UpdateBlocker>,
) -> Option<EvidenceInputs<'a>> {
    let candidate = r.candidate;
    if !reference(&candidate.version)
        || candidate.os != "macos"
        || candidate.arch != "aarch64"
        || std::env::consts::OS != candidate.os
        || std::env::consts::ARCH != candidate.arch
        || candidate.target_schema < database.schema
        || candidate.commit_sha.len() != 40
        || !candidate
            .commit_sha
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        blockers.push(UpdateBlocker::CandidateUnsupported);
    }
    match r.approval {
        None => blockers.push(UpdateBlocker::ApprovalMissing),
        Some(a) if a.candidate != candidate || !reference(a.approval_ref) => {
            blockers.push(UpdateBlocker::ApprovalMismatch)
        }
        _ => {}
    }
    for (present, missing) in [
        (r.artifact_path.is_some(), UpdateBlocker::ArtifactMissing),
        (r.backup.is_some(), UpdateBlocker::BackupMissing),
        (r.migration.is_some(), UpdateBlocker::MigrationProofMissing),
        (r.recovery.is_some(), UpdateBlocker::RecoveryProofMissing),
    ] {
        if !present {
            blockers.push(missing);
        }
    }
    Some(EvidenceInputs {
        approval: r.approval?,
        artifact: r.artifact_path?,
        backup: r.backup?,
        migration: r.migration?,
        recovery: r.recovery?,
    })
}

struct VerifiedEvidence<'a> {
    approval_ref: &'a str,
    artifact: FilePin,
    backup: FilePin,
    migrated: FilePin,
    restored: FilePin,
    references: [&'a str; 3],
}

fn verify_evidence<'a>(
    candidate: &DevelopmentUpdate,
    database: &UpdateDatabaseBoundary,
    input: EvidenceInputs<'a>,
) -> std::result::Result<VerifiedEvidence<'a>, UpdateBlocker> {
    use UpdateBlocker as Blocker;
    let artifact = file_pin(
        &UpdateEvidenceFile {
            path: input.artifact,
            sha256: &candidate.artifact_sha256,
        },
        false,
    )
    .ok_or(Blocker::ArtifactMismatch)?;
    if input.backup.database != database || !reference(input.backup.receipt_ref) {
        return Err(Blocker::BackupMismatch);
    }
    let (backup, copied_schema, copied_seq) =
        sqlite_evidence(&input.backup.backup).ok_or(Blocker::BackupMismatch)?;
    if copied_schema != database.schema
        || copied_seq != database.sequence
        || (backup.device, backup.inode) == (database.device, database.inode)
    {
        return Err(Blocker::BackupMismatch);
    }
    let migration = input.migration;
    if migration.candidate != candidate
        || migration.input_backup_sha256 != &backup.digest
        || migration.source_schema != copied_schema
        || !reference(migration.rehearsal_ref)
    {
        return Err(Blocker::MigrationProofMismatch);
    }
    let (migrated, target_schema, target_seq) =
        sqlite_evidence(&migration.result).ok_or(Blocker::MigrationProofMismatch)?;
    if target_schema != candidate.target_schema
        || target_seq < copied_seq
        || !separate_database_copy(&migrated, database, &[&backup])
    {
        return Err(Blocker::MigrationProofMismatch);
    }
    let recovery = input.recovery;
    if recovery.candidate != candidate
        || recovery.migration_rehearsal_ref != migration.rehearsal_ref
        || !reference(recovery.rehearsal_ref)
        || recovery.restored.sha256 != &backup.digest
    {
        return Err(Blocker::RecoveryProofMismatch);
    }
    let (restored, restored_schema, restored_seq) =
        sqlite_evidence(&recovery.restored).ok_or(Blocker::RecoveryProofMismatch)?;
    if restored_schema != copied_schema
        || restored_seq != copied_seq
        || !separate_database_copy(&restored, database, &[&backup, &migrated])
    {
        return Err(Blocker::RecoveryProofMismatch);
    }
    Ok(VerifiedEvidence {
        approval_ref: input.approval.approval_ref,
        artifact,
        backup,
        migrated,
        restored,
        references: [
            input.backup.receipt_ref,
            migration.rehearsal_ref,
            recovery.rehearsal_ref,
        ],
    })
}

fn separate_database_copy(
    file: &FilePin,
    live: &UpdateDatabaseBoundary,
    others: &[&FilePin],
) -> bool {
    let identity = (file.device, file.inode);
    identity != (live.device, live.inode)
        && others
            .iter()
            .all(|other| identity != (other.device, other.inode))
}

fn file_pin(proof: &UpdateEvidenceFile<'_>, private: bool) -> Option<FilePin> {
    let path = proof.path;
    if !path.is_absolute() || path.canonicalize().ok()?.as_path() != path {
        return None;
    }
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & (if private { 0o077 } else { 0o022 }) != 0
    {
        return None;
    }
    if private {
        let parent = fs::metadata(path.parent()?).ok()?;
        if parent.mode() & 0o077 != 0 || parent.uid() != metadata.uid() {
            return None;
        }
    }
    // The existing bounded selected-file verifier only reads bytes.
    verify_executable(path, proof.sha256).ok()?;
    let after = fs::symlink_metadata(path).ok()?;
    if (
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.uid(),
        metadata.nlink(),
    ) != (
        after.dev(),
        after.ino(),
        after.mode(),
        after.uid(),
        after.nlink(),
    ) {
        return None;
    }
    Some(FilePin {
        path: path.into(),
        device: metadata.dev(),
        inode: metadata.ino(),
        digest: proof.sha256.clone(),
    })
}

fn standalone(path: &Path) -> bool {
    ["-wal", "-shm", "-journal"].iter().all(|suffix| {
        let mut name = OsString::from(path.as_os_str());
        name.push(suffix);
        fs::symlink_metadata(name).is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    })
}

fn sqlite_evidence(proof: &UpdateEvidenceFile<'_>) -> Option<(FilePin, u32, u64)> {
    let before = file_pin(proof, true)?;
    if !standalone(proof.path) {
        return None;
    }
    // Immutable read-only URI: no lock/SHM/journal writes. Percent-encode
    // the absolute path so a filename cannot set SQLite URI options.
    let mut uri = "file:".to_owned();
    for byte in proof.path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/._-".contains(byte) {
            uri.push(char::from(*byte));
        } else {
            write!(uri, "%{byte:02X}").ok()?;
        }
    }
    uri.push_str("?mode=ro&immutable=1");
    let db = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .ok()?;
    db.pragma_update(None, "trusted_schema", false).ok()?;
    let integrity: String = db
        .query_row("PRAGMA quick_check(1)", [], |r| r.get(0))
        .ok()?;
    let inspected = (schema(&db).ok()?, sequence(&db).ok()?);
    if integrity != "ok" || !standalone(proof.path) || file_pin(proof, true)? != before {
        return None;
    }
    Some((before, inspected.0, inspected.1))
}
