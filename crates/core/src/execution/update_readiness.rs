//! Private backup evidence and preflight for a specifically approved development update.
//! The caller must resolve evidence in the trusted actor, then stop new
//! admissions and recheck under its exclusive maintenance boundary.
use crate::{
    background_status::BackgroundStatus,
    storage::{db_error, Store},
    CoreError, Result,
};
use agentbaton_providers::installation::{executable_digest, verify_executable, BinaryDigest};
use rusqlite::{
    backup::{Backup, StepResult},
    Connection, OpenFlags,
};
use rustix::fs::{self as rfs, AtFlags, Mode, OFlags};
use std::{
    ffi::{OsStr, OsString},
    fmt::Write,
    fs::{self, File},
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;

const UPDATE_SCHEMA: u32 = 19;
const BACKUP_TEMP_NAME: &str = "snapshot.sqlite3";
const SIDECAR_SUFFIXES: [&str; 3] = ["-wal", "-shm", "-journal"];

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

/// A private backup issued by the Store. The receipt can become stale when
/// the Store or the evidence file changes. It grants no update authority.
#[derive(Debug)]
pub struct IssuedUpdateBackup {
    receipt_ref: String,
    database: UpdateDatabaseBoundary,
    path: PathBuf,
    digest: BinaryDigest,
}

impl IssuedUpdateBackup {
    pub fn proof(&self) -> UpdateBackupProof<'_> {
        UpdateBackupProof {
            receipt_ref: &self.receipt_ref,
            database: &self.database,
            backup: UpdateEvidenceFile {
                path: &self.path,
                sha256: &self.digest,
            },
        }
    }
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

    /// Copy the current Store through its existing SQLite connection. The
    /// trusted actor chooses an unused absolute path in a private directory.
    /// An online backup does not settle work or acquire maintenance authority.
    pub fn issue_update_backup(&mut self, destination: &Path) -> Result<IssuedUpdateBackup> {
        let mut stage = BackupStage::allocate(destination)?;
        let result = (|| {
            let database = copy_update_database(&mut self.conn, &mut stage)?;
            finish_update_backup(&self.conn, &mut stage, database)
        })();
        if result.is_ok() {
            stage.published = false;
        } else if stage.discard().is_err() {
            return Err(backup_cleanup_failed());
        }
        result
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

fn backup_failed() -> CoreError {
    CoreError::new(
        "UPDATE_BACKUP_FAILED",
        "Cannot verify a private Store backup",
    )
}

fn backup_occupied() -> CoreError {
    CoreError::new(
        "UPDATE_BACKUP_DESTINATION_OCCUPIED",
        "The backup path or a SQLite sidecar already exists",
    )
}

fn backup_source_changed() -> CoreError {
    CoreError::new(
        "UPDATE_BACKUP_SOURCE_CHANGED",
        "The Store changed before the backup proof could be issued",
    )
}

fn backup_cleanup_failed() -> CoreError {
    let mut error = CoreError::new(
        "UPDATE_BACKUP_CLEANUP_FAILED",
        "Private backup cleanup could not be confirmed",
    );
    error.side_effect_state = "unknown".into();
    error
}

fn same_descriptor_entry(file: &File, directory: &File, name: &OsStr) -> bool {
    match (
        rfs::fstat(file),
        rfs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW),
    ) {
        (Ok(held), Ok(named)) => (held.st_dev, held.st_ino) == (named.st_dev, named.st_ino),
        _ => false,
    }
}

fn with_suffix(name: &OsStr, suffix: &str) -> OsString {
    let mut result = name.to_os_string();
    result.push(suffix);
    result
}

struct BackupStage<'a> {
    destination: &'a Path,
    name: &'a OsStr,
    parent_path: &'a Path,
    parent: File,
    directory_name: String,
    directory: File,
    receipt: Uuid,
    file: Option<File>,
    staging_file_exists: bool,
    staging_directory_exists: bool,
    published: bool,
}

