use super::*;
use serde_json::{json, Value};
use std::os::unix::fs::{symlink, OpenOptionsExt, PermissionsExt};

fn root() -> tempfile::TempDir {
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    root
}
fn parent(root: &Path) -> File {
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags((OFlags::DIRECTORY | OFlags::NOFOLLOW).bits() as i32)
        .open(root)
        .unwrap()
}
fn descriptor() -> Value {
    json!({"schema_version":1,"nonce":"a".repeat(32),"source":{"canonical_root":"/unopened-fixture-source","device":"9007199254740993","inode":"18446744073709551615"},"files":[{"artifact_id":"file-1","relative_path":"large.bin","max_bytes":67108864,"expected_sha256":"b".repeat(64)}],"limits":{"max_file_bytes":67108864,"max_total_bytes":67108864,"max_files":4}})
}
fn write(root: &Path, value: &Value) {
    std::fs::write(root.join(CONFIG), serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::set_permissions(root.join(CONFIG), std::fs::Permissions::from_mode(0o600)).unwrap();
}
fn load(root: &Path) -> Arc<CopyEvidenceGate> {
    CopyEvidenceGate::load(root, &parent(root))
        .unwrap()
        .unwrap()
}
fn contract() -> DataSelectionContract {
    DataSelectionContract {
        format_version: 1,
        approval_reference: "original-preview".into(),
        source: agentbaton_workspaces::data_workspace::DirectoryPin {
            canonical_root: "/unopened-fixture-source".into(),
            device: 9_007_199_254_740_993,
            inode: u64::MAX,
        },
        files: serde_json::from_value(descriptor()["files"].clone()).unwrap(),
        limits: serde_json::from_value(descriptor()["limits"].clone()).unwrap(),
    }
}
fn observed_ready(root: &Path) -> Value {
    let end = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(raw) = std::fs::read(root.join(READY)) {
            if let Ok(v) = serde_json::from_slice(&raw) {
                return v;
            }
        }
        assert!(
            Instant::now() < end,
            "private ready handshake not published"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn missing_is_off_and_descriptor_does_not_open_source_or_hot_reload() {
    let r = root();
    let off = CopyEvidenceGate::load(r.path(), &parent(r.path())).unwrap();
    assert!(off.is_none());
    write(r.path(), &descriptor());
    assert!(
        off.is_none(),
        "constructor result cannot gain an enable switch later"
    );
    let gate = load(r.path());
    assert!(gate.matches(&contract()));
    // The generated descriptor points at no real source; load never opens it.
    assert_eq!(gate.descriptor.source.device, "9007199254740993");
    assert_eq!(gate.descriptor.source.inode, u64::MAX.to_string());
    std::fs::write(r.path().join(CONFIG), b"malformed after startup").unwrap();
    assert!(
        gate.matches(&contract()),
        "startup descriptor is immutable in memory"
    );
}

#[test]
fn malformed_schema_identity_selection_policy_and_extra_fields_refuse_startup() {
    let r = root();
    let mut cases = Vec::new();
    for (field, value) in [
        ("schema_version", json!(2)),
        ("schema_version", json!(1.0)),
        ("nonce", json!("short")),
        ("nonce", json!("A".repeat(32))),
        ("unexpected", json!(true)),
    ] {
        let mut d = descriptor();
        d[field] = value;
        cases.push(d);
    }
    for v in ["01", "-1", "18446744073709551616", "1\n"] {
        let mut d = descriptor();
        d["source"]["device"] = json!(v);
        cases.push(d);
    }
    for v in [
        "relative",
        "/source/../other",
        "/source/./other",
        "/source//other",
        "/source\0other",
    ] {
        let mut d = descriptor();
        d["source"]["canonical_root"] = json!(v);
        cases.push(d);
    }
    for v in ["../secret", "data/./part", "/absolute", "data\0part"] {
        let mut d = descriptor();
        d["files"][0]["relative_path"] = json!(v);
        cases.push(d);
    }
    for (field, value) in [
        ("max_files", json!(5)),
        ("max_file_bytes", json!(67108865)),
        ("max_total_bytes", json!(67108865)),
    ] {
        let mut d = descriptor();
        d["limits"][field] = value;
        cases.push(d);
    }
    for d in cases {
        write(r.path(), &d);
        assert!(CopyEvidenceGate::load(r.path(), &parent(r.path())).is_err());
    }
    std::fs::write(r.path().join(CONFIG), b"{}").unwrap();
    assert!(CopyEvidenceGate::load(r.path(), &parent(r.path())).is_err());
    std::fs::write(
        r.path().join(CONFIG),
        br#"{"schema_version":1,"schema_version":1}"#,
    )
    .unwrap();
    assert!(CopyEvidenceGate::load(r.path(), &parent(r.path())).is_err());
}

#[test]
fn config_symlink_fifo_hardlink_nonprivate_and_oversize_are_rejected() {
    let r = root();
    let other = r.path().join("other");
    std::fs::write(&other, serde_json::to_vec(&descriptor()).unwrap()).unwrap();
    std::fs::set_permissions(&other, std::fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&other, r.path().join(CONFIG)).unwrap();
    assert!(CopyEvidenceGate::load(r.path(), &parent(r.path())).is_err());
    std::fs::remove_file(r.path().join(CONFIG)).unwrap();
    let fifo = std::ffi::CString::new(r.path().join(CONFIG).to_str().unwrap()).unwrap();
    // Own generated fixture only; this creates a FIFO but never opens its body.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert!(CopyEvidenceGate::load(r.path(), &parent(r.path())).is_err());
    std::fs::remove_file(r.path().join(CONFIG)).unwrap();
    std::fs::hard_link(&other, r.path().join(CONFIG)).unwrap();
    assert!(CopyEvidenceGate::load(r.path(), &parent(r.path())).is_err());
    std::fs::remove_file(r.path().join(CONFIG)).unwrap();
    write(r.path(), &descriptor());
    std::fs::set_permissions(
        r.path().join(CONFIG),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(CopyEvidenceGate::load(r.path(), &parent(r.path())).is_err());
    std::fs::set_permissions(
        r.path().join(CONFIG),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    std::fs::write(r.path().join(CONFIG), vec![b' '; MAX_CONFIG as usize + 1]).unwrap();
    assert!(CopyEvidenceGate::load(r.path(), &parent(r.path())).is_err());
    write(r.path(), &descriptor());
    std::fs::set_permissions(r.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(CopyEvidenceGate::load(r.path(), &parent(r.path())).is_err());
}

#[test]
fn different_validated_source_files_or_policy_never_hit_or_publish_ready() {
    let r = root();
    write(r.path(), &descriptor());
    let gate = load(r.path());
    let mut cases = Vec::new();
    let original = contract();
    let mut c = original.clone();
    c.source.canonical_root = "/another-source".into();
    cases.push(c);
    let mut c = original.clone();
    c.source.device = 1;
    cases.push(c);
    let mut c = original.clone();
    c.source.inode = 1;
    cases.push(c);
    let mut c = original.clone();
    c.files[0].relative_path = "other.bin".into();
    cases.push(c);
    let mut c = original.clone();
    c.files[0].expected_sha256 = Some("c".repeat(64));
    cases.push(c);
    let mut c = original.clone();
    c.files[0].artifact_id = "other".into();
    cases.push(c);
    let mut c = original.clone();
    c.limits.max_total_bytes -= 1;
    cases.push(c);
    for c in cases {
        gate.wait(
            "actual-creation",
            "actual-project",
            &c,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
    }
    assert!(!r.path().join(READY).exists());
    assert!(!gate.used.load(Ordering::SeqCst));
}

#[test]
fn actual_waiter_binds_ready_then_observes_cancel_once_without_success() {
    let r = root();
    write(r.path(), &descriptor());
    let gate = load(r.path());
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker_gate = gate.clone();
    let thread = std::thread::spawn(move || {
        worker_gate.wait(
            "actual-creation",
            "actual-project",
            &contract(),
            &worker_cancel,
            Instant::now() + Duration::from_secs(2),
        )
    });
    let ready = observed_ready(r.path());
    assert_eq!(
        ready,
        json!({"schema_version":1,"gate":MARKER,"stage":"before_copy","nonce":"a".repeat(32),"descriptor_sha256":gate.digest,"creation_id":"actual-creation","project_id":"actual-project"})
    );
    let info = std::fs::symlink_metadata(r.path().join(READY)).unwrap();
    assert!(info.is_file());
    assert_eq!(info.nlink(), 1);
    assert_eq!(info.mode() & 0o7777, 0o600);
    assert!(!thread.is_finished(), "ready cannot force copy success");
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(thread.join().unwrap().unwrap_err().code, "CANCEL_REQUESTED");
    let before = std::fs::read(r.path().join(READY)).unwrap();
    gate.wait(
        "actual-creation",
        "actual-project",
        &contract(),
        &cancel,
        Instant::now() + Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(before, std::fs::read(r.path().join(READY)).unwrap());
}

#[test]
fn original_timeout_and_preexisting_marker_fail_without_overwrite_or_scan() {
    let r = root();
    write(r.path(), &descriptor());
    let gate = load(r.path());
    let result = gate.wait(
        "actual-creation",
        "actual-project",
        &contract(),
        &AtomicBool::new(false),
        Instant::now() + Duration::from_millis(10),
    );
    assert_eq!(result.unwrap_err().code, "DEADLINE_EXCEEDED");
    assert!(r.path().join(READY).exists());
    let r = root();
    write(r.path(), &descriptor());
    let gate = load(r.path());
    std::fs::write(r.path().join(READY), b"retained unknown marker").unwrap();
    assert_eq!(
        gate.wait(
            "actual-creation",
            "actual-project",
            &contract(),
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(1)
        )
        .unwrap_err()
        .code,
        "CONFIG_CONFLICT"
    );
    assert_eq!(
        std::fs::read(r.path().join(READY)).unwrap(),
        b"retained unknown marker"
    );
}

#[test]
fn cancelled_or_elapsed_before_ready_and_replaced_runtime_fail_closed() {
    let r = root();
    write(r.path(), &descriptor());
    let gate = load(r.path());
    assert_eq!(
        gate.wait(
            "actual-creation",
            "actual-project",
            &contract(),
            &AtomicBool::new(true),
            Instant::now() + Duration::from_secs(1)
        )
        .unwrap_err()
        .code,
        "CANCEL_REQUESTED"
    );
    assert!(!r.path().join(READY).exists());
    let gate = load(r.path());
    assert_eq!(
        gate.wait(
            "actual-creation",
            "actual-project",
            &contract(),
            &AtomicBool::new(false),
            Instant::now()
        )
        .unwrap_err()
        .code,
        "DEADLINE_EXCEEDED"
    );
    assert!(!r.path().join(READY).exists());
    let held = parent(r.path());
    let moved = r.path().with_extension("held");
    std::fs::rename(r.path(), &moved).unwrap();
    std::fs::create_dir(r.path()).unwrap();
    std::fs::set_permissions(r.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(CopyEvidenceGate::load(r.path(), &held).is_err());
    // Only this test's generated directory is removed, never a caller's source.
    std::fs::remove_dir_all(&moved).unwrap();
}

#[test]
fn authorized_copy_worker_cancel_joins_before_original_finished_receipt_without_replay() {
    use crate::{
        data_workspace_workers::DataWorkspaceWorkers,
        data_workspaces::{DataWorkspaceCreation, DataWorkspaceSelection},
        storage::Store,
    };
    struct Owned {
        workers: DataWorkspaceWorkers,
        store: Store,
    }
    impl Drop for Owned {
        fn drop(&mut self) {
            self.workers.reap_owned_workers();
        }
    }
    let r = root();
    let runtime = r.path().join("runtime");
    let source = r.path().join("source");
    let managed = runtime.join("managed-data");
    std::fs::create_dir(&runtime).unwrap();
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("large.bin"), b"generated exact copy data").unwrap();
    std::fs::create_dir(&managed).unwrap();
    std::fs::set_permissions(&managed, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut store = Store::open(&runtime.join("baton.sqlite3")).unwrap();
    let project = store
        .import_project(&source, "Generated fixture", "project")
        .unwrap();
    let mut d = descriptor();
    d["source"] = json!({"canonical_root":project.identity.canonical_root,"device":project.identity.device.to_string(),"inode":project.identity.inode.to_string()});
    d["files"][0]["expected_sha256"] = json!(agentbaton_workspaces::validation::sha256(
        b"generated exact copy data"
    ));
    write(&runtime, &d);
    let selection = DataWorkspaceSelection {
        project_id: project.id.clone(),
        files: serde_json::from_value(d["files"].clone()).unwrap(),
        limits: serde_json::from_value(d["limits"].clone()).unwrap(),
    };
    let mut owned = Owned {
        workers: DataWorkspaceWorkers::new(&runtime).unwrap(),
        store,
    };
    let preview = owned
        .workers
        .preview(&mut owned.store, &selection, project.revision, "preview")
        .unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    while owned.workers.has_workers() {
        owned.workers.tick(&mut owned.store).unwrap();
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    let verified = owned.store.get_data_workspace_preview(&preview.id).unwrap();
    assert_eq!(verified.state, "verified");
    let proposal = owned
        .store
        .propose_data_workspace(&verified.id, verified.revision, "proposal", &managed)
        .unwrap();
    let proposed_id = proposal.snapshot["intent"]["creation_id"].as_str().unwrap();
    assert!(
        owned.workers.copy(&mut owned.store, proposed_id).is_err(),
        "a proposal alone is not copy authority"
    );
    assert!(!runtime.join(READY).exists());
    // Existing trusted Store fixture only: not a Native OS confirmation or IPC run.
    let result = owned
        .store
        .finalize_proposal(&proposal.id, proposal.revision)
        .unwrap();
    let record: DataWorkspaceCreation = serde_json::from_value(result).unwrap();
    assert_eq!(record.state, "creating");
    owned.workers.copy(&mut owned.store, &record.id).unwrap();
    let ready = observed_ready(&runtime);
    assert_eq!(ready["creation_id"], record.id);
    assert_eq!(ready["project_id"], project.id);
    assert_eq!(ready["stage"], "before_copy");
    assert!(
        !record.destination.exists(),
        "ready is before copying, not a partial write"
    );
    assert!(owned.workers.has_workers());
    let requested = owned
        .store
        .cancel_data_workspace_creation(&record.id, record.revision, "cancel")
        .unwrap();
    assert_eq!(requested.state, "cancel_requested");
    owned.workers.cancel(&record.id);
    let end = Instant::now() + Duration::from_secs(2);
    while owned.workers.has_workers() {
        owned.workers.tick(&mut owned.store).unwrap();
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    let finished = owned.store.get_data_workspace_creation(&record.id).unwrap();
    assert_eq!(finished.state, "cancelled");
    assert!(finished.cancel_requested);
    assert_eq!(finished.io_completion.as_deref(), Some("worker_joined"));
    assert_eq!(finished.error.unwrap().code, "CANCEL_REQUESTED");
    assert!(finished.contract.is_none());
    assert!(!record.destination.exists());
    let mut query=owned.store.conn.prepare("SELECT json_extract(document,'$.kind') FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind') IN('data_workspace.copy_cancel_requested','data_workspace.copy_finished') ORDER BY sequence").unwrap();
    let kinds: Vec<String> = query
        .query_map([&record.id], |row| row.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    assert_eq!(
        kinds,
        vec![
            "data_workspace.copy_cancel_requested",
            "data_workspace.copy_finished"
        ]
    );
    drop(query);
    let sequence = owned.store.sequence().unwrap();
    let marker = std::fs::read(runtime.join(READY)).unwrap();
    owned.workers.copy(&mut owned.store, &record.id).unwrap();
    let replay = owned
        .store
        .cancel_data_workspace_creation(&record.id, record.revision, "cancel")
        .unwrap();
    assert_eq!(replay.state, "cancel_requested");
    assert_eq!(owned.store.sequence().unwrap(), sequence);
    assert!(!owned.workers.has_workers());
    assert_eq!(std::fs::read(runtime.join(READY)).unwrap(), marker);
    assert_eq!(
        std::fs::read(source.join("large.bin")).unwrap(),
        b"generated exact copy data"
    );
}
