use agentbaton_config::{sha256, Edit, ErrorCode, FileState, ManagedRoot, Ownership, Phase};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::PathBuf,
};
struct Fixture {
    _temp: tempfile::TempDir,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().canonicalize().unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self { _temp: temp, path }
    }
    fn root(&self) -> ManagedRoot {
        ManagedRoot::open(&self.path, Ownership::BatonManaged).unwrap()
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        fs::write(self.path.join(name), bytes).unwrap();
        fs::set_permissions(self.path.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }
}
#[test]
fn apply_and_restart_recovery_and_conditional_rollback_preserve_unknown_bytes() {
    let fixture = Fixture::new();
    let original = b"# keep comment\nunknown_provider_option = 'keep'\nmanaged = 1\n";
    let updated = b"# keep comment\nunknown_provider_option = 'keep'\nmanaged = 2\n";
    fixture.write("config.toml", original);
    let root = fixture.root();
    let change = root
        .plan(vec![Edit::nonsecret_replacement(
            "config.toml",
            Some(sha256(original)),
            updated.to_vec(),
        )])
        .unwrap();
    let preview = change.preview();
    assert_eq!(preview[0].before_sha256, Some(sha256(original)));
    assert_eq!(preview[0].after_sha256, sha256(updated));
    let serialized = serde_json::to_string(&preview).unwrap();
    assert!(!serialized.contains("keep comment"));
    let report = root.apply(change).unwrap();
    assert_eq!(report.phase, Phase::Applied);
    assert_eq!(fs::read(fixture.path.join("config.toml")).unwrap(), updated);
    drop(root);
    let root = fixture.root();
    let recovered = root.recover(&report.transaction_id).unwrap();
    assert_eq!(recovered.phase, Phase::Applied);
    let rolled_back = root.rollback(&report.transaction_id).unwrap();
    assert_eq!(rolled_back.phase, Phase::RolledBack);
    assert_eq!(
        fs::read(fixture.path.join("config.toml")).unwrap(),
        original
    );
    assert_eq!(
        root.recover(&report.transaction_id).unwrap().phase,
        Phase::RolledBack
    );
    assert_eq!(
        root.rollback(&report.transaction_id).unwrap().phase,
        Phase::RolledBack
    );
}
#[test]
fn drift_after_preview_aborts_before_any_file_is_changed() {
    let fixture = Fixture::new();
    fixture.write("one", b"one");
    fixture.write("two", b"two");
    let root = fixture.root();
    let changes = root
        .plan(vec![
            Edit::nonsecret_replacement("one", Some(sha256(b"one")), b"updated".to_vec()),
            Edit::nonsecret_replacement("two", Some(sha256(b"two")), b"updated".to_vec()),
        ])
        .unwrap();
    fixture.write("two", b"external");
    assert_eq!(root.apply(changes).unwrap_err().code, ErrorCode::Conflict);
    assert_eq!(fs::read(fixture.path.join("one")).unwrap(), b"one");
    assert_eq!(fs::read(fixture.path.join("two")).unwrap(), b"external");
}
#[test]
fn rollback_never_overwrites_later_external_edit() {
    let fixture = Fixture::new();
    fixture.write("config", b"old");
    let root = fixture.root();
    let change = root
        .plan(vec![Edit::nonsecret_replacement(
            "config",
            Some(sha256(b"old")),
            b"new".to_vec(),
        )])
        .unwrap();
    let applied = root.apply(change).unwrap();
    fixture.write("config", b"external");
    let rolled_back = root.rollback(&applied.transaction_id).unwrap();
    assert_eq!(rolled_back.phase, Phase::Conflict);
    assert_eq!(rolled_back.files[0].state, FileState::Conflict);
    assert_eq!(fs::read(fixture.path.join("config")).unwrap(), b"external");
}
#[test]
fn rollback_creation_removes_only_matching_generated_file() {
    let fixture = Fixture::new();
    let root = fixture.root();
    let change = root
        .plan(vec![Edit::nonsecret_replacement(
            "config",
            None,
            b"new".to_vec(),
        )])
        .unwrap();
    let applied = root.apply(change).unwrap();
    root.rollback(&applied.transaction_id).unwrap();
    assert!(!fixture.path.join("config").exists());
}
#[test]
fn native_connected_and_nonprivate_roots_are_rejected_without_permission_changes() {
    let fixture = Fixture::new();
    assert!(
        matches!(ManagedRoot::open(&fixture.path, Ownership::NativeConnected), Err(e) if e.code == ErrorCode::PermissionDenied)
    );
    fs::set_permissions(&fixture.path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        matches!(ManagedRoot::open(&fixture.path, Ownership::BatonManaged), Err(e) if e.code == ErrorCode::PermissionDenied)
    );
    assert_eq!(
        fs::metadata(&fixture.path).unwrap().permissions().mode() & 0o777,
        0o755
    );
}
#[test]
fn traversal_absolute_symlink_and_reserved_names_fail_closed() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    outside.write("outside", b"untouched");
    let root = fixture.root();
    for path in [
        "../outside",
        "/etc/passwd",
        ".baton-transactions/bad",
        ".",
        "",
    ] {
        assert!(root
            .plan(vec![Edit::nonsecret_replacement(
                path,
                None,
                b"bad".to_vec()
            )])
            .is_err());
    }
    symlink(outside.path.join("outside"), fixture.path.join("link")).unwrap();
    assert!(root
        .plan(vec![Edit::nonsecret_replacement(
            "link",
            Some(sha256(b"untouched")),
            b"bad".to_vec()
        )])
        .is_err());
    symlink(&outside.path, fixture.path.join("dir-link")).unwrap();
    assert!(root
        .plan(vec![Edit::nonsecret_replacement(
            "dir-link/outside",
            Some(sha256(b"untouched")),
            b"bad".to_vec()
        )])
        .is_err());
    assert_eq!(
        fs::read(outside.path.join("outside")).unwrap(),
        b"untouched"
    );
    assert!(root.recover("../elsewhere").is_err());
}
#[test]
fn oversized_replacement_and_duplicate_paths_are_rejected() {
    let fixture = Fixture::new();
    let root = fixture.root();
    assert!(
        matches!(root.plan(vec![Edit::nonsecret_replacement("config", None, vec![0; 1_048_577])]), Err(e) if e.code == ErrorCode::ResourceLimit)
    );
    assert!(root
        .plan(vec![
            Edit::nonsecret_replacement("config", None, b"1".to_vec()),
            Edit::nonsecret_replacement("config", None, b"2".to_vec())
        ])
        .is_err());
}
#[test]
fn a_second_managed_root_detects_conflicting_completed_change() {
    let fixture = Fixture::new();
    fixture.write("config", b"old");
    let first = fixture.root();
    let second = fixture.root();
    let a = first
        .plan(vec![Edit::nonsecret_replacement(
            "config",
            Some(sha256(b"old")),
            b"a".to_vec(),
        )])
        .unwrap();
    let b = second
        .plan(vec![Edit::nonsecret_replacement(
            "config",
            Some(sha256(b"old")),
            b"b".to_vec(),
        )])
        .unwrap();
    assert_eq!(first.apply(a).unwrap().phase, Phase::Applied);
    assert_eq!(second.apply(b).unwrap_err().code, ErrorCode::Conflict);
    assert_eq!(fs::read(fixture.path.join("config")).unwrap(), b"a");
}
#[test]
fn corrupted_backup_is_not_restored() {
    let fixture = Fixture::new();
    fixture.write("config", b"old");
    let root = fixture.root();
    let change = root
        .plan(vec![Edit::nonsecret_replacement(
            "config",
            Some(sha256(b"old")),
            b"new".to_vec(),
        )])
        .unwrap();
    let applied = root.apply(change).unwrap();
    let backup = fixture
        .path
        .join(".baton-transactions")
        .join(&applied.transaction_id)
        .join("backup-0.bin");
    fs::write(backup, b"corrupt").unwrap();
    assert_eq!(
        root.rollback(&applied.transaction_id).unwrap_err().code,
        ErrorCode::Conflict
    );
    assert_eq!(fs::read(fixture.path.join("config")).unwrap(), b"new");
}
#[test]
fn two_concurrent_writers_have_one_winner_and_never_overwrite_it() {
    let fixture = Fixture::new();
    fixture.write("config", b"old");
    let first = fixture.root();
    let second = fixture.root();
    let a = first
        .plan(vec![Edit::nonsecret_replacement(
            "config",
            Some(sha256(b"old")),
            b"a".to_vec(),
        )])
        .unwrap();
    let b = second
        .plan(vec![Edit::nonsecret_replacement(
            "config",
            Some(sha256(b"old")),
            b"b".to_vec(),
        )])
        .unwrap();
    let start = std::sync::Arc::new(std::sync::Barrier::new(2));
    let first_start = start.clone();
    let first = std::thread::spawn(move || {
        first_start.wait();
        first.apply(a)
    });
    let second = std::thread::spawn(move || {
        start.wait();
        second.apply(b)
    });
    let first = first.join().unwrap();
    let second = second.join().unwrap();
    assert_ne!(first.is_ok(), second.is_ok());
    let expected = if first.is_ok() { b"a" } else { b"b" };
    assert_eq!(fs::read(fixture.path.join("config")).unwrap(), expected);
}
#[test]
fn pending_crash_intent_is_reconciled_from_hash_without_replaying_write() {
    let fixture = Fixture::new();
    fixture.write("config", b"old");
    let root = fixture.root();
    let change = root
        .plan(vec![Edit::nonsecret_replacement(
            "config",
            Some(sha256(b"old")),
            b"new".to_vec(),
        )])
        .unwrap();
    let applied = root.apply(change).unwrap();
    let manifest = fixture
        .path
        .join(".baton-transactions")
        .join(&applied.transaction_id)
        .join("manifest.json");
    let mut intent: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    intent["phase"] = "Applying".into();
    intent["files"][0]["state"] = "Pending".into();
    fs::write(&manifest, serde_json::to_vec(&intent).unwrap()).unwrap();
    drop(root);
    let root = fixture.root();
    let reconciled = root.recover(&applied.transaction_id).unwrap();
    assert_eq!(reconciled.phase, Phase::Applied);
    assert_eq!(reconciled.files[0].state, FileState::Written);
    assert_eq!(fs::read(fixture.path.join("config")).unwrap(), b"new");
}
#[test]
fn changeset_cannot_cross_managed_roots_even_with_identical_baselines() {
    let first = Fixture::new();
    let second = Fixture::new();
    first.write("config", b"same");
    second.write("config", b"same");
    let root_a = first.root();
    let root_b = second.root();
    let change = root_a
        .plan(vec![Edit::nonsecret_replacement(
            "config",
            Some(sha256(b"same")),
            b"updated".to_vec(),
        )])
        .unwrap();
    assert_eq!(root_b.apply(change).unwrap_err().code, ErrorCode::Conflict);
    assert_eq!(fs::read(first.path.join("config")).unwrap(), b"same");
    assert_eq!(fs::read(second.path.join("config")).unwrap(), b"same");
}
#[test]
fn transplanted_journal_cannot_rollback_another_root() {
    let first = Fixture::new();
    let second = Fixture::new();
    first.write("config", b"same");
    second.write("config", b"same");
    let root_a = first.root();
    let root_b = second.root();
    let change = root_a
        .plan(vec![Edit::nonsecret_replacement(
            "config",
            Some(sha256(b"same")),
            b"updated".to_vec(),
        )])
        .unwrap();
    let report = root_a.apply(change).unwrap();
    second.write("config", b"updated");
    let transaction = format!(".baton-transactions/{}", report.transaction_id);
    fs::create_dir(second.path.join(&transaction)).unwrap();
    fs::set_permissions(
        second.path.join(&transaction),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    for name in ["manifest.json", "backup-0.bin"] {
        fs::copy(
            first.path.join(&transaction).join(name),
            second.path.join(&transaction).join(name),
        )
        .unwrap();
    }
    assert_eq!(
        root_b.recover(&report.transaction_id).unwrap_err().code,
        ErrorCode::Conflict
    );
    assert_eq!(
        root_b.rollback(&report.transaction_id).unwrap_err().code,
        ErrorCode::Conflict
    );
    assert_eq!(fs::read(second.path.join("config")).unwrap(), b"updated");
}
