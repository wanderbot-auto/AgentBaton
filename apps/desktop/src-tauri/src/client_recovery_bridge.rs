//! Durable, Runtime-scoped recovery of exactly two creation operations. The
//! renderer selects an entry ID; keys/bindings come from the private journal.
use agentbaton_core::{
    client_recovery_journal::{
        ClientRecoveryJournal, ClientRecoveryMarker, ManagedCreationBinding, ManagedCreationKind,
        ManagedCreationOperation,
    },
    sessions::{
        ManagedMcpOperation, ManagedMcpSelection, ManagedSkillOperation, ManagedSkillSelection,
    },
    CoreError,
};
use serde_json::{json, Value};
use std::path::Path;
type WireResult = Result<Value, String>;
fn error(error: CoreError) -> Value {
    json!({"request_id":null,"error":error,"revision":0,"warnings":[]})
}
fn uncertain(marker: &ClientRecoveryMarker, confirmed: bool) -> Value {
    let mut e=CoreError::new("OUTCOME_UNKNOWN","The original managed creation needs recovery; open Settings → Creation recovery and use its original record");
    e.side_effect_state = if confirmed { "confirmed" } else { "unknown" }.into();
    e.details_ref = Some(format!("client-recovery:{}", marker.entry_id));
    error(e)
}
fn exact(value: &Value, fields: &[&str]) -> bool {
    value.as_object().is_some_and(|row| {
        row.len() == fields.len() && fields.iter().all(|key| row.contains_key(*key))
    })
}
fn hash(value: &Value) -> bool {
    value.as_str().is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn id(value: &Value) -> bool {
    value.as_str().is_some_and(|v| {
        !v.is_empty()
            && v.len() <= 256
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
    })
}
fn revision(value: &Value) -> bool {
    value
        .as_u64()
        .is_some_and(|v| v > 0 && v <= 9_007_199_254_740_991)
}
fn binding(
    kind: ManagedCreationKind,
    selection: &Value,
    key: &str,
) -> Result<ManagedCreationBinding, CoreError> {
    let invalid = || {
        CoreError::new(
            "INVALID_REQUEST",
            "Managed creation selection is invalid; nothing was dispatched",
        )
    };
    let (base, rev, digest, operation) = match kind {
        ManagedCreationKind::ManagedMcp => {
            let s: ManagedMcpSelection =
                serde_json::from_value(selection.clone()).map_err(|_| invalid())?;
            (
                s.request.base_connection_id,
                s.request.expected_base_revision,
                s.expected_preview_digest,
                match s.request.operation {
                    ManagedMcpOperation::Replace => ManagedCreationOperation::Replace,
                    ManagedMcpOperation::Remove => ManagedCreationOperation::Remove,
                },
            )
        }
        ManagedCreationKind::ManagedSkill => {
            let s: ManagedSkillSelection =
                serde_json::from_value(selection.clone()).map_err(|_| invalid())?;
            (
                s.request.base_connection_id,
                s.request.expected_base_revision,
                s.expected_preview_digest,
                match s.request.operation {
                    ManagedSkillOperation::Replace => ManagedCreationOperation::Replace,
                    ManagedSkillOperation::Remove => ManagedCreationOperation::Remove,
                },
            )
        }
    };
    Ok(ManagedCreationBinding {
        kind,
        request_key: key.into(),
        preview_digest: digest,
        base_connection_id: base,
        base_connection_revision: rev,
        operation,
    })
}
/// Reserve durably BEFORE operation() can read/propose/confirm anything in Core.
pub(crate) fn create(
    runtime: &Path,
    kind: ManagedCreationKind,
    selection: &Value,
    key: &str,
    operation: impl FnOnce() -> WireResult,
    mut core: impl FnMut(Value) -> WireResult,
) -> WireResult {
    let journal = match ClientRecoveryJournal::new(runtime) {
        Ok(j) => j,
        Err(e) => return Ok(error(e)),
    };
    let binding = match binding(kind, selection, key) {
        Ok(b) => b,
        Err(e) => return Ok(error(e)),
    };
    let marker = match journal.reserve(binding) {
        Ok(m) => m,
        Err(e) => return Ok(error(e)),
    };
    let response = match operation() {
        Ok(r) => r,
        Err(_) => return Ok(uncertain(&marker, false)),
    };
    if response.get("error").is_none() {
        if !creation_matches(&response, &marker) {
            return Ok(uncertain(&marker, false));
        }
        return Ok(clear_terminal(&journal, &marker, response));
    }
    // Never infer a whole-operation no-start from transport/timeout/ErrString,
    // a missing result, probe scheduling or a marker that merely exists.
    let failure = &response["error"];
    let definite = exact(
        failure,
        &[
            "code",
            "message",
            "retryable",
            "side_effect_state",
            "details_ref",
        ],
    ) && serde_json::from_value::<CoreError>(failure.clone())
        .ok()
        .is_some_and(|e| {
            e.code != "OUTCOME_UNKNOWN"
                && matches!(e.side_effect_state.as_str(), "none" | "not_started")
        });
    if !definite {
        return Ok(response);
    }
    // Seal the original failed/cancelled intent before allowing another key.
    // A concurrent commit wins and must not be erased or called not-started.
    let sealed = match core(json!({"method":"managed_creations.abandon","params":marker})) {
        Ok(r) => r,
        Err(_) => return Ok(uncertain(&marker, false)),
    };
    match outcome_state(&sealed, &marker) {
        Ok("abandoned") => match journal.clear(&marker) {
            Ok(_) => Ok(response),
            Err(_) => Ok(uncertain(&marker, false)),
        },
        Ok("confirmed") => Ok(uncertain(&marker, true)),
        _ => Ok(uncertain(&marker, false)),
    }
}
fn creation_matches(response: &Value, marker: &ClientRecoveryMarker) -> bool {
    let r = &response["result"];
    let c = &r["connection"];
    let p = &r["profile"];
    exact(
        r,
        &[
            "connection",
            "profile",
            "request_key",
            "preview_digest",
            "base_connection_id",
            "base_connection_revision",
            "operation",
        ],
    ) && r["request_key"] == marker.request_key
        && r["preview_digest"] == marker.preview_digest
        && r["base_connection_id"] == marker.base_connection_id
        && r["base_connection_revision"] == marker.base_connection_revision
        && r["operation"] == json!(marker.operation)
        && id(&c["id"])
        && c["id"] != marker.base_connection_id
        && revision(&c["revision"])
        && p["connection_id"] == c["id"]
        && p["connection_revision"] == c["revision"]
        && p["config_revision"] == c["config_revision"]
        && hash(&p["runtime_plan_digest"])
        && match marker.kind {
            ManagedCreationKind::ManagedMcp => p.get("managed_mcp").is_some(),
            ManagedCreationKind::ManagedSkill => p.get("managed_skill").is_some(),
        }
}
pub(crate) fn list(runtime: &Path) -> WireResult {
    Ok(
        match ClientRecoveryJournal::new(runtime).and_then(|j| j.list()) {
            Ok(entries) => json!({"result":{"entries":entries},"revision":0,"warnings":[]}),
            Err(e) => error(e),
        },
    )
}
pub(crate) fn captured(runtime: &Path, entry_id: &str) -> Result<ClientRecoveryMarker, CoreError> {
    ClientRecoveryJournal::new(runtime)?
        .list()?
        .into_iter()
        .find(|m| m.entry_id == entry_id)
        .ok_or_else(|| {
            CoreError::new(
                "NOT_FOUND",
                "This recovery entry is no longer present; refresh the original recovery list",
            )
        })
}
pub(crate) fn still_current(
    runtime: &Path,
    marker: &ClientRecoveryMarker,
) -> Result<(), CoreError> {
    let current = captured(runtime, &marker.entry_id)?;
    if current != *marker {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Recovery entry changed while Native review was open; refresh the original record",
        ));
    }
    Ok(())
}
pub(crate) fn get(
    runtime: &Path,
    entry_id: &str,
    mut core: impl FnMut(Value) -> WireResult,
) -> WireResult {
    let marker = match captured(runtime, entry_id) {
        Ok(m) => m,
        Err(e) => return Ok(error(e)),
    };
    let response = core(json!({"method":"managed_creations.get","params":marker}))?;
    reconcile_response(runtime, &marker, response)
}
/// Only an authenticated Core result can settle this captured journal entry.
pub(crate) fn reconcile_response(
    runtime: &Path,
    marker: &ClientRecoveryMarker,
    response: Value,
) -> WireResult {
    if response.get("error").is_some() {
        return Ok(response);
    }
    let state = match outcome_state(&response, marker) {
        Ok(state) => state,
        Err(_) => return Ok(uncertain(marker, false)),
    };
    if state == "unresolved" {
        return Ok(response);
    }
    let journal = match ClientRecoveryJournal::new(runtime) {
        Ok(j) => j,
        Err(_) => return Ok(with_warning(response)),
    };
    Ok(clear_terminal(&journal, marker, response))
}
fn with_warning(mut response: Value) -> Value {
    let text=json!("The known outcome is preserved, but its private recovery entry could not be cleared; refresh Creation recovery before another creation");
    if let Some(warnings) = response.get_mut("warnings").and_then(Value::as_array_mut) {
        warnings.push(text);
    } else {
        response["warnings"] = json!([text]);
    }
    response
}
fn clear_terminal(
    journal: &ClientRecoveryJournal,
    marker: &ClientRecoveryMarker,
    response: Value,
) -> Value {
    match journal.clear(marker) {
        Ok(_) => response,
        Err(_) => with_warning(response),
    }
}
pub(crate) fn outcome_state<'a>(
    response: &'a Value,
    marker: &ClientRecoveryMarker,
) -> Result<&'a str, String> {
    let r = &response["result"];
    if response.get("error").is_some()
        || !exact(r, &["marker", "state", "connection"])
        || r["marker"] != json!(marker)
    {
        return Err("Recovery result is not bound to the original entry".into());
    }
    match r["state"].as_str() {
        Some("unresolved" | "abandoned") if r["connection"].is_null() => {
            Ok(r["state"].as_str().expect("matched state"))
        }
        Some("confirmed") => {
            let c = &r["connection"];
            if !exact(
                c,
                &[
                    "id",
                    "name",
                    "provider",
                    "protocol_route",
                    "config_revision",
                    "revision",
                    "probe_state",
                    "runtime_plan_digest",
                ],
            ) || !id(&c["id"])
                || c["id"] == marker.base_connection_id
                || !revision(&c["revision"])
                || !id(&c["config_revision"])
                || !hash(&c["runtime_plan_digest"])
                || c["provider"] != "codex"
                || !matches!(
                    c["protocol_route"].as_str(),
                    Some("codex_app_server" | "codex_acp_adapter")
                )
                || marker.kind == ManagedCreationKind::ManagedMcp
                    && c["protocol_route"] != "codex_app_server"
                || !matches!(
                    c["probe_state"].as_str(),
                    Some("pending" | "ready" | "failed")
                )
                || !c["name"].as_str().is_some_and(|name| {
                    !name.trim().is_empty()
                        && name.len() <= 128
                        && !name.chars().any(char::is_control)
                })
            {
                return Err("Recovery connection binding is unsupported".into());
            }
            Ok("confirmed")
        }
        _ => Err("Recovery state is unsupported".into()),
    }
}
/// Native review is cancellation only. It never recreates/finalizes a connection
/// and never runs a version probe, task, model, MCP program or installer.
pub(crate) fn confirm_abandon(
    marker: &ClientRecoveryMarker,
    response: Value,
    confirm: impl FnOnce(&str) -> bool,
    finalize: impl FnOnce(Value) -> WireResult,
) -> WireResult {
    if response.get("error").is_some() {
        return Ok(response);
    }
    if outcome_state(&response, marker).ok() != Some("unresolved") {
        return Ok(uncertain(marker, false));
    }
    let rendered = serde_json::to_string_pretty(marker)
        .map_err(|_| "Recovery identity could not be rendered".to_owned())?;
    if rendered.len() > 4096 {
        return Ok(uncertain(marker, false));
    }
    let message=format!("Abandon ONLY this original pending managed connection creation. Permanently seal its exact kind/key/binding against any late proposal or confirmation. If creation already committed concurrently, the server returns that connection instead; it is not deleted, stopped or rolled back. Another attempt requires a NEW preview, key and separate Native approval. No Task/Grant/budget change or program execution. Cancel keeps the unresolved entry.\n\n仅取消以下原创建请求：永久封存原键，阻止迟到提案或确认。若连接已提交则返回已有结果，不删除连接、不停止探测或任务、不撤销外部写入。新的创建须重新预览并确认；取消此对话保留原未决记录。\n\n{rendered}");
    if !confirm(&message) {
        return Ok(error(CoreError::new(
            "PERMISSION_DENIED",
            "Native cancellation was declined; the original recovery entry remains unresolved",
        )));
    }
    finalize(json!({"method":"managed_creations.abandon","params":marker}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::PathBuf,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    const KEY: &str = "8e883b9c-4a27-432d-a793-723a44db1dce";
    const NEXT: &str = "c375fe56-efc4-427f-8290-1928107404ed";
    fn fixture() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "baton-creation-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        p
    }
    fn selection() -> Value {
        json!({"request":{"base_connection_id":"base-connection","expected_base_revision":2,"new_name":"DO-NOT-PERSIST-NAME","operation":"replace","server":{"name":"DO-NOT-PERSIST-TOOL","executable":"/DO-NOT-PERSIST-PATH","enabled_tools":["DO_NOT_PERSIST_TOOL"]}},"expected_preview_digest":"a".repeat(64)})
    }
    fn entry(root: &Path) -> ClientRecoveryMarker {
        ClientRecoveryJournal::new(root)
            .unwrap()
            .list()
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }
    fn response(marker: &ClientRecoveryMarker, state: &str) -> Value {
        let c = if state == "confirmed" {
            json!({"id":"new-connection","name":"Recovered connection","provider":"codex","protocol_route":"codex_app_server","config_revision":"configuration","revision":1,"probe_state":"pending","runtime_plan_digest":"b".repeat(64)})
        } else {
            Value::Null
        };
        json!({"result":{"marker":marker,"state":state,"connection":c},"warnings":[]})
    }
    fn success(marker: &ClientRecoveryMarker) -> Value {
        json!({"result":{"connection":{"id":"new-connection","revision":1,"config_revision":"configuration"},"profile":{"connection_id":"new-connection","connection_revision":1,"config_revision":"configuration","runtime_plan_digest":"b".repeat(64),"managed_mcp":null},"request_key":marker.request_key,"preview_digest":marker.preview_digest,"base_connection_id":marker.base_connection_id,"base_connection_revision":marker.base_connection_revision,"operation":marker.operation},"warnings":[]})
    }
    fn reserve(root: &Path, key: &str) -> ClientRecoveryMarker {
        let b = binding(ManagedCreationKind::ManagedMcp, &selection(), key).unwrap();
        ClientRecoveryJournal::new(root)
            .unwrap()
            .reserve(b)
            .unwrap()
    }
    #[test]
    fn durable_capture_precedes_dispatch_and_contains_no_configuration_material() {
        let root = fixture();
        let out = create(
            &root,
            ManagedCreationKind::ManagedMcp,
            &selection(),
            KEY,
            || {
                let marker = entry(&root);
                assert_eq!(marker.request_key, KEY);
                let bytes = fs::read_to_string(root.join("client-recovery.json")).unwrap();
                for secret in [
                    "DO-NOT-PERSIST",
                    "DO_NOT_PERSIST",
                    "executable",
                    "enabled_tools",
                    "instruction",
                ] {
                    assert!(!bytes.contains(secret));
                }
                Err("transport uncertain".into())
            },
            |_| panic!("transport ambiguity cannot auto-abandon"),
        )
        .unwrap();
        assert_eq!(out["error"]["side_effect_state"], "unknown");
        assert_eq!(entry(&root).request_key, KEY);
        let blocked = create(
            &root,
            ManagedCreationKind::ManagedMcp,
            &selection(),
            NEXT,
            || panic!("another key must not dispatch"),
            |_| panic!("must not touch Core"),
        )
        .unwrap();
        assert_eq!(blocked["error"]["code"], "CLIENT_RECOVERY_REQUIRED");
    }
    #[test]
    fn corrupt_or_unwritable_capture_blocks_all_core_io() {
        let root = fixture();
        fs::write(root.join("client-recovery.json"), b"{corrupt}").unwrap();
        fs::set_permissions(
            root.join("client-recovery.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let result = create(
            &root,
            ManagedCreationKind::ManagedMcp,
            &selection(),
            KEY,
            || panic!("no dispatch without durable capture"),
            |_| panic!("no Core call"),
        )
        .unwrap();
        assert_eq!(result["error"]["code"], "CLIENT_RECOVERY_UNAVAILABLE");
        assert_eq!(
            fs::read(root.join("client-recovery.json")).unwrap(),
            b"{corrupt}"
        );
    }
    #[test]
    fn known_no_start_is_sealed_before_clearing_and_committed_races_are_preserved() {
        for state in ["abandoned", "confirmed", "unresolved"] {
            let root = fixture();
            let original = error(CoreError::new(
                "PERMISSION_DENIED",
                "Native review cancelled",
            ));
            let result = create(
                &root,
                ManagedCreationKind::ManagedMcp,
                &selection(),
                KEY,
                || Ok(original.clone()),
                |command| {
                    assert_eq!(command["method"], "managed_creations.abandon");
                    let marker: ClientRecoveryMarker =
                        serde_json::from_value(command["params"].clone()).unwrap();
                    assert_eq!(entry(&root), marker);
                    Ok(response(&marker, state))
                },
            )
            .unwrap();
            if state == "abandoned" {
                assert_eq!(result, original);
                assert!(ClientRecoveryJournal::new(&root)
                    .unwrap()
                    .list()
                    .unwrap()
                    .is_empty());
            } else {
                assert_eq!(entry(&root).request_key, KEY);
                assert_eq!(result["error"]["code"], "OUTCOME_UNKNOWN");
                assert_eq!(
                    result["error"]["side_effect_state"],
                    if state == "confirmed" {
                        "confirmed"
                    } else {
                        "unknown"
                    }
                );
            }
        }
    }
    #[test]
    fn old_completion_never_clears_a_new_generation_or_changes_known_success() {
        let root = fixture();
        let journal = ClientRecoveryJournal::new(&root).unwrap();
        let out = create(
            &root,
            ManagedCreationKind::ManagedMcp,
            &selection(),
            KEY,
            || {
                let old = entry(&root);
                assert!(journal.clear(&old).unwrap());
                reserve(&root, NEXT);
                Ok(success(&old))
            },
            |_| panic!("known success must not reexecute"),
        )
        .unwrap();
        assert!(out.get("error").is_none());
        assert_eq!(entry(&root).request_key, NEXT);
        let root = fixture();
        let out = create(
            &root,
            ManagedCreationKind::ManagedMcp,
            &selection(),
            KEY,
            || {
                let old = entry(&root);
                fs::set_permissions(
                    root.join("client-recovery.json"),
                    fs::Permissions::from_mode(0o644),
                )
                .unwrap();
                Ok(success(&old))
            },
            |_| panic!("known result must not be retried"),
        )
        .unwrap();
        assert!(out.get("error").is_none());
        assert!(!out["warnings"].as_array().unwrap().is_empty());
        fs::set_permissions(
            root.join("client-recovery.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        assert_eq!(entry(&root).request_key, KEY);
    }
    #[test]
    fn readonly_reconciliation_requires_complete_binding_and_never_infers_no_start() {
        let root = fixture();
        let marker = reserve(&root, KEY);
        let unresolved = get(&root, &marker.entry_id, |command| {
            assert_eq!(command["method"], "managed_creations.get");
            assert_eq!(command["params"], json!(marker));
            Ok(response(&marker, "unresolved"))
        })
        .unwrap();
        assert_eq!(unresolved["result"]["state"], "unresolved");
        assert_eq!(entry(&root), marker);
        let mut forged = response(&marker, "confirmed");
        forged["result"]["marker"]["preview_digest"] = json!("f".repeat(64));
        let result = reconcile_response(&root, &marker, forged).unwrap();
        assert_eq!(result["error"]["side_effect_state"], "unknown");
        assert_eq!(entry(&root), marker);
        let confirmed = get(&root, &marker.entry_id, |command| {
            assert_eq!(command["method"], "managed_creations.get");
            Ok(response(&marker, "confirmed"))
        })
        .unwrap();
        assert_eq!(confirmed["result"]["state"], "confirmed");
        assert!(ClientRecoveryJournal::new(&root)
            .unwrap()
            .list()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn native_abandon_only_seals_original_key_and_revalidates_current_marker() {
        let root = fixture();
        let marker = reserve(&root, KEY);
        let unknown = response(&marker, "unresolved");
        let denied = confirm_abandon(
            &marker,
            unknown.clone(),
            |_| false,
            |_| panic!("Native cancellation must not mutate Core"),
        )
        .unwrap();
        assert_eq!(denied["error"]["side_effect_state"], "not_started");
        assert_eq!(entry(&root), marker);
        let out = confirm_abandon(
            &marker,
            unknown,
            |text| {
                assert!(text.contains(KEY));
                assert!(!text.contains("DO-NOT-PERSIST"));
                true
            },
            |command| {
                assert_eq!(
                    command,
                    json!({"method":"managed_creations.abandon","params":marker})
                );
                assert!(ClientRecoveryJournal::new(&root)
                    .unwrap()
                    .clear(&marker)
                    .unwrap());
                let newer = reserve(&root, NEXT);
                assert!(still_current(&root, &marker).is_err());
                assert_eq!(entry(&root), newer);
                Ok(response(&marker, "abandoned"))
            },
        )
        .unwrap();
        let _ = reconcile_response(&root, &marker, out).unwrap();
        assert_eq!(entry(&root).request_key, NEXT);
    }
    #[test]
    #[ignore = "subprocess-only crash boundary; executed by parent recovery test"]
    fn captured_client_exits_before_any_core_request() {
        let Some(root) = std::env::var_os("BATON_CREATION_QUIT_FIXTURE") else {
            return;
        };
        let root = PathBuf::from(root);
        let _ = create(
            &root,
            ManagedCreationKind::ManagedMcp,
            &selection(),
            KEY,
            || std::process::exit(0),
            |_| panic!("helper exits before Core request"),
        );
        panic!("helper must exit");
    }
    #[test]
    fn a_real_client_exit_preserves_key_and_reopen_only_reads_then_explicitly_cancels() {
        let root = fixture();
        let module = module_path!().split_once("::").unwrap().1;
        let name = format!("{module}::captured_client_exits_before_any_core_request");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", name.as_str(), "--nocapture"])
            .env("BATON_CREATION_QUIT_FIXTURE", &root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break s;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(status.success());
        let marker = entry(&root);
        assert_eq!(marker.request_key, KEY);
        let mut store = agentbaton_core::storage::Store::open(&root.join("test.sqlite3")).unwrap();
        let queried = get(&root, &marker.entry_id, |command| {
            assert_eq!(command["method"], "managed_creations.get");
            let m = serde_json::from_value(command["params"].clone()).unwrap();
            Ok(json!({"result":store.managed_creation_outcome(&m).unwrap()}))
        })
        .unwrap();
        assert_eq!(queried["result"]["state"], "unresolved");
        assert_eq!(entry(&root), marker);
        let abandoned = confirm_abandon(
            &marker,
            queried,
            |_| true,
            |command| {
                assert_eq!(command["method"], "managed_creations.abandon");
                let m = serde_json::from_value(command["params"].clone()).unwrap();
                Ok(json!({"result":store.abandon_managed_creation(&m).unwrap()}))
            },
        )
        .unwrap();
        let result = reconcile_response(&root, &marker, abandoned).unwrap();
        assert_eq!(result["result"]["state"], "abandoned");
        assert!(ClientRecoveryJournal::new(&root)
            .unwrap()
            .list()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn another_process_lock_refuses_dispatch_promptly_without_changing_the_journal() {
        use std::io::BufRead;
        let root = fixture();
        let script="import os,sys,fcntl; f=os.open(sys.argv[1],os.O_CREAT|os.O_RDWR,0o600); fcntl.flock(f,fcntl.LOCK_EX|fcntl.LOCK_NB); print('locked',flush=True); sys.stdin.read(); os.close(f)";
        let mut holder = Command::new("python3")
            .args(["-I", "-S", "-c", script])
            .arg(root.join("client-recovery.lock"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut line = String::new();
        std::io::BufReader::new(holder.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        assert_eq!(line.trim(), "locked");
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let runtime = root.clone();
        let operation = std::thread::spawn(move || {
            let result = create(
                &runtime,
                ManagedCreationKind::ManagedMcp,
                &selection(),
                KEY,
                || panic!("locked journal must prevent original creation"),
                |_| panic!("locked journal must prevent Core IO"),
            );
            let _ = send.send(result);
        });
        let result = receive.recv_timeout(Duration::from_secs(1));
        // Always release this owned helper before checking timing, so a
        // blocking-lock regression fails boundedly instead of hanging tests.
        drop(holder.stdin.take());
        assert!(holder.wait().unwrap().success());
        let joined = operation.join();
        let response = result
            .expect("journal contention must reject promptly")
            .unwrap();
        assert!(joined.is_ok());
        assert_eq!(response["error"]["side_effect_state"], "not_started");
        assert!(response.get("error").is_some());
        assert!(!root.join("client-recovery.json").exists());
    }
}
