use crate::protocol::*;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Store {
    pub(crate) db: Connection,
    pub config: Config,
    pub(crate) root: File,
}

pub fn private_dir(path: &Path) -> Result<()> {
    require(
        path.is_absolute() && std::fs::canonicalize(path)? == path,
        "directory",
    )?;
    let m = std::fs::symlink_metadata(path)?;
    require(
        m.is_dir() && m.uid() == unsafe { libc::geteuid() } && m.permissions().mode() & 0o077 == 0,
        "directory_permissions",
    )
}
pub fn private_file(path: &Path, max: u64) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    require(
        m.is_file()
            && !m.file_type().is_symlink()
            && m.uid() == unsafe { libc::geteuid() }
            && m.permissions().mode() & 0o077 == 0
            && m.nlink() == 1
            && m.len() <= max,
        "file_permissions",
    )
}
pub fn load_config(path: &Path) -> Result<Config> {
    require(
        path.is_absolute() && std::fs::canonicalize(path)? == path,
        "configuration_path",
    )?;
    private_dir(
        path.parent()
            .ok_or_else(|| Error::new("configuration_path"))?,
    )?;
    private_file(path, MAX_FRAME as u64)?;
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let m = f.metadata()?;
    require(
        m.is_file()
            && m.nlink() == 1
            && m.uid() == unsafe { libc::geteuid() }
            && m.permissions().mode() & 0o077 == 0
            && m.len() <= MAX_FRAME as u64,
        "configuration_permissions",
    )?;
    let mut bytes = Vec::new();
    f.by_ref()
        .take((MAX_FRAME + 1) as u64)
        .read_to_end(&mut bytes)?;
    require(bytes.len() <= MAX_FRAME, "oversize")?;
    let config: Config = serde_json::from_slice(&bytes)?;
    config.validate()?;
    Ok(config)
}
impl Store {
    pub fn open(config: Config, activate: bool) -> Result<Self> {
        config.validate()?;
        private_dir(&config.state_dir)?;
        private_dir(&config.root)?;
        require(
            !config.state_dir.starts_with(&config.root)
                && !config.root.starts_with(&config.state_dir),
            "mapping",
        )?;
        let root = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(&config.root)?;
        let m = root.metadata()?;
        let by_path = std::fs::metadata(&config.root)?;
        require(
            m.is_dir()
                && m.uid() == unsafe { libc::geteuid() }
                && m.mode() & 0o077 == 0
                && m.ino() == by_path.ino()
                && m.dev() == by_path.dev(),
            "directory_drift",
        )?;
        let path = config.state_dir.join("journal.sqlite3");
        for name in [
            "journal.sqlite3",
            "journal.sqlite3-wal",
            "journal.sqlite3-shm",
            "journal.sqlite3-journal",
        ] {
            let p = config.state_dir.join(name);
            if p.symlink_metadata().is_ok() {
                require(
                    !p.symlink_metadata()?.file_type().is_symlink(),
                    "journal_path",
                )?;
                private_file(&p, 33554432)?;
            }
        }
        if !path.exists() {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&path)?;
        }
        let db = Connection::open(&path)?;
        db.busy_timeout(Duration::from_millis(500))?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        require(version <= VERSION, "journal_version")?;
        db.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA max_page_count=8192;",
        )?;
        db.execute_batch("BEGIN IMMEDIATE;
   CREATE TABLE IF NOT EXISTS meta (id INTEGER PRIMARY KEY CHECK(id=1), identity TEXT NOT NULL, grant_hash TEXT NOT NULL, fence INTEGER NOT NULL, highwater INTEGER NOT NULL, paused INTEGER NOT NULL, revoked INTEGER NOT NULL, cancelled INTEGER NOT NULL, used_ops INTEGER NOT NULL, used_bytes INTEGER NOT NULL, ceiling TEXT NOT NULL);
   CREATE TABLE IF NOT EXISTS events (sequence INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, job TEXT);
   CREATE TABLE IF NOT EXISTS jobs (id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, content TEXT NOT NULL, digest TEXT NOT NULL, delay_ms INTEGER NOT NULL, state TEXT NOT NULL, stop_evidence TEXT, dispatch_fence INTEGER NOT NULL, grant_hash TEXT NOT NULL);
   CREATE TABLE IF NOT EXISTS receipts (id TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, reply TEXT NOT NULL);
   PRAGMA user_version=1; COMMIT;")?;
        let mut s = Self { db, config, root };
        s.initialize(activate)?;
        Ok(s)
    }
    fn initialize(&mut self, activate: bool) -> Result<()> {
        let cfg = &self.config;
        let g = &cfg.grant;
        let identity = digest(&serde_json::to_vec(&(
            &cfg.host_identity,
            &cfg.root,
            &g.binding,
            self.root.metadata()?.dev(),
            self.root.metadata()?.ino(),
        ))?);
        let grant_hash = digest(&serde_json::to_vec(g)?);
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = now_ms()?;
        let previous: Option<(String, String, i64, i64, String)> = tx
            .query_row(
                "SELECT identity,grant_hash,fence,highwater,ceiling FROM meta WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        if let Some((old_identity, old_hash, fence, highwater, ceiling)) = previous {
            require(identity == old_identity, "identity_drift")?;
            require(
                g.fence >= fence && (g.fence > fence || old_hash == grant_hash),
                "stale_grant",
            )?;
            if g.fence > fence {
                let ceiling: Grant = serde_json::from_str(&ceiling)?;
                require(
                    g.max_operations <= ceiling.max_operations
                        && g.max_bytes <= ceiling.max_bytes
                        && (!g.allow_record || ceiling.allow_record),
                    "grant_expansion",
                )?;
                require(
                    activate && now >= highwater && now >= g.issued_ms && now < g.expires_ms,
                    "lease",
                )?;
                tx.execute("UPDATE jobs SET state='outcome_unknown' WHERE stop_evidence IS NULL AND state IN ('reserved','running','effect_intent')",[])?;
                // A fresh local owner provision may narrow/renew the same binding. Usage never resets.
                tx.execute(
                    "UPDATE meta SET grant_hash=?1,fence=?2,paused=0,ceiling=?3 WHERE id=1",
                    params![grant_hash, g.fence, serde_json::to_string(g)?],
                )?;
                tx.execute(
                    "INSERT INTO events(kind) VALUES('local_grant_installed')",
                    [],
                )?;
            } else if activate {
                // Restart cannot establish continuity of a monotonic deadline: pause, reconcile, require fresh local grant.
                tx.execute("UPDATE meta SET paused=1 WHERE id=1", [])?;
                tx.execute("UPDATE jobs SET state='outcome_unknown' WHERE stop_evidence IS NULL AND state IN ('reserved','running','effect_intent')",[])?;
                tx.execute("INSERT INTO events(kind) VALUES('reopened_paused')", [])?;
            }
        } else {
            require(
                activate && now >= g.issued_ms && now < g.expires_ms,
                "lease",
            )?;
            tx.execute(
                "INSERT INTO meta VALUES(1,?1,?2,?3,?4,0,0,0,0,0,?5)",
                params![
                    identity,
                    grant_hash,
                    g.fence,
                    now,
                    serde_json::to_string(g)?
                ],
            )?;
            tx.execute(
                "INSERT INTO events(kind) VALUES('local_grant_installed')",
                [],
            )?;
        }
        event_limit(&tx, 3 * MAX_JOBS)?;
        tx.commit()?;
        Ok(())
    }
    pub fn handle(&mut self, request: &Request) -> Result<Reply> {
        let g = &self.config.grant;
        require(request.version == VERSION, "version")?;
        require(identifier(&request.request_id), "request_id")?;
        require(
            request.host_identity == self.config.host_identity && request.binding == g.binding,
            "identity_binding",
        )?;
        require(
            request.controller == g.controller
                && request.credential.len() <= 128
                && digest(request.credential.as_bytes()) == g.credential_hash,
            "unauthenticated",
        )?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = now_ms()?; // Sample after acquiring the writer lock.
        let (fence,highwater,paused,revoked,cancelled,used_ops,used_bytes): (i64,i64,bool,bool,bool,i64,i64) = tx.query_row("SELECT fence,highwater,paused,revoked,cancelled,used_ops,used_bytes FROM meta WHERE id=1",[],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?;
        require(
            request.fence == fence && g.fence == fence,
            "stale_controller",
        )?;
        if now < highwater {
            tx.execute("UPDATE meta SET paused=1 WHERE id=1", [])?;
            if matches!(request.action, Action::Record { .. }) {
                tx.commit()?;
                return Err(Error::new("clock_rollback"));
            }
        }
        tx.execute(
            "UPDATE meta SET highwater=?1 WHERE id=1",
            [now.max(highwater)],
        )?;
        if let Action::Reconcile { cursor } = request.action {
            let snapshot = snapshot(&tx, g, cursor)?;
            tx.commit()?;
            return Ok(Reply::Snapshot {
                snapshot: Box::new(snapshot),
            });
        }
        let fingerprint = digest(&serde_json::to_vec(&(
            &request.binding,
            &request.controller,
            request.fence,
            &request.action,
        ))?);
        let receipt: Option<(String, String)> = tx
            .query_row(
                "SELECT fingerprint,reply FROM receipts WHERE id=?1",
                [&request.request_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((previous, reply)) = receipt {
            require(previous == fingerprint, "idempotency_conflict")?;
            let reply = serde_json::from_str(&reply)?;
            tx.commit()?;
            return Ok(reply);
        }
        let count: i64 = tx.query_row("SELECT count(*) FROM receipts", [], |r| r.get(0))?;
        require(count < MAX_RECEIPTS as i64, "receipt_capacity")?;
        let reply = match &request.action {
            Action::Record {
                name,
                content,
                delay_ms,
            } => {
                require(!paused && !revoked && !cancelled, "paused")?;
                require(now >= g.issued_ms && now < g.expires_ms, "expired")?;
                require(g.allow_record, "permission")?;
                require(
                    identifier(name)
                        && *delay_ms >= 0
                        && *delay_ms <= 5000
                        && content.len() <= MAX_CONTENT,
                    "operation_bounds",
                )?;
                require(
                    used_ops < g.max_operations && used_bytes + content.len() as i64 <= g.max_bytes,
                    "budget_exhausted",
                )?;
                let jobs: i64 = tx.query_row("SELECT count(*) FROM jobs", [], |r| r.get(0))?;
                require(jobs < MAX_JOBS as i64, "job_capacity")?;
                let exists: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM jobs WHERE name=?1)",
                    [name],
                    |r| r.get(0),
                )?;
                require(!exists, "destination_conflict")?;
                tx.execute(
                    "INSERT INTO jobs VALUES(?1,?2,?3,?4,?5,'reserved',NULL,?6,?7)",
                    params![
                        request.request_id,
                        name,
                        content,
                        digest(content.as_bytes()),
                        delay_ms,
                        g.fence,
                        digest(&serde_json::to_vec(g)?)
                    ],
                )?;
                tx.execute(
                    "UPDATE meta SET used_ops=used_ops+1,used_bytes=used_bytes+?1 WHERE id=1",
                    [content.len() as i64],
                )?;
                tx.execute(
                    "INSERT INTO events(kind,job) VALUES('dispatch_reserved',?1)",
                    [&request.request_id],
                )?;
                Reply::Accepted {
                    job: request.request_id.clone(),
                }
            }
            Action::Cancel => {
                tx.execute("UPDATE meta SET cancelled=1 WHERE id=1", [])?;
                tx.execute("INSERT INTO events(kind) VALUES('cancel_requested')", [])?;
                Reply::CancelRequested
            }
            Action::Revoke => {
                tx.execute("UPDATE meta SET revoked=1 WHERE id=1", [])?;
                tx.execute("INSERT INTO events(kind) VALUES('revoked')", [])?;
                Reply::Revoked
            }
            Action::Reconcile { .. } => unreachable!(),
        };
        tx.execute(
            "INSERT INTO receipts VALUES(?1,?2,?3)",
            params![
                request.request_id,
                fingerprint,
                serde_json::to_string(&reply)?
            ],
        )?;
        event_limit(&tx, 3 * MAX_JOBS)?;
        tx.commit()?;
        Ok(reply)
    }
    pub fn needs_spawn(&self, job: &str) -> Result<bool> {
        Ok(self.db.query_row(
            "SELECT state='reserved' AND stop_evidence IS NULL FROM jobs WHERE id=?1",
            [job],
            |r| r.get(0),
        )?)
    }
    pub fn spawn_failed(&mut self, job: &str) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE jobs SET state='spawn_failed_no_effect',stop_evidence='spawn_failed_no_child' WHERE id=?1",[job])?;
        tx.execute(
            "INSERT INTO events(kind,job) VALUES('spawn_failed_no_child',?1)",
            [job],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn child_exited(&mut self, job: &str, success: bool) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE jobs SET stop_evidence=?1,state=CASE WHEN state IN ('reserved','running','effect_intent') THEN 'outcome_unknown' ELSE state END WHERE id=?2",params![if success { "owned_child_wait_exit_0" } else { "owned_child_wait_nonzero" },job])?;
        tx.execute(
            "INSERT INTO events(kind,job) VALUES('child_wait_confirmed',?1)",
            [job],
        )?;
        event_limit(&tx, 0)?;
        tx.commit()?;
        Ok(())
    }
    pub fn db_pause(&self) -> Result<()> {
        self.db.execute(
            "UPDATE meta SET paused=1 WHERE id=1 AND fence=?1 AND grant_hash=?2",
            params![
                self.config.grant.fence,
                digest(&serde_json::to_vec(&self.config.grant)?)
            ],
        )?;
        Ok(())
    }
    pub fn path(&self) -> PathBuf {
        self.config.state_dir.join("journal.sqlite3")
    }
}
fn event_limit(db: &Connection, reserved: usize) -> Result<()> {
    let n: i64 = db.query_row("SELECT count(*) FROM events", [], |r| r.get(0))?;
    require(n <= 10000 - reserved as i64, "journal_capacity")
}
fn snapshot(db: &Connection, grant: &Grant, cursor: i64) -> Result<Snapshot> {
    let last: i64 = db.query_row("SELECT coalesce(max(sequence),0) FROM events", [], |r| {
        r.get(0)
    })?;
    require(cursor >= 0 && cursor <= last, "cursor_ahead")?;
    let events = db
        .prepare(
            "SELECT sequence,kind,job FROM events WHERE sequence>?1 ORDER BY sequence LIMIT ?2",
        )?
        .query_map(params![cursor, MAX_EVENTS as i64], |r| {
            Ok(Event {
                sequence: r.get(0)?,
                kind: r.get(1)?,
                job: r.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let next = events.last().map_or(cursor, |e| e.sequence);
    let jobs = db
        .prepare("SELECT id,name,digest,state,stop_evidence FROM jobs ORDER BY rowid LIMIT 32")?
        .query_map([], |r| {
            Ok(Job {
                id: r.get(0)?,
                name: r.get(1)?,
                digest: r.get(2)?,
                state: r.get(3)?,
                stop_evidence: r.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let (fence, paused, revoked, cancelled, used_operations, used_bytes): (
        i64,
        bool,
        bool,
        bool,
        i64,
        i64,
    ) = db.query_row(
        "SELECT fence,paused,revoked,cancelled,used_ops,used_bytes FROM meta WHERE id=1",
        [],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        },
    )?;
    let confirmed_stopped = cancelled && jobs.iter().all(|j| j.stop_evidence.is_some());
    Ok(Snapshot {
        binding: grant.binding.clone(),
        fence,
        events,
        cursor: next,
        more: next < last,
        jobs,
        cancel_requested: cancelled,
        confirmed_stopped,
        paused,
        revoked,
        used_operations,
        used_bytes,
        history_owner: "controller_local_primary".into(),
        cache_policy: "remote_pinned_until_explicit_project_retention_integration".into(),
    })
}
