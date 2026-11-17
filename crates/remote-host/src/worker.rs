//! The only foundation operation: a bounded, exclusive UTF-8 record in the approved mapping.
//! No shell, arbitrary executable, Provider, network, or directory synchronization interface.
use crate::{protocol::*, store::Store};
use rusqlite::{params, TransactionBehavior};
use std::ffi::CString;
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};
use std::time::{Duration, Instant};

pub fn run(mut store: Store, job: &str) -> Result<()> {
    let (name, content, delay, state): (String, String, i64, String) = store.db.query_row(
        "SELECT name,content,delay_ms,state FROM jobs WHERE id=?1",
        [job],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    require(state == "reserved", "already_dispatched")?;
    let (dispatch_fence, grant_hash): (i64, String) = store.db.query_row(
        "SELECT dispatch_fence,grant_hash FROM jobs WHERE id=?1",
        [job],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    require(
        dispatch_fence == store.config.grant.fence
            && grant_hash == digest(&serde_json::to_vec(&store.config.grant)?),
        "stale_dispatch",
    )?;
    let start = Instant::now();
    let wall_start = now_ms()?;
    let monotonic_limit = Duration::from_millis(
        store
            .config
            .grant
            .expires_ms
            .saturating_sub(wall_start)
            .max(0) as u64,
    );
    loop {
        let tx = store
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = now_ms()?;
        let (fence, high, paused, revoked, cancelled): (i64, i64, bool, bool, bool) = tx
            .query_row(
                "SELECT fence,highwater,paused,revoked,cancelled FROM meta WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )?;
        let state: String =
            tx.query_row("SELECT state FROM jobs WHERE id=?1", [job], |r| r.get(0))?;
        if !matches!(state.as_str(), "reserved" | "running") {
            return Ok(());
        }
        let rollback = now < high;
        let expired = now < store.config.grant.issued_ms
            || now >= store.config.grant.expires_ms
            || start.elapsed() >= monotonic_limit;
        if rollback
            || expired
            || paused
            || revoked
            || cancelled
            || fence != store.config.grant.fence
        {
            tx.execute("UPDATE meta SET paused=CASE WHEN ?1 OR ?2 THEN 1 ELSE paused END,highwater=max(highwater,?3) WHERE id=1 AND fence=?4",params![rollback,expired,now,store.config.grant.fence])?;
            tx.execute(
                "UPDATE jobs SET state='stopped_without_effect' WHERE id=?1",
                [job],
            )?;
            tx.execute(
                "INSERT INTO events(kind,job) VALUES('worker_effects_stopped',?1)",
                [job],
            )?;
            tx.commit()?;
            return Ok(());
        }
        tx.execute("UPDATE meta SET highwater=?1 WHERE id=1", [now])?;
        if start.elapsed() < Duration::from_millis(delay as u64) {
            tx.execute("UPDATE jobs SET state='running' WHERE id=?1", [job])?;
            tx.commit()?;
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        // The intent was durable before spawning. A crash/failed commit here is unknown, never redispatched.
        tx.execute("UPDATE jobs SET state='effect_intent' WHERE id=?1", [job])?;
        require(
            identifier(&name) && content.len() <= MAX_CONTENT,
            "operation_bounds",
        )?;
        let leaf = CString::new(name).map_err(|_| Error::new("path"))?;
        let fd = unsafe {
            libc::openat(
                store.root.as_raw_fd(),
                leaf.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            tx.execute(
                "UPDATE jobs SET state='failed_no_effect' WHERE id=?1",
                [job],
            )?;
            tx.execute(
                "INSERT INTO events(kind,job) VALUES('exclusive_create_failed',?1)",
                [job],
            )?;
            tx.commit()?;
            return Ok(());
        }
        let mut output = unsafe { File::from_raw_fd(fd) };
        // Any IO failure after create leaves an uncertain partial effect, intentionally not auto-retried.
        output.write_all(content.as_bytes())?;
        output.sync_all()?;
        store.root.sync_all()?;
        tx.execute("UPDATE jobs SET state='succeeded' WHERE id=?1", [job])?;
        tx.execute(
            "INSERT INTO events(kind,job) VALUES('record_durable',?1)",
            [job],
        )?;
        tx.commit()?;
        return Ok(());
    }
}
