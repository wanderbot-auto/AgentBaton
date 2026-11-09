//! Strict scalar-only marker retention after the fixture's actual daemon join.
//! Unavailable evidence establishes no stage, owned Goal, stop or timeout cause.
use agentbaton_workspaces::validation::sha256;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
};

const MAX_BYTES: u64 = 16384;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    version: u8,
    dimension: Dimension,
    records: Vec<Record>,
}

#[derive(Deserialize, Serialize)]
enum Dimension {
    #[serde(rename = "generated_parent_mcp_stages")]
    GeneratedParentMcpStages,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Stage {
    ParentTaskStatus,
    DelegateChild,
    ChildPlanningStatus,
    SubmitChildPlan,
    ChildExecutionStatus,
    ExecuteChildGoal,
}

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Event {
    Before,
    Reply,
    After,
}

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Layer {
    Boundary,
    Jsonrpc,
    ToolsCall,
    ActualTool,
    Local,
}

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ReplyClass {
    Unobserved,
    Result,
    Error,
    Malformed,
    Returned,
    JsonDecodeError,
    Assertion,
    BrokenPipe,
    Eof,
    OsError,
    KeyError,
    TypeError,
    ValueError,
    OtherFailure,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Code {
    WorkspaceBusy,
    ResourceLimit,
    DeadlineExceeded,
    ProviderDisconnected,
    OutcomeUnknown,
    BudgetUnknown,
    InputChanged,
    AuthRequired,
    StaleRevision,
    ConfigConflict,
    PermissionDenied,
    NotFound,
    InvalidRequest,
    Internal,
    #[serde(rename = "parse_error")]
    ParseError,
    #[serde(rename = "invalid_request")]
    RpcInvalidRequest,
    #[serde(rename = "method_not_found")]
    MethodNotFound,
    #[serde(rename = "invalid_params")]
    InvalidParams,
    #[serde(rename = "internal_error")]
    InternalError,
    #[serde(rename = "other")]
    Other,
    #[serde(rename = "invalid")]
    Invalid,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Flag {
    True,
    False,
    Absent,
    Invalid,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Effect {
    NotStarted,
    Confirmed,
    Unknown,
    Absent,
    Invalid,
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    input: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(input).map(Some)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    stage: Stage,
    event: Event,
    layer: Layer,
    reply_class: ReplyClass,
    elapsed_ms: u64,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    code: Option<Code>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    retryable: Option<Flag>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    side_effect_state: Option<Effect>,
}

fn project(raw: &[u8]) -> Option<Vec<u8>> {
    // Derived struct visitors reject unknown AND duplicate object keys; no
    // untrusted Value/string/map survives into the canonical scalar output.
    let marker: Marker = serde_json::from_slice(raw).ok()?;
    if marker.version != 1 || marker.records.len() > 32 {
        return None;
    }
    for record in &marker.records {
        let error_fields = record.code.is_some()
            && record.retryable.is_some()
            && record.side_effect_state.is_some();
        let no_error_fields = record.code.is_none()
            && record.retryable.is_none()
            && record.side_effect_state.is_none();
        let valid = match (&record.event, &record.layer, &record.reply_class) {
            (Event::Before, Layer::Boundary, ReplyClass::Unobserved)
            | (Event::After, Layer::Boundary, ReplyClass::Returned)
            | (
                Event::Reply,
                Layer::Jsonrpc | Layer::ActualTool,
                ReplyClass::Result | ReplyClass::Malformed,
            )
            | (
                Event::Reply,
                Layer::ToolsCall,
                ReplyClass::Result | ReplyClass::Error | ReplyClass::Malformed,
            )
            | (
                Event::After,
                Layer::Local,
                ReplyClass::JsonDecodeError
                | ReplyClass::Assertion
                | ReplyClass::BrokenPipe
                | ReplyClass::Eof
                | ReplyClass::OsError
                | ReplyClass::KeyError
                | ReplyClass::TypeError
                | ReplyClass::ValueError
                | ReplyClass::OtherFailure,
            ) => no_error_fields,
            (Event::Reply, Layer::Jsonrpc | Layer::ActualTool, ReplyClass::Error) => error_fields,
            _ => false,
        };
        if !valid {
            return None;
        }
    }
    let safe = serde_json::to_vec(&marker).ok()?;
    (safe.len() as u64 <= MAX_BYTES).then_some(safe)
}

fn pin(info: &Metadata) -> [u64; 11] {
    [
        info.dev(),
        info.ino(),
        u64::from(info.uid()),
        u64::from(info.gid()),
        u64::from(info.mode()),
        info.nlink(),
        info.len(),
        info.mtime() as u64,
        info.mtime_nsec() as u64,
        info.ctime() as u64,
        info.ctime_nsec() as u64,
    ]
}

fn private_marker(info: &Metadata, uid: u32) -> bool {
    info.is_file()
        && info.uid() == uid
        && info.nlink() == 1
        && info.mode() & 0o7777 == 0o600
        && (1..=MAX_BYTES).contains(&info.len())
}

pub(super) fn retain_deadline_parent_stages(workspace: &Path, evidence: &Path) -> Value {
    retain_with_hook(workspace, evidence, || {})
}

fn retain_with_hook(workspace: &Path, evidence: &Path, after_read: impl FnOnce()) -> Value {
    let uid = unsafe { libc::geteuid() };
    let reports = workspace.join("reports");
    let directory = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_DIRECTORY)
        .open(&reports)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return json!({"state":"unobserved"})
        }
        Err(_) => return json!({"state":"read_error"}),
    };
    let directory_before = match directory.metadata() {
        Ok(info) if info.is_dir() && info.uid() == uid && info.mode() & 0o022 == 0 => info,
        _ => return json!({"state":"unsafe_file"}),
    };
    let source = reports.join("goal-deadline-parent-stages.json");
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            c"goal-deadline-parent-stages.json".as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
            json!({"state":"unobserved"})
        } else {
            json!({"state":"read_error"})
        };
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let before = match file.metadata() {
        Ok(info) if private_marker(&info, uid) => info,
        _ => return json!({"state":"unsafe_file"}),
    };
    if !fs::symlink_metadata(&source).is_ok_and(|info| pin(&info) == pin(&before)) {
        return json!({"state":"identity_changed"});
    }
    let mut raw = Vec::new();
    if Read::by_ref(&mut file)
        .take(MAX_BYTES + 1)
        .read_to_end(&mut raw)
        .is_err()
    {
        return json!({"state":"read_error"});
    }
    after_read();
    if raw.len() as u64 != before.len()
        || !file.metadata().is_ok_and(|info| pin(&info) == pin(&before))
        || !fs::symlink_metadata(&source).is_ok_and(|info| pin(&info) == pin(&before))
        || !directory
            .metadata()
            .is_ok_and(|info| pin(&info) == pin(&directory_before))
        || !fs::symlink_metadata(&reports).is_ok_and(|info| pin(&info) == pin(&directory_before))
    {
        return json!({"state":"identity_changed"});
    }
    let Some(safe) = project(&raw) else {
        return json!({"state":"invalid_schema"});
    };
    let destination = evidence.join("deadline-parent-stages.json");
    let mut output = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(destination)
    {
        Ok(file) => file,
        Err(_) => return json!({"state":"write_error"}),
    };
    if output.write_all(&safe).is_err() {
        return json!({"state":"write_error"});
    }
    json!({"state":"retained","bytes":safe.len(),"sha256":sha256(&safe)})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::{
        ffi::OsStrExt,
        fs::{symlink, PermissionsExt},
    };

