//! Native-only approval bridge. No finalize method is registered with Tauri.
use serde_json::{json, Value};

pub enum ApprovalKind {
    Connection,
    Grant,
    Validation,
    DataValidation,
    CommandValidation,
    Goal,
    DataWorkspace,
    GoalExecution,
}
impl ApprovalKind {
    pub fn proposal_method(&self) -> &'static str {
        match self {
            Self::Connection => "connections.propose",
            Self::Grant => "grants.propose",
            Self::Validation => "validation_policies.propose",
            Self::DataValidation => "data_validation_policies.propose",
            Self::CommandValidation => "command_validation_policies.propose",
            Self::Goal => "goal_scopes.propose",
            Self::DataWorkspace => "data_workspaces.propose",
            Self::GoalExecution => "goal_runs.propose",
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::Connection => "connection",
            Self::Grant => "grant",
            Self::Validation => "validation",
            Self::DataValidation => "data_validation",
            Self::CommandValidation => "command_validation",
            Self::Goal => "goal",
            Self::DataWorkspace => "data_workspace",
            Self::GoalExecution => "goal_execution",
        }
    }
    fn action(&self) -> &'static str {
        match self {
            Self::Connection => {
                "Register this exact executable and run its bounded --version probe"
            }
            Self::Grant => {
                "Grant this exact recipient and workspace the bounded execution scope below"
            }
            Self::Validation => "Approve only the exact local artifact-integrity rule below; this does not accept a task or authorize integration",
            Self::DataValidation => "Approve only this exact JSON data-table contract: fixed file hashes, field types, numeric ranges and declared metadata. Dataset population, units and timezone are owner-declared values to match, not independently verified scientific facts. No reproduction script is executed; this does not establish algorithm correctness, accept a task or authorize integration.\n\n仅批准以下固定 JSON 数据表合同。行数总体、单位、时区按负责人声明核对；不运行复现脚本，不证明算法或科学结论，不验收任务、不授权采纳。",
            Self::CommandValidation => "Approve ONE run of this exact server-resolved fixed command, executable/script hashes, literal argv, constant environment, working directory, file contracts and time/capture limits. The program can access this user's resources. No OS filesystem/network sandbox, descendant containment, immutable inputs or scientific correctness is promised. Retained output limits are not filesystem quotas. Strict unsupported requirements must deny before spawn. Command results never accept the task, authorize Git integration or production release.\n\n仅批准该精确固定命令的一次运行。请完整核对程序/脚本哈希、参数、环境、工作目录及限额；不保证系统隔离或后代进程收容，命令结果不代表任务、Git 或生产验收。",
            Self::Goal => "Approve this separate Goal scope against the EXACT original root grant revision. Review grant_before and grant_after: only the displayed child/depth orchestration ceilings and grant revision change. Connection, environment, identity, host, workspace, recipients and execution ceilings MUST NOT expand. Review real project/root consumed, active and children counters; shared consumption is retained, never reset. Approval requires no active or unknown execution. Historical runtime/output/token/cost may be unknown and must never be advertised as zero. Owner acceptance criteria define a future review rule, NOT a passed receipt or Task acceptance. This version stores and previews plans only: no reservation, dispatch, model call, Git integration or production release occurs.\n\n独立 Goal 原生确认：核对原根授予版本及新增子任务/深度上限，其他连接、身份、主机、工作区、接收方和执行限额不可扩张。真实共享已消耗/活动计数不重置；历史未计量保持未知。验收条件不是通过凭据；本单元仅保存和预览计划，不预留或派发。",
            Self::GoalExecution => "Authorize execution within this SERVER-resolved Goal scope, exact initial plan and shared cumulative limits. Only safe plan revisions inside this same scope may advance without another confirmation. Existing connection, identity, host, input and workspace permissions MUST NOT expand. This does not grant child workspace creation, an OS filesystem/network sandbox, a filesystem output quota, Task acceptance, integration or production release. Reserved bounds are not measured usage; unknown effects block automatic replay.\n\n独立 Goal 执行批准：完整核对原有身份、主机、输入、工作区及共享累计限额；仅同范围内的安全计划修订可继续。不得创建新工作区或扩大权限；预留量不等于实测，节点完成不等于任务验收，未知副作用不能自动重放。",
            Self::DataWorkspace => "Approve ONE exact selected-file dataset copy from the server-resolved registered source to its host-managed destination. Review the project, preview revision, every file path, actual SHA-256/bytes, dataset revision and destination in the COMPLETE snapshot. Preview verification alone does not authorize copying. No original source writes, Git initialization, scripts, model calls or execution grant. Selected-file before/after observations are NOT an atomic whole-directory freeze, continuous immutability, DLP or an OS filesystem sandbox. Cancellation is a request; completed or partial copies can remain and must be reconciled, never silently replayed or deleted.\n\n独立数据工作区原生批准：完整核对项目、预览版本、所选文件路径/实际哈希/字节、数据集版本及后台目标。仅批准该精确副本，不修改原源、不初始化 Git、不执行脚本或模型、不授予执行权限；不保证整目录冻结、DLP 或系统隔离。取消请求不等于已停止或删除。",
        }
    }
}

