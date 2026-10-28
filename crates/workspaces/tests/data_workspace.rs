use agentbaton_workspaces::{
    data_workspace::*,
    validation::{sha256, ArtifactRequirement},
};
use std::{
    fs::{self, File, Permissions},
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
    time::{Duration, Instant},
};
use tempfile::TempDir;

struct Fixture {
    _base: TempDir,
    source: PathBuf,
    managed: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let base = tempfile::tempdir().unwrap();
        let root = base.path().canonicalize().unwrap();
        let source = root.join("source");
        let managed = root.join("managed");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, Permissions::from_mode(0o700)).unwrap();
        Self {
            _base: base,
            source,
            managed,
        }
    }
    fn put(&self, name: &str, bytes: &[u8]) {
        let path = self.source.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn selection(&self, names: &[&str]) -> ApprovedDataSelection {
        ApprovedDataSelection::from_verified_host(
            &self.source,
            "owner-approval-1".into(),
            requirements(names),
            DataLimits::default(),
        )
        .unwrap()
    }
    fn snapshot(&self, names: &[&str]) -> VerifiedDatasetSnapshot {
        verify_dataset(&self.selection(names)).unwrap()
    }
    fn managed(&self) -> ApprovedManagedRoot {
        ApprovedManagedRoot::from_verified_host(&self.managed).unwrap()
    }
    fn ready(&self, names: &[&str], id: &str) -> VerifiedDataWorkspace {
        let creation = create_data_workspace(&self.snapshot(names), &self.managed(), id).unwrap();
        assert_eq!(creation.view().state, CreationState::Ready);
        creation.into_verified().unwrap()
    }
}
fn requirements(names: &[&str]) -> Vec<ArtifactRequirement> {
    names
        .iter()
        .enumerate()
        .map(|(i, path)| ArtifactRequirement {
            artifact_id: format!("dataset-{i}"),
            relative_path: path.into(),
            max_bytes: 1024,
            expected_sha256: None,
        })
        .collect()
}
fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().mode() & 0o7777
}
fn assert_no_ready(path: &Path) {
    assert!(!path.join(".baton-data-ready.json").exists());
}

