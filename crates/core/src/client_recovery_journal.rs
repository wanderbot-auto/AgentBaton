//! Durable client intent only: never includes configuration bodies or authority.
use crate::CoreError;
use fs2::FileExt;
use rustix::fs::{fstat, openat, renameat, statat, AtFlags, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;
type Result<T> = std::result::Result<T, CoreError>;
const LIMIT: u64 = 16 * 1024;
const LOCK: &str = "client-recovery.lock";
const JOURNAL: &str = "client-recovery.json";
const MESSAGE_LOCK: &str = "task-message-recovery.lock";
const MESSAGE_JOURNAL: &str = "task-message-recovery.json";
pub(crate) const MESSAGE_LIMIT: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaskMessageIntent {
    pub session_id: String,
    pub request_key: String,
    pub expected_revision: u64,
    pub text_sha256: String,
}

impl TaskMessageIntent {
    pub(crate) fn validate(&self) -> Result<()> {
        if !valid_uuid(&self.session_id)
            || !valid_uuid(&self.request_key)
            || self.expected_revision == 0
            || self.expected_revision > 9_007_199_254_740_991
            || self.text_sha256.len() != 64
            || !self
                .text_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(err());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedCreationKind {
    ManagedMcp,
    ManagedSkill,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedCreationOperation {
    Replace,
    Remove,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagedCreationBinding {
    pub kind: ManagedCreationKind,
    pub request_key: String,
    pub preview_digest: String,
    pub base_connection_id: String,
    pub base_connection_revision: u64,
    pub operation: ManagedCreationOperation,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClientRecoveryMarker {
    pub entry_id: String,
    pub kind: ManagedCreationKind,
    pub request_key: String,
    pub preview_digest: String,
    pub base_connection_id: String,
    pub base_connection_revision: u64,
    pub operation: ManagedCreationOperation,
    pub created_at: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal<T> {
    version: u32,
    entries: T,
}
fn err() -> CoreError {
    CoreError::new(
        "CLIENT_RECOVERY_UNAVAILABLE",
        "Private recovery journal is unavailable or invalid; no new request may be dispatched",
    )
}
fn valid_uuid(s: &str) -> bool {
    Uuid::parse_str(s).is_ok_and(|u| u.to_string() == s)
}
pub fn validate_binding(b: &ManagedCreationBinding) -> Result<()> {
    if !valid_uuid(&b.request_key)
        || b.preview_digest.len() != 64
        || !b
            .preview_digest
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
        || b.base_connection_id.is_empty()
        || b.base_connection_id.len() > 128
        || !b
            .base_connection_id
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"._:-".contains(&v))
        || b.base_connection_revision == 0
        || b.base_connection_revision > 9_007_199_254_740_991
    {
        return Err(err());
    }
    Ok(())
}
/// Validates the closed DTO without reading any configuration or source.
pub fn validate_marker(m: &ClientRecoveryMarker) -> Result<()> {
    validate_binding(&ManagedCreationBinding {
        kind: m.kind,
        request_key: m.request_key.clone(),
        preview_digest: m.preview_digest.clone(),
        base_connection_id: m.base_connection_id.clone(),
        base_connection_revision: m.base_connection_revision,
        operation: m.operation,
    })?;
    if !valid_uuid(&m.entry_id) || !valid_timestamp(&m.created_at) {
        return Err(err());
    }
    Ok(())
}
fn valid_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return false;
    }
    let number = |a: usize, z: usize| {
        s.get(a..z).and_then(|v| {
            if v.bytes().all(|b| b.is_ascii_digit()) {
                v.parse::<u16>().ok()
            } else {
                None
            }
        })
    };
    let (Some(y), Some(m), Some(d), Some(h), Some(n), Some(sec)) = (
        number(0, 4),
        number(5, 7),
        number(8, 10),
        number(11, 13),
        number(14, 16),
        number(17, 19),
    ) else {
        return false;
    };
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    y > 0 && d > 0 && d <= days && h < 24 && n < 60 && sec < 60
}
struct JournalLock(File);
impl Drop for JournalLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}
pub struct ClientRecoveryJournal {
    directory: File,
    root: PathBuf,
}
impl ClientRecoveryJournal {
    fn open_directory(root: &Path) -> Result<File> {
        if !root.is_absolute() {
            return Err(err());
        }
        let mut directory: File = rustix::fs::open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| err())?
        .into();
        for part in root.components() {
            match part {
                Component::RootDir => {}
                Component::Normal(p) => {
                    directory = openat(
                        &directory,
                        p,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| err())?
                    .into();
                }
                _ => return Err(err()),
            }
        }
        Ok(directory)
    }
    pub fn new(root: &Path) -> Result<Self> {
        let directory = Self::open_directory(root)?;
        let this = Self {
            directory,
            root: root.to_owned(),
        };
        this.check_directory()?;
        Ok(this)
    }
    fn check_directory(&self) -> Result<()> {
        let m = self.directory.metadata().map_err(|_| err())?;
        let named = Self::open_directory(&self.root)?
            .metadata()
            .map_err(|_| err())?;
        if named.dev() != m.dev() || named.ino() != m.ino() {
            return Err(err());
        }
        if !m.is_dir()
            || m.uid() != rustix::process::geteuid().as_raw()
            || m.mode() & 0o7777 != 0o700
        {
            return Err(err());
        }
        Ok(())
    }
    fn check_file(f: &File) -> Result<()> {
        let m = f.metadata().map_err(|_| err())?;
        if !m.is_file()
            || m.nlink() != 1
            || m.uid() != rustix::process::geteuid().as_raw()
            || m.mode() & 0o7777 != 0o600
        {
            return Err(err());
        }
        Ok(())
    }
    fn lock(&self) -> Result<JournalLock> {
        self.lock_file(LOCK)
    }

    fn lock_file(&self, name: &str) -> Result<JournalLock> {
        self.check_directory()?;
        let flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let open_existing = || openat(&self.directory, name, flags, Mode::empty());
        // Keep creation exclusive. On macOS two concurrent nonexclusive
        // O_CREAT|O_NOFOLLOW opens can report ENOENT for the losing opener.
        let lock: File = match open_existing() {
            Err(rustix::io::Errno::NOENT) => match openat(
                &self.directory,
                name,
                flags | OFlags::CREATE | OFlags::EXCL,
                Mode::from_raw_mode(0o600),
            ) {
                Err(rustix::io::Errno::EXIST) => open_existing(),
                result => result,
            },
            result => result,
        }
        .map_err(|_| err())?
        .into();
        Self::check_file(&lock)?;
        lock.try_lock_exclusive().map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                CoreError::new(
                    "CLIENT_RECOVERY_BUSY",
                    "Private recovery journal is busy; no request was dispatched",
                )
            } else {
                err()
            }
        })?;
        let st = statat(&self.directory, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|_| err())?;
        let fd_stat = fstat(&lock).map_err(|_| err())?;
        if st.st_ino != fd_stat.st_ino || st.st_dev != fd_stat.st_dev {
            return Err(err());
        }
        Self::check_file(&lock)?;
        Ok(JournalLock(lock))
    }
    fn read_entries<T: serde::de::DeserializeOwned>(&self, name: &str) -> Result<Vec<T>> {
        let f = match openat(
            &self.directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(f) => File::from(f),
            Err(rustix::io::Errno::NOENT) => return Ok(vec![]),
            Err(_) => return Err(err()),
        };
        Self::check_file(&f)?;
        if f.metadata().map_err(|_| err())?.len() > LIMIT {
            return Err(err());
        }
        let mut bytes = Vec::new();
        f.take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| err())?;
        if bytes.len() as u64 > LIMIT {
            return Err(err());
        }
        let journal: Journal<Vec<T>> = serde_json::from_slice(&bytes).map_err(|_| err())?;
        if journal.version != 1 {
            return Err(err());
        }
        Ok(journal.entries)
    }
    fn read(&self) -> Result<Vec<ClientRecoveryMarker>> {
        let entries: Vec<ClientRecoveryMarker> = self.read_entries(JOURNAL)?;
        if entries.len() > 2 {
            return Err(err());
        }
        for (i, m) in entries.iter().enumerate() {
            validate_marker(m)?;
            if entries[..i]
                .iter()
                .any(|p| p.kind == m.kind || p.entry_id == m.entry_id)
            {
                return Err(err());
            }
        }
        Ok(entries)
    }
    fn write_entries<T: Serialize>(&self, destination: &str, entries: &[T]) -> Result<()> {
        let bytes = serde_json::to_vec(&Journal {
            version: 1,
            entries,
        })
        .map_err(|_| err())?;
        if bytes.len() as u64 > LIMIT {
            return Err(err());
        }
        let name = format!(".client-recovery-{}.tmp", Uuid::new_v4());
        let mut f: File = openat(
            &self.directory,
            name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| err())?
        .into();
        let result = (|| {
            Self::check_file(&f)?;
            f.write_all(&bytes).map_err(|_| err())?;
            f.sync_all().map_err(|_| err())?;
            renameat(&self.directory, name.as_str(), &self.directory, destination)
                .map_err(|_| err())?;
            self.directory.sync_all().map_err(|_| {
                let mut e = err();
                e.side_effect_state = "unknown".into();
                e
            })?;
            Ok(())
        })();
        if result.is_err() {
            let _ = rustix::fs::unlinkat(&self.directory, name.as_str(), AtFlags::empty());
        }
        result
    }
    pub fn reserve(&self, b: ManagedCreationBinding) -> Result<ClientRecoveryMarker> {
        validate_binding(&b)?;
        let _lock = self.lock()?;
        let mut entries = self.read()?;
        if entries.iter().any(|m| m.kind == b.kind) {
            return Err(CoreError::new(
                "CLIENT_RECOVERY_REQUIRED",
                "An existing creation request must be reconciled before another request",
            ));
        }
        let marker = ClientRecoveryMarker {
            entry_id: Uuid::new_v4().to_string(),
            kind: b.kind,
            request_key: b.request_key,
            preview_digest: b.preview_digest,
            base_connection_id: b.base_connection_id,
            base_connection_revision: b.base_connection_revision,
            operation: b.operation,
            created_at: OffsetDateTime::now_utc()
                .replace_nanosecond(0)
                .map_err(|_| err())?
                .format(&Rfc3339)
                .map_err(|_| err())?,
        };
        validate_marker(&marker)?;
        entries.push(marker.clone());
        self.write_entries(JOURNAL, &entries)?;
        Ok(marker)
    }
    pub fn list(&self) -> Result<Vec<ClientRecoveryMarker>> {
        let _lock = self.lock()?;
        self.read()
    }
    pub fn clear(&self, marker: &ClientRecoveryMarker) -> Result<bool> {
        validate_marker(marker)?;
        let _lock = self.lock()?;
        let mut entries = self.read()?;
        let Some(index) = entries.iter().position(|m| m == marker) else {
            return Ok(false);
        };
        entries.remove(index);
        self.write_entries(JOURNAL, &entries)?;
        Ok(true)
    }

    pub(crate) fn task_messages(&self) -> Result<TaskMessageJournal<'_>> {
        let lock = self.lock_file(MESSAGE_LOCK)?;
        let entries: Vec<TaskMessageIntent> = self.read_entries(MESSAGE_JOURNAL)?;
        if entries.len() > MESSAGE_LIMIT {
            return Err(err());
        }
        for (i, entry) in entries.iter().enumerate() {
            entry.validate()?;
            if entries[..i]
                .iter()
                .any(|previous| previous.session_id == entry.session_id)
            {
                return Err(err());
            }
        }
        Ok(TaskMessageJournal {
            journal: self,
            _lock: lock,
            entries,
        })
    }
}

