//! Task-local control-side reconciliation journal. Never stores transport credentials.
use crate::{
    protocol::*,
    store::{private_dir, private_file},
};
use rusqlite::{params, Connection, TransactionBehavior};
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;

pub struct Ledger {
    db: Connection,
    binding: Binding,
}
impl Ledger {
    pub fn open(dir: &Path, binding: Binding) -> Result<Self> {
        private_dir(dir)?;
        let path = dir.join("controller.sqlite3");
        if !path.exists() {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&path)?;
        }
        for suffix in ["", "-journal", "-wal", "-shm"] {
            let p = dir.join(format!("controller.sqlite3{suffix}"));
            if let Ok(m) = p.symlink_metadata() {
                require(!m.file_type().is_symlink(), "journal_path")?;
                private_file(&p, 33554432)?;
            }
        }
        let db = Connection::open(path)?;
        db.busy_timeout(Duration::from_millis(500))?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        require(version <= VERSION, "journal_version")?;
        db.execute_batch("PRAGMA synchronous=FULL; PRAGMA max_page_count=8192;")?;
        db.execute_batch("BEGIN IMMEDIATE; CREATE TABLE IF NOT EXISTS control (id INTEGER PRIMARY KEY CHECK(id=1), binding TEXT NOT NULL,cursor INTEGER NOT NULL,cancel_id TEXT,confirmed INTEGER NOT NULL,fence INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS received(sequence INTEGER PRIMARY KEY,payload TEXT NOT NULL); PRAGMA user_version=1; COMMIT;")?;
        let encoded = serde_json::to_string(&binding)?;
        db.execute(
            "INSERT OR IGNORE INTO control VALUES(1,?1,0,NULL,0,0)",
            [&encoded],
        )?;
        let old: String =
            db.query_row("SELECT binding FROM control WHERE id=1", [], |r| r.get(0))?;
        require(old == encoded, "identity_drift")?;
        Ok(Self { db, binding })
    }
    pub fn request_cancel(&self, id: &str) -> Result<()> {
        require(identifier(id), "request_id")?;
        self.db.execute(
            "UPDATE control SET cancel_id=coalesce(cancel_id,?1) WHERE id=1",
            [id],
        )?;
        Ok(())
    }
    pub fn pending_cancel(&self, template: &Request) -> Result<Option<Request>> {
        require(template.binding == self.binding, "identity_binding")?;
        let (id, confirmed): (Option<String>, bool) = self.db.query_row(
            "SELECT cancel_id,confirmed FROM control WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(if confirmed {
            None
        } else {
            id.map(|id| {
                let mut r = template.clone();
                r.request_id = digest(format!("{id}:{}", r.fence).as_bytes());
                r.action = Action::Cancel;
                r
            })
        })
    }
    pub fn cursor(&self) -> Result<i64> {
        Ok(self
            .db
            .query_row("SELECT cursor FROM control WHERE id=1", [], |r| r.get(0))?)
    }
    pub fn confirmed_stopped(&self) -> Result<bool> {
        Ok(self
            .db
            .query_row("SELECT confirmed FROM control WHERE id=1", [], |r| r.get(0))?)
    }
    pub fn ingest(&mut self, snapshot: &Snapshot) -> Result<()> {
        require(
            snapshot.binding == self.binding
                && snapshot.events.len() <= MAX_EVENTS
                && snapshot.jobs.len() <= MAX_JOBS,
            "snapshot_binding",
        )?;
        require(
            !snapshot.confirmed_stopped
                || (snapshot.cancel_requested
                    && snapshot.jobs.iter().all(|j| {
                        j.stop_evidence.as_deref().is_some_and(|s| {
                            matches!(
                                s,
                                "owned_child_wait_exit_0"
                                    | "owned_child_wait_nonzero"
                                    | "spawn_failed_no_child"
                            )
                        })
                    })),
            "stop_evidence",
        )?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (mut cursor, fence): (i64, i64) =
            tx.query_row("SELECT cursor,fence FROM control WHERE id=1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        require(snapshot.fence >= fence, "stale_event")?;
        let old_cursor = cursor;
        let mut previous = 0;
        for event in &snapshot.events {
            require(
                event.sequence > previous
                    && event.sequence > 0
                    && event.sequence <= 10000
                    && event.kind.len() <= 64
                    && event.job.as_deref().is_none_or(identifier),
                "event_order",
            )?;
            previous = event.sequence;
            let payload = serde_json::to_string(event)?;
            if event.sequence <= cursor {
                let old: String = tx.query_row(
                    "SELECT payload FROM received WHERE sequence=?1",
                    [event.sequence],
                    |r| r.get(0),
                )?;
                require(old == payload, "event_conflict")?;
            } else {
                require(event.sequence == cursor + 1, "event_gap")?;
                tx.execute(
                    "INSERT INTO received VALUES(?1,?2)",
                    params![event.sequence, payload],
                )?;
                cursor = event.sequence;
            }
        }
        require(
            snapshot
                .events
                .last()
                .map_or(snapshot.cursor, |e| e.sequence)
                == snapshot.cursor
                && snapshot.cursor <= cursor,
            "cursor",
        )?;
        tx.execute(
            "UPDATE control SET cursor=?1,fence=?2,confirmed=CASE WHEN ?4 THEN ?3 ELSE confirmed END WHERE id=1",
            params![cursor, snapshot.fence, snapshot.confirmed_stopped, snapshot.cursor >= old_cursor && !snapshot.more],
        )?;
        tx.commit()?;
        Ok(())
    }
}