#[test]
fn copies_only_explicit_files_with_private_nonexecuting_outputs_without_source_writes() {
    let f = Fixture::new();
    f.put("data/table.json", br#"[{"x":1}]"#);
    f.put("unused.txt", b"not selected");
    fs::set_permissions(
        f.source.join("data/table.json"),
        Permissions::from_mode(0o755),
    )
    .unwrap();
    let before = fs::metadata(f.source.join("data/table.json")).unwrap();
    let root_before = fs::metadata(&f.source).unwrap();
    let snapshot = f.snapshot(&["data/table.json"]);
    assert!(!snapshot.view().atomic_directory_freeze);
    let result = create_data_workspace(&snapshot, &f.managed(), "normal").unwrap();
    assert_eq!(result.view().copied_files, 1);
    assert!(!result.view().filesystem_sandbox);
    let ready = result.into_verified().unwrap();
    let contract = ready.contract_view();
    let dest = &contract.destination.canonical_root;
    assert_eq!(
        fs::read(dest.join("inputs/data/table.json")).unwrap(),
        br#"[{"x":1}]"#
    );
    assert!(!dest.join("inputs/unused.txt").exists());
    assert!(!f.source.join(".git").exists());
    assert_eq!(mode(dest), 0o700);
    for dir in &contract.directories {
        assert_eq!(mode(&dir.canonical_root), 0o700);
    }
    for file in [
        "inputs/data/table.json",
        ".baton-data-creating.json",
        ".baton-data-ready.json",
    ] {
        assert_eq!(mode(&dest.join(file)), 0o600);
    }
    let after = fs::metadata(f.source.join("data/table.json")).unwrap();
    assert_eq!(
        (
            before.ino(),
            before.mode(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec()
        ),
        (
            after.ino(),
            after.mode(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec()
        )
    );
    let root_after = fs::metadata(&f.source).unwrap();
    assert_eq!(
        (root_before.mtime(), root_before.mtime_nsec()),
        (root_after.mtime(), root_after.mtime_nsec())
    );
    let token = ApprovedRestoreContract::from_verified_host(contract.clone()).unwrap();
    assert_eq!(
        restore_data_workspace(&token, &f.managed())
            .unwrap()
            .contract_view(),
        contract
    );
}

#[test]
fn ready_restore_does_not_require_original_source_after_changes_moves_or_deletion() {
    let f = Fixture::new();
    f.put("a.csv", b"approved bytes");
    let ready = f.ready(&["a.csv"], "history");
    let token = ApprovedRestoreContract::from_verified_host(ready.contract_view()).unwrap();
    f.put("a.csv", b"changed source");
    assert!(restore_data_workspace(&token, &f.managed()).is_ok());
    let moved = f.source.with_file_name("moved-source");
    fs::rename(&f.source, &moved).unwrap();
    assert!(restore_data_workspace(&token, &f.managed()).is_ok());
    fs::remove_dir_all(moved).unwrap();
    let restored = restore_data_workspace(&token, &f.managed()).unwrap();
    assert_eq!(
        fs::read(
            restored
                .contract_view()
                .destination
                .canonical_root
                .join("inputs/a.csv")
        )
        .unwrap(),
        b"approved bytes"
    );
}

#[test]
fn content_revision_is_path_id_and_bytes_bound_not_source_inode_or_approval_label() {
    let f = Fixture::new();
    f.put("a", b"same");
    let original = f.snapshot(&["a"]);
    let g = Fixture::new();
    g.put("a", b"same");
    let other = g.snapshot(&["a"]);
    assert_eq!(original.dataset_revision(), other.dataset_revision());
    assert_ne!(
        original.view().selection_digest,
        other.view().selection_digest
    );
    g.put("a", b"changed");
    assert_ne!(
        original.dataset_revision(),
        g.snapshot(&["a"]).dataset_revision()
    );
    let mut files = requirements(&["a"]);
    files[0].artifact_id = "another-id".into();
    let selection = ApprovedDataSelection::from_verified_host(
        &f.source,
        "another-approval".into(),
        files,
        DataLimits::default(),
    )
    .unwrap();
    assert_ne!(
        original.dataset_revision(),
        verify_dataset(&selection).unwrap().dataset_revision()
    );
    f.put("renamed", b"same");
    assert_ne!(
        original.dataset_revision(),
        f.snapshot(&["renamed"]).dataset_revision()
    );
}

#[test]
fn source_drift_keeps_partial_files_and_failure_evidence_without_overwrite_or_resume() {
    let f = Fixture::new();
    f.put("a", b"first");
    f.put("b", b"second");
    let snapshot = f.snapshot(&["a", "b"]);
    f.put("b", b"changed after approval");
    let result = create_data_workspace(&snapshot, &f.managed(), "partial").unwrap();
    let view = result.view();
    assert_eq!(view.state, CreationState::Failed);
    assert_eq!(view.error, Some(DataWorkspaceError::SourceChanged));
    assert_eq!(view.copied_files, 1);
    assert!(result.verified().is_none());
    assert_eq!(
        fs::read(view.destination.join("inputs/a")).unwrap(),
        b"first"
    );
    assert_no_ready(&view.destination);
    assert_eq!(
        mode(&view.destination.join(".baton-data-failed.json")),
        0o600
    );
    fs::write(view.destination.join("owner-added"), b"must survive").unwrap();
    let restored = inspect_incomplete_creation(&snapshot, &f.managed(), "partial").unwrap();
    assert_eq!(restored.state, CreationState::Failed);
    let repeated = create_data_workspace(&snapshot, &f.managed(), "partial");
    assert_eq!(
        repeated.err(),
        Some(DataWorkspaceError::DestinationConflict)
    );
    assert_eq!(
        fs::read(view.destination.join("owner-added")).unwrap(),
        b"must survive"
    );
}

#[test]
fn same_bytes_replaced_source_file_and_root_identity_are_not_accepted() {
    let f = Fixture::new();
    f.put("a", b"same");
    let snapshot = f.snapshot(&["a"]);
    fs::rename(f.source.join("a"), f.source.join("old-a")).unwrap();
    f.put("a", b"same");
    let view = create_data_workspace(&snapshot, &f.managed(), "inode")
        .unwrap()
        .view();
    assert_eq!(view.state, CreationState::Failed);
    assert_no_ready(&view.destination);
    let snapshot = f.snapshot(&["a"]);
    fs::rename(&f.source, f.source.with_file_name("old-root")).unwrap();
    fs::create_dir(&f.source).unwrap();
    f.put("a", b"same");
    assert_eq!(
        create_data_workspace(&snapshot, &f.managed(), "root").err(),
        Some(DataWorkspaceError::SourceChanged)
    );
    assert!(!f.managed.join("data-root").exists());
}

#[test]
fn managed_root_cannot_be_equal_to_or_inside_source_even_with_private_permissions() {
    let f = Fixture::new();
    f.put("a", b"x");
    fs::set_permissions(&f.source, Permissions::from_mode(0o700)).unwrap();
    let snapshot = f.snapshot(&["a"]);
    let same = ApprovedManagedRoot::from_verified_host(&f.source).unwrap();
    assert_eq!(
        create_data_workspace(&snapshot, &same, "same").err(),
        Some(DataWorkspaceError::ManagedRootInvalid)
    );
    let inner = f.source.join("host-managed");
    fs::create_dir(&inner).unwrap();
    fs::set_permissions(&inner, Permissions::from_mode(0o700)).unwrap();
    let managed = ApprovedManagedRoot::from_verified_host(&inner).unwrap();
    assert_eq!(
        create_data_workspace(&snapshot, &managed, "nested").err(),
        Some(DataWorkspaceError::ManagedRootInvalid)
    );
    assert!(!inner.join("data-nested").exists());
}

#[test]
fn unprivate_or_replaced_managed_root_is_refused_without_chmod() {
    let f = Fixture::new();
    f.put("a", b"x");
    fs::set_permissions(&f.managed, Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        ApprovedManagedRoot::from_verified_host(&f.managed).err(),
        Some(DataWorkspaceError::ManagedRootInvalid)
    );
    assert_eq!(mode(&f.managed), 0o755);
    fs::set_permissions(&f.managed, Permissions::from_mode(0o700)).unwrap();
    let managed = f.managed();
    let snapshot = f.snapshot(&["a"]);
    fs::rename(&f.managed, f.managed.with_file_name("old-managed")).unwrap();
    fs::create_dir(&f.managed).unwrap();
    fs::set_permissions(&f.managed, Permissions::from_mode(0o700)).unwrap();
    assert!(create_data_workspace(&snapshot, &managed, "replaced").is_err());
    assert!(!f.managed.join("data-replaced").exists());
}

#[test]
fn traversal_noncanonical_auth_paths_duplicate_ids_and_case_prefix_aliases_are_rejected() {
    let f = Fixture::new();
    f.put("a", b"x");
    for names in [
        vec!["../a"],
        vec!["/etc/passwd"],
        vec!["./a"],
        vec!["a//b"],
        vec!["a/../b"],
        vec![".ssh/id"],
        vec!["nested/auth.json"],
        vec!["a", "A"],
        vec!["a", "a-b", "a/c"],
        vec!["Dir/a", "dir/b"],
    ] {
        assert!(
            ApprovedDataSelection::from_verified_host(
                &f.source,
                "owner".into(),
                requirements(&names),
                DataLimits::default()
            )
            .is_err(),
            "{names:?}"
        );
    }
    let mut duplicates = requirements(&["a", "b"]);
    duplicates[1].artifact_id = duplicates[0].artifact_id.clone();
    assert!(ApprovedDataSelection::from_verified_host(
        &f.source,
        "owner".into(),
        duplicates,
        DataLimits::default()
    )
    .is_err());
    for id in ["../escape", "", "bad/id", "."] {
        assert_eq!(
            create_data_workspace(&f.snapshot(&["a"]), &f.managed(), id).err(),
            Some(DataWorkspaceError::InvalidSelection)
        );
    }
    assert_eq!(fs::read_dir(&f.managed).unwrap().count(), 0);
}

#[test]
fn roots_reject_symlinks_and_whole_home_or_private_native_auth_directory() {
    let f = Fixture::new();
    let link = f.source.with_file_name("link");
    symlink(&f.source, &link).unwrap();
    assert!(ApprovedDataSelection::from_verified_host(
        &link,
        "owner".into(),
        requirements(&["a"]),
        DataLimits::default()
    )
    .is_err());
    assert!(ApprovedDataSelection::from_verified_host(
        Path::new("/"),
        "owner".into(),
        requirements(&["a"]),
        DataLimits::default()
    )
    .is_err());
    if let Some(home) = std::env::var_os("HOME").and_then(|p| fs::canonicalize(p).ok()) {
        assert_eq!(
            ApprovedDataSelection::from_verified_host(
                &home,
                "owner".into(),
                requirements(&["a"]),
                DataLimits::default()
            )
            .err(),
            Some(DataWorkspaceError::UnauthorizedPath)
        );
    }
    let auth = f.source.join(".codex");
    fs::create_dir(&auth).unwrap();
    assert!(ApprovedDataSelection::from_verified_host(
        &auth,
        "owner".into(),
        requirements(&["auth.json"]),
        DataLimits::default()
    )
    .is_err());
}

#[test]
fn symlink_parent_leaf_hardlink_and_nonregular_directory_are_never_copied() {
    let f = Fixture::new();
    f.put("a", b"x");
    fs::hard_link(f.source.join("a"), f.source.join("hard")).unwrap();
    assert_eq!(
        verify_dataset(&f.selection(&["a"])).err(),
        Some(DataWorkspaceError::UnsafeFile)
    );
    fs::remove_file(f.source.join("hard")).unwrap();
    symlink("a", f.source.join("link")).unwrap();
    assert!(verify_dataset(&f.selection(&["link"])).is_err());
    fs::create_dir(f.source.join("directory")).unwrap();
    assert_eq!(
        verify_dataset(&f.selection(&["directory"])).err(),
        Some(DataWorkspaceError::UnsafeFile)
    );
    symlink(&f.source, f.source.join("alias-parent")).unwrap();
    assert!(verify_dataset(&f.selection(&["alias-parent/a"])).is_err());
}

#[test]
fn fifo_read_is_bounded_in_killable_child() {
    if std::env::var_os("BATON_DATA_FIFO_CHILD").is_some() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let f = Fixture::new();
        let path = f.source.join("fifo");
        let name = CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let start = Instant::now();
        assert_eq!(
            verify_dataset(&f.selection(&["fifo"])).err(),
            Some(DataWorkspaceError::UnsafeFile)
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "fifo_read_is_bounded_in_killable_child",
            "--nocapture",
        ])
        .env("BATON_DATA_FIFO_CHILD", "1")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("FIFO child exceeded bound");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn bounded_file_total_count_and_expected_hash_are_enforced_including_exact_boundary() {
    let f = Fixture::new();
    f.put("a", b"1234");
    f.put("b", b"5678");
    let mut files = requirements(&["a"]);
    files[0].max_bytes = 4;
    files[0].expected_sha256 = Some(sha256(b"1234"));
    let limits = DataLimits {
        max_file_bytes: 4,
        max_total_bytes: 4,
        max_files: 1,
    };
    assert!(verify_dataset(
        &ApprovedDataSelection::from_verified_host(
            &f.source,
            "owner".into(),
            files.clone(),
            limits.clone()
        )
        .unwrap()
    )
    .is_ok());
    files[0].max_bytes = 3;
    assert_eq!(
        verify_dataset(
            &ApprovedDataSelection::from_verified_host(&f.source, "owner".into(), files, limits)
                .unwrap()
        )
        .err(),
        Some(DataWorkspaceError::ResourceLimit)
    );
    let limits = DataLimits {
        max_file_bytes: 1024,
        max_total_bytes: 7,
        max_files: 2,
    };
    assert_eq!(
        verify_dataset(
            &ApprovedDataSelection::from_verified_host(
                &f.source,
                "owner".into(),
                requirements(&["a", "b"]),
                limits
            )
            .unwrap()
        )
        .err(),
        Some(DataWorkspaceError::ResourceLimit)
    );
    let mut files = requirements(&["a"]);
    files[0].expected_sha256 = Some(sha256(b"not approved"));
    assert_eq!(
        verify_dataset(
            &ApprovedDataSelection::from_verified_host(
                &f.source,
                "owner".into(),
                files,
                DataLimits::default()
            )
            .unwrap()
        )
        .err(),
        Some(DataWorkspaceError::SourceChanged)
    );
    assert!(ApprovedDataSelection::from_verified_host(
        &f.source,
        "owner".into(),
        requirements(&["a", "b"]),
        DataLimits {
            max_files: 1,
            ..DataLimits::default()
        }
    )
    .is_err());
    assert!(ApprovedDataSelection::from_verified_host(
        &f.source,
        "owner".into(),
        requirements(&["a"]),
        DataLimits {
            max_files: 65,
            ..DataLimits::default()
        }
    )
    .is_err());
}

#[test]
fn concurrent_same_creation_id_never_overwrites_existing_destination() {
    let f = Fixture::new();
    f.put("a", b"data");
    let snapshot = Arc::new(f.snapshot(&["a"]));
    let managed = Arc::new(f.managed());
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let (snapshot, managed, barrier) = (snapshot.clone(), managed.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                create_data_workspace(&snapshot, &managed, "unique").map(|r| r.view().state)
            })
        })
        .collect();
    let states: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(
        states
            .iter()
            .filter(|r| matches!(r, Ok(CreationState::Ready)))
            .count(),
        1
    );
    assert_eq!(
        states
            .iter()
            .filter(|r| matches!(r, Err(DataWorkspaceError::DestinationConflict)))
            .count(),
        1
    );
    assert_eq!(
        fs::read(f.managed.join("data-unique/inputs/a")).unwrap(),
        b"data"
    );
}

