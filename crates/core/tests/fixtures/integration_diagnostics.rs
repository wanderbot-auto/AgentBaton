//! Test-only, read-only observations. These do not select Git, authorize work,
//! change assertions, or establish a platform pass. Never emit IPC message,
//! params, tokens, successful replies, pointer contents or OS error text.
use serde_json::{json, Value};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

// Only this fixture's fixed methods can appear in output. Unknown input is a
// constant, even if the caller supplied a long string containing credentials.
pub(super) fn method(value: &Value) -> &'static str {
    match value.as_str() {
        Some("approvals.finalize") => "approvals.finalize",
        Some("command_validation_policies.list") => "command_validation_policies.list",
        Some("command_validation_policies.propose") => "command_validation_policies.propose",
        Some("connections.list") => "connections.list",
        Some("connections.propose") => "connections.propose",
        Some("goal_graph_runs.get") => "goal_graph_runs.get",
        Some("goal_graph_runs.propose") => "goal_graph_runs.propose",
        Some("goal_node_approvals.propose") => "goal_node_approvals.propose",
        Some("goal_plans.propose") => "goal_plans.propose",
        Some("goal_scopes.propose") => "goal_scopes.propose",
        Some("grants.list") => "grants.list",
        Some("grants.propose") => "grants.propose",
        Some("integration_context.get") => "integration_context.get",
        Some("integration_policies.get") => "integration_policies.get",
        Some("integration_policies.list") => "integration_policies.list",
        Some("integration_policies.propose") => "integration_policies.propose",
        Some("integration_policies.revoke") => "integration_policies.revoke",
        Some("integration_targets.list") => "integration_targets.list",
        Some("integrations.cancel") => "integrations.cancel",
        Some("integrations.get") => "integrations.get",
        Some("integrations.list") => "integrations.list",
        Some("integrations.start") => "integrations.start",
        Some("projects.import") => "projects.import",
        Some("sessions.get") => "sessions.get",
        Some("system.snapshot") => "system.snapshot",
        Some("tasks.create") => "tasks.create",
        Some("tasks.get") => "tasks.get",
        Some("workspaces.list") => "workspaces.list",
        Some("workspaces.prepare_task_git") => "workspaces.prepare_task_git",
        Some("workspaces.resolve") => "workspaces.resolve",
        _ => "other",
    }
}

fn code(value: &Value) -> &'static str {
    match value.as_str() {
        Some("AUTH_REQUIRED") => "AUTH_REQUIRED",
        Some("BUDGET_UNKNOWN") => "BUDGET_UNKNOWN",
        Some("CANCEL_REQUESTED") => "CANCEL_REQUESTED",
        Some("CAPABILITY_UNSUPPORTED") => "CAPABILITY_UNSUPPORTED",
        Some("CHILD_CANCELLED") => "CHILD_CANCELLED",
        Some("CONFIG_CONFLICT") => "CONFIG_CONFLICT",
        Some("DEADLINE_EXCEEDED") => "DEADLINE_EXCEEDED",
        Some("ENFORCEMENT_UNAVAILABLE") => "ENFORCEMENT_UNAVAILABLE",
        Some("INPUT_CHANGED") => "INPUT_CHANGED",
        Some("INTERNAL_ERROR") => "INTERNAL_ERROR",
        Some("INVALID_REQUEST") => "INVALID_REQUEST",
        Some("NOT_FOUND") => "NOT_FOUND",
        Some("OUTCOME_UNKNOWN") => "OUTCOME_UNKNOWN",
        Some("PERMISSION_DENIED") => "PERMISSION_DENIED",
        Some("RESOURCE_LIMIT") => "RESOURCE_LIMIT",
        Some("STALE_REVISION") => "STALE_REVISION",
        Some("TIMEOUT") => "TIMEOUT",
        Some("VALIDATION_FAILED") => "VALIDATION_FAILED",
        Some("VERSION_UNSUPPORTED") => "VERSION_UNSUPPORTED",
        Some("WORKSPACE_BUSY") => "WORKSPACE_BUSY",
        _ => "other",
    }
}