pub(crate) fn task_message_recovery_required() -> CoreError {
    let mut error = CoreError::new(
        "TASK_MESSAGE_RECOVERY_REQUIRED",
        "Check the previous message receipt before sending again",
    );
    error.side_effect_state = "unknown".into();
    error
}

pub(crate) struct TaskMessageJournal<'a> {
    journal: &'a ClientRecoveryJournal,
    _lock: JournalLock,
    entries: Vec<TaskMessageIntent>,
}

impl TaskMessageJournal<'_> {
    pub(crate) fn entries(&self) -> &[TaskMessageIntent] {
        &self.entries
    }

    pub(crate) fn reserve(&mut self, intent: TaskMessageIntent) -> Result<()> {
        intent.validate()?;
        if self
            .entries
            .iter()
            .any(|previous| previous.session_id == intent.session_id)
        {
            return Err(task_message_recovery_required());
        }
        if self.entries.len() == MESSAGE_LIMIT {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Too many unresolved task messages",
            ));
        }
        self.entries.push(intent);
        self.journal.write_entries(MESSAGE_JOURNAL, &self.entries)
    }

    pub(crate) fn clear(&mut self, session_id: &str) -> Result<()> {
        self.entries.retain(|entry| entry.session_id != session_id);
        self.journal.write_entries(MESSAGE_JOURNAL, &self.entries)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{symlink, PermissionsExt},
    };
    fn fixture() -> (tempfile::TempDir, ClientRecoveryJournal) {
        let d = tempfile::tempdir().unwrap();
        fs::set_permissions(d.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let j = ClientRecoveryJournal::new(d.path()).unwrap();
        (d, j)
    }
    fn binding() -> ManagedCreationBinding {
        ManagedCreationBinding {
            kind: ManagedCreationKind::ManagedMcp,
            request_key: Uuid::new_v4().to_string(),
            preview_digest: "a".repeat(64),
            base_connection_id: Uuid::new_v4().to_string(),
            base_connection_revision: 1,
            operation: ManagedCreationOperation::Replace,
        }
    }
    #[test]
    fn cross_handle_persistence_and_generation_cas() {
        let (d, j) = fixture();
        let b = binding();
        let first = j.reserve(b.clone()).unwrap();
        let second = ClientRecoveryJournal::new(d.path()).unwrap();
        assert_eq!(second.list().unwrap(), vec![first.clone()]);
        assert_eq!(
            second.reserve(b.clone()).unwrap_err().code,
            "CLIENT_RECOVERY_REQUIRED"
        );
        assert!(second.reserve(binding()).is_err());
        let mut wrong = first.clone();
        wrong.preview_digest = "b".repeat(64);
        assert!(!second.clear(&wrong).unwrap());
        assert!(second.clear(&first).unwrap());
        let fresh = second.reserve(b).unwrap();
        assert_ne!(fresh.entry_id, first.entry_id);
        assert!(!j.clear(&first).unwrap());
        assert_eq!(j.list().unwrap(), vec![fresh]);
    }
    #[test]
    fn kinds_bounded_and_fields_closed() {
        let (d, j) = fixture();
        j.reserve(binding()).unwrap();
        let mut b = binding();
        b.kind = ManagedCreationKind::ManagedSkill;
        j.reserve(b).unwrap();
        assert_eq!(j.list().unwrap().len(), 2);
        let path = d.path().join(JOURNAL);
        let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        v["entries"][0]["body"] = serde_json::json!("forbidden");
        fs::write(path, serde_json::to_vec(&v).unwrap()).unwrap();
        assert!(j.list().is_err());
        assert!(j.reserve(binding()).is_err());
    }
    #[test]
    fn corrupted_oversized_and_unsafe_files_never_overwritten() {
        for bytes in [b"invalid".to_vec(), vec![b' '; LIMIT as usize + 1]] {
            let (d, j) = fixture();
            let p = d.path().join(JOURNAL);
            fs::write(&p, &bytes).unwrap();
            fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
            assert!(j.reserve(binding()).is_err());
            assert_eq!(fs::read(p).unwrap(), bytes);
        }
        let (d, j) = fixture();
        j.reserve(binding()).unwrap();
        let p = d.path().join(JOURNAL);
        fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(j.list().is_err());
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&p, d.path().join("hard")).unwrap();
        assert!(j.list().is_err());
    }
    #[test]
    fn links_and_write_failure_fail_closed() {
        for name in [LOCK, JOURNAL] {
            let (d, j) = fixture();
            let target = d.path().join("target");
            fs::write(&target, b"untouched").unwrap();
            symlink(&target, d.path().join(name)).unwrap();
            assert!(j.reserve(binding()).is_err());
            assert_eq!(fs::read(target).unwrap(), b"untouched");
        }
        let (d, j) = fixture();
        fs::create_dir(d.path().join(JOURNAL)).unwrap();
        assert!(j.reserve(binding()).is_err());
        assert!(d.path().join(JOURNAL).is_dir());
        fs::set_permissions(d.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(j.list().is_err());
        assert!(ClientRecoveryJournal::new(d.path()).is_err());
    }
    #[test]
    fn invalid_binding_and_timestamp_are_rejected() {
        let (_d, j) = fixture();
        for id in [
            "/Users/private/config",
            "base connection",
            "base\tconnection",
            "路径",
            "",
        ] {
            let mut b = binding();
            b.base_connection_id = id.into();
            assert!(j.reserve(b).is_err());
        }
        let mut allowed = binding();
        allowed.base_connection_id = "base-connection.v1:test_id".into();
        validate_binding(&allowed).unwrap();
        let mut b = binding();
        b.request_key = "not-uuid".into();
        assert!(j.reserve(b).is_err());
        let mut b = binding();
        b.base_connection_revision = 9_007_199_254_740_992;
        assert!(j.reserve(b).is_err());
        let mut m = j.reserve(binding()).unwrap();
        m.created_at = "2025-02-29T00:00:00Z".into();
        assert!(validate_marker(&m).is_err());
        assert!(valid_timestamp("2024-02-29T00:00:00Z"));
    }
    #[test]
    fn subprocess_reserve_fixture() {
        let Some(root) = std::env::var_os("BATON_JOURNAL_TEST_ROOT") else {
            return;
        };
        let journal = ClientRecoveryJournal::new(Path::new(&root)).unwrap();
        let code = match journal.reserve(binding()) {
            Ok(_) => 10,
            Err(e) if e.code == "CLIENT_RECOVERY_REQUIRED" => 11,
            Err(e) if e.code == "CLIENT_RECOVERY_BUSY" => 13,
            Err(error) => {
                eprintln!("unexpected journal reserve error: {error:?}");
                12
            }
        };
        std::process::exit(code);
    }
    #[test]
    fn processes_serialize_and_marker_survives_process_exit() {
        let (d, journal) = fixture();
        let spawn = || {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "client_recovery_journal::tests::subprocess_reserve_fixture",
                    "--nocapture",
                ])
                .env("BATON_JOURNAL_TEST_ROOT", d.path())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        };
        let a = spawn();
        let b = spawn();
        let a = a.wait_with_output().unwrap();
        let b = b.wait_with_output().unwrap();
        let mut codes = [a.status.code().unwrap(), b.status.code().unwrap()];
        codes.sort();
        assert!(
            codes == [10, 11] || codes == [10, 13],
            "{codes:?}: {} {}",
            String::from_utf8_lossy(&a.stderr),
            String::from_utf8_lossy(&b.stderr)
        );
        assert_eq!(
            journal.reserve(binding()).unwrap_err().code,
            "CLIENT_RECOVERY_REQUIRED"
        );
        let stored = journal.list().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(
            ClientRecoveryJournal::new(d.path())
                .unwrap()
                .list()
                .unwrap(),
            stored
        );
    }
    #[test]
    fn subprocess_hold_lock_fixture() {
        let Some(root) = std::env::var_os("BATON_JOURNAL_HOLD_ROOT") else {
            return;
        };
        let journal = ClientRecoveryJournal::new(Path::new(&root)).unwrap();
        let _lock = journal.lock().unwrap();
        std::io::stdout().write_all(b"LOCKED\n").unwrap();
        std::io::stdout().flush().unwrap();
        let mut release = [0u8];
        std::io::stdin().read_exact(&mut release).unwrap();
    }
    #[test]
    fn held_process_lock_refuses_promptly_without_changing_journal() {
        use std::io::BufRead;
        let (d, journal) = fixture();
        let marker = journal.reserve(binding()).unwrap();
        let bytes = fs::read(d.path().join(JOURNAL)).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "client_recovery_journal::tests::subprocess_hold_lock_fixture",
                "--nocapture",
            ])
            .env("BATON_JOURNAL_HOLD_ROOT", d.path())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
        loop {
            let mut line = String::new();
            assert!(output.read_line(&mut line).unwrap() > 0);
            if line.contains("LOCKED") {
                break;
            }
        }
        let other = ClientRecoveryJournal::new(d.path()).unwrap();
        let started = std::time::Instant::now();
        let error = other.reserve(binding()).unwrap_err();
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert_eq!(error.code, "CLIENT_RECOVERY_BUSY");
        assert_eq!(error.side_effect_state, "not_started");
        assert_eq!(other.list().unwrap_err().code, "CLIENT_RECOVERY_BUSY");
        assert_eq!(
            other.clear(&marker).unwrap_err().code,
            "CLIENT_RECOVERY_BUSY"
        );
        assert_eq!(fs::read(d.path().join(JOURNAL)).unwrap(), bytes);
        child.stdin.take().unwrap().write_all(b"x").unwrap();
        assert!(child.wait().unwrap().success());
        assert_eq!(other.list().unwrap(), vec![marker]);
        assert_eq!(
            other.reserve(binding()).unwrap_err().code,
            "CLIENT_RECOVERY_REQUIRED"
        );
    }
}