impl<'a> BackupStage<'a> {
    fn allocate(destination: &'a Path) -> Result<Self> {
        let invalid = || {
            CoreError::new(
                "UPDATE_BACKUP_PATH_INVALID",
                "Backup destination requires a canonical owned private directory",
            )
        };
        let name = destination.file_name().ok_or_else(invalid)?;
        let parent_path = destination.parent().ok_or_else(invalid)?;
        if !destination.is_absolute()
            || fs::canonicalize(parent_path).ok().as_deref() != Some(parent_path)
        {
            return Err(invalid());
        }
        let directory_flags =
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let parent = File::from(
            rfs::open(parent_path, directory_flags, Mode::empty()).map_err(|_| invalid())?,
        );
        let metadata = parent.metadata().map_err(|_| invalid())?;
        if metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o7777 != 0o700
        {
            return Err(invalid());
        }
        Self::ensure_unused(&parent, name)?;
        let receipt = Uuid::new_v4();
        let directory_name = format!(".update-backup-{receipt}");
        rfs::mkdirat(&parent, directory_name.as_str(), Mode::RWXU).map_err(|_| backup_failed())?;
        let directory = match rfs::openat(
            &parent,
            directory_name.as_str(),
            directory_flags,
            Mode::empty(),
        ) {
            Ok(fd) => File::from(fd),
            Err(_) => {
                rfs::unlinkat(&parent, directory_name.as_str(), AtFlags::REMOVEDIR)
                    .map_err(|_| backup_cleanup_failed())?;
                return Err(backup_failed());
            }
        };
        Ok(Self {
            destination,
            name,
            parent_path,
            parent,
            directory_name,
            directory,
            receipt,
            file: None,
            staging_file_exists: false,
            staging_directory_exists: true,
            published: false,
        })
    }

    fn ensure_unused(directory: &File, name: &OsStr) -> Result<()> {
        for entry in std::iter::once(name.to_os_string())
            .chain(SIDECAR_SUFFIXES.map(|suffix| with_suffix(name, suffix)))
        {
            match rfs::statat(directory, &entry, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(_) => return Err(backup_occupied()),
                Err(rustix::io::Errno::NOENT) => (),
                Err(_) => return Err(backup_failed()),
            }
        }
        Ok(())
    }

    fn staging_path(&self) -> PathBuf {
        self.parent_path
            .join(&self.directory_name)
            .join(BACKUP_TEMP_NAME)
    }

    fn check_directories(&self) -> Result<()> {
        let named = fs::symlink_metadata(self.parent_path).map_err(|_| backup_failed())?;
        let held = self.parent.metadata().map_err(|_| backup_failed())?;
        let stage = self.directory.metadata().map_err(|_| backup_failed())?;
        let uid = rustix::process::geteuid().as_raw();
        if (named.dev(), named.ino()) != (held.dev(), held.ino())
            || !named.is_dir()
            || named.uid() != uid
            || named.mode() & 0o7777 != 0o700
            || fs::canonicalize(self.parent_path).ok().as_deref() != Some(self.parent_path)
            || !stage.is_dir()
            || stage.uid() != uid
            || stage.mode() & 0o7777 != 0o700
            || !same_descriptor_entry(&self.directory, &self.parent, self.directory_name.as_ref())
        {
            return Err(backup_failed());
        }
        Ok(())
    }