    fn marker() -> Value {
        json!({"version":1,"dimension":"generated_parent_mcp_stages","records":[
            {"stage":"delegate_child","event":"before","layer":"boundary","reply_class":"unobserved","elapsed_ms":0},
            {"stage":"delegate_child","event":"reply","layer":"actual_tool","reply_class":"error","elapsed_ms":1,
             "code":"WORKSPACE_BUSY","retryable":"true","side_effect_state":"not_started"}]})
    }
    fn setup() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("reports")).unwrap();
        let evidence = temp.path().join("evidence");
        fs::create_dir(&evidence).unwrap();
        let source = temp.path().join("reports/goal-deadline-parent-stages.json");
        (temp, source, evidence)
    }
    fn write(path: &Path, raw: &[u8]) {
        fs::write(path, raw).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn strict_marker_projection_rejects_canaries_duplicates_types_and_all_unknown_fields() {
        let safe = serde_json::to_vec(&marker()).unwrap();
        assert!(project(&safe).is_some());
        let mut variants = vec![];
        for field in ["token", "message", "details_ref", "request_id"] {
            let mut value = marker();
            value[field] = json!("PRIVATE_CANARY");
            variants.push(value);
            let mut value = marker();
            value["records"][0][field] = json!("PRIVATE_CANARY");
            variants.push(value);
        }
        for (field, value) in [
            ("stage", json!("PRIVATE_CANARY")),
            ("elapsed_ms", json!(true)),
            ("elapsed_ms", json!(-1)),
            ("elapsed_ms", json!(1.0)),
            ("code", json!(null)),
        ] {
            let mut bad = marker();
            bad["records"][0][field] = value;
            variants.push(bad);
        }
        let mut many = marker();
        many["records"] = json!(vec![marker()["records"][0].clone(); 33]);
        variants.push(many);
        for value in variants {
            assert!(project(&serde_json::to_vec(&value).unwrap()).is_none());
        }
        for raw in [b"{\"version\":1,\"version\":1,\"dimension\":\"generated_parent_mcp_stages\",\"records\":[]}".as_slice(),
            b"{\"version\":1,\"dimension\":\"generated_parent_mcp_stages\",\"records\":[{\"stage\":\"delegate_child\",\"stage\":\"delegate_child\",\"event\":\"before\",\"layer\":\"boundary\",\"reply_class\":\"unobserved\",\"elapsed_ms\":0}]}".as_slice()] {
            assert!(project(raw).is_none());
        }
    }

    #[test]
    fn marker_retention_writes_only_canonical_private_scalars_and_preserves_unobserved() {
        let (temp, source, evidence) = setup();
        assert_eq!(
            retain_deadline_parent_stages(temp.path(), &evidence)["state"],
            "unobserved"
        );
        let raw = serde_json::to_vec_pretty(&marker()).unwrap();
        write(&source, &raw);
        let result = retain_deadline_parent_stages(temp.path(), &evidence);
        assert_eq!(result["state"], "retained");
        let target = evidence.join("deadline-parent-stages.json");
        let kept = fs::read(&target).unwrap();
        assert_eq!(kept, project(&raw).unwrap());
        assert_ne!(kept, raw);
        assert_eq!(fs::metadata(target).unwrap().mode() & 0o7777, 0o600);
    }

    #[test]
    fn marker_retention_never_opens_links_fifo_directory_or_copies_invalid_or_oversized_data() {
        for mode in [
            "symlink",
            "hardlink",
            "fifo",
            "directory",
            "oversize",
            "public",
            "canary",
            "reports_symlink",
        ] {
            let (temp, source, evidence) = setup();
            let raw = serde_json::to_vec(&marker()).unwrap();
            match mode {
                "symlink" | "hardlink" => {
                    let other = temp.path().join("other");
                    write(&other, &raw);
                    if mode == "symlink" {
                        symlink(other, &source).unwrap();
                    } else {
                        fs::hard_link(other, &source).unwrap();
                    }
                }
                "fifo" => {
                    let name = std::ffi::CString::new(source.as_os_str().as_bytes()).unwrap();
                    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
                }
                "directory" => fs::create_dir(&source).unwrap(),
                "oversize" => write(&source, &vec![b' '; MAX_BYTES as usize + 1]),
                "public" => {
                    write(&source, &raw);
                    fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
                }
                "canary" => write(&source, b"{\"token\":\"PRIVATE_CANARY\"}"),
                "reports_symlink" => {
                    fs::rename(
                        temp.path().join("reports"),
                        temp.path().join("other_reports"),
                    )
                    .unwrap();
                    symlink(
                        temp.path().join("other_reports"),
                        temp.path().join("reports"),
                    )
                    .unwrap();
                    write(&source, &raw);
                }
                _ => unreachable!(),
            }
            let result = retain_deadline_parent_stages(temp.path(), &evidence);
            assert_ne!(result["state"], "retained", "{mode}");
            assert!(
                !evidence.join("deadline-parent-stages.json").exists(),
                "{mode}"
            );
            assert!(!result.to_string().contains("PRIVATE_CANARY"));
        }
    }

    #[test]
    fn marker_identity_replacement_and_foreign_owner_metadata_are_refused() {
        let (temp, source, evidence) = setup();
        let raw = serde_json::to_vec(&marker()).unwrap();
        write(&source, &raw);
        let info = fs::metadata(&source).unwrap();
        assert!(!private_marker(&info, info.uid().wrapping_add(1)));
        let replacement = temp.path().join("replacement");
        write(&replacement, &raw);
        let result = retain_with_hook(temp.path(), &evidence, || {
            fs::rename(replacement, &source).unwrap()
        });
        assert_eq!(result["state"], "identity_changed");
        assert!(!evidence.join("deadline-parent-stages.json").exists());
    }
}
