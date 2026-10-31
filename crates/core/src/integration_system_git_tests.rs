//! Policy models and private-file refusal controls. No model proves macOS SSV
//! availability, Apple toolchain behavior or a successful integration runtime.
use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};

fn volume() -> AppleSystemVolume {
    AppleSystemVolume {
        device: 42,
        fsid: 7,
        flags: APPLE_ROOT_FLAGS,
        flags_ext: 0,
        vfs_flags: 1,
        owner: 0,
        fs_type: 24,
        fs_subtype: 0,
        filesystem: b"apfs".to_vec(),
        mountpoint: b"/".to_vec(),
        source: b"/dev/disk3s1s1".to_vec(),
        readonly: true,
    }
}
fn program() -> AppleFileIdentity {
    AppleFileIdentity {
        device: 42,
        inode: 101,
        uid: 0,
        mode: 0o100755,
        links: 78,
        flags: 0,
    }
}
fn apple_path() -> &'static Path {
    Path::new(agentbaton_workspaces::MACOS_SYSTEM_GIT)
}

#[test]
fn system_git_policy_allows_only_the_exact_apple_entry_on_the_protected_root_snapshot() {
    let file = program();
    let volume = volume();
    assert!(apple_git_allowed(apple_path(), &file, &volume));
    for path in [
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
        "/usr/bin/./git",
        "/usr/bin/../bin/git",
        "/usr/bin/git-alias",
        "usr/bin/git",
    ] {
        assert!(
            !apple_git_allowed(Path::new(path), &file, &volume),
            "{path}"
        );
    }
    // Single-link Apple images still need the same protected volume proof.
    let single = AppleFileIdentity { links: 1, ..file };
    assert!(apple_git_allowed(apple_path(), &single, &volume));
    let writable = AppleSystemVolume {
        readonly: false,
        ..volume
    };
    assert!(!apple_git_allowed(apple_path(), &single, &writable));
}

#[test]
fn system_git_policy_refuses_writable_unknown_data_or_non_snapshot_mounts() {
    let mutations: &[(&str, fn(&mut AppleSystemVolume))] = &[
        ("vfs writable", |v| v.readonly = false),
        ("inconsistent vfs flags", |v| v.vfs_flags = 0),
        ("mount writable", |v| v.flags &= !0x0000_0001),
        ("not local", |v| v.flags &= !0x0000_1000),
        ("not rootfs", |v| v.flags &= !0x0000_4000),
        ("not snapshot", |v| v.flags &= !0x4000_0000),
        ("noexec", |v| v.flags |= 0x0000_0004),
        ("ignore ownership", |v| v.flags |= 0x0020_0000),
        ("user mount", |v| v.owner = 501),
        ("wrong filesystem", |v| v.filesystem = b"hfs".to_vec()),
        ("data mount", |v| {
            v.mountpoint = b"/System/Volumes/Data".to_vec()
        }),
        ("other readonly mount", |v| {
            v.mountpoint = b"/Volumes/test".to_vec()
        }),
        ("unknown source", |v| v.source = b"unknown".to_vec()),
        ("network source", |v| v.source = b"server:/disk".to_vec()),
    ];
    for (reason, mutate) in mutations {
        let mut observed = volume();
        mutate(&mut observed);
        assert!(
            !apple_git_allowed(apple_path(), &program(), &observed),
            "{reason}"
        );
    }
}

#[test]
fn system_git_policy_refuses_user_owned_writable_nonregular_or_unlinked_images() {
    let mutations: &[(&str, fn(&mut AppleFileIdentity))] = &[
        ("user owner", |f| f.uid = 501),
        ("group write", |f| f.mode |= 0o020),
        ("world write", |f| f.mode |= 0o002),
        ("nonexecutable", |f| f.mode &= !0o111),
        ("directory", |f| f.mode = 0o040755),
        ("symlink", |f| f.mode = 0o120755),
        ("unlinked", |f| f.links = 0),
        ("different device", |f| f.device = 43),
    ];
    for (reason, mutate) in mutations {
        let mut observed = program();
        mutate(&mut observed);
        assert!(
            !apple_git_allowed(apple_path(), &observed, &volume()),
            "{reason}"
        );
    }
}