fn details(value: &Value) -> Option<&'static str> {
    if value.is_null() {
        return None;
    }
    // The production integration reader already emits these fixed tags. Never
    // copy an arbitrary details_ref (which may be an identity or caller text).
    const PHASES: &[&str] = &[
        "integration-read-phase:worker-original-control",
        "integration-read-phase:worker-database-open",
        "integration-read-phase:worker-row-recheck",
        "integration-read-phase:worker-scope-material",
        "integration-read-phase:worker-validator-catalog",
        "integration-read-phase:worker-material-consistency",
        "integration-read-phase:worker-tree-observation",
        "integration-read-phase:worker-system-git-pin",
        "integration-read-phase:worker-original-grant",
        "integration-read-phase:worker-connection-program",
        "integration-read-phase:worker-grant-workspace-inspection",
        "integration-read-phase:worker-project-inspection",
        "integration-read-phase:worker-source-inspection",
        "integration-read-phase:worker-input-binding",
        "integration-read-phase:worker-validator-file-binding",
        "integration-read-phase:worker-target-inspection",
        "integration-read-phase:worker-final-observation",
        "integration-read-phase:worker-final-database-pin",
        "integration-read-phase:worker-context-envelope",
        "integration-read-phase:worker-final-identity",
    ];
    let Some(text) = value.as_str().filter(|text| text.len() <= 96) else {
        return Some("other");
    };
    Some(
        PHASES
            .iter()
            .copied()
            .find(|phase| *phase == text)
            .unwrap_or("other"),
    )
}

fn error_projection(method: &'static str, error: &Value) -> Value {
    let effect = match error["side_effect_state"].as_str() {
        Some("not_started") => "not_started",
        Some("confirmed") => "confirmed",
        Some("unknown") => "unknown",
        _ => "other",
    };
    json!({
        "method":method, "code":code(&error["code"]),
        "retryable":error["retryable"].as_bool(), "side_effect_state":effect,
        "details_ref":details(&error["details_ref"])
    })
}

pub(super) fn ipc_error(method: &'static str, error: &Value) {
    eprintln!("INTEGRATION_IPC_ERROR {}", error_projection(method, error));
}

fn kind(metadata: &Metadata) -> &'static str {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        "symlink"
    } else if file_type.is_file() {
        "file"
    } else if file_type.is_dir() {
        "directory"
    } else {
        "other"
    }
}

fn metadata(path: &Path, follow: bool) -> Value {
    let observed = if follow {
        fs::metadata(path)
    } else {
        fs::symlink_metadata(path)
    };
    match observed {
        Ok(m) => json!({"status":"observed", "type":kind(&m), "device":m.dev(),
            "inode":m.ino(), "uid":m.uid(), "gid":m.gid(), "nlink":m.nlink(), "mode":m.mode()}),
        Err(error) => json!({"status":"unavailable", "os_code":error.raw_os_error()}),
    }
}