    fn create_file(&mut self) -> Result<()> {
        self.check_directories()?;
        let file = File::from(
            rfs::openat(
                &self.directory,
                BACKUP_TEMP_NAME,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map_err(|_| backup_failed())?,
        );
        self.file = Some(file);
        self.staging_file_exists = true;
        Ok(())
    }

    fn check_file_at(&self, directory: &File, name: &OsStr) -> Result<&File> {
        let file = self.file.as_ref().ok_or_else(backup_failed)?;
        let meta = file.metadata().map_err(|_| backup_failed())?;
        if !meta.is_file()
            || meta.uid() != rustix::process::geteuid().as_raw()
            || meta.mode() & 0o7777 != 0o600
            || !same_descriptor_entry(file, directory, name)
        {
            return Err(backup_failed());
        }
        Ok(file)
    }

    fn remove_file_at(&self, directory: &File, name: &OsStr) -> Result<()> {
        if !self
            .file
            .as_ref()
            .is_some_and(|file| same_descriptor_entry(file, directory, name))
        {
            return Err(backup_cleanup_failed());
        }
        rfs::unlinkat(directory, name, AtFlags::empty()).map_err(|_| backup_cleanup_failed())
    }

    fn publish(&mut self) -> Result<()> {
        self.check_directories()?;
        let staged = self.check_file_at(&self.directory, BACKUP_TEMP_NAME.as_ref())?;
        staged.sync_all().map_err(|_| backup_failed())?;
        self.directory.sync_all().map_err(|_| backup_failed())?;
        Self::ensure_unused(&self.parent, self.name)?;
        match rfs::linkat(
            &self.directory,
            BACKUP_TEMP_NAME,
            &self.parent,
            self.name,
            AtFlags::empty(),
        ) {
            Ok(()) => self.published = true,
            Err(rustix::io::Errno::EXIST) => return Err(backup_occupied()),
            Err(_) => return Err(backup_failed()),
        }
        self.remove_stage()?;
        self.parent.sync_all().map_err(|_| backup_failed())?;
        Ok(())
    }

    fn remove_stage(&mut self) -> Result<()> {
        if !self.staging_directory_exists {
            return Ok(());
        }
        if !same_descriptor_entry(&self.directory, &self.parent, self.directory_name.as_ref()) {
            return Err(backup_cleanup_failed());
        }
        if self.staging_file_exists {
            self.remove_file_at(&self.directory, BACKUP_TEMP_NAME.as_ref())?;
            self.staging_file_exists = false;
        }
        // The fresh directory belongs only to this backup operation. Remove
        // the SQLite-owned sidecar names without following or recursing links.
        for suffix in SIDECAR_SUFFIXES {
            let name = with_suffix(BACKUP_TEMP_NAME.as_ref(), suffix);
            match rfs::statat(&self.directory, &name, AtFlags::SYMLINK_NOFOLLOW) {
                Err(rustix::io::Errno::NOENT) => (),
                Ok(meta)
                    if rfs::FileType::from_raw_mode(meta.st_mode) == rfs::FileType::RegularFile
                        && meta.st_uid == rustix::process::geteuid().as_raw()
                        && meta.st_nlink == 1 =>
                {
                    rfs::unlinkat(&self.directory, &name, AtFlags::empty())
                        .map_err(|_| backup_cleanup_failed())?;
                }
                _ => return Err(backup_cleanup_failed()),
            }
        }
        self.directory
            .sync_all()
            .map_err(|_| backup_cleanup_failed())?;
        rfs::unlinkat(
            &self.parent,
            self.directory_name.as_str(),
            AtFlags::REMOVEDIR,
        )
        .map_err(|_| backup_cleanup_failed())?;
        self.staging_directory_exists = false;
        Ok(())
    }

    fn discard(&mut self) -> Result<()> {
        let final_cleanup = (|| {
            if self.published {
                self.remove_file_at(&self.parent, self.name)?;
                self.published = false;
            }
            Ok(())
        })();
        let stage_cleanup = self.remove_stage();
        let sync = self.parent.sync_all().map_err(|_| backup_cleanup_failed());
        final_cleanup.and(stage_cleanup).and(sync)
    }
}

impl Drop for BackupStage<'_> {
    fn drop(&mut self) {
        if self.published || self.staging_directory_exists {
            let _ = self.discard();
        }
    }
}