/// The renderer selects references only. The native confirmation binds the
/// complete server proposal to those exact references before private finalization.
pub fn confirm_goal_execution_and_finalize(
    response: Value,
    selection: &Value,
    confirm_native: impl FnOnce(&str) -> bool,
    finalize: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    let id = |value: &Value| {
        value.as_str().is_some_and(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._:-".contains(&c))
        })
    };
    let revision = |value: &Value| {
        value
            .as_u64()
            .is_some_and(|n| n > 0 && n <= 9_007_199_254_740_991)
    };
    let hash = |value: &Value| {
        value.as_str().is_some_and(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        })
    };
    let snapshot = &response["result"]["snapshot"];
    let scope = &snapshot["scope"];
    let plan = &snapshot["plan"];
    let complete = exact_fields(
        selection,
        &[
            "scope_id",
            "expected_plan_revision",
            "expected_ledger_revision",
        ],
    ) && id(&selection["scope_id"])
        && revision(&selection["expected_plan_revision"])
        && revision(&selection["expected_ledger_revision"])
        && snapshot["intent"] == *selection
        && snapshot["execution"] == "bounded_existing_workspace_only"
        && snapshot["new_workspace_authority"] == false
        && snapshot["allow_in_scope_plan_revisions"] == true
        && snapshot["enforcement"]["filesystem_sandbox"] == false
        && snapshot["enforcement"]["network_sandbox"] == false
        && snapshot["enforcement"]["output_quota"] == false
        && scope["id"] == selection["scope_id"]
        && id(&scope["task_id"])
        && id(&scope["project_id"])
        && id(&scope["root_grant_id"])
        && id(&scope["workspace_id"])
        && scope["revoked"] == false
        && scope["planning_only"] == true
        && revision(&scope["authority_revision"])
        && hash(&scope["scope_digest"])
        && hash(&scope["program_sha256"])
        && scope["binding"].is_object()
        && scope["binding"]["workspace_id"] == scope["workspace_id"]
        && scope["binding"]["input_revision"] == scope["input_revision"]
        && scope["goal"]["task_id"] == scope["task_id"]
        && scope["goal"]["root_task_id"] == scope["task_id"]
        && scope["goal"]["project_id"] == scope["project_id"]
        && revision(&scope["root_grant_revision"])
        && revision(&scope["goal"]["spec_revision"])
        && id(&scope["input_revision"])
        && scope["goal"]["input_revision"] == scope["input_revision"]
        && snapshot["root_grant"]["id"] == scope["root_grant_id"]
        && snapshot["root_grant"]["root_grant_id"] == scope["root_grant_id"]
        && snapshot["root_grant"]
            .get("parent_id")
            .is_some_and(Value::is_null)
        && snapshot["root_grant"]["project_id"] == scope["project_id"]
        && snapshot["root_grant"]["revision"] == scope["root_grant_revision"]
        && snapshot["root_grant"]["revoked"] == false
        && snapshot["root_grant"]["scope"].is_object()
        && snapshot["root_grant"]["scope"]["workspace_ids"]
            .as_array()
            .is_some_and(|ids| ids.contains(&scope["workspace_id"]))
        && snapshot["root_grant"]["scope"]["connection_ids"]
            .as_array()
            .is_some_and(|ids| ids.contains(&scope["binding"]["connection_id"]))
        && snapshot["root_grant"]["scope"]["host_ids"]
            .as_array()
            .is_some_and(|ids| ids.contains(&scope["binding"]["host_id"]))
        && id(&scope["binding"]["connection_id"])
        && snapshot["existing_connection"]["id"] == scope["binding"]["connection_id"]
        && [
            "environment_id",
            "identity_ref",
            "host_id",
            "config_revision",
            "capability_snapshot_id",
        ]
        .iter()
        .all(|field| {
            id(&scope["binding"][*field])
                && snapshot["existing_connection"][*field] == scope["binding"][*field]
        })
        && snapshot["existing_connection"]["executable_sha256"] == scope["program_sha256"]
        && scope["root_budget"].is_object()
        && scope["project_budget"].is_object()
        && plan["scope_id"] == selection["scope_id"]
        && plan["proposal"]["schema_version"] == 1
        && id(&plan["proposal"]["plan_id"])
        && plan["proposal"]["revision"] == selection["expected_plan_revision"]
        && plan["proposal"]["scope_revision"] == scope["authority_revision"]
        && plan["proposal"]["goal"] == scope["goal"]
        && plan["proposal"]["nodes"]
            .as_array()
            .is_some_and(|nodes| !nodes.is_empty() && nodes.len() <= 256)
        && hash(&plan["digest"])
        && plan["dispatched"] == false;
    if response.get("error").is_none() && !complete {
        return Ok(
            json!({"request_id":null,"error":{"code":"INVALID_APPROVAL_PROPOSAL","message":"Goal execution proposal does not match the exact requested scope, plan and ledger or its complete existing-permission boundaries; no approval was finalized","retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":0,"warnings":[]}),
        );
    }
    confirm_and_finalize(
        response,
        ApprovalKind::GoalExecution,
        confirm_native,
        finalize,
    )
}