#[test]
fn system_git_recheck_requires_identical_link_count_identity_flags_and_mount_proof() {
    let file = program();
    let volume = volume();
    assert!(apple_git_observation_matches(
        apple_path(),
        &file,
        &volume,
        &file,
        &volume
    ));
    for mutate in [
        (|f: &mut AppleFileIdentity| f.links = 1) as fn(&mut AppleFileIdentity),
        |f| f.inode += 1,
        |f| f.flags ^= 1,
        |f| f.mode = 0o100555,
    ] {
        let mut observed = file.clone();
        mutate(&mut observed);
        assert!(!apple_git_observation_matches(
            apple_path(),
            &file,
            &volume,
            &observed,
            &volume
        ));
    }
    let changes: &[fn(&mut AppleSystemVolume)] = &[
        |v| v.fsid += 1,
        |v| v.flags_ext ^= 1,
        |v| v.vfs_flags ^= 2,
        |v| v.fs_type += 1,
        |v| v.fs_subtype += 1,
        |v| v.source = b"/dev/disk4s1s1".to_vec(),
    ];
    for mutate in changes {
        let mut observed = volume.clone();
        mutate(&mut observed);
        assert!(!apple_git_observation_matches(
            apple_path(),
            &file,
            &volume,
            &file,
            &observed
        ));
    }
}

#[test]
fn system_git_ancestors_must_be_root_owned_nofollow_directories_on_the_same_protected_volume() {
    let volume = volume();
    let directory = AppleFileIdentity {
        mode: 0o040755,
        links: 3,
        ..program()
    };
    for path in ["/", "/usr", "/usr/bin"] {
        assert!(apple_ancestor_allowed(
            Path::new(path),
            &directory,
            &volume,
            &volume
        ));
    }
    assert!(!apple_ancestor_allowed(
        Path::new("/usr/local"),
        &directory,
        &volume,
        &volume
    ));
    for changed in [
        AppleFileIdentity {
            uid: 501,
            ..directory.clone()
        },
        AppleFileIdentity {
            mode: 0o040775,
            ..directory.clone()
        },
        AppleFileIdentity {
            mode: 0o120755,
            ..directory.clone()
        },
    ] {
        assert!(!apple_ancestor_allowed(
            Path::new("/usr"),
            &changed,
            &volume,
            &volume
        ));
    }
    let other = AppleSystemVolume {
        fsid: 8,
        ..volume.clone()
    };
    assert!(!apple_ancestor_allowed(
        Path::new("/usr"),
        &directory,
        &other,
        &volume
    ));
}

#[test]
fn generic_leaf_keeps_single_link_and_symlink_refusals_and_detects_link_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .canonicalize()
        .unwrap()
        .join("ordinary-file");
    fs::write(&path, b"fixed private bytes\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let pin = LeafPin::capture(&path).unwrap();
    pin.recheck().unwrap();
    let peer = path.with_file_name("ordinary-hardlink");
    fs::hard_link(&path, &peer).unwrap();
    assert!(pin.recheck().is_err());
    assert!(LeafPin::capture(&path).is_err());
    let alias = path.with_file_name("ordinary-symlink");
    symlink(&path, &alias).unwrap();
    assert!(LeafPin::capture(&alias).is_err());
    assert!(LeafPin::capture(path.parent().unwrap()).is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_mount_bits_match_the_locked_native_api_constants() {
    assert_eq!(
        APPLE_VFS_READONLY,
        rustix::fs::StatVfsMountFlags::RDONLY.bits()
    );
    assert_eq!(
        APPLE_ROOT_FLAGS,
        (libc::MNT_RDONLY | libc::MNT_LOCAL | libc::MNT_ROOTFS | libc::MNT_SNAPSHOT) as u32
    );
    assert_eq!(
        APPLE_FORBIDDEN_FLAGS,
        (libc::MNT_NOEXEC | libc::MNT_IGNORE_OWNERSHIP) as u32
    );
}