fn check_backup_boundary(conn: &Connection, database: &UpdateDatabaseBoundary) -> Result<()> {
    if database_boundary(conn).ok().as_ref() != Some(database) {
        return Err(backup_source_changed());
    }
    Ok(())
}

fn copy_update_database(
    conn: &mut Connection,
    stage: &mut BackupStage<'_>,
) -> Result<UpdateDatabaseBoundary> {
    let database = database_boundary(conn)?;
    if database.schema != UPDATE_SCHEMA {
        return Err(CoreError::new(
            "VERSION_UNSUPPORTED",
            "Update backups require the current Core Store schema",
        ));
    }
    let source = conn.transaction().map_err(db_error)?;
    check_backup_boundary(&source, &database)?;
    stage.create_file()?;
    let mut target = Connection::open_with_flags(
        stage.staging_path(),
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(db_error)?;
    stage.check_directories()?;
    stage.check_file_at(&stage.directory, BACKUP_TEMP_NAME.as_ref())?;
    target
        .busy_timeout(Duration::from_millis(100))
        .map_err(db_error)?;
    target
        .pragma_update(None, "synchronous", "FULL")
        .map_err(db_error)?;
    {
        let backup = Backup::new(&source, &mut target).map_err(db_error)?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if Instant::now() >= deadline {
                return Err(CoreError::new(
                    "WORKSPACE_BUSY",
                    "The update backup deadline expired",
                ));
            }
            match backup.step(64).map_err(db_error)? {
                StepResult::Done => break,
                StepResult::More => (),
                StepResult::Busy | StepResult::Locked => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                _ => return Err(backup_failed()),
            }
        }
    }
    target.close().map_err(|(_, error)| db_error(error))?;
    check_backup_boundary(&source, &database)?;
    source.commit().map_err(db_error)?;
    check_backup_boundary(conn, &database)?;
    Ok(database)
}

