//! Pure child-command construction; never spawn tools or change process env.
use super::*;

#[test]
fn apple_git_environment_policy_removes_tool_selectors_and_loader_injection() {
    for name in [
        "GIT_DIR",
        "GIT_CONFIG_COUNT",
        "DEVELOPER_DIR",
        "SDKROOT",
        "TOOLCHAINS",
        "DT_TOOLCHAIN_DIR",
        "XCODE_DEVELOPER_DIR_PATH",
        "__XPC_DEVELOPER_DIR",
        "XCRUN_TOOLCHAIN",
        "xcrun_sdk",
        "DYLD_INSERT_LIBRARIES",
        "_DYLD_SHARED_CACHE_DIR",
        "__XPC_DYLD_LIBRARY_PATH",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
    ] {
        assert!(git_environment_selector(OsStr::new(name), true), "{name}");
    }
    for name in ["HOME", "LANG", "TMPDIR", "PATH", "GITHUB_ACTIONS"] {
        assert!(!git_environment_selector(OsStr::new(name), true), "{name}");
    }
}

#[test]
fn apple_command_uses_only_the_safe_system_path_without_mutating_global_environment() {
    let mut command = Command::new(MACOS_SYSTEM_GIT);
    command
        .env("DEVELOPER_DIR", "/untrusted-developer")
        .env("SDKROOT", "/untrusted-sdk")
        .env("TOOLCHAINS", "untrusted-toolchain")
        .env("DYLD_INSERT_LIBRARIES", "/untrusted-library")
        .env("PATH", "/opt/homebrew/bin:/untrusted-path");
    configure_git_environment_for_platform(&mut command, true);
    let entries: std::collections::HashMap<_, _> = command.get_envs().collect();
    for name in [
        "DEVELOPER_DIR",
        "SDKROOT",
        "TOOLCHAINS",
        "DYLD_INSERT_LIBRARIES",
    ] {
        assert_eq!(entries.get(OsStr::new(name)), Some(&None));
    }
    assert_eq!(
        entries.get(OsStr::new("PATH")),
        Some(&Some(OsStr::new("/usr/bin:/bin:/usr/sbin:/sbin")))
    );
    assert_eq!(command.get_program(), OsStr::new("/usr/bin/git"));
}

#[test]
fn nonapple_command_retains_its_original_path_policy_and_only_strips_git_selectors() {
    for name in [
        "DEVELOPER_DIR",
        "SDKROOT",
        "TOOLCHAINS",
        "DYLD_INSERT_LIBRARIES",
        "LD_PRELOAD",
        "PATH",
    ] {
        assert!(!git_environment_selector(OsStr::new(name), false));
    }
    assert!(git_environment_selector(OsStr::new("GIT_DIR"), false));
    let mut command = Command::new("git");
    command
        .env("PATH", "/same-original-path")
        .env("DEVELOPER_DIR", "/same-original-developer")
        .env("GIT_DIR", "/must-be-removed");
    configure_git_environment_for_platform(&mut command, false);
    let entries: std::collections::HashMap<_, _> = command.get_envs().collect();
    assert_eq!(
        entries.get(OsStr::new("PATH")),
        Some(&Some(OsStr::new("/same-original-path")))
    );
    assert_eq!(
        entries.get(OsStr::new("DEVELOPER_DIR")),
        Some(&Some(OsStr::new("/same-original-developer")))
    );
    assert_eq!(entries.get(OsStr::new("GIT_DIR")), Some(&None));
}

#[test]
fn actual_workspace_builder_uses_the_platform_program_with_original_local_argv_guards() {
    let command = git_command(Path::new("/private-generated-root"));
    #[cfg(target_os = "macos")]
    assert_eq!(command.get_program(), OsStr::new(MACOS_SYSTEM_GIT));
    #[cfg(not(target_os = "macos"))]
    assert_eq!(command.get_program(), OsStr::new("git"));
    let args: Vec<_> = command.get_args().collect();
    assert!(args.contains(&OsStr::new("--no-optional-locks")));
    for guard in [
        "core.hooksPath=/dev/null",
        "protocol.allow=never",
        "protocol.file.allow=never",
        "core.fsmonitor=false",
    ] {
        assert!(args.contains(&OsStr::new(guard)));
    }
}