fn system_path(path: &Path) -> Option<&str> {
    // Paths are the only non-enumerated text in the environment observation.
    // Restrict them to bounded ASCII system executable trees; never emit home,
    // repository, TempDir names, raw PATH, environment entries or OS messages.
    const ROOTS: &[&str] = &[
        "/usr",
        "/bin",
        "/sbin",
        "/opt/homebrew",
        "/opt/hostedtoolcache",
        "/Library/Developer",
        "/Applications/Xcode.app",
    ];
    let text = path.to_str()?;
    if text.len() > 512
        || !text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._+-".contains(&b))
        || path.components().any(|part| {
            !matches!(
                part,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
        || !ROOTS.iter().any(|root| path.starts_with(root))
    {
        return None;
    }
    Some(text)
}

fn executable(path: &Path) -> Value {
    let canonical = path.canonicalize().ok();
    json!({"candidate":system_path(path), "canonical":canonical.as_deref().and_then(system_path),
        "canonicalized":canonical.is_some(), "candidate_metadata":metadata(path, false),
        "canonical_metadata":canonical.as_deref().map(|path| metadata(path, false)),
        "effective_metadata":metadata(path, true)})
}

fn first_path_git() -> Value {
    let Some(path) = std::env::var_os("PATH") else {
        return json!({"status":"path_missing"});
    };
    // Mirror the production *observation*, without spawning any candidate,
    // changing PATH or substituting /usr/bin/git. Keep observation work finite.
    for (index, directory) in std::env::split_paths(&path).take(129).enumerate() {
        if index == 128 {
            return json!({"status":"path_component_limit"});
        }
        if !directory.is_absolute() {
            return json!({"status":"relative_directory", "index":index});
        }
        let candidate = directory.join("git");
        match fs::metadata(&candidate) {
            Ok(m) if m.is_file() && m.mode() & 0o111 != 0 => {
                return json!({"status":"selected", "index":index, "program":executable(&candidate)});
            }
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => {
                return json!({"status":"metadata_error", "index":index, "os_code":error.raw_os_error()});
            }
        }
    }
    json!({"status":"not_found"})
}

pub(super) fn git_environment() {
    eprintln!(
        "INTEGRATION_GIT_METADATA {}",
        json!({"path_git":first_path_git(), "usr_bin_git":executable(Path::new("/usr/bin/git"))})
    );
}

fn git_pointer(root: &Path) -> Value {
    let path = root.join(".git");
    let mut result = json!({"entry":metadata(&path, false), "pointer_read":false,
        "pointer_bounded":null, "pointer_format":null, "pointer_absolute":null,
        "target_canonicalized":null, "target_spelling_canonical":null});
    if !fs::symlink_metadata(&path).is_ok_and(|m| m.is_file()) {
        return result;
    }
    let file: Result<File, _> = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(&path);
    let Ok(file) = file else {
        return result;
    };
    let mut bytes = Vec::new();
    if file.take(4097).read_to_end(&mut bytes).is_err() {
        return result;
    }
    result["pointer_read"] = json!(true);
    result["pointer_bounded"] = json!(bytes.len() <= 4096);
    if bytes.len() > 4096 {
        return result;
    }
    let pointer = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|s| s.strip_prefix("gitdir: "))
        .map(|s| s.trim_end_matches(['\r', '\n']))
        .filter(|s| !s.is_empty() && !s.chars().any(char::is_control));
    result["pointer_format"] = json!(pointer.is_some());
    if let Some(pointer) = pointer {
        let pointer = Path::new(pointer);
        result["pointer_absolute"] = json!(pointer.is_absolute());
        let target = root.join(pointer);
        let canonical = target.canonicalize().ok();
        result["target_canonicalized"] = json!(canonical.is_some());
        result["target_spelling_canonical"] = json!(canonical.as_ref() == Some(&target));
    }
    result
}

fn root_projection(root: &Path) -> Value {
    let canonical = root.canonicalize().ok();
    json!({"canonicalized":canonical.is_some(), "spelling_canonical":canonical.as_deref()==Some(root),
        "entry":metadata(root, false), "git":git_pointer(root)})
}

pub(super) fn fixture(stage: &'static str, root: &Path, project: &Path, managed: Option<&Path>) {
    let stage = match stage {
        "preview" => "preview",
        "runtime" => "runtime",
        "runtime-managed" => "runtime-managed",
        _ => "other",
    };
    eprintln!(
        "INTEGRATION_FIXTURE_METADATA {}",
        json!({"stage":stage, "root":root_projection(root),
        "project":root_projection(project), "managed":managed.map(root_projection)})
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_projection_preserves_fixed_fields_without_payload_or_messages() {
        let error = json!({"code":"INPUT_CHANGED", "retryable":false,
            "side_effect_state":"not_started", "details_ref":"integration-read-phase:worker-system-git-pin",
            "message":"SECRET-MESSAGE", "token":"SECRET-TOKEN", "params":{"secret":"SECRET-PARAM"}});
        let projected = error_projection(method(&json!("integration_context.get")), &error);
        assert_eq!(
            projected,
            json!({"method":"integration_context.get", "code":"INPUT_CHANGED",
            "retryable":false, "side_effect_state":"not_started", "details_ref":"integration-read-phase:worker-system-git-pin"})
        );
        assert!(!projected.to_string().contains("SECRET"));
    }

    #[test]
    fn unknown_and_oversize_error_text_is_never_copied() {
        let secret = "SECRET".repeat(100_000);
        let error = json!({"code":secret, "retryable":"SECRET", "side_effect_state":secret,
            "details_ref":secret, "message":secret});
        let projected = error_projection(method(&json!(secret)), &error);
        assert_eq!(
            projected,
            json!({"method":"other", "code":"other", "retryable":null,
            "side_effect_state":"other", "details_ref":"other"})
        );
        assert!(projected.to_string().len() < 160);
        assert_eq!(details(&Value::Null), None);
    }

    #[test]
    fn phase_prefix_alone_never_admits_arbitrary_details() {
        assert_eq!(
            details(&json!("integration-read-phase:SECRET")),
            Some("other")
        );
        assert_eq!(
            details(&json!(
                "integration-read-phase:worker-tree-observation\nSECRET"
            )),
            Some("other")
        );
        assert_eq!(details(&json!({"secret":"SECRET"})), Some("other"));
    }

    #[test]
    fn path_projection_is_bounded_and_restricts_system_trees() {
        assert_eq!(
            system_path(Path::new("/opt/homebrew/bin/git")),
            Some("/opt/homebrew/bin/git")
        );
        assert_eq!(system_path(Path::new("/usr/bin/git")), Some("/usr/bin/git"));
        for path in [
            "/Users/SECRET/bin/git",
            "/tmp/SECRET/git",
            "/usr-secret/git",
            "/usr/../Users/SECRET/git",
            "/usr/bin/git\nSECRET",
            "/usr/bin/git?SECRET",
        ] {
            assert_eq!(system_path(Path::new(path)), None);
        }
        assert_eq!(
            system_path(Path::new(&format!("/usr/{}", "x".repeat(512)))),
            None
        );
    }
}