#[test]
fn existing_directory_and_symlink_destinations_are_not_adopted_or_cleaned() {
    let f = Fixture::new();
    f.put("a", b"x");
    let destination = f.managed.join("data-existing");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("owner"), b"keep").unwrap();
    let snapshot = f.snapshot(&["a"]);
    let managed = f.managed();
    assert_eq!(
        create_data_workspace(&snapshot, &managed, "existing").err(),
        Some(DataWorkspaceError::DestinationConflict)
    );
    assert_eq!(fs::read(destination.join("owner")).unwrap(), b"keep");
    symlink(&f.source, f.managed.join("data-link")).unwrap();
    assert_eq!(
        create_data_workspace(&snapshot, &managed, "link").err(),
        Some(DataWorkspaceError::DestinationConflict)
    );
    assert_eq!(fs::read(f.source.join("a")).unwrap(), b"x");
}

#[test]
fn restore_rejects_changed_bytes_permissions_file_identity_or_parent_directory() {
    for mutation in ["bytes", "mode", "inode", "parent", "hardlink"] {
        let f = Fixture::new();
        f.put("sub/a", b"approved");
        let ready = f.ready(&["sub/a"], "tampered");
        let contract = ready.contract_view();
        let token = ApprovedRestoreContract::from_verified_host(contract.clone()).unwrap();
        let path = contract.destination.canonical_root.join("inputs/sub/a");
        match mutation {
            "bytes" => fs::write(&path, b"tampered").unwrap(),
            "mode" => fs::set_permissions(&path, Permissions::from_mode(0o644)).unwrap(),
            "inode" => {
                fs::rename(&path, path.with_file_name("old")).unwrap();
                fs::write(&path, b"approved").unwrap();
                fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
            }
            "parent" => {
                fs::set_permissions(path.parent().unwrap(), Permissions::from_mode(0o755)).unwrap()
            }
            "hardlink" => fs::hard_link(&path, path.with_file_name("alias")).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            restore_data_workspace(&token, &f.managed()).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn restore_rejects_destination_root_replacement_and_marker_tampering_or_oversize() {
    for mutation in ["root", "ready", "intent", "oversize"] {
        let f = Fixture::new();
        f.put("a", b"approved");
        let ready = f.ready(&["a"], "tampered");
        let contract = ready.contract_view();
        let token = ApprovedRestoreContract::from_verified_host(contract.clone()).unwrap();
        let destination = &contract.destination.canonical_root;
        match mutation {
            "root" => {
                fs::rename(destination, destination.with_file_name("old")).unwrap();
                fs::create_dir(destination).unwrap();
                fs::set_permissions(destination, Permissions::from_mode(0o700)).unwrap();
            }
            "ready" => fs::write(destination.join(".baton-data-ready.json"), b"{}").unwrap(),
            "intent" => fs::write(destination.join(".baton-data-creating.json"), b"{}").unwrap(),
            "oversize" => {
                File::options()
                    .write(true)
                    .open(destination.join(".baton-data-ready.json"))
                    .unwrap()
                    .set_len(4 * 1024 * 1024 + 1)
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            restore_data_workspace(&token, &f.managed()).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn disk_ready_label_and_recomputed_untrusted_views_never_create_verified_tokens() {
    let f = Fixture::new();
    f.put("a", b"x");
    let snapshot = f.snapshot(&["a"]);
    let creation = create_data_workspace(&snapshot, &f.managed(), "ready").unwrap();
    let observed = inspect_incomplete_creation(&snapshot, &f.managed(), "ready").unwrap();
    assert_eq!(observed.state, CreationState::OutcomeUnknown);
    let mut forged = creation.into_verified().unwrap().contract_view();
    forged.snapshot.dataset_revision = "a".repeat(64);
    assert_eq!(
        ApprovedRestoreContract::from_verified_host(forged).err(),
        Some(DataWorkspaceError::MetadataInvalid)
    );
}

#[test]
fn selection_bounds_are_checked_before_source_io_or_sorting() {
    let f = Fixture::new();
    let absent = f.source.join("nonexistent");
    let files = (0..65)
        .map(|i| ArtifactRequirement {
            artifact_id: format!("input-{i}"),
            relative_path: format!("file-{i}").into(),
            max_bytes: 100,
            expected_sha256: None,
        })
        .collect();
    assert_eq!(
        ApprovedDataSelection::from_verified_host(
            &absent,
            "owner".into(),
            files,
            DataLimits::default()
        )
        .err(),
        Some(DataWorkspaceError::InvalidSelection)
    );
    assert!(!absent.exists());
}

#[test]
fn managed_and_relative_path_shapes_are_bounded_before_allocation_or_root_io() {
    let f = Fixture::new();
    let control = f.managed.join("control\nroot");
    fs::create_dir(&control).unwrap();
    fs::set_permissions(&control, Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        ApprovedManagedRoot::from_verified_host(&control).err(),
        Some(DataWorkspaceError::ManagedRootInvalid)
    );
    assert_eq!(mode(&control), 0o700);
    assert_eq!(fs::read_dir(&control).unwrap().count(), 0);
    let long = f.managed.join("a".repeat(4097));
    assert_eq!(
        ApprovedManagedRoot::from_verified_host(&long).err(),
        Some(DataWorkspaceError::ManagedRootInvalid)
    );
    for path in [
        "a".repeat(4097),
        "parent/".repeat(17) + "leaf",
        "control\nrelative".into(),
    ] {
        let mut files = requirements(&["unused"]);
        files[0].relative_path = path.into();
        assert_eq!(
            ApprovedDataSelection::from_verified_host(
                &f.source,
                "owner".into(),
                files,
                DataLimits::default()
            )
            .err(),
            Some(DataWorkspaceError::InvalidSelection)
        );
    }
}

#[test]
fn verified_handoff_detects_selected_file_drift_but_allows_unrelated_workspace_outputs() {
    let f = Fixture::new();
    f.put("input", b"fixed");
    let ready = f.ready(&["input"], "handoff");
    ready.recheck_observed_identities().unwrap();
    let destination = ready.contract_view().destination.canonical_root;
    fs::write(
        destination.join("report.txt"),
        b"unrelated permitted output",
    )
    .unwrap();
    ready.recheck_observed_identities().unwrap();
    fs::write(destination.join("inputs/input"), b"changed").unwrap();
    assert_eq!(
        ready.recheck_observed_identities().err(),
        Some(DataWorkspaceError::DestinationChanged)
    );
}
#[test]
fn controlled_preview_copy_cancel_and_elapsed_deadline_are_not_copy_authority() {
    use std::sync::atomic::AtomicBool;
    let f = Fixture::new();
    f.put("input", b"fixed");
    let cancelled = AtomicBool::new(true);
    let control = CopyControl::new(&cancelled, Instant::now() + Duration::from_secs(30));
    assert_eq!(
        verify_dataset_controlled(&f.selection(&["input"]), &control).err(),
        Some(DataWorkspaceError::CancelRequested)
    );
    let snapshot = f.snapshot(&["input"]);
    assert_eq!(
        create_data_workspace_controlled(&snapshot, &f.managed(), "cancelled", &control).err(),
        Some(DataWorkspaceError::CancelRequested)
    );
    let active = AtomicBool::new(false);
    let expired = CopyControl::new(&active, Instant::now());
    assert_eq!(
        create_data_workspace_controlled(&snapshot, &f.managed(), "expired", &expired).err(),
        Some(DataWorkspaceError::DeadlineExceeded)
    );
    assert_eq!(fs::read_dir(&f.managed).unwrap().count(), 0);
}

#[test]
fn maximum_source_depth_remains_valid_after_private_inputs_prefix_without_widening_source_scope() {
    let f = Fixture::new();
    let relative = (0..16)
        .map(|i| format!("p{i}"))
        .collect::<Vec<_>>()
        .join("/");
    f.put(&relative, b"deep fixed dataset");
    let ready = f.ready(&[&relative], "depth-16");
    assert_eq!(
        fs::read(
            ready
                .contract_view()
                .destination
                .canonical_root
                .join("inputs")
                .join(&relative)
        )
        .unwrap(),
        b"deep fixed dataset"
    );
    let invalid = relative.clone() + "/seventeenth";
    assert_eq!(
        ApprovedDataSelection::from_verified_host(
            &f.source,
            "owner".into(),
            requirements(&[&invalid]),
            DataLimits::default()
        )
        .err(),
        Some(DataWorkspaceError::InvalidSelection)
    );
}

#[test]
fn full_internal_target_path_budget_is_rejected_before_reserving_any_directory() {
    let f = Fixture::new();
    let root_bytes = f.source.as_os_str().len();
    let component_bytes = ((4090 - root_bytes - 16) / 16).min(250);
    assert!(component_bytes >= 2);
    let relative = (0..16)
        .map(|i| format!("{i:02}{}", "x".repeat(component_bytes - 2)))
        .collect::<Vec<_>>()
        .join("/");
    f.put(&relative, b"fixed input");
    let snapshot = f.snapshot(&[&relative]);
    let id = "i".repeat(80);
    assert!(
        f.managed
            .join(format!("data-{id}"))
            .join("inputs")
            .join(&relative)
            .as_os_str()
            .len()
            > 4096
    );
    assert_eq!(
        create_data_workspace(&snapshot, &f.managed(), &id).err(),
        Some(DataWorkspaceError::ResourceLimit)
    );
    assert_eq!(fs::read_dir(&f.managed).unwrap().count(), 0);
}

#[test]
fn host_excluded_directory_identity_blocks_source_walk_even_after_rename_into_selected_path() {
    use std::sync::atomic::AtomicBool;
    let f = Fixture::new();
    f.put("selected/marker", b"public original marker");
    let runtime = f.source.join("private-runtime");
    fs::create_dir(&runtime).unwrap();
    fs::write(runtime.join("marker"), b"benign private-runtime marker").unwrap();
    let held = File::open(&runtime).unwrap();
    let metadata = held.metadata().unwrap();
    let selection = f.selection(&["selected/marker"]);
    fs::rename(
        f.source.join("selected"),
        f.source.join("old-public-selected"),
    )
    .unwrap();
    fs::rename(runtime, f.source.join("selected")).unwrap();
    let active = AtomicBool::new(false);
    let control = CopyControl::new(&active, Instant::now() + Duration::from_secs(30))
        .exclude_directory(metadata.dev(), metadata.ino());
    assert_eq!(
        verify_dataset_controlled(&selection, &control).err(),
        Some(DataWorkspaceError::ExcludedSource)
    );
    assert_eq!(
        fs::read(f.source.join("old-public-selected/marker")).unwrap(),
        b"public original marker"
    );
}