/// Bind this action to its exact preview CAS before any owner prompt. The
/// proposal's own revision is a separate server value used only by finalize.
pub fn confirm_data_workspace_and_finalize(
    response: Value,
    preview_id: &str,
    preview_revision: u64,
    confirm_native: impl FnOnce(&str) -> bool,
    finalize: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    let snapshot = &response["result"]["snapshot"];
    let observed = &snapshot["snapshot"];
    let hash = |value: &Value| {
        value.as_str().is_some_and(|text| {
            text.len() == 64
                && text
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    };
    let files = observed["files"].as_array();
    let complete = snapshot["scope"] == "explicit_selected_dataset_copy_only"
        && snapshot["project_name"]
            .as_str()
            .is_some_and(|name| !name.is_empty())
        && snapshot["intent"]["project_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
        && snapshot["intent"]["project_revision"]
            .as_u64()
            .is_some_and(|revision| revision > 0)
        && snapshot["intent"]["destination"]
            .as_str()
            .is_some_and(|path| path.starts_with('/'))
        && snapshot["intent"]["managed_root"]["canonical_root"]
            .as_str()
            .is_some_and(|path| path.starts_with('/'))
        && observed["contract"]["source"]["canonical_root"]
            .as_str()
            .is_some_and(|path| path.starts_with('/'))
        && hash(&observed["dataset_revision"])
        && hash(&observed["selection_digest"])
        && observed["total_bytes"].as_u64().is_some()
        && observed["atomic_directory_freeze"] == false
        && observed["source_observation_scope"] == "selected_files_before_after_only"
        && snapshot["guarantees"]["filesystem_sandbox"] == false
        && snapshot["guarantees"]["atomic_directory_freeze"] == false
        && snapshot["guarantees"]["continuous_input_immutability"] == false
        && snapshot["guarantees"]["credential_detection"] == "host_selection_required_not_dlp"
        && files.is_some_and(|files| {
            !files.is_empty()
                && files.len() <= 64
                && files.iter().all(|file| {
                    hash(&file["sha256"])
                        && file["bytes"].as_u64().is_some()
                        && file["requirement"]["relative_path"]
                            .as_str()
                            .is_some_and(|path| !path.is_empty())
                        && file["requirement"]["artifact_id"]
                            .as_str()
                            .is_some_and(|id| !id.is_empty())
                })
        });
    if response.get("error").is_none()
        && (preview_revision == 0
            || !complete
            || response["result"]["snapshot"]["intent"]["preview_id"].as_str() != Some(preview_id)
            || response["result"]["snapshot"]["intent"]["preview_revision"].as_u64()
                != Some(preview_revision))
    {
        return Ok(
            json!({"request_id":null,"error":{"code":"INVALID_APPROVAL_PROPOSAL","message":"Data copy proposal does not match the exact requested preview and revision; no approval was finalized","retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":0,"warnings":[]}),
        );
    }
    confirm_and_finalize(
        response,
        ApprovalKind::DataWorkspace,
        confirm_native,
        finalize,
    )
}

// This display is specific to the DataWorkspace server snapshot. Columns are
// fixed, not supplied by JavaScript. Every value is JSON escaped and retained.
const REQUIREMENT_FIELDS: [&str; 4] = [
    "artifact_id",
    "relative_path",
    "max_bytes",
    "expected_sha256",
];
const OBSERVATION_FIELDS: [&str; 3] = ["bytes", "destination_relative_path", "sha256"];
const SOURCE_PIN_FIELDS: [&str; 10] = [
    "bytes",
    "ctime",
    "ctime_nsec",
    "device",
    "gid",
    "inode",
    "mode",
    "mtime",
    "mtime_nsec",
    "uid",
];
fn data_file_columns() -> Vec<String> {
    REQUIREMENT_FIELDS
        .iter()
        .map(|field| format!("requirement.{field}"))
        .chain(OBSERVATION_FIELDS.iter().map(|field| (*field).to_owned()))
        .chain(
            SOURCE_PIN_FIELDS
                .iter()
                .map(|field| format!("source_identity.{field}")),
        )
        .collect()
}
fn exact_fields(value: &Value, fields: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field))
    })
}
/// Reconstitute both original file lists. The duplicate requirement is valid
/// only when the source lists were exactly equal, including null/unknown values.
fn restore_data_file_table(display: &Value) -> Option<Value> {
    if !exact_fields(
        display,
        &[
            "snapshot_without_file_lists",
            "file_table_columns",
            "file_table_rows",
        ],
    ) || display["file_table_columns"] != json!(data_file_columns())
    {
        return None;
    }
    let mut snapshot = display["snapshot_without_file_lists"].clone();
    let observed = snapshot.get_mut("snapshot")?.as_object_mut()?;
    if observed.contains_key("files") || observed.get("contract")?.get("files").is_some() {
        return None;
    }
    let rows = display["file_table_rows"].as_array()?;
    if rows.is_empty() || rows.len() > 64 {
        return None;
    }
    let mut requirements = Vec::new();
    let mut files = Vec::new();
    for row in rows {
        let row = row.as_array()?;
        if row.len()
            != REQUIREMENT_FIELDS.len() + OBSERVATION_FIELDS.len() + SOURCE_PIN_FIELDS.len()
        {
            return None;
        }
        let requirement: serde_json::Map<String, Value> = REQUIREMENT_FIELDS
            .iter()
            .zip(row.iter())
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect();
        let mut file: serde_json::Map<String, Value> = OBSERVATION_FIELDS
            .iter()
            .zip(row.iter().skip(REQUIREMENT_FIELDS.len()))
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect();
        let source_pin: serde_json::Map<String, Value> = SOURCE_PIN_FIELDS
            .iter()
            .zip(
                row.iter()
                    .skip(REQUIREMENT_FIELDS.len() + OBSERVATION_FIELDS.len()),
            )
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect();
        requirements.push(Value::Object(requirement.clone()));
        file.insert("requirement".into(), Value::Object(requirement));
        file.insert("source_identity".into(), Value::Object(source_pin));
        files.push(Value::Object(file));
    }
    observed
        .get_mut("contract")?
        .as_object_mut()?
        .insert("files".into(), json!(requirements));
    observed.insert("files".into(), json!(files));
    Some(snapshot)
}
/// Lossless table for a large DataWorkspace snapshot. Unrecognized file
/// structure is refused; unrelated metadata remains complete and counts against
/// the unchanged limit. Reconstruction equality is checked before any prompt.
fn compact_data_workspace_snapshot(snapshot: &Value) -> Option<String> {
    let observed = snapshot.get("snapshot")?;
    let requirements = observed.get("contract")?.get("files")?.as_array()?;
    let files = observed.get("files")?.as_array()?;
    if files.is_empty() || files.len() > 64 || files.len() != requirements.len() {
        return None;
    }
    let mut rows = Vec::new();
    for (requirement, file) in requirements.iter().zip(files) {
        if requirement != file.get("requirement")?
            || !exact_fields(requirement, &REQUIREMENT_FIELDS)
            || !exact_fields(
                file,
                &[
                    "requirement",
                    "bytes",
                    "destination_relative_path",
                    "sha256",
                    "source_identity",
                ],
            )
            || !exact_fields(&file["source_identity"], &SOURCE_PIN_FIELDS)
        {
            return None;
        }
        let row: Vec<Value> = REQUIREMENT_FIELDS
            .iter()
            .map(|field| requirement[*field].clone())
            .chain(OBSERVATION_FIELDS.iter().map(|field| file[*field].clone()))
            .chain(
                SOURCE_PIN_FIELDS
                    .iter()
                    .map(|field| file["source_identity"][*field].clone()),
            )
            .collect();
        rows.push(row);
    }
    let mut remainder = snapshot.clone();
    remainder
        .get_mut("snapshot")?
        .as_object_mut()?
        .remove("files")?;
    remainder
        .get_mut("snapshot")?
        .get_mut("contract")?
        .as_object_mut()?
        .remove("files")?;
    let display = json!({"snapshot_without_file_lists":remainder,
        "file_table_columns":data_file_columns(),"file_table_rows":rows});
    if restore_data_file_table(&display).as_ref() != Some(snapshot) {
        return None;
    }
    // One complete row per line, with explicit column names shown once. Normal
    // pretty JSON expands 64 x 17 scalar cells beyond the review limit.
    let lines: Option<Vec<String>> = display["file_table_rows"]
        .as_array()?
        .iter()
        .map(|row| serde_json::to_string(row).ok())
        .collect();
    let rendered = format!("{{\n\"snapshot_without_file_lists\":{},\n\"file_table_columns\":{},\n\"file_table_rows\":[\n{}\n]\n}}",
        serde_json::to_string_pretty(&display["snapshot_without_file_lists"]).ok()?,
        serde_json::to_string(&display["file_table_columns"]).ok()?, lines?.join(",\n"));
    (rendered.len() <= 16_384).then_some(rendered)
}