fn finish_update_backup(
    conn: &Connection,
    stage: &mut BackupStage<'_>,
    database: UpdateDatabaseBoundary,
) -> Result<IssuedUpdateBackup> {
    stage.check_directories()?;
    let staged = stage.staging_path();
    let digest = executable_digest(&staged).map_err(|_| backup_failed())?;
    let (copy, copied_schema, copied_sequence) = sqlite_evidence(&UpdateEvidenceFile {
        path: &staged,
        sha256: &digest,
    })
    .ok_or_else(backup_failed)?;
    if copied_schema != database.schema
        || copied_sequence != database.sequence
        || !separate_database_copy(&copy, &database, &[])
    {
        return Err(backup_failed());
    }
    check_backup_boundary(conn, &database)?;
    stage.publish()?;
    let final_file = stage.check_file_at(&stage.parent, stage.name)?;
    let metadata = final_file.metadata().map_err(|_| backup_failed())?;
    let path = stage.destination;
    if (metadata.dev(), metadata.ino()) != (copy.device, copy.inode)
        || !standalone(path)
        || file_pin(
            &UpdateEvidenceFile {
                path,
                sha256: &digest,
            },
            true,
        ) != Some(FilePin {
            path: path.into(),
            ..copy
        })
    {
        return Err(backup_failed());
    }
    check_backup_boundary(conn, &database)?;
    Ok(IssuedUpdateBackup {
        receipt_ref: format!("update-backup:{}", stage.receipt),
        database,
        path: path.into(),
        digest,
    })
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
    if database.schema != UPDATE_SCHEMA {
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
            &["completed", "failed", "cancelled"],
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
          OR EXISTS(SELECT 1 FROM dispatch_intents WHERE COALESCE(json_extract(document,'$.state'),'')!=state)
          OR EXISTS(SELECT 1 FROM sessions WHERE COALESCE(json_extract(document,'$.state'),'')!=state)
          OR EXISTS(SELECT 1 FROM conversations WHERE COALESCE(json_extract(document,'$.state'),'')!=state)
          OR EXISTS(SELECT 1 FROM goal_run_reservations WHERE COALESCE(json_extract(document,'$.settled'),0)!=1)
          OR EXISTS(SELECT 1 FROM factory_budget_reservations WHERE COALESCE(json_extract(document,'$.settled'),0)!=1)
          OR EXISTS(SELECT 1 FROM factory_attempts WHERE COALESCE(json_extract(document,'$.stop_confirmed'),0)!=1)
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
    SIDECAR_SUFFIXES.iter().all(|suffix| {
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

#[cfg(test)]
mod update_backup_issuer_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn private_fixture() -> (tempfile::TempDir, PathBuf, Store) {
        let sandbox = tempfile::tempdir().unwrap();
        let root = sandbox.path().canonicalize().unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let store = Store::open(&root.join("core.sqlite3")).unwrap();
        (sandbox, root, store)
    }

    #[test]
    fn issued_snapshot_is_rejected_and_cleaned_on_eventless_external_write() {
        let (_sandbox, root, mut store) = private_fixture();
        let database = root.join("core.sqlite3");
        store.create("fixture", "Draft", "backup-drift").unwrap();
        let destination = root.join("backup.sqlite3");
        let mut stage = BackupStage::allocate(&destination).unwrap();
        let captured = copy_update_database(&mut store.conn, &mut stage).unwrap();
        let external = Connection::open(&database).unwrap();
        external
            .execute_batch("CREATE TABLE update_backup_eventless_drift(id INTEGER);")
            .unwrap();
        let changed = store.update_database_boundary().unwrap();
        assert_eq!(captured.sequence, changed.sequence);
        assert_ne!(captured.external_version, changed.external_version);
        assert_eq!(
            finish_update_backup(&store.conn, &mut stage, captured)
                .unwrap_err()
                .code,
            "UPDATE_BACKUP_SOURCE_CHANGED"
        );
        let staged_path = stage.staging_path();
        stage.discard().unwrap();
        assert!(!destination.exists());
        assert!(!staged_path.parent().unwrap().exists());
        assert_eq!(store.update_database_boundary().unwrap(), changed);
    }

    #[test]
    fn late_destination_collision_cleans_only_the_owned_staging_directory() {
        let (_sandbox, root, mut store) = private_fixture();
        let destination = root.join("backup.sqlite3");
        let mut stage = BackupStage::allocate(&destination).unwrap();
        let captured = copy_update_database(&mut store.conn, &mut stage).unwrap();
        fs::write(&destination, b"existing evidence").unwrap();
        assert_eq!(
            finish_update_backup(&store.conn, &mut stage, captured.clone())
                .unwrap_err()
                .code,
            "UPDATE_BACKUP_DESTINATION_OCCUPIED"
        );
        let staged_path = stage.staging_path();
        stage.discard().unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"existing evidence");
        assert!(!staged_path.parent().unwrap().exists());
        assert_eq!(store.update_database_boundary().unwrap(), captured);
    }

    #[test]
    fn staged_copy_with_sidecar_is_rejected_and_removed() {
        let (_sandbox, root, mut store) = private_fixture();
        let destination = root.join("backup.sqlite3");
        let mut stage = BackupStage::allocate(&destination).unwrap();
        let captured = copy_update_database(&mut store.conn, &mut stage).unwrap();
        let staged_path = stage.staging_path();
        let sidecar = with_suffix(staged_path.as_os_str(), "-journal");
        fs::write(&sidecar, b"staged sidecar").unwrap();
        assert_eq!(
            finish_update_backup(&store.conn, &mut stage, captured.clone())
                .unwrap_err()
                .code,
            "UPDATE_BACKUP_FAILED"
        );
        stage.discard().unwrap();
        assert!(!destination.exists());
        assert!(!staged_path.parent().unwrap().exists());
        assert_eq!(store.update_database_boundary().unwrap(), captured);
    }
}
