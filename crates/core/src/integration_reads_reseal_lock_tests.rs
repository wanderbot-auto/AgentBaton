//! Actual cross-process SQLite locking, limited to DatabasePin descriptor lifetime.
//! This does not fabricate an admitted IntegrationStageTicket or call full reseal.
//! WAL writer locks and rollback-journal POSIX locks are separate evidence.
use super::*;
use std::{
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    process::{Child, Command, ExitStatus, Stdio},
};

const PROBE_DB: &str = "AGENTBATON_RESEAL_LOCK_PROBE_DB";
const PROBE_RESULT: &str = "AGENTBATON_RESEAL_LOCK_PROBE_RESULT";

struct RetainedRoot(Option<tempfile::TempDir>);
impl RetainedRoot {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(Some(directory))
    }
    fn path(&self) -> PathBuf {
        self.0.as_ref().unwrap().path().canonicalize().unwrap()
    }
}
impl Drop for RetainedRoot {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(directory) = self.0.take() {
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "Retained SQLite descriptor failure fixture: {}",
                    directory.keep().display()
                );
            }
        }
    }
}

struct OwnedProbe(Option<Child>);
impl OwnedProbe {
    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.0.as_mut().unwrap().try_wait().unwrap() {
                self.0.take(); // try_wait consumed the real exit receipt.
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "SQLite probe observation expired"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for OwnedProbe {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            // The original direct child is always reaped, including panic.
            let _ = child.wait();
        }
    }
}

fn competing_writer(root: &Path, path: &Path, sequence: u32) -> String {
    let result = root.join(format!("probe-{sequence}.result"));
    let log_path = root.join(format!("probe-{sequence}.log"));
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&log_path)
        .unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            concat!(module_path!(), "::competing_sqlite_writer_probe"),
            "--nocapture",
        ])
        .env(PROBE_DB, path)
        .env(PROBE_RESULT, &result)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log.try_clone().unwrap())
        .spawn()
        .unwrap();
    let status = OwnedProbe(Some(child)).wait();
    let mut diagnostic = String::new();
    std::fs::File::open(&log_path)
        .unwrap()
        .take(16_385)
        .read_to_string(&mut diagnostic)
        .unwrap();
    assert!(diagnostic.len() <= 16_384, "bounded child test diagnostics");
    assert!(status.success(), "SQLite probe failed: {diagnostic}");
    assert!(std::fs::metadata(&result).unwrap().len() <= 32);
    std::fs::read_to_string(result).unwrap()
}

// This one extra libtest helper is inert unless this module's owned parent
// supplies both per-child paths. It does not open a socket or expose product IPC.
#[test]
fn competing_sqlite_writer_probe() {
    let Some(path) = std::env::var_os(PROBE_DB) else {
        assert!(std::env::var_os(PROBE_RESULT).is_none());
        return;
    };
    let path = PathBuf::from(path);
    let result = PathBuf::from(std::env::var_os(PROBE_RESULT).unwrap());
    assert!(path.is_absolute());
    assert_eq!(path.canonicalize().unwrap(), path);
    assert_eq!(path.parent(), result.parent());
    let parent = std::fs::metadata(path.parent().unwrap()).unwrap();
    let file = std::fs::metadata(&path).unwrap();
    assert_eq!(parent.mode() & 0o7777, 0o700);
    assert_eq!(parent.uid(), rustix::process::geteuid().as_raw());
    assert!(file.is_file());
    assert_eq!(file.mode() & 0o7777, 0o600);
    assert_eq!(file.uid(), parent.uid());
    assert_eq!(file.nlink(), 1);
    let connection = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .unwrap();
    connection.busy_timeout(Duration::ZERO).unwrap();
    let observed = match connection.execute_batch("BEGIN IMMEDIATE;") {
        Ok(()) => {
            connection.execute_batch("ROLLBACK;").unwrap();
            "acquired\n"
        }
        Err(rusqlite::Error::SqliteFailure(code, _))
            if code.code == rusqlite::ErrorCode::DatabaseBusy =>
        {
            "busy\n"
        }
        Err(error) => panic!("unexpected SQLite probe error: {error:?}"),
    };
    let mut result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(result)
        .unwrap();
    result.write_all(observed.as_bytes()).unwrap();
    result.sync_all().unwrap();
}

#[test]
fn original_database_pin_clone_drop_keeps_real_writer_lock_until_commit() {
    // Store's real default WAL configuration is tested separately from DELETE.
    // The same pin type and Arc clone/drop used by corrected reseal are tested;
    // full policy/admission/reseal is covered by the separate actual IPC chain.
    for journal in ["wal", "delete"] {
        let directory = RetainedRoot::new();
        let root = directory.path();
        let path = root.join("baton.sqlite3");
        let store = Store::open(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        if journal == "delete" {
            let selected: String = store
                .conn
                .query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0))
                .unwrap();
            assert_eq!(selected, "delete");
        }
        let actual: String = store
            .conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(actual, journal);
        let original = DatabasePin::capture(&store.conn).unwrap();
        let transaction = rusqlite::Transaction::new_unchecked(
            &store.conn,
            rusqlite::TransactionBehavior::Immediate,
        )
        .unwrap();
        assert_eq!(competing_writer(&root, &path, 0), "busy\n");
        // Crucially, this is DatabasePin::clone, not File::try_clone/new open.
        let temporary_plan_owner = original.clone();
        temporary_plan_owner.recheck().unwrap();
        drop(temporary_plan_owner);
        original.recheck().unwrap();
        assert!(!store.conn.is_autocommit());
        assert_eq!(competing_writer(&root, &path, 1), "busy\n");
        transaction.commit().unwrap();
        assert_eq!(competing_writer(&root, &path, 2), "acquired\n");
        drop(original);
        drop(store);
    }
}

#[test]
fn separate_raw_database_close_control_releases_rollback_journal_posix_lock() {
    // A deliberately separate DELETE-journal control demonstrates the POSIX
    // same-inode close hazard. It is NOT a claim that Core's default WAL writer
    // lock resides on the main DB inode; WAL writer locks use the -shm file.
    let directory = RetainedRoot::new();
    let root = directory.path();
    let path = root.join("control.sqlite3");
    let connection = Connection::open(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let selected: String = connection
        .query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0))
        .unwrap();
    assert_eq!(selected, "delete");
    connection
        .execute_batch("CREATE TABLE inert(value INTEGER NOT NULL);")
        .unwrap();
    let original = DatabasePin::capture(&connection).unwrap();
    let transaction =
        rusqlite::Transaction::new_unchecked(&connection, rusqlite::TransactionBehavior::Immediate)
            .unwrap();
    assert_eq!(competing_writer(&root, &path, 0), "busy\n");
    let separately_opened = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .unwrap();
    assert_eq!(separately_opened.metadata().unwrap().ino(), original.inode);
    drop(separately_opened);
    assert!(!connection.is_autocommit());
    // Child only BEGIN/ROLLBACKs: it does not write a row or corrupt this parent.
    assert_eq!(competing_writer(&root, &path, 1), "acquired\n");
    transaction.commit().unwrap();
    assert_eq!(competing_writer(&root, &path, 2), "acquired\n");
    drop(original);
    drop(connection);
}
