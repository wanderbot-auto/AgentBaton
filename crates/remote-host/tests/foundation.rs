use agentbaton_remote_host::{
    controller::Ledger,
    protocol::*,
    store::{load_config, Store},
    transport::{Client, LinkState, SshConfig},
};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    cfg: Config,
    path: PathBuf,
    token: String,
    control: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        let root = base.join("mapping");
        let state = base.join("state");
        let control = base.join("control");
        for p in [&root, &state, &control] {
            fs::create_dir(p).unwrap();
            fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
        }
        // Fresh random test identity/credential; no private user authentication data.
        let mut random = [0u8; 32];
        std::io::Read::read_exact(&mut fs::File::open("/dev/urandom").unwrap(), &mut random)
            .unwrap();
        let token = digest(&random);
        let host = format!("h{}", &token[..16]);
        let now = now_ms().unwrap();
        let binding = Binding {
            host: host.clone(),
            project: "p".into(),
            connection: "c".into(),
            attempt: "a".into(),
            input_revision: "rev1".into(),
            configuration: "cfg1".into(),
            identity_ref: "test_account".into(),
            mapping: "approved_mapping".into(),
            recipient: host.clone(),
            sensitivity: "synthetic".into(),
        };
        let cfg = Config {
            schema: 1,
            host_identity: host,
            root,
            state_dir: state,
            grant: Grant {
                binding,
                controller: "ctrl1".into(),
                fence: 1,
                credential_hash: digest(token.as_bytes()),
                issued_ms: now - 50,
                expires_ms: now + 60000,
                max_operations: 4,
                max_bytes: 16384,
                allow_record: true,
            },
        };
        let path = base.join("trusted.json");
        let f = Self {
            _temp: temp,
            cfg,
            path,
            token,
            control,
        };
        f.save();
        f
    }
    fn save(&self) {
        fs::write(&self.path, serde_json::to_vec(&self.cfg).unwrap()).unwrap();
        fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn req(&self, id: &str, action: Action) -> Request {
        Request {
            version: 1,
            request_id: id.into(),
            host_identity: self.cfg.host_identity.clone(),
            binding: self.cfg.grant.binding.clone(),
            controller: self.cfg.grant.controller.clone(),
            fence: self.cfg.grant.fence,
            credential: self.token.clone(),
            action,
        }
    }
    fn client(&self) -> Client {
        Client::local(
            std::path::Path::new(env!("CARGO_BIN_EXE_baton-host-companion")),
            &self.path,
        )
        .unwrap()
    }
    fn record(&self, id: &str, name: &str, delay: i64) -> Request {
        self.req(
            id,
            Action::Record {
                name: name.into(),
                content: "approved synthetic record".into(),
                delay_ms: delay,
            },
        )
    }
    fn snapshot(&self, c: &mut Client) -> Snapshot {
        match c
            .request(&self.req("sync", Action::Reconcile { cursor: 0 }))
            .unwrap()
        {
            Reply::Snapshot { snapshot } => *snapshot,
            _ => panic!(),
        }
    }
    fn stopped(&self, c: &mut Client) -> Snapshot {
        let deadline = Instant::now() + Duration::from_secs(7);
        loop {
            let s = self.snapshot(c);
            if !s.jobs.is_empty() && s.jobs.iter().all(|j| j.stop_evidence.is_some()) {
                return s;
            }
            assert!(Instant::now() < deadline, "worker was not joined: {s:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
fn code<T>(r: Result<T>, expected: &str) {
    match r {
        Err(e) => assert_eq!(e.code, expected),
        Ok(_) => panic!("expected {expected}"),
    }
}

#[test]
fn real_child_persistence_duplicates_reopen_and_local_event_replay() {
    let f = Fixture::new();
    let mut c = f.client();
    code(
        c.request(&f.record("write1", "output", 20)),
        "reconcile_required",
    );
    f.snapshot(&mut c);
    assert!(matches!(
        c.request(&f.record("write1", "output", 20)).unwrap(),
        Reply::Accepted { .. }
    ));
    let s = f.stopped(&mut c);
    assert_eq!(s.jobs[0].state, "succeeded");
    assert_eq!(s.used_operations, 1);
    assert_eq!(
        fs::read_to_string(f.cfg.root.join("output")).unwrap(),
        "approved synthetic record"
    );
    c.request(&f.record("write1", "output", 20)).unwrap();
    assert_eq!(f.snapshot(&mut c).used_operations, 1);
    code(
        c.request(&f.record("write1", "different", 20)),
        "idempotency_conflict",
    );
    let mut ledger = Ledger::open(&f.control, f.cfg.grant.binding.clone()).unwrap();
    ledger.ingest(&s).unwrap();
    ledger.ingest(&s).unwrap();
    assert_eq!(ledger.cursor().unwrap(), s.cursor);
    let mut bad = s.clone();
    bad.events[0].kind = "forged".into();
    code(ledger.ingest(&bad), "event_conflict");
    let mut unordered = s.clone();
    unordered.events.reverse();
    assert!(ledger.ingest(&unordered).is_err());
    c.close().unwrap();
    let mut c = f.client();
    let reopened = f.snapshot(&mut c);
    assert!(reopened.paused);
    c.request(&f.record("write1", "output", 20)).unwrap();
    code(c.request(&f.record("write2", "extra", 0)), "paused");
    assert_eq!(fs::read_dir(&f.cfg.root).unwrap().count(), 1);
    c.close().unwrap();
}
#[test]
fn actual_cancel_waits_for_child_exit_and_offline_intent_survives_controller_reopen() {
    let f = Fixture::new();
    let mut c = f.client();
    f.snapshot(&mut c);
    c.request(&f.record("slow", "never", 1500)).unwrap();
    let ledger = Ledger::open(&f.control, f.cfg.grant.binding.clone()).unwrap();
    ledger.request_cancel("cancel1").unwrap();
    drop(ledger);
    let mut ledger = Ledger::open(&f.control, f.cfg.grant.binding.clone()).unwrap();
    assert!(!ledger.confirmed_stopped().unwrap());
    let cancel = ledger
        .pending_cancel(&f.req("template", Action::Cancel))
        .unwrap()
        .unwrap();
    assert!(matches!(
        c.request(&cancel).unwrap(),
        Reply::CancelRequested
    ));
    assert!(!ledger.confirmed_stopped().unwrap());
    c.request(&cancel).unwrap();
    let s = f.stopped(&mut c);
    assert!(s.cancel_requested && s.confirmed_stopped);
    assert_eq!(s.jobs[0].state, "stopped_without_effect");
    assert!(!f.cfg.root.join("never").exists());
    ledger.ingest(&s).unwrap();
    assert!(ledger.confirmed_stopped().unwrap());
    assert!(ledger.pending_cancel(&cancel).unwrap().is_none());
    c.close().unwrap();
}
#[test]
fn identity_authority_fencing_and_cumulative_budget() {
    let mut f = Fixture::new();
    f.cfg.grant.max_operations = 1;
    f.save();
    let mut store = Store::open(f.cfg.clone(), true).unwrap();
    let mut bad = f.record("x", "n", 0);
    bad.credential = "untrusted".into();
    code(store.handle(&bad), "unauthenticated");
    for field in 0..4 {
        let mut bad = f.record("x", "n", 0);
        match field {
            0 => bad.host_identity = "other".into(),
            1 => bad.binding.input_revision = "other".into(),
            2 => bad.binding.identity_ref = "other".into(),
            _ => bad.binding.mapping = "other".into(),
        };
        code(store.handle(&bad), "identity_binding");
    }
    let old = f.record("one", "one", 0);
    store.handle(&old).unwrap();
    code(store.handle(&f.record("two", "two", 0)), "budget_exhausted");
    f.cfg.grant.fence = 2;
    f.cfg.grant.controller = "ctrl2".into();
    let mut fresh = Store::open(f.cfg.clone(), true).unwrap();
    code(fresh.handle(&old), "unauthenticated");
    let mut old_fence = f.record("old", "old", 0);
    old_fence.fence = 1;
    code(fresh.handle(&old_fence), "stale_controller");
    code(fresh.handle(&f.record("new", "new", 0)), "budget_exhausted");
    f.cfg.grant.fence = 3;
    f.cfg.grant.max_operations = 2;
    code(
        Store::open(f.cfg.clone(), true).map(|_| ()),
        "grant_expansion",
    );
    assert_eq!(fs::read_dir(&f.cfg.root).unwrap().count(), 0);
}
#[test]
fn lease_expiry_rollback_replay_and_revocation_are_fail_closed() {
    let mut f = Fixture::new();
    f.cfg.grant.expires_ms = now_ms().unwrap() + 150;
    f.save();
    let mut s = Store::open(f.cfg.clone(), true).unwrap();
    std::thread::sleep(Duration::from_millis(180));
    code(s.handle(&f.record("expired", "n", 0)), "expired");
    drop(s);
    let f = Fixture::new();
    let mut s = Store::open(f.cfg.clone(), true).unwrap();
    let db = rusqlite::Connection::open(s.path()).unwrap();
    db.execute("UPDATE meta SET highwater=highwater+10000", [])
        .unwrap();
    code(s.handle(&f.record("rollback", "n", 0)), "clock_rollback");
    db.execute("UPDATE meta SET highwater=0", []).unwrap();
    code(s.handle(&f.record("paused", "n", 0)), "paused");
    drop(s);
    let mut s = Store::open(f.cfg.clone(), true).unwrap();
    code(s.handle(&f.record("restart", "n", 0)), "paused");
    s.handle(&f.req("revoke", Action::Revoke)).unwrap();
    let mut next = f.cfg.clone();
    next.grant.fence += 1;
    let mut renewed = Store::open(next, true).unwrap();
    let mut r = f.record("revoked", "n", 0);
    r.fence += 1;
    code(renewed.handle(&r), "paused");
}
#[test]
fn directory_symlink_absolute_traversal_config_owner_and_unknown_version() {
    let f = Fixture::new();
    let mut s = Store::open(f.cfg.clone(), true).unwrap();
    for name in ["../outside", "/absolute", "a/b", "a;touch bad", ""] {
        code(s.handle(&f.record("bad", name, 0)), "operation_bounds");
    }
    let mut r = f.record("v", "n", 0);
    r.version = 2;
    code(s.handle(&r), "version");
    let mut drift = f.cfg.clone();
    drift.grant.binding.input_revision = "rev2".into();
    code(Store::open(drift, true).map(|_| ()), "identity_drift");
    let link = f.cfg.root.parent().unwrap().join("root-link");
    symlink(&f.cfg.root, &link).unwrap();
    let mut cfg = f.cfg.clone();
    cfg.root = link;
    code(Store::open(cfg, true).map(|_| ()), "directory");
    let config_link = f.path.with_extension("link");
    symlink(&f.path, &config_link).unwrap();
    code(load_config(&config_link), "configuration_path");
    fs::set_permissions(&f.path, fs::Permissions::from_mode(0o644)).unwrap();
    code(load_config(&f.path), "file_permissions");
    fs::set_permissions(f.path.parent().unwrap(), fs::Permissions::from_mode(0o755)).unwrap();
    code(load_config(&f.path), "directory_permissions");
}
#[test]
fn journal_failure_rolls_back_reservation_receipt_and_budget() {
    let f = Fixture::new();
    let mut s = Store::open(f.cfg.clone(), true).unwrap();
    let db = rusqlite::Connection::open(s.path()).unwrap();
    db.execute_batch("CREATE TRIGGER fail_event BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'write failure'); END;").unwrap();
    code(s.handle(&f.record("write", "absent", 0)), "journal");
    for table in ["jobs", "receipts"] {
        assert_eq!(
            db.query_row::<i64, _, _>(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap(),
            0
        );
    }
    assert_eq!(
        db.query_row::<i64, _, _>("SELECT used_ops FROM meta", [], |r| r.get(0))
            .unwrap(),
        0
    );
    assert!(!f.cfg.root.join("absent").exists());
    db.execute_batch("DROP TRIGGER fail_event; BEGIN EXCLUSIVE;")
        .unwrap();
    code(s.handle(&f.record("busy", "absent", 0)), "journal");
    db.execute_batch("ROLLBACK;").unwrap();
}
#[test]
fn strict_ssh_argv_has_no_shell_injection_and_pins_known_hosts_file() {
    let f = Fixture::new();
    let base = f.path.parent().unwrap();
    let known = base.join("known_hosts");
    let identity = base.join("generated_identity");
    fs::write(&known, "generated-host-key-fixture").unwrap();
    fs::write(&identity, "synthetic-argv-fixture-not-a-real-key").unwrap();
    for p in [&known, &identity] {
        fs::set_permissions(p, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut ssh = SshConfig {
        expected_host_identity: f.cfg.host_identity.clone(),
        host: "host.example".into(),
        user: "baton".into(),
        port: 22,
        host_key_alias: "pinned_host".into(),
        known_hosts: known.clone(),
        known_hosts_digest: digest(&fs::read(&known).unwrap()),
        identity_file: identity,
        remote_config: "/opt/baton/approved.json".into(),
    };
    let args = ssh.argv().unwrap();
    assert!(args.contains(&"-oStrictHostKeyChecking=yes".into()));
    assert!(args.contains(&"-oIdentityAgent=none".into()));
    assert_eq!(
        args.last().unwrap(),
        "exec /usr/local/libexec/baton-host-companion --serve /opt/baton/approved.json"
    );
    for host in [
        "-oProxyCommand=bad",
        "h;touch bad",
        "h\nwhoami",
        "user@host",
    ] {
        ssh.host = host.into();
        code(ssh.argv(), "ssh_host");
    }
    ssh.host = "host.example".into();
    ssh.remote_config = "/tmp/x;evil".into();
    code(ssh.argv(), "ssh_remote_path");
    ssh.remote_config = "/tmp/a/../x".into();
    code(ssh.argv(), "ssh_remote_path");
    ssh.remote_config = "/opt/baton/approved.json".into();
    fs::write(known, "changed-host-key").unwrap();
    code(ssh.argv(), "host_key_drift");
}
#[test]
fn malformed_events_cannot_forge_confirmed_stop_or_advance_cursor() {
    let f = Fixture::new();
    let mut c = f.client();
    let mut s = f.snapshot(&mut c);
    let mut ledger = Ledger::open(&f.control, f.cfg.grant.binding.clone()).unwrap();
    s.confirmed_stopped = true;
    code(ledger.ingest(&s), "stop_evidence");
    s.confirmed_stopped = false;
    s.events[0].sequence = 5;
    s.cursor = 5;
    code(ledger.ingest(&s), "event_gap");
    assert_eq!(ledger.cursor().unwrap(), 0);
    c.close().unwrap();
    assert_eq!(c.link, LinkState::Disconnected);
}

#[test]
fn lock_wait_does_not_masquerade_as_clock_rollback() {
    let f = Fixture::new();
    let mut store = Store::open(f.cfg.clone(), true).unwrap();
    let db = rusqlite::Connection::open(store.path()).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let request = f.record("after_lock", "ok", 0);
    let thread = std::thread::spawn(move || store.handle(&request));
    std::thread::sleep(Duration::from_millis(60));
    db.execute("UPDATE meta SET highwater=?1", [now_ms().unwrap()])
        .unwrap();
    db.execute_batch("COMMIT").unwrap();
    assert!(thread.join().unwrap().is_ok());
    assert!(!db
        .query_row::<bool, _, _>("SELECT paused FROM meta", [], |r| r.get(0))
        .unwrap());
}
#[test]
fn root_replacement_and_private_journal_permissions_are_rejected() {
    let f = Fixture::new();
    let store = Store::open(f.cfg.clone(), true).unwrap();
    let path = store.path();
    drop(store);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    code(
        Store::open(f.cfg.clone(), true).map(|_| ()),
        "file_permissions",
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::rename(&f.cfg.root, f.cfg.root.with_extension("old")).unwrap();
    fs::create_dir(&f.cfg.root).unwrap();
    fs::set_permissions(&f.cfg.root, fs::Permissions::from_mode(0o700)).unwrap();
    code(
        Store::open(f.cfg.clone(), true).map(|_| ()),
        "identity_drift",
    );
}
#[test]
fn reserved_dispatch_does_not_acquire_a_replacement_grant() {
    let mut f = Fixture::new();
    let mut store = Store::open(f.cfg.clone(), true).unwrap();
    store.handle(&f.record("old_job", "no", 0)).unwrap();
    drop(store);
    f.cfg.grant.fence += 1;
    f.cfg.grant.expires_ms += 1000;
    f.save();
    let store = Store::open(f.cfg.clone(), true).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_baton-host-companion"))
        .arg("--worker")
        .arg(&f.path)
        .arg("old_job")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
    assert_eq!(fs::read_dir(&f.cfg.root).unwrap().count(), 0);
    let db = rusqlite::Connection::open(store.path()).unwrap();
    assert_eq!(
        db.query_row::<String, _, _>("SELECT state FROM jobs", [], |r| r.get(0))
            .unwrap(),
        "outcome_unknown"
    );
    assert_eq!(
        db.query_row::<i64, _, _>("SELECT used_ops FROM meta", [], |r| r.get(0))
            .unwrap(),
        1
    );
}
#[test]
fn transport_registration_prevents_cross_host_bytes_and_reconciliation_is_bound() {
    let f = Fixture::new();
    let mut client = f.client();
    f.snapshot(&mut client);
    let mut foreign = f.record("bad", "bad", 0);
    foreign.host_identity = "foreign".into();
    code(client.request(&foreign), "transport_host_binding");
    let mut revision = f.record("rev", "bad", 0);
    revision.binding.input_revision = "other".into();
    code(client.request(&revision), "reconcile_required");
    assert_eq!(f.snapshot(&mut client).used_operations, 0);
    client.close().unwrap();
}
#[test]
fn generated_ssh_identity_effective_configuration_without_connecting() {
    let f = Fixture::new();
    let parent = f.path.parent().unwrap();
    let identity = parent.join("test_identity");
    let known = parent.join("known_hosts");
    let status = std::process::Command::new("/usr/bin/ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&identity)
        .status()
        .unwrap();
    assert!(status.success());
    fs::write(
        &known,
        format!(
            "pinned_host {}",
            fs::read_to_string(identity.with_extension("pub")).unwrap()
        ),
    )
    .unwrap();
    fs::set_permissions(&known, fs::Permissions::from_mode(0o600)).unwrap();
    let ssh = SshConfig {
        expected_host_identity: f.cfg.host_identity.clone(),
        host: "never-connect.example.invalid".into(),
        user: "baton".into(),
        port: 22,
        host_key_alias: "pinned_host".into(),
        known_hosts: known.clone(),
        known_hosts_digest: digest(&fs::read(&known).unwrap()),
        identity_file: identity,
        remote_config: "/opt/baton/approved.json".into(),
    };
    let output = std::process::Command::new("/usr/bin/ssh")
        .arg("-G")
        .args(ssh.argv().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("stricthostkeychecking true"));
    assert!(text.contains("identityagent none"));
    assert!(text.contains("hostkeyalias pinned_host"));
    assert!(text.contains("controlmaster false"));
}

#[test]
fn paged_reconciliation_is_required_and_event_capacity_reserves_completion_space() {
    let f = Fixture::new();
    let mut store = Store::open(f.cfg.clone(), true).unwrap();
    for i in 0..70 {
        store
            .handle(&f.req(&format!("cancel{i}"), Action::Cancel))
            .unwrap();
    }
    drop(store);
    let mut c = f.client();
    let first = f.snapshot(&mut c);
    assert!(first.more);
    assert_eq!(first.events.len(), 64);
    code(
        c.request(&f.record("too_soon", "none", 0)),
        "reconcile_required",
    );
    let second = match c
        .request(&f.req(
            "page2",
            Action::Reconcile {
                cursor: first.cursor,
            },
        ))
        .unwrap()
    {
        Reply::Snapshot { snapshot } => *snapshot,
        _ => panic!(),
    };
    assert!(!second.more);
    let mut ledger = Ledger::open(&f.control, f.cfg.grant.binding.clone()).unwrap();
    ledger.ingest(&first).unwrap();
    ledger.ingest(&second).unwrap();
    let cursor = ledger.cursor().unwrap();
    ledger.ingest(&first).unwrap();
    assert_eq!(ledger.cursor().unwrap(), cursor);
    assert!(ledger.confirmed_stopped().unwrap());
    c.close().unwrap();
    let f = Fixture::new();
    let mut store = Store::open(f.cfg.clone(), true).unwrap();
    let db = rusqlite::Connection::open(store.path()).unwrap();
    db.execute_batch("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<9903) INSERT INTO events(kind) SELECT 'capacity_fixture' FROM n;").unwrap();
    code(
        store.handle(&f.record("capacity", "absent", 0)),
        "journal_capacity",
    );
    assert!(!f.cfg.root.join("absent").exists());
    assert_eq!(
        db.query_row::<i64, _, _>("SELECT used_ops FROM meta", [], |r| r.get(0))
            .unwrap(),
        0
    );
}
#[test]
fn maximum_snapshot_fits_one_frame() {
    let f = Fixture::new();
    let mut c = f.client();
    let mut s = f.snapshot(&mut c);
    c.close().unwrap();
    s.binding = Binding {
        host: "x".repeat(64),
        project: "x".repeat(64),
        connection: "x".repeat(64),
        attempt: "x".repeat(64),
        input_revision: "x".repeat(64),
        configuration: "x".repeat(64),
        identity_ref: "x".repeat(64),
        mapping: "x".repeat(64),
        recipient: "x".repeat(64),
        sensitivity: "x".repeat(64),
    };
    s.events = (1..=64)
        .map(|i| Event {
            sequence: i,
            kind: "x".repeat(64),
            job: Some("x".repeat(64)),
        })
        .collect();
    s.jobs = (0..32)
        .map(|_| Job {
            id: "x".repeat(64),
            name: "x".repeat(64),
            digest: "x".repeat(64),
            state: "outcome_unknown".into(),
            stop_evidence: Some("owned_child_wait_nonzero".into()),
        })
        .collect();
    let response = Response {
        version: 1,
        request_id: "x".repeat(64),
        result: Ok(Reply::Snapshot {
            snapshot: Box::new(s),
        }),
    };
    assert!(serde_json::to_vec(&response).unwrap().len() < MAX_FRAME);
}

#[test]
fn future_wal_schemas_are_rejected_before_journal_mode_mutation() {
    let f = Fixture::new();
    let store = Store::open(f.cfg.clone(), true).unwrap();
    let path = store.path();
    drop(store);
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA user_version=99;")
        .unwrap();
    code(
        Store::open(f.cfg.clone(), true).map(|_| ()),
        "journal_version",
    );
    assert_eq!(
        db.query_row::<String, _, _>("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap(),
        "wal"
    );
    assert_eq!(
        db.query_row::<i64, _, _>("PRAGMA user_version", [], |r| r.get(0))
            .unwrap(),
        99
    );
    let ledger = Ledger::open(&f.control, f.cfg.grant.binding.clone()).unwrap();
    drop(ledger);
    let db = rusqlite::Connection::open(f.control.join("controller.sqlite3")).unwrap();
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA user_version=99;")
        .unwrap();
    code(
        Ledger::open(&f.control, f.cfg.grant.binding.clone()).map(|_| ()),
        "journal_version",
    );
    assert_eq!(
        db.query_row::<String, _, _>("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap(),
        "wal"
    );
    assert_eq!(
        db.query_row::<i64, _, _>("PRAGMA user_version", [], |r| r.get(0))
            .unwrap(),
        99
    );
}