pub fn confirm_and_finalize(
    response: Value,
    kind: ApprovalKind,
    confirm_native: impl FnOnce(&str) -> bool,
    finalize: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    // A proposal may already be persisted, but none of these failures has
    // called native confirmation or finalize. Scope this receipt to approval,
    // never to a transport failure after finalize may have committed.
    let refused = |message: &str| -> Result<Value, String> {
        Ok(
            json!({"request_id":null,"error":{"code":"INVALID_APPROVAL_PROPOSAL","message":message,"retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":0,"warnings":[]}),
        )
    };
    if let Some(error) = response.get("error") {
        let valid = error.is_object()
            && error.get("code").and_then(Value::as_str).is_some()
            && error.get("message").and_then(Value::as_str).is_some()
            && error.get("retryable").and_then(Value::as_bool).is_some()
            && error
                .get("side_effect_state")
                .and_then(Value::as_str)
                .is_some();
        if valid {
            return Ok(response);
        }
        // Even error:null beside a result is not authority to approve.
        return refused(
            "Server returned a malformed error before approval; no approval was finalized",
        );
    }
    let Some(proposal) = response.get("result") else {
        return refused("Server returned no approval proposal; no approval was finalized");
    };
    if proposal.get("kind").and_then(Value::as_str) != Some(kind.kind()) {
        return refused(
            "Server proposal kind does not match this native action; no approval was finalized",
        );
    }
    let Some(id) = proposal
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128)
    else {
        return refused("Invalid server proposal ID; no approval was finalized");
    };
    let Some(revision) = proposal
        .get("revision")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
    else {
        return refused("Invalid server proposal revision; no approval was finalized");
    };
    let Some(snapshot) = proposal.get("snapshot").filter(|value| value.is_object()) else {
        return refused("Server returned no approval snapshot; no approval was finalized");
    };
    // Show the exact complete snapshot or refuse. Never truncate review scope.
    let snapshot = match serde_json::to_string_pretty(snapshot) {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return refused("Approval snapshot could not be serialized; no approval was finalized")
        }
    };
    let uses_data_table = snapshot.len() > 16_384 && matches!(kind, ApprovalKind::DataWorkspace);
    let uses_compact_goal = snapshot.len() > 16_384 && matches!(kind, ApprovalKind::GoalExecution);
    let snapshot = if snapshot.len() <= 16_384 {
        snapshot
    } else if matches!(kind, ApprovalKind::DataWorkspace) {
        let Some(display) = compact_data_workspace_snapshot(&proposal["snapshot"]) else {
            return refused(
                "Approval snapshot exceeds the lossless native review limit; nothing was approved",
            );
        };
        display
    } else if matches!(kind, ApprovalKind::GoalExecution) {
        // Compact JSON still contains every original value. No Goal fields,
        // permission metadata or plan nodes are summarized or omitted.
        let Ok(compact) = serde_json::to_string(&proposal["snapshot"]) else {
            return refused(
                "Complete Goal execution snapshot could not be serialized; nothing was approved",
            );
        };
        if compact.len() > 16_384 {
            return refused("Complete Goal execution snapshot exceeds the native review limit; nothing was approved");
        }
        compact
    } else {
        return refused("Approval snapshot exceeds the native review limit; nothing was approved");
    };
    let table_explanation = if uses_data_table {
        "Data file table: each JSON-escaped row follows the exact displayed column order. requirement.* values occur identically in contract.files and each observed file's requirement; they are displayed once, with every actual hash, destination, byte count and source_identity field. snapshot_without_file_lists contains ALL remaining original fields. No summary or omitted permission fields.\n\n"
    } else if uses_compact_goal {
        "The following is complete compact JSON, including all original Goal scope, plan and permission fields; only formatting whitespace is removed.\n\n"
    } else {
        ""
    };
    let message = format!("{}\n\nThis confirmation comes from the native application, outside the web renderer. Review the SERVER-resolved snapshot in full. Values below are JSON-escaped.\n\nNo verified OS filesystem/network sandbox or native account isolation is claimed. A selected program may access resources available to this user. Version evidence does not prove authentication or model compatibility.\n\n原生负责人确认：请完整核对以下后台解析的程序、参数或执行范围。程序可能访问当前账号可用资源；不保证系统沙箱、网络隔离或原生账号隔离。取消将阻止本次批准，输入变化后必须重新提议。\n\n{table_explanation}{snapshot}\n\nCancel denies this action. Approval is bound to this server proposal and its revision; changed inputs must be proposed again.", kind.action());
    if !confirm_native(&message) {
        return Ok(
            json!({"request_id":null,"error":{"code":"PERMISSION_DENIED","message":"Native owner confirmation was declined; no action was approved","retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":0,"warnings":[]}),
        );
    }
    finalize(
        json!({"method":"approvals.finalize","params":{"proposal_id":id},"expected_revision":revision,"idempotency_key":format!("proposal:{id}")}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn proposal(kind: &str) -> Value {
        json!({"result":{"id":"proposal-1","kind":kind,"revision":3,"snapshot":{"canonical_executable":"/exact/program","executable_sha256":"a".repeat(64),"arguments":["--value=one\nsecond-line"]}},"warnings":[]})
    }
    #[test]
    fn denial_never_invokes_finalize() {
        let called = Cell::new(false);
        let result = confirm_and_finalize(
            proposal("connection"),
            ApprovalKind::Connection,
            |message| {
                assert!(message.contains("/exact/program"));
                assert!(message.contains("one\\nsecond-line"));
                false
            },
            |_| {
                called.set(true);
                Ok(json!({}))
            },
        )
        .unwrap();
        assert!(!called.get());
        assert_eq!(result["error"]["code"], "PERMISSION_DENIED");
    }
    #[test]
    fn approval_sends_only_the_server_proposal_reference_and_revision() {
        confirm_and_finalize(proposal("grant"), ApprovalKind::Grant, |_| true, |command| {
            assert_eq!(command, json!({"method":"approvals.finalize","params":{"proposal_id":"proposal-1"},"expected_revision":3,"idempotency_key":"proposal:proposal-1"}));
            assert!(command["params"].get("approved").is_none());
            assert!(command["params"].get("binding").is_none());
            Ok(json!({"result":{}}))
        }).unwrap();
    }
    fn assert_preapproval_refused(value: Value, kind: ApprovalKind) {
        let response = confirm_and_finalize(
            value,
            kind,
            |_| panic!("invalid proposal must not show approval"),
            |_| panic!("invalid proposal must not finalize"),
        )
        .unwrap();
        assert_eq!(response["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
        assert_eq!(response["error"]["side_effect_state"], "not_started");
        assert_eq!(response["error"]["retryable"], false);
    }
    #[test]
    fn all_preapproval_validation_failures_are_explicitly_not_started() {
        assert_preapproval_refused(json!({"warnings":[]}), ApprovalKind::Grant);
        assert_preapproval_refused(proposal("connection"), ApprovalKind::Grant);
        for (field, value) in [
            ("id", json!("")),
            ("id", json!("x".repeat(129))),
            ("revision", json!(0)),
            ("revision", json!(-1)),
            ("revision", json!(1.5)),
            ("snapshot", Value::Null),
            ("snapshot", json!("not-an-object")),
        ] {
            let mut invalid = proposal("grant");
            invalid["result"][field] = value;
            assert_preapproval_refused(invalid, ApprovalKind::Grant);
        }
        let mut large = proposal("grant");
        large["result"]["snapshot"]["arguments"] = json!("x".repeat(20_000));
        assert_preapproval_refused(large, ApprovalKind::Grant);
    }
    #[test]
    fn finalize_transport_failure_is_not_downgraded_to_not_started() {
        let called = Cell::new(false);
        let result = confirm_and_finalize(
            proposal("grant"),
            ApprovalKind::Grant,
            |_| true,
            |_| {
                called.set(true);
                Err("finalize response lost".into())
            },
        );
        assert!(called.get());
        assert_eq!(result.unwrap_err(), "finalize response lost");
        let server_error = json!({"error":{"code":"OUTCOME_UNKNOWN","message":"Outcome unknown","retryable":false,"side_effect_state":"unknown"}});
        let response = confirm_and_finalize(
            server_error.clone(),
            ApprovalKind::Grant,
            |_| panic!("server error must not confirm"),
            |_| unreachable!(),
        )
        .unwrap();
        assert_eq!(response, server_error);
        let result = confirm_and_finalize(
            proposal("grant"),
            ApprovalKind::Grant,
            |_| true,
            |_| Ok(server_error.clone()),
        )
        .unwrap();
        assert_eq!(result, server_error);
    }
    #[test]
    fn renderer_has_no_native_dialog_filesystem_or_shell_plugin_permissions() {
        let capability: Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        assert!(capability["permissions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn data_contract_confirmation_is_distinct_and_cannot_reuse_integrity_approval() {
        assert_eq!(
            ApprovalKind::DataValidation.proposal_method(),
            "data_validation_policies.propose"
        );
        assert_preapproval_refused(proposal("validation"), ApprovalKind::DataValidation);
        let approved = confirm_and_finalize(
            proposal("data_validation"),
            ApprovalKind::DataValidation,
            |message| {
                assert!(message.contains("owner-declared"));
                assert!(message.contains("No reproduction script is executed"));
                assert!(message.contains("algorithm correctness"));
                assert!(message.contains("/exact/program"));
                true
            },
            |command| {
                assert_eq!(command["method"], "approvals.finalize");
                assert_eq!(command["params"], json!({"proposal_id":"proposal-1"}));
                assert_eq!(command["expected_revision"], 3);
                Ok(json!({"result":{"approved_proposal":"proposal-1"}}))
            },
        )
        .unwrap();
        assert_eq!(approved["result"]["approved_proposal"], "proposal-1");
        let denied = confirm_and_finalize(
            proposal("data_validation"),
            ApprovalKind::DataValidation,
            |_| false,
            |_| panic!("declined data contract must not be finalized"),
        )
        .unwrap();
        assert_eq!(denied["error"]["side_effect_state"], "not_started");
    }
    #[test]
    fn command_confirmation_is_scoped_one_run_and_cannot_reuse_other_approval_kinds() {
        assert_eq!(
            ApprovalKind::CommandValidation.proposal_method(),
            "command_validation_policies.propose"
        );
        for kind in ["validation", "data_validation", "grant", "connection"] {
            assert_preapproval_refused(proposal(kind), ApprovalKind::CommandValidation);
        }
        confirm_and_finalize(
            proposal("command_validation"),
            ApprovalKind::CommandValidation,
            |message| {
                assert!(message.contains("ONE run"));
                assert!(message.contains("/exact/program"));
                assert!(message.contains("one\\nsecond-line"));
                assert!(message.contains("No OS filesystem/network sandbox"));
                true
            },
            |request| {
                assert_eq!(request["params"], json!({"proposal_id":"proposal-1"}));
                assert_eq!(request["expected_revision"], 3);
                Ok(json!({"result":{}}))
            },
        )
        .unwrap();
        let denied = confirm_and_finalize(
            proposal("command_validation"),
            ApprovalKind::CommandValidation,
            |_| false,
            |_| panic!("decline cannot execute/finalize"),
        )
        .unwrap();
        assert_eq!(denied["error"]["side_effect_state"], "not_started");
        let mut large = proposal("command_validation");
        large["result"]["snapshot"]["arguments"] = json!("x".repeat(20_000));
        assert_preapproval_refused(large, ApprovalKind::CommandValidation);
    }
    #[test]
    fn malformed_preapproval_errors_never_confirm_and_postfinalize_values_stay_unknown() {
        for error in [
            Value::Null,
            json!(false),
            json!("invalid"),
            json!({}),
            json!({"code":"X","message":"refused","retryable":"false","side_effect_state":"unknown"}),
        ] {
            let confirms = Cell::new(0);
            let finalizes = Cell::new(0);
            let mut malformed = proposal("grant");
            malformed["error"] = error;
            let response = confirm_and_finalize(
                malformed,
                ApprovalKind::Grant,
                |_| {
                    confirms.set(confirms.get() + 1);
                    true
                },
                |_| {
                    finalizes.set(finalizes.get() + 1);
                    Ok(json!({}))
                },
            )
            .unwrap();
            assert_eq!(response["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
            assert_eq!(response["error"]["side_effect_state"], "not_started");
            assert_eq!(confirms.get(), 0);
            assert_eq!(finalizes.get(), 0);
        }
        let malformed = json!({"error":null});
        let result = confirm_and_finalize(
            proposal("grant"),
            ApprovalKind::Grant,
            |_| true,
            |_| Ok(malformed.clone()),
        )
        .unwrap();
        assert_eq!(result, malformed);
    }
    #[test]
    fn goal_confirmation_is_separate_and_never_reuses_execution_or_validation_authority() {
        assert_eq!(ApprovalKind::Goal.proposal_method(), "goal_scopes.propose");
        for kind in [
            "grant",
            "validation",
            "data_validation",
            "command_validation",
            "connection",
        ] {
            assert_preapproval_refused(proposal(kind), ApprovalKind::Goal);
        }
        let mut exact = proposal("goal");
        exact["result"]["snapshot"] = json!({"resolved":{"grant_before":{"revision":7,"scope":{"max_children":0,"max_depth":0}},"grant_after":{"revision":8,"scope":{"max_children":3,"max_depth":1}},"project_consumed":2,"project_active":0,"root_consumed":1,"root_active":0},"owner_acceptance_criteria":"Review exact outputs"});
        confirm_and_finalize(
            exact.clone(),
            ApprovalKind::Goal,
            |message| {
                assert!(message.contains("grant_before"));
                assert!(message.contains("grant_after"));
                assert!(message.contains("project_consumed"));
                assert!(message.contains("root_active"));
                assert!(message.contains("never reset"));
                assert!(message.contains("no reservation, dispatch"));
                assert!(message.contains("NOT a passed receipt"));
                true
            },
            |request| {
                assert_eq!(request["method"], "approvals.finalize");
                assert_eq!(request["params"], json!({"proposal_id":"proposal-1"}));
                assert_eq!(request["expected_revision"], 3);
                Ok(json!({"result":{}}))
            },
        )
        .unwrap();
        let denied = confirm_and_finalize(
            exact,
            ApprovalKind::Goal,
            |_| false,
            |_| panic!("declined Goal cannot finalize"),
        )
        .unwrap();
        assert_eq!(denied["error"]["side_effect_state"], "not_started");
    }
    fn data_proposal() -> Value {
        let mut value = proposal("data_workspace");
        value["result"]["snapshot"] = json!({"intent":{"preview_id":"preview-1","preview_revision":2,"project_id":"project-1","project_revision":4,"managed_root":{"canonical_root":"/managed"},"destination":"/managed/data-1"},"project_name":"Explicit data files","scope":"explicit_selected_dataset_copy_only","snapshot":{"contract":{"source":{"canonical_root":"/registered/source"}},"dataset_revision":"d".repeat(64),"selection_digest":"c".repeat(64),"total_bytes":12,"atomic_directory_freeze":false,"source_observation_scope":"selected_files_before_after_only","files":[{"requirement":{"artifact_id":"input","relative_path":"data.txt"},"bytes":12,"sha256":"a".repeat(64)}]},"guarantees":{"filesystem_sandbox":false,"atomic_directory_freeze":false,"continuous_input_immutability":false,"credential_detection":"host_selection_required_not_dlp"}});
        value
    }
    #[test]
    fn data_workspace_confirmation_binds_exact_preview_and_separate_proposal_revision() {
        assert_eq!(
            ApprovalKind::DataWorkspace.proposal_method(),
            "data_workspaces.propose"
        );
        let confirms = Cell::new(0);
        let finalizes = Cell::new(0);
        confirm_data_workspace_and_finalize(data_proposal(),"preview-1",2,|message|{
            confirms.set(confirms.get()+1);assert!(message.contains("data.txt"));assert!(message.contains(&"a".repeat(64)));assert!(message.contains("/managed/data-1"));assert!(message.contains("NOT an atomic"));assert!(message.contains("DLP"));true
        },|command|{finalizes.set(finalizes.get()+1);assert_eq!(command,json!({"method":"approvals.finalize","params":{"proposal_id":"proposal-1"},"expected_revision":3,"idempotency_key":"proposal:proposal-1"}));Ok(json!({"result":{"state":"creating"}}))}).unwrap();
        assert_eq!(confirms.get(), 1);
        assert_eq!(finalizes.get(), 1);
    }
    #[test]
    fn malformed_data_copy_proposals_never_prompt_or_finalize() {
        let good = data_proposal();
        let mut cases = vec![json!({}), json!({"error":null})];
        for kind in [
            "data_validation",
            "validation",
            "grant",
            "goal",
            "command_validation",
            "connection",
        ] {
            let mut v = good.clone();
            v["result"]["kind"] = json!(kind);
            cases.push(v);
        }
        for (pointer, value) in [
            ("/result/snapshot/intent/preview_id", json!("foreign")),
            ("/result/snapshot/intent/preview_revision", json!(1)),
            ("/result/snapshot/snapshot/files", json!([])),
            (
                "/result/snapshot/snapshot/dataset_revision",
                json!("invalid"),
            ),
            (
                "/result/snapshot/guarantees/filesystem_sandbox",
                json!(true),
            ),
            ("/result/revision", json!(0)),
            ("/result/id", json!("")),
        ] {
            let mut v = good.clone();
            *v.pointer_mut(pointer).unwrap() = value;
            cases.push(v);
        }
        let mut large = good;
        large["result"]["snapshot"]["extra"] = json!("x".repeat(16384));
        cases.push(large);
        for value in cases {
            let response = confirm_data_workspace_and_finalize(
                value,
                "preview-1",
                2,
                |_| panic!("malformed must not prompt"),
                |_| panic!("malformed must not finalize"),
            )
            .unwrap();
            assert_eq!(response["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
            assert_eq!(response["error"]["side_effect_state"], "not_started");
        }
    }
    #[test]
    fn data_copy_decline_and_postfinalize_uncertainty_are_not_completion_receipts() {
        let denied = confirm_data_workspace_and_finalize(
            data_proposal(),
            "preview-1",
            2,
            |_| false,
            |_| panic!("decline must not finalize"),
        )
        .unwrap();
        assert_eq!(denied["error"]["code"], "PERMISSION_DENIED");
        assert_eq!(denied["error"]["side_effect_state"], "not_started");
        let called = Cell::new(0);
        let failed = confirm_data_workspace_and_finalize(
            data_proposal(),
            "preview-1",
            2,
            |_| true,
            |_| {
                called.set(called.get() + 1);
                Err("finalize response lost".into())
            },
        );
        assert_eq!(called.get(), 1);
        assert_eq!(failed.unwrap_err(), "finalize response lost");
        let post = json!({"error":null});
        assert_eq!(
            confirm_data_workspace_and_finalize(
                data_proposal(),
                "preview-1",
                2,
                |_| true,
                |_| Ok(post.clone())
            )
            .unwrap(),
            post
        );
    }
    // Same complete schema as the generated 64-file Core proposal (including
    // every source FD pin); synthetic paths/ids are never finalizer authority.
    fn sixty_four_file_proposal() -> Value {
        let mut proposal = data_proposal();
        let snapshot = &mut proposal["result"]["snapshot"];
        snapshot["intent"] = json!({"creation_id":"generated-copy","destination":"/managed/data-generated-copy","managed_root":{"canonical_root":"/managed","device":34,"inode":832765},"preview_id":"preview-1","preview_revision":2,"project_id":"project-1","project_revision":4,"workspace_id":"generated-workspace"});
        let requirements: Vec<Value> = (0..64).map(|i|json!({"artifact_id":format!("input-{i:02}"),"relative_path":format!("file-{i:02}.csv"),"max_bytes":16,"expected_sha256":null})).collect();
        let files: Vec<Value> = requirements.iter().enumerate().map(|(i,r)|json!({"requirement":r,"bytes":1,"destination_relative_path":format!("inputs/file-{i:02}.csv"),"sha256":"2d711642b726b04401627ca9fbac32f5c8530fb1903cc4db02258717921a4881","source_identity":{"bytes":1,"ctime":1793091116,"ctime_nsec":744701071,"device":34,"gid":1000,"inode":832793+i,"mode":33188,"mtime":1793091116,"mtime_nsec":744701071,"uid":1000}})).collect();
        snapshot["snapshot"] = json!({"atomic_directory_freeze":false,"contract":{"approval_reference":"preview-1","files":requirements,"format_version":1,"limits":{"max_file_bytes":16,"max_files":64,"max_total_bytes":1024},"source":{"canonical_root":"/registered/source","device":34,"inode":832748}},"dataset_revision":"d".repeat(64),"files":files,"selection_digest":"c".repeat(64),"source_observation_scope":"selected_files_before_after_only","total_bytes":64});
        proposal
    }
    fn assert_data_refused_without_native_calls(proposal: Value) {
        let result = confirm_data_workspace_and_finalize(
            proposal,
            "preview-1",
            2,
            |_| panic!("refused proposal must not prompt"),
            |_| panic!("refused proposal must not finalize"),
        )
        .unwrap();
        assert_eq!(result["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
        assert_eq!(result["error"]["side_effect_state"], "not_started");
    }
    #[test]
    fn sixty_four_file_native_review_roundtrips_every_snapshot_field_and_fixed_finalize() {
        let proposal = sixty_four_file_proposal();
        let original = &proposal["result"]["snapshot"];
        assert!(serde_json::to_string_pretty(original).unwrap().len() > 16_384);
        let display = compact_data_workspace_snapshot(original).unwrap();
        assert!(display.len() <= 16_384);
        let parsed: Value = serde_json::from_str(&display).unwrap();
        assert_eq!(restore_data_file_table(&parsed).as_ref(), Some(original));
        assert_eq!(parsed["file_table_rows"].as_array().unwrap().len(), 64);
        let confirmed = Cell::new(0);
        confirm_data_workspace_and_finalize(proposal,"preview-1",2,|message|{
            confirmed.set(confirmed.get()+1);
            assert!(message.contains("source_identity.ctime_nsec"));
            assert!(message.contains("file-63.csv"));
            assert!(message.contains("No summary"));true
        },|request|{
            assert_eq!(request,json!({"method":"approvals.finalize","params":{"proposal_id":"proposal-1"},"expected_revision":3,"idempotency_key":"proposal:proposal-1"}));
            Ok(json!({"result":{}}))
        }).unwrap();
        assert_eq!(confirmed.get(), 1);
    }
    #[test]
    fn compact_table_preserves_unicode_escape_null_unique_hashes_and_unknown_metadata() {
        let mut proposal = sixty_four_file_proposal();
        let snapshot = &mut proposal["result"]["snapshot"];
        let r = json!({"artifact_id":"输入\\\"\n","relative_path":"目录/文件\\\"\n.csv","max_bytes":19,"expected_sha256":"b".repeat(64)});
        snapshot["snapshot"]["contract"]["files"][0] = r.clone();
        snapshot["snapshot"]["files"][0]["requirement"] = r;
        snapshot["snapshot"]["files"][0]["sha256"] = json!("e".repeat(64));
        snapshot["extra_owner_warning"] = json!({"unknown":null,"warning":"数据\\\"\n"});
        snapshot["snapshot"]["files"][63]["source_identity"]["uid"] = json!(u64::MAX);
        let rendered = compact_data_workspace_snapshot(snapshot).unwrap();
        assert!(rendered.contains("\\n"));
        let restored = restore_data_file_table(&serde_json::from_str(&rendered).unwrap()).unwrap();
        assert_eq!(restored, *snapshot);
    }
    #[test]
    fn conflicting_or_unknown_file_structure_and_oversize_metadata_never_prompt() {
        for pointer in [
            "/snapshot/files/0/requirement/max_bytes",
            "/snapshot/files/0/source_identity/new_field",
            "/snapshot/files/0/new_field",
            "/snapshot/contract/files/0/new_field",
        ] {
            let mut proposal = sixty_four_file_proposal();
            let full = format!("/result/snapshot{pointer}");
            if pointer.ends_with("new_field") {
                let (parent, key) = full.rsplit_once('/').unwrap();
                proposal
                    .pointer_mut(parent)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), json!("must not be dropped"));
            } else {
                *proposal.pointer_mut(&full).unwrap() = json!(17);
            }
            assert_data_refused_without_native_calls(proposal);
        }
        let mut proposal = sixty_four_file_proposal();
        proposal["result"]["snapshot"]["unknown_future_permission"] = json!("x".repeat(16_384));
        assert_data_refused_without_native_calls(proposal);
        let mut proposal = sixty_four_file_proposal();
        proposal["result"]["snapshot"]["snapshot"]["contract"]["files"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert_data_refused_without_native_calls(proposal);
        let mut proposal = sixty_four_file_proposal();
        let file = proposal["result"]["snapshot"]["snapshot"]["files"][0].clone();
        proposal["result"]["snapshot"]["snapshot"]["files"]
            .as_array_mut()
            .unwrap()
            .push(file);
        assert_data_refused_without_native_calls(proposal);
    }
    #[test]
    fn data_compaction_is_kind_specific_and_small_display_stays_complete_pretty_json() {
        for kind in [
            ApprovalKind::Grant,
            ApprovalKind::Goal,
            ApprovalKind::CommandValidation,
            ApprovalKind::Validation,
            ApprovalKind::DataValidation,
            ApprovalKind::Connection,
        ] {
            let mut proposal = sixty_four_file_proposal();
            proposal["result"]["kind"] = json!(kind.kind());
            assert_preapproval_refused(proposal, kind);
        }
        let proposal = data_proposal();
        let pretty = serde_json::to_string_pretty(&proposal["result"]["snapshot"]).unwrap();
        confirm_data_workspace_and_finalize(
            proposal,
            "preview-1",
            2,
            |message| {
                assert!(message.contains(&pretty));
                assert!(!message.contains("file_table_rows"));
                false
            },
            |_| panic!("declined"),
        )
        .unwrap();
        let proposal = sixty_four_file_proposal();
        let result = confirm_data_workspace_and_finalize(
            proposal,
            "preview-1",
            2,
            |_| false,
            |_| panic!("declined"),
        )
        .unwrap();
        assert_eq!(result["error"]["code"], "PERMISSION_DENIED");
    }
    #[test]
    fn identical_expected_and_actual_hashes_remain_distinct_fields_and_finalize_errors_stay_unknown(
    ) {
        let mut proposal = sixty_four_file_proposal();
        let hash = proposal["result"]["snapshot"]["snapshot"]["files"][0]["sha256"].clone();
        proposal["result"]["snapshot"]["snapshot"]["files"][0]["requirement"]["expected_sha256"] =
            hash.clone();
        proposal["result"]["snapshot"]["snapshot"]["contract"]["files"][0]["expected_sha256"] =
            hash.clone();
        let snapshot = &proposal["result"]["snapshot"];
        let display: Value =
            serde_json::from_str(&compact_data_workspace_snapshot(snapshot).unwrap()).unwrap();
        assert_eq!(
            display["file_table_columns"][3],
            "requirement.expected_sha256"
        );
        assert_eq!(display["file_table_columns"][6], "sha256");
        assert_eq!(display["file_table_rows"][0][3], hash);
        assert_eq!(display["file_table_rows"][0][6], hash);
        assert_eq!(restore_data_file_table(&display).as_ref(), Some(snapshot));
        let called = Cell::new(false);
        let result = confirm_data_workspace_and_finalize(
            proposal,
            "preview-1",
            2,
            |_| true,
            |_| {
                called.set(true);
                Err("finalize response lost: outcome unknown".into())
            },
        );
        assert!(called.get());
        assert_eq!(
            result.unwrap_err(),
            "finalize response lost: outcome unknown"
        );
    }
    fn goal_execution_proposal() -> (Value, Value) {
        let selection =
            json!({"scope_id":"scope-1","expected_plan_revision":3,"expected_ledger_revision":7});
        let binding = json!({"connection_id":"connection-1","environment_id":"environment-1","identity_ref":"identity-1","host_id":"local","workspace_id":"workspace-1","config_revision":"config-1","input_revision":"input-1","capability_snapshot_id":"caps-1","policy_revision":"grant-1:2"});
        let goal = json!({"task_id":"task-1","root_task_id":"task-1","project_id":"project-1","spec_revision":1,"input_revision":"input-1"});
        let budget = json!({"attempts":4,"parallel":1,"runtime_ms":10000,"output_bytes":4096,"children":3,"reworks":1,"tokens":null,"cost_microunits":null});
        let scope = json!({"id":"scope-1","task_id":"task-1","project_id":"project-1","root_grant_id":"grant-1","root_grant_revision":2,"workspace_id":"workspace-1","input_revision":"input-1","revoked":false,"planning_only":true,"authority_revision":2,"scope_digest":"a".repeat(64),"program_sha256":"b".repeat(64),"binding":binding,"goal":goal,"root_budget":budget,"project_budget":budget});
        let root_grant = json!({"id":"grant-1","root_grant_id":"grant-1","parent_id":null,"project_id":"project-1","revision":2,"revoked":false,"scope":{"workspace_ids":["workspace-1"],"connection_ids":["connection-1"],"host_ids":["local"],"recipients":[],"network":{"mode":"inherited"},"max_attempts":4,"max_concurrency":1,"max_duration_ms":10000,"max_children":3,"max_depth":1}});
        let connection = json!({"id":"connection-1","environment_id":"environment-1","identity_ref":"identity-1","host_id":"local","config_revision":"config-1","capability_snapshot_id":"caps-1","executable_sha256":"b".repeat(64)});
        let plan = json!({"scope_id":"scope-1","proposal":{"schema_version":1,"plan_id":"plan-1","revision":3,"scope_revision":2,"goal":goal,"nodes":[{"id":"work","goal":"Read exact input and create a candidate"}]},"digest":"c".repeat(64),"dispatched":false});
        let snapshot = json!({"intent":selection,"scope":scope,"root_grant":root_grant,"existing_connection":connection,"plan":plan,"execution":"bounded_existing_workspace_only","new_workspace_authority":false,"allow_in_scope_plan_revisions":true,"enforcement":{"filesystem_sandbox":false,"network_sandbox":false,"output_quota":false},"warning":"Candidate only; unknown effects block replay"});
        (
            json!({"result":{"id":"execution-proposal","kind":"goal_execution","revision":4,"snapshot":snapshot},"warnings":[]}),
            selection,
        )
    }
    fn assert_goal_execution_refused(value: Value, selection: &Value) {
        let prompts = Cell::new(0);
        let finalizations = Cell::new(0);
        let response = confirm_goal_execution_and_finalize(
            value,
            selection,
            |_| {
                prompts.set(prompts.get() + 1);
                true
            },
            |_| {
                finalizations.set(finalizations.get() + 1);
                Ok(json!({}))
            },
        )
        .unwrap();
        assert_eq!(response["error"]["side_effect_state"], "not_started");
        assert_eq!((prompts.get(), finalizations.get()), (0, 0));
    }
    #[test]
    fn goal_execution_native_confirmation_binds_scope_plan_ledger_and_full_existing_permissions() {
        let (proposal, selection) = goal_execution_proposal();
        let snapshot = proposal["result"]["snapshot"].clone();
        let result=confirm_goal_execution_and_finalize(proposal,&selection,|message|{
            assert!(message.contains(&serde_json::to_string_pretty(&snapshot).unwrap()));
            assert!(message.contains("Reserved bounds are not measured usage"));
            assert!(message.contains("Native owner")||message.contains("native application"));
            assert!(message.contains("same scope"));true
        },|frame|{assert_eq!(frame,json!({"method":"approvals.finalize","params":{"proposal_id":"execution-proposal"},"expected_revision":4,"idempotency_key":"proposal:execution-proposal"}));Ok(json!({"result":{"state":"queued","task_accepted":false}}))}).unwrap();
        assert_eq!(result["result"]["task_accepted"], false);
    }
    #[test]
    fn goal_execution_rejects_wrong_intent_or_new_authority_before_any_native_prompt() {
        let (proposal, selection) = goal_execution_proposal();
        for path in [
            "scope",
            "plan",
            "ledger",
            "workspace",
            "sandbox",
            "quota",
            "revisions",
            "grant",
            "connection",
            "revoked",
            "kind",
            "empty",
        ] {
            let mut value = proposal.clone();
            match path {
                "scope" => value["result"]["snapshot"]["intent"]["scope_id"] = json!("foreign"),
                "plan" => value["result"]["snapshot"]["plan"]["proposal"]["revision"] = json!(99),
                "ledger" => {
                    value["result"]["snapshot"]["intent"]["expected_ledger_revision"] = json!(99)
                }
                "workspace" => value["result"]["snapshot"]["new_workspace_authority"] = json!(true),
                "sandbox" => {
                    value["result"]["snapshot"]["enforcement"]["network_sandbox"] = json!(true)
                }
                "quota" => value["result"]["snapshot"]["enforcement"]["output_quota"] = json!(true),
                "revisions" => value["result"]["snapshot"]
                    .as_object_mut()
                    .unwrap()
                    .remove("allow_in_scope_plan_revisions")
                    .map(|_| ())
                    .unwrap(),
                "grant" => value["result"]["snapshot"]["root_grant"]["revision"] = json!(1),
                "connection" => {
                    value["result"]["snapshot"]["existing_connection"]["identity_ref"] =
                        json!("another-account")
                }
                "revoked" => value["result"]["snapshot"]["scope"]["revoked"] = json!(true),
                "kind" => value["result"]["kind"] = json!("goal"),
                "empty" => value["result"]["snapshot"]["plan"]["proposal"]["nodes"] = json!([]),
                _ => unreachable!(),
            }
            assert_goal_execution_refused(value, &selection);
        }
        let mut unknown = selection.clone();
        unknown["authority"] = json!(true);
        assert_goal_execution_refused(proposal, &unknown);
    }
    #[test]
    fn goal_execution_decline_never_finalizes_and_postfinalize_uncertainty_is_preserved() {
        let (proposal, selection) = goal_execution_proposal();
        let response = confirm_goal_execution_and_finalize(
            proposal.clone(),
            &selection,
            |_| false,
            |_| panic!("decline finalized"),
        )
        .unwrap();
        assert_eq!(response["error"]["code"], "PERMISSION_DENIED");
        assert_eq!(response["error"]["side_effect_state"], "not_started");
        let failure = json!({"error":{"code":"OUTCOME_UNKNOWN","message":"reply lost","side_effect_state":"unknown"}});
        let response = confirm_goal_execution_and_finalize(
            proposal.clone(),
            &selection,
            |_| true,
            |_| Ok(failure.clone()),
        )
        .unwrap();
        assert_eq!(response, failure);
        assert_eq!(
            confirm_goal_execution_and_finalize(
                proposal,
                &selection,
                |_| true,
                |_| Err("unresolved after finalize".into())
            )
            .unwrap_err(),
            "unresolved after finalize"
        );
    }
    #[test]
    fn complete_goal_snapshot_can_use_lossless_compact_json_but_never_raise_the_limit() {
        let (mut proposal, selection) = goal_execution_proposal();
        proposal["result"]["snapshot"]["extra_review_metadata"] =
            json!(vec!["full original value"; 500]);
        let snapshot = proposal["result"]["snapshot"].clone();
        let compact = serde_json::to_string(&snapshot).unwrap();
        let pretty = serde_json::to_string_pretty(&snapshot).unwrap();
        assert!(pretty.len() > 16384);
        assert!(compact.len() <= 16384);
        confirm_goal_execution_and_finalize(
            proposal.clone(),
            &selection,
            |message| {
                assert!(message.contains(&compact));
                assert!(message.contains("complete compact JSON"));
                true
            },
            |_| Ok(json!({})),
        )
        .unwrap();
        proposal["result"]["snapshot"]["extra_review_metadata"] = json!("z".repeat(16385));
        assert_goal_execution_refused(proposal, &selection);
    }
    #[test]
    fn goal_execution_preapproval_core_errors_and_all_original_kind_boundaries_stay_separate() {
        let (_, selection) = goal_execution_proposal();
        let denied = json!({"error":{"code":"PERMISSION_DENIED","message":"scope expired","retryable":false,"side_effect_state":"not_started"}});
        assert_eq!(
            confirm_goal_execution_and_finalize(
                denied.clone(),
                &selection,
                |_| panic!("error prompted"),
                |_| panic!("error finalized")
            )
            .unwrap(),
            denied
        );
        for kind in [
            "connection",
            "grant",
            "validation",
            "data_validation",
            "command_validation",
            "goal",
            "data_workspace",
        ] {
            let (mut value, selection) = goal_execution_proposal();
            value["result"]["kind"] = json!(kind);
            assert_goal_execution_refused(value, &selection);
        }
    }
}
