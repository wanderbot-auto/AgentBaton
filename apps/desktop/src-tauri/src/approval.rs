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

fn graph_id(value: &Value) -> bool {
    value.as_str().is_some_and(|id| {
        !id.is_empty()
            && id.len() <= 256
            && id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._:-".contains(&c))
    })
}
fn graph_revision(value: &Value) -> bool {
    value
        .as_u64()
        .is_some_and(|n| n > 0 && n <= 9_007_199_254_740_991)
}
fn graph_hash(value: &Value) -> bool {
    value.as_str().is_some_and(|hash| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}
fn graph_ids(value: &Value, nonempty: bool) -> bool {
    value.as_array().is_some_and(|ids| {
        (!nonempty || !ids.is_empty())
            && ids.len() <= 64
            && ids.iter().all(graph_id)
            && ids
                .iter()
                .enumerate()
                .all(|(index, id)| !ids[..index].contains(id))
    })
}
fn graph_artifact_matches(reference: &Value, workspace: &Value) -> bool {
    [
        "candidate_id",
        "artifact_id",
        "contract_id",
        "producer_node_id",
        "producer_attempt_id",
    ]
    .iter()
    .all(|key| graph_id(&reference[*key]))
        && graph_hash(&reference["sha256"])
        && reference["bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes <= 67_108_864)
        && reference["writer_epoch"].as_u64().is_some()
        && reference["root"] == *workspace
        && reference["relative_path"].as_str().is_some_and(|path| {
            !path.is_empty()
                && path.len() <= 4096
                && !path.starts_with('/')
                && !path.bytes().any(|c| c < 32 || c == 127 || c == b'\\')
                && path
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
        })
        && ["device", "inode", "bytes", "uid", "gid", "mode"]
            .iter()
            .all(|key| reference["file_pin"][*key].as_u64().is_some())
        && ["mtime", "mtime_nsec", "ctime", "ctime_nsec"]
            .iter()
            .all(|key| reference["file_pin"][*key].as_i64().is_some())
        && reference["file_pin"]["bytes"] == reference["bytes"]
}
fn graph_execution_snapshot_matches(snapshot: &Value, selection: &Value) -> bool {
    let scope = &snapshot["scope"];
    let grant = &snapshot["root_grant"];
    let plan = &snapshot["plan"];
    let rebinding = &snapshot["subject_rebinding"];
    let artifact_ids: Vec<Value> = scope["outputs"]
        .as_object()
        .map(|outputs| outputs.keys().map(|id| json!(id)).collect())
        .unwrap_or_default();
    exact_fields(
        selection,
        &[
            "scope_id",
            "expected_plan_revision",
            "expected_ledger_revision",
        ],
    ) && graph_id(&selection["scope_id"])
        && graph_revision(&selection["expected_plan_revision"])
        && graph_revision(&selection["expected_ledger_revision"])
        && snapshot["graph_contract_version"] == 1
        && snapshot["intent"] == *selection
        && scope["id"] == selection["scope_id"]
        && snapshot["execution"] == "goal_graph_v1_existing_workspace"
        && snapshot["new_workspace_authority"] == false
        && snapshot["allow_in_scope_plan_revisions"] == true
        && snapshot["enforcement"]
            == json!({"filesystem_sandbox":false,"network_sandbox":false,"output_quota":false})
        && [
            "task_id",
            "project_id",
            "root_grant_id",
            "workspace_id",
            "input_revision",
        ]
        .iter()
        .all(|key| graph_id(&scope[*key]))
        && scope["revoked"] == false
        && scope["planning_only"] == true
        && graph_revision(&scope["authority_revision"])
        && graph_revision(&scope["root_grant_revision"])
        && graph_hash(&scope["scope_digest"])
        && graph_hash(&scope["program_sha256"])
        && scope["goal"]["task_id"] == scope["task_id"]
        && scope["goal"]["root_task_id"] == scope["task_id"]
        && scope["goal"]["project_id"] == scope["project_id"]
        && scope["goal"]["input_revision"] == scope["input_revision"]
        && graph_revision(&scope["goal"]["spec_revision"])
        && scope["binding"]["workspace_id"] == scope["workspace_id"]
        && scope["binding"]["input_revision"] == scope["input_revision"]
        && grant["id"] == scope["root_grant_id"]
        && grant["root_grant_id"] == scope["root_grant_id"]
        && grant.get("parent_id").is_some_and(Value::is_null)
        && grant["project_id"] == scope["project_id"]
        && grant["revision"] == scope["root_grant_revision"]
        && grant["revoked"] == false
        && [
            ("workspace_ids", "workspace_id"),
            ("connection_ids", "connection_id"),
            ("host_ids", "host_id"),
        ]
        .iter()
        .all(|(ids, key)| {
            grant["scope"][*ids].as_array().is_some_and(|ids| {
                ids.contains(if *key == "workspace_id" {
                    &scope["workspace_id"]
                } else {
                    &scope["binding"][*key]
                })
            })
        })
        && snapshot["existing_connection"]["id"] == scope["binding"]["connection_id"]
        && graph_id(&scope["binding"]["connection_id"])
        && [
            "environment_id",
            "identity_ref",
            "host_id",
            "config_revision",
            "capability_snapshot_id",
        ]
        .iter()
        .all(|key| {
            graph_id(&scope["binding"][*key])
                && snapshot["existing_connection"][*key] == scope["binding"][*key]
        })
        && snapshot["existing_connection"]["executable_sha256"] == scope["program_sha256"]
        && plan["scope_id"] == selection["scope_id"]
        && plan["proposal"]["schema_version"] == 1
        && graph_id(&plan["proposal"]["plan_id"])
        && plan["proposal"]["revision"] == selection["expected_plan_revision"]
        && plan["proposal"]["scope_revision"] == scope["authority_revision"]
        && plan["proposal"]["goal"] == scope["goal"]
        && graph_hash(&plan["digest"])
        && plan["state"] == "validated_proposal"
        && plan["dispatched"] == false
        && plan["proposal"]["nodes"]
            .as_array()
            .is_some_and(|nodes| !nodes.is_empty() && nodes.len() <= 256)
        && snapshot["definition_reuse"]
            .as_array()
            .is_some_and(|definitions| {
                definitions.len() <= 64
                    && definitions.iter().enumerate().all(|(index, definition)| {
                        let action = match definition["source"]["kind"].as_str() {
                            Some("integrity") => "artifact_integrity_only",
                            Some("data") => "data_contract_only",
                            Some("command") => "approved_deterministic_command_only",
                            _ => return false,
                        };
                        graph_id(&definition["rule_id"])
                            && graph_id(&definition["source"]["policy_id"])
                            && graph_revision(&definition["source"]["revision"])
                            && graph_hash(&definition["definition_digest"])
                            && scope["rules"][definition["rule_id"].as_str().unwrap_or("")]
                                == definition["definition_digest"]
                            && definition["definition"].is_object()
                            && definition["allowed_actions"] == json!([action])
                            && definition["goal_acceptance"] == false
                            && graph_ids(&definition["artifact_ids"], true)
                            && definition["artifact_ids"]
                                .as_array()
                                .is_some_and(|ids| ids.iter().all(|id| artifact_ids.contains(id)))
                            && !definitions[..index]
                                .iter()
                                .any(|old| old["rule_id"] == definition["rule_id"])
                    })
            })
        && exact_fields(
            rebinding,
            &[
                "mode",
                "project_id",
                "workspace_id",
                "root_grant_id",
                "artifact_ids",
                "requires_current_scope_and_inputs",
                "requires_owned_stop_and_capture",
                "requires_current_definition_digest",
                "allows_legacy_candidate_backfill",
                "allows_old_policy_execution_reuse",
                "allows_new_workspace",
            ],
        )
        && rebinding["mode"] == "this_run_current_plan_owned_producer_only"
        && rebinding["project_id"] == scope["project_id"]
        && rebinding["workspace_id"] == scope["workspace_id"]
        && rebinding["root_grant_id"] == scope["root_grant_id"]
        && graph_ids(&rebinding["artifact_ids"], true)
        && rebinding["artifact_ids"].as_array().is_some_and(|ids| {
            ids.len() == artifact_ids.len() && artifact_ids.iter().all(|id| ids.contains(id))
        })
        && [
            "requires_current_scope_and_inputs",
            "requires_owned_stop_and_capture",
            "requires_current_definition_digest",
        ]
        .iter()
        .all(|key| rebinding[*key] == true)
        && [
            "allows_legacy_candidate_backfill",
            "allows_old_policy_execution_reuse",
            "allows_new_workspace",
        ]
        .iter()
        .all(|key| rebinding[*key] == false)
        && snapshot["budgets"]["root"] == scope["root_budget"]
        && snapshot["budgets"]["project"] == scope["project_budget"]
        && scope["root_budget"].is_object()
        && scope["project_budget"].is_object()
        && snapshot["budgets"]["usage"].is_object()
        && snapshot["io_limits"]
            == json!({"max_total_artifact_bytes":16_777_216,"max_artifact_refs":64,"worker_within_attempt_runtime_reservation":true,"cancel_requires_actual_join":true,"timeout_without_join_is_unknown":true})
        && snapshot["warning"]
            .as_str()
            .is_some_and(|warning| !warning.is_empty())
}
fn bind_graph_run_finish(
    response: Value,
    proposal: &Value,
    selection: &Value,
) -> Result<Value, String> {
    if let Some(error) = response.get("error") {
        if error.is_object()
            && error["code"].is_string()
            && error["message"].is_string()
            && error["retryable"].is_boolean()
            && error["side_effect_state"].is_string()
        {
            return Ok(response);
        }
    } else {
        let value = &response["result"];
        let run = &value["run"];
        let snapshot = &proposal["snapshot"];
        if value["graph_contract_version"] == 1
            && run["id"] == proposal["id"]
            && run["scope_id"] == selection["scope_id"]
            && run["project_id"] == snapshot["scope"]["project_id"]
            && run["task_id"] == snapshot["scope"]["task_id"]
            && run["root_grant_id"] == snapshot["scope"]["root_grant_id"]
            && run["plan_revision"] == selection["expected_plan_revision"]
            && run["plan_digest"] == snapshot["plan"]["digest"]
            && graph_revision(&run["revision"])
            && value["ledger_revision"].as_u64().is_some_and(|n| {
                n <= 9_007_199_254_740_991
                    && Some(n) >= selection["expected_ledger_revision"].as_u64()
            })
            && value["task_accepted"] == false
            && run["task_accepted"] == false
        {
            return Ok(response);
        }
    }
    Ok(
        json!({"request_id":null,"error":{"code":"OUTCOME_UNKNOWN","message":"Graph finalization returned an unbound receipt; reconcile the exact original request","retryable":false,"side_effect_state":"unknown","details_ref":null},"revision":0,"warnings":[]}),
    )
}
pub fn confirm_goal_graph_and_finalize(
    response: Value,
    selection: &Value,
    confirm_native: impl FnMut(&str) -> bool,
    finalize: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    if let Some(error) = graph_server_error(&response) {
        return error;
    }
    if !graph_execution_snapshot_matches(&response["result"]["snapshot"], selection) {
        return graph_refused("Graph proposal does not match the exact existing scope/plan/ledger, definition reuse, subject rebinding and shared budget boundaries");
    }
    let proposal = response["result"].clone();
    confirm_paged_graph_and_finalize(
        response,
        GraphApprovalKind::Execution,
        GraphDecision::Approve,
        confirm_native,
        |frame| bind_graph_run_finish(finalize(frame)?, &proposal, selection),
    )
}

fn graph_candidate_receipts_match(snapshot: &Value, binding: &Value, ledger: &Value) -> bool {
    let Some(receipts) = snapshot["candidate_receipts"].as_array() else {
        return false;
    };
    let Some(sequence) = ledger.as_u64() else {
        return false;
    };
    if receipts.is_empty() || receipts.len() > 64 {
        return false;
    }
    let valid = receipts.iter().enumerate().all(|(index, c)| {
        graph_id(&c["id"])
            && c["graph_contract_version"] == 1
            && c["run_id"] == binding["run_id"]
            && c["scope_id"] == binding["scope_id"]
            && c["task_id"] == binding["task_id"]
            && c["spec_revision"] == binding["spec_revision"]
            && c["input_revision"] == binding["input_revision"]
            && c["workspace_id"] == binding["workspace_id"]
            && c["capability_digest"] == binding["capability_digest"]
            && graph_revision(&c["plan_revision"])
            && c["plan_revision"].as_u64() <= binding["plan_revision"].as_u64()
            && graph_hash(&c["plan_digest"])
            && (c["plan_revision"] != binding["plan_revision"]
                || c["plan_digest"] == binding["plan_digest"])
            && ["node_id", "attempt_id", "session_id", "capture_job_id"]
                .iter()
                .all(|key| graph_id(&c[*key]))
            && c["stop_receipt_sequence"].as_u64().is_some_and(|stop| {
                stop > 0
                    && stop < sequence
                    && c["capture_receipt_sequence"]
                        .as_u64()
                        .is_some_and(|capture| capture > stop && capture <= sequence)
            })
            && c["source"] == "baton_goal_evidence"
            && c["integrity"] == "verified"
            && c["task_acceptance"] == "not_established"
            && c["created_at"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
            && c["artifacts"].as_array().is_some_and(|artifacts| {
                artifacts.len() <= 64
                    && artifacts.iter().all(|r| {
                        graph_artifact_matches(r, &binding["workspace"])
                            && r["candidate_id"] == c["id"]
                            && r["producer_node_id"] == c["node_id"]
                            && r["producer_attempt_id"] == c["attempt_id"]
                    })
            })
            && c["consumed_inputs"].as_array().is_some_and(|inputs| {
                inputs.len() <= 64
                    && inputs.iter().all(|input| {
                        graph_id(&input["artifact_id"])
                            && graph_id(&input["contract_id"])
                            && graph_hash(&input["sha256"])
                            && (input["candidate"].is_null()
                                || graph_artifact_matches(
                                    &input["candidate"],
                                    &binding["workspace"],
                                ) && input["candidate"]["sha256"] == input["sha256"]
                                    && input["candidate"]["artifact_id"] == input["artifact_id"]
                                    && input["candidate"]["contract_id"] == input["contract_id"])
                    })
            })
            && !receipts[..index].iter().any(|old| old["id"] == c["id"])
    });
    valid
        && snapshot["candidate_refs"].as_array().is_some_and(|refs| {
            refs.iter().all(|reference| {
                receipts.iter().any(|c| {
                    c["id"] == reference["candidate_id"]
                        && c["artifacts"]
                            .as_array()
                            .is_some_and(|artifacts| artifacts.contains(reference))
                })
            })
        })
}
fn graph_node_snapshot_matches(
    snapshot: &Value,
    selection: &Value,
    expected_revision: u64,
) -> bool {
    let binding = &snapshot["run_binding"];
    let node = &snapshot["node"];
    let criterion = &snapshot["criterion"];
    let refs = snapshot["candidate_refs"].as_array();
    let checks = snapshot["validation_refs"].as_array();
    let inputs = snapshot["input_bindings"].as_array();
    exact_fields(
        selection,
        &[
            "run_id",
            "node_id",
            "expected_plan_revision",
            "expected_ledger_revision",
        ],
    ) && graph_id(&selection["run_id"])
        && graph_id(&selection["node_id"])
        && graph_revision(&selection["expected_plan_revision"])
        && graph_revision(&selection["expected_ledger_revision"])
        && snapshot["graph_contract_version"] == 1
        && snapshot["intent"] == *selection
        && expected_revision > 0
        && expected_revision <= 9_007_199_254_740_991
        && snapshot["expected_run_revision"].as_u64() == Some(expected_revision)
        && snapshot["expected_ledger_revision"] == selection["expected_ledger_revision"]
        && binding["run_id"] == selection["run_id"]
        && binding["plan_revision"] == selection["expected_plan_revision"]
        && [
            "scope_id",
            "task_id",
            "project_id",
            "root_grant_id",
            "input_revision",
            "workspace_id",
        ]
        .iter()
        .all(|key| graph_id(&binding[*key]))
        && ["plan_digest", "scope_digest", "capability_digest"]
            .iter()
            .all(|key| graph_hash(&binding[*key]))
        && graph_revision(&binding["spec_revision"])
        && snapshot["capability_digest"] == binding["capability_digest"]
        && graph_candidate_receipts_match(snapshot, binding, &selection["expected_ledger_revision"])
        && binding["workspace"]["canonical_root"]
            .as_str()
            .is_some_and(|root| root.starts_with('/') && root.len() <= 4096)
        && binding["workspace"]["device"].as_u64().is_some()
        && binding["workspace"]["inode"].as_u64().is_some()
        && binding["workspace"]["kind"]["kind"]
            .as_str()
            .is_some_and(|kind| ["directory", "git"].contains(&kind))
        && node["id"] == selection["node_id"]
        && node["kind"] == "owner_approval"
        && node["workspace_id"] == binding["workspace_id"]
        && node["executor"]["kind"] == "owner"
        && node["executor"]["rule_id"] == criterion["rule_id"]
        && graph_id(&criterion["rule_id"])
        && criterion["criterion_text"]
            .as_str()
            .is_some_and(|text| !text.trim().is_empty() && text.len() <= 2048)
        && graph_hash(&criterion["criterion_digest"])
        && graph_ids(&criterion["artifact_ids"], true)
        && refs.is_some_and(|refs| {
            !refs.is_empty()
                && refs.len() <= 64
                && refs
                    .iter()
                    .all(|r| graph_artifact_matches(r, &binding["workspace"]))
                && criterion["artifact_ids"].as_array().is_some_and(|ids| {
                    ids.iter()
                        .all(|id| refs.iter().any(|reference| reference["artifact_id"] == *id))
                })
        })
        && checks.is_some_and(|checks| {
            checks.len() <= 64
                && checks.iter().all(|check| {
                    check["graph_contract_version"] == 1
                        && check["run_id"] == selection["run_id"]
                        && graph_id(&check["id"])
                        && graph_id(&check["node_id"])
                        && graph_id(&check["attempt_id"])
                        && graph_id(&check["rule_id"])
                        && graph_revision(&check["plan_revision"])
                        && graph_hash(&check["plan_digest"])
                        && graph_hash(&check["definition_digest"])
                        && graph_hash(&check["result_digest"])
                        && check["observation"].is_object()
                        && check["observation"]["goal_io"].is_object()
                        && check["observation"]["goal_io"]["state"]
                            .as_str()
                            .is_some_and(|state| !state.is_empty() && state.len() <= 128)
                        && check["candidate_refs"].as_array().is_some_and(|items| {
                            items.len() <= 64
                                && items
                                    .iter()
                                    .all(|item| graph_artifact_matches(item, &binding["workspace"]))
                        })
                        && check["source"] == "baton_goal_validator"
                        && match check["scope"].as_str() {
                            Some("artifact_integrity_only") => {
                                check["correctness"] == "not_established"
                            }
                            Some("data_contract_only") => {
                                check["correctness"] == "approved_schema_ranges_metadata_only"
                            }
                            Some("approved_deterministic_command_only") => {
                                check["correctness"] == "approved_fixed_command_observations_only"
                            }
                            _ => false,
                        }
                        && check["task_acceptance"] == "not_established"
                        && ["passed", "failed", "cancelled"]
                            .iter()
                            .any(|state| check["state"] == *state)
                        && ["worker_joined", "not_started"]
                            .iter()
                            .any(|state| check["io_completion"] == *state)
                        && [
                            "artifact_integrity_only",
                            "data_contract_only",
                            "approved_deterministic_command_only",
                        ]
                        .iter()
                        .any(|scope| check["scope"] == *scope)
                        && (check["state"] == "passed")
                            == check["passed"].as_bool().unwrap_or(false)
                        && check["passed"].is_boolean()
                        && check["stop_confirmed"].is_boolean()
                        && (check["passed"] != true
                            || check["stop_confirmed"] == true
                                && check["io_completion"] == "worker_joined")
                        && check["input_bindings"].is_array()
                })
        })
        && inputs.is_some_and(|inputs| {
            inputs.len() <= 64
                && inputs.iter().all(|input| {
                    graph_id(&input["artifact_id"])
                        && graph_id(&input["contract_id"])
                        && graph_hash(&input["sha256"])
                        && (input["candidate"].is_null()
                            || graph_artifact_matches(&input["candidate"], &binding["workspace"])
                                && input["candidate"]["sha256"] == input["sha256"]
                                && input["candidate"]["artifact_id"] == input["artifact_id"]
                                && input["candidate"]["contract_id"] == input["contract_id"])
                })
        })
}
fn bind_graph_owner_finish(
    response: Value,
    proposal_id: &Value,
    selection: &Value,
    run_revision: u64,
    decision: &str,
) -> Result<Value, String> {
    if let Some(error) = response.get("error") {
        if error.is_object()
            && error["code"].is_string()
            && error["message"].is_string()
            && error["retryable"].is_boolean()
            && error["side_effect_state"].is_string()
        {
            return Ok(response);
        }
    } else {
        let result = &response["result"];
        if result["graph_contract_version"] == 1
            && result["approval_id"] == *proposal_id
            && result["run_id"] == selection["run_id"]
            && result["node_id"] == selection["node_id"]
            && result["plan_revision"] == selection["expected_plan_revision"]
            && result["decision"] == decision
            && graph_id(&result["owner_decision_id"])
            && result["run_revision"].as_u64().is_some_and(|revision| {
                revision > run_revision && revision <= 9_007_199_254_740_991
            })
            && result["task_accepted"].is_boolean()
            && (decision != "rejected" || result["task_accepted"] == false)
        {
            return Ok(response);
        }
    }
    Ok(
        json!({"request_id":null,"error":{"code":"OUTCOME_UNKNOWN","message":"Owner finalization returned an unbound receipt; reconcile this exact original request before any further action","retryable":false,"side_effect_state":"unknown","details_ref":null},"revision":0,"warnings":[]}),
    )
}
/// Fixed Owner acceptance only; the renderer cannot select the private decision.
pub fn confirm_goal_node_and_finalize(
    response: Value,
    selection: &Value,
    expected_revision: u64,
    confirm_native: impl FnMut(&str) -> bool,
    finalize: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    if let Some(error) = graph_server_error(&response) {
        return error;
    }
    if !graph_node_snapshot_matches(
        &response["result"]["snapshot"],
        selection,
        expected_revision,
    ) {
        return graph_refused("Owner node proposal does not match the exact current run, node, CAS, criterion and complete artifact/validation bindings");
    }
    let proposal_id = response["result"]["id"].clone();
    confirm_paged_graph_and_finalize(
        response,
        GraphApprovalKind::NodeAcceptance,
        GraphDecision::Approve,
        confirm_native,
        |frame| {
            bind_graph_owner_finish(
                finalize(frame)?,
                &proposal_id,
                selection,
                expected_revision,
                "accepted",
            )
        },
    )
}
/// Fixed explicit Owner rejection; review Cancel never reaches this route.
pub fn confirm_goal_node_rejection_and_decline(
    response: Value,
    selection: &Value,
    expected_revision: u64,
    confirm_native: impl FnMut(&str) -> bool,
    decline: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    if let Some(error) = graph_server_error(&response) {
        return error;
    }
    if !graph_node_snapshot_matches(
        &response["result"]["snapshot"],
        selection,
        expected_revision,
    ) {
        return graph_refused("Owner rejection proposal does not match the exact current run, node, CAS, criterion and complete artifact/validation bindings");
    }
    let proposal_id = response["result"]["id"].clone();
    confirm_paged_graph_and_finalize(
        response,
        GraphApprovalKind::NodeAcceptance,
        GraphDecision::Reject,
        confirm_native,
        |frame| {
            bind_graph_owner_finish(
                decline(frame)?,
                &proposal_id,
                selection,
                expected_revision,
                "rejected",
            )
        },
    )
}

#[derive(Clone, Copy)]
enum GraphApprovalKind {
    Execution,
    NodeAcceptance,
}
impl GraphApprovalKind {
    fn kind(self) -> &'static str {
        match self {
            Self::Execution => "goal_graph_execution",
            Self::NodeAcceptance => "goal_node_acceptance",
        }
    }
    fn action(self) -> &'static str {
        match self {
            Self::Execution => "Review bounded execution of this exact Goal graph within the existing scope. Only same-scope safe plan revisions may advance. Authorizes the displayed pinned validator definitions to be reused for exact declared candidate subjects in the same workspace; review every definition_reuse and subject_rebinding field. Existing permissions and cumulative budgets never reset or expand. Candidate, integrity/schema checks and exit zero are not semantic Task acceptance. No OS filesystem/network sandbox, output quota, integration or release is granted. / 核对同范围内图执行、固定检查定义复用、同工作区产物对象绑定及累计共享限额；不扩大权限，不授予系统隔离、集成或发布。",
            Self::NodeAcceptance => "Review acceptance of this exact Owner node and its exact candidate artifacts, original attempt, plan/input revisions, validation references and criterion. Acceptance applies only to the complete snapshot below. A stopped process or successful structural check is not proof of semantic correctness. / 核对固定负责人节点、产物哈希/字节、尝试、输入/计划、验证引用与验收条件；不能用进程停止或结构检查代替语义判断。",
        }
    }
}
fn graph_refused(message: &str) -> Result<Value, String> {
    Ok(
        json!({"request_id":null,"error":{"code":"INVALID_APPROVAL_PROPOSAL","message":message,"retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":0,"warnings":[]}),
    )
}
fn graph_declined() -> Result<Value, String> {
    Ok(
        json!({"request_id":null,"error":{"code":"PERMISSION_DENIED","message":"Native review was cancelled; no approval or Owner rejection receipt was created","retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":0,"warnings":[]}),
    )
}
fn graph_server_error(response: &Value) -> Option<Result<Value, String>> {
    response.get("error").map(|error| {
        if error.is_object()
            && error["code"].is_string()
            && error["message"].is_string()
            && error["retryable"].is_boolean()
            && error["side_effect_state"].is_string()
        {
            Ok(response.clone())
        } else {
            graph_refused("Malformed server error before graph review; no approval was finalized")
        }
    })
}
#[derive(Clone, Copy)]
enum GraphDecision {
    Approve,
    Reject,
}
fn confirm_paged_graph_and_finalize(
    response: Value,
    kind: GraphApprovalKind,
    decision: GraphDecision,
    mut confirm_native: impl FnMut(&str) -> bool,
    finalize: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    if let Some(error) = graph_server_error(&response) {
        return error;
    }
    if matches!(decision, GraphDecision::Reject)
        && !matches!(kind, GraphApprovalKind::NodeAcceptance)
    {
        return graph_refused("Only an exact Owner node can use the fixed rejection action");
    }
    let decision_name = match decision {
        GraphDecision::Approve => "APPROVE",
        GraphDecision::Reject => "REJECT",
    };
    let proposal = &response["result"];
    if serde_json::to_vec(&response)
        .map_or(true, |encoded| encoded.len() > GRAPH_REVIEW_SNAPSHOT_BYTES)
    {
        return graph_refused("Complete graph proposal exceeds the native envelope byte limit");
    }
    let Some(id) = proposal["id"].as_str().filter(|id| {
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._:-".contains(&c))
    }) else {
        return graph_refused("Invalid graph proposal reference; no review was opened");
    };
    let Some(revision) = proposal["revision"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
    else {
        return graph_refused("Invalid graph proposal revision; no review was opened");
    };
    if proposal["kind"] != kind.kind() {
        return graph_refused("Wrong graph approval kind; no review was opened");
    }
    let pages = match graph_review_pages(&proposal["snapshot"]) {
        Ok(pages) => pages,
        Err(error) => return graph_refused(&error),
    };
    let total = pages.iter().map(String::len).sum::<usize>();
    let mut offset = 0;
    for (index, page) in pages.iter().enumerate() {
        let end = offset + page.len();
        let message=format!("{}\n\nNative application review — {} / proposal {} / revision {}. Page {}/{}; exact JSON UTF-8 bytes {}..{} of {}. This is a CONTINUATION of complete compact JSON; no field is omitted. Read every page before final approval. OK reviews this page only and does not authorize execution or acceptance. Cancel stops review without an Owner decision receipt.\n\n完整原生审查分页：OK仅确认读完本页，全部页后仍需最终批准。取消不生成验收或拒绝凭据。\n\n{}",format_args!("{}\nRequested final decision: {}. / 最终请求决定：{}。",kind.action(),decision_name,decision_name),kind.kind(),id,revision,index+1,pages.len(),offset,end,total,page);
        if message.len() > 16_384 {
            return graph_refused("Native graph review page exceeds the display limit");
        }
        if !confirm_native(&message) {
            return graph_declined();
        }
        offset = end;
    }
    let final_message=format!("{}\n\nFINAL native decision {} — {} / proposal {} / revision {}. All {} complete JSON pages ({} UTF-8 bytes) were reviewed. OK records the stated decision only for this exact server proposal and revision; changed inputs, scope, artifacts, validation or plan must be proposed again. Cancel does not create an Owner rejection. Unknown effects require reconciliation, never automatic replay.\n\n最终原生决定：仅对已完整审阅的固定提议与版本记录显示的批准或拒绝，输入变化须重新提议。取消不产生负责人拒绝凭据。",kind.action(),decision_name,kind.kind(),id,revision,pages.len(),total);
    if !confirm_native(&final_message) {
        return graph_declined();
    }
    let (method, key) = match decision {
        GraphDecision::Approve => ("approvals.finalize", format!("proposal:{id}")),
        GraphDecision::Reject => ("approvals.decline", format!("proposal-decline:{id}")),
    };
    finalize(
        json!({"method":method,"params":{"proposal_id":id},"expected_revision":revision,"idempotency_key":key}),
    )
}

// Graph reviews have a separate bounded host-only review path. Existing kinds
// keep the original single-dialog 16KiB limit above.
const GRAPH_REVIEW_SNAPSHOT_BYTES: usize = 1_048_576;
const GRAPH_REVIEW_PAGE_BYTES: usize = 12_288;

fn graph_review_pages(snapshot: &Value) -> Result<Vec<String>, String> {
    if !snapshot.is_object() {
        return Err("Graph approval requires a complete object snapshot".into());
    }
    let complete = serde_json::to_string(snapshot).map_err(|error| error.to_string())?;
    if complete.len() > GRAPH_REVIEW_SNAPSHOT_BYTES {
        return Err("Graph approval exceeds the complete native review byte limit".into());
    }
    let mut pages = Vec::new();
    let mut start = 0;
    while start < complete.len() {
        let mut end = (start + GRAPH_REVIEW_PAGE_BYTES).min(complete.len());
        while !complete.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            return Err("Graph review could not preserve a UTF-8 boundary".into());
        }
        pages.push(complete[start..end].to_owned());
        start = end;
    }
    // A review is a lossless encoding of the entire server snapshot, not a
    // digest, field allowlist or renderer-provided projection.
    if pages.concat() != complete {
        return Err("Graph review could not preserve the complete snapshot".into());
    }
    Ok(pages)
}

// Factory authority is independent of Goal. This fixed route shares only the
// lossless bounded page splitter; it never upgrades an older approval kind.
fn factory_ids(value: &Value, nonempty: bool) -> bool {
    value.as_array().is_some_and(|ids| {
        (!nonempty || !ids.is_empty())
            && ids.len() <= 256
            && ids.iter().all(graph_id)
            && ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id))
    })
}
fn factory_safe(value: &Value, min: u64, max: u64) -> bool {
    value
        .as_u64()
        .is_some_and(|n| n >= min && n <= max && n <= 9_007_199_254_740_991)
}
fn factory_budget(value: &Value) -> bool {
    exact_fields(
        value,
        &[
            "attempts",
            "parallel",
            "runtime_ms",
            "output_bytes",
            "children",
            "reworks",
            "tokens",
            "cost_microunits",
        ],
    ) && factory_safe(&value["attempts"], 1, 100000)
        && factory_safe(
            &value["parallel"],
            1,
            value["attempts"].as_u64().unwrap_or(0),
        )
        && factory_safe(&value["runtime_ms"], 1, 604800000)
        && factory_safe(&value["output_bytes"], 1, 9_007_199_254_740_991)
        && ["children", "reworks"]
            .iter()
            .all(|key| factory_safe(&value[*key], 0, 1000))
        && ["tokens", "cost_microunits"].iter().all(|key| {
            value[*key].is_null() || factory_safe(&value[*key], 0, 9_007_199_254_740_991)
        })
}
fn factory_artifacts(value: &Value) -> bool {
    value.as_array().is_some_and(|items| {
        items.len() <= 64
            && items.iter().enumerate().all(|(i, a)| {
                exact_fields(
                    a,
                    &[
                        "artifact_id",
                        "relative_path",
                        "max_bytes",
                        "expected_sha256",
                    ],
                ) && graph_id(&a["artifact_id"])
                    && a["relative_path"].as_str().is_some_and(|p| {
                        !p.is_empty()
                            && p.len() <= 4096
                            && !p.starts_with('/')
                            && !p.contains('\\')
                            && !(p.len() > 1
                                && p.as_bytes()[0].is_ascii_alphabetic()
                                && p.as_bytes()[1] == b':')
                            && !p.bytes().any(|c| c < 32 || c == 127)
                            && p.split('/')
                                .all(|part| !part.is_empty() && part != "." && part != "..")
                    })
                    && factory_safe(&a["max_bytes"], 1, 9_007_199_254_740_991)
                    && (a["expected_sha256"].is_null() || graph_hash(&a["expected_sha256"]))
                    && !items[..i].iter().any(|old| {
                        old["artifact_id"] == a["artifact_id"]
                            || old["relative_path"] == a["relative_path"]
                    })
            })
    })
}
fn factory_selection_matches(value: &Value) -> bool {
    exact_fields(
        value,
        &[
            "root_task_id",
            "expected_task_revision",
            "grant_id",
            "expected_grant_revision",
            "connection_ids",
            "workspace_ids",
            "inputs",
            "outputs",
            "recipients",
            "max_depth",
            "max_children",
            "max_total_nodes",
            "root_budget",
            "project_budget",
            "ttl_ms",
            "capabilities",
        ],
    ) && graph_id(&value["root_task_id"])
        && graph_id(&value["grant_id"])
        && graph_revision(&value["expected_task_revision"])
        && graph_revision(&value["expected_grant_revision"])
        && factory_ids(&value["connection_ids"], true)
        && factory_ids(&value["workspace_ids"], true)
        && factory_ids(&value["recipients"], false)
        && factory_artifacts(&value["inputs"])
        && factory_artifacts(&value["outputs"])
        && factory_safe(&value["max_depth"], 1, 32)
        && factory_safe(&value["max_children"], 1, 1000)
        && factory_safe(&value["max_total_nodes"], 2, 1000)
        && factory_budget(&value["root_budget"])
        && factory_budget(&value["project_budget"])
        && factory_safe(&value["ttl_ms"], 1, 3600000)
        && exact_fields(
            &value["capabilities"],
            &[
                "delegate_task",
                "resume_parent_as_new_attempt",
                "child_planning",
                "child_goal_execution",
                "create_workspace",
            ],
        )
        && value["capabilities"]["delegate_task"] == true
        && value["capabilities"]["resume_parent_as_new_attempt"] == true
        && value["connection_ids"]
            .as_array()
            .is_some_and(|v| v.len() <= 4)
        && value["workspace_ids"]
            .as_array()
            .is_some_and(|v| v.len() == 1)
        && value["recipients"]
            .as_array()
            .is_some_and(|v| v.len() <= 64)
        && ["inputs", "outputs"].iter().all(|k| {
            value[*k]
                .as_array()
                .is_some_and(|v| !v.is_empty() && v.len() <= 16)
        })
        && value["max_children"].as_u64() < value["max_total_nodes"].as_u64()
        && ["root_budget", "project_budget"].iter().all(|k| {
            value[*k]["reworks"] == 0
                && value[*k]["tokens"].is_null()
                && value[*k]["cost_microunits"].is_null()
        })
        && [
            "attempts",
            "parallel",
            "runtime_ms",
            "output_bytes",
            "children",
            "reworks",
        ]
        .iter()
        .all(|k| value["root_budget"][*k].as_u64() <= value["project_budget"][*k].as_u64())
        && ["child_planning", "child_goal_execution", "create_workspace"]
            .iter()
            .all(|key| value["capabilities"][*key] == false)
}
fn factory_contract_matches(contract: &Value, requirements: &Value, workspaces: &Value) -> bool {
    exact_fields(contract,&["canonical_root","root_device","root_inode","requirement"])
    &&contract["canonical_root"].as_str().is_some_and(|p|!p.is_empty()&&p.len()<=4096)
    // These remain original serde u64, without the renderer safe-number cap.
    &&contract["root_device"].as_u64().is_some()&&contract["root_inode"].as_u64().is_some()
    &&requirements.as_array().is_some_and(|items|items.contains(&contract["requirement"]))
    &&workspaces.as_array().is_some_and(|items|items.iter().any(|w|w["root"]==contract["canonical_root"]))
}
fn factory_snapshot_matches(snapshot: &Value, selection: &Value) -> bool {
    let b = &snapshot["bindings"];
    let task = &b["root_task"];
    let grant = &b["approved_grant"];
    if !exact_fields(
        snapshot,
        &["schema_version", "selection", "bindings", "contract_digest"],
    ) || snapshot["schema_version"] != 1
        || snapshot["selection"] != *selection
        || !factory_selection_matches(selection)
        || !graph_hash(&snapshot["contract_digest"])
        || !exact_fields(
            b,
            &[
                "root_task",
                "approved_grant",
                "connections",
                "workspaces",
                "inputs",
                "outputs",
                "companion",
                "guarantees",
                "start_contract",
            ],
        )
        || task["id"] != selection["root_task_id"]
        || task["revision"] != selection["expected_task_revision"]
        || !graph_id(&task["project_id"])
        || !graph_revision(&task["spec_revision"])
        || !task["goal"]
            .as_str()
            .is_some_and(|v| !v.is_empty() && v.len() <= 16384)
        || !task["created_at"].is_string()
        || !task["updated_at"].is_string()
        || task["state"] != "draft"
        || !task["acceptance_ref"].is_null()
        || !task["acceptance_sequence"].is_null()
        || grant["id"] != selection["grant_id"]
        || grant["root_grant_id"] != grant["id"]
        || !grant["parent_id"].is_null()
        || grant["project_id"] != task["project_id"]
        || grant["revision"] != selection["expected_grant_revision"]
        || grant["revoked"] != false
    {
        return false;
    }
    let root = &selection["root_budget"];
    if !grant["scope"]["max_attempts"]
        .as_u64()
        .is_some_and(|n| root["attempts"].as_u64().unwrap_or(u64::MAX) <= n)
        || !grant["scope"]["max_concurrency"]
            .as_u64()
            .is_some_and(|n| root["parallel"].as_u64().unwrap_or(u64::MAX) <= n)
        || !grant["scope"]["max_duration_ms"].as_u64().is_some_and(|n| {
            root["runtime_ms"].as_u64().unwrap_or(u64::MAX)
                <= n.saturating_mul(grant["scope"]["max_attempts"].as_u64().unwrap_or(0))
        })
        || root["children"].as_u64().unwrap_or(u64::MAX)
            >= selection["max_total_nodes"].as_u64().unwrap_or(0)
    {
        return false;
    }
    for (field, ids) in [
        ("connections", "connection_ids"),
        ("workspaces", "workspace_ids"),
    ] {
        let Some(rows) = b[field].as_array() else {
            return false;
        };
        let Some(selected) = selection[ids].as_array() else {
            return false;
        };
        if rows.len() != selected.len()
            || !rows.iter().enumerate().all(|(i, row)| {
                selected.contains(&row["id"])
                    && graph_revision(&row["revision"])
                    && !rows[..i].iter().any(|old| old["id"] == row["id"])
            })
            || !grant["scope"][ids]
                .as_array()
                .is_some_and(|allowed| selected.iter().all(|id| allowed.contains(id)))
        {
            return false;
        }
    }
    if !b["connections"].as_array().is_some_and(|rows| {
        rows.iter().all(|c| {
            graph_hash(&c["executable_sha256"])
                && [
                    "environment_id",
                    "identity_ref",
                    "host_id",
                    "config_revision",
                    "capability_snapshot_id",
                ]
                .iter()
                .all(|k| c[*k].as_str().is_some_and(|v| !v.is_empty()))
        })
    }) || !b["workspaces"].as_array().is_some_and(|rows| {
        rows.iter().all(|w| {
            w["project_id"] == task["project_id"]
                && w["root"].as_str().is_some_and(|p| !p.is_empty())
                && w["state"] == "ready"
        })
    }) {
        return false;
    }
    for (field, input) in [("inputs", true), ("outputs", false)] {
        let Some(rows) = b[field].as_array() else {
            return false;
        };
        let Some(requirements) = selection[field].as_array() else {
            return false;
        };
        if rows.len() != requirements.len()
            || !rows.iter().enumerate().all(|(i, row)| {
                let c = if input { &row["contract"] } else { row };
                factory_contract_matches(c, &selection[field], &b["workspaces"])
                    && (!input
                        || (exact_fields(row, &["contract", "sha256", "bytes"])
                            && graph_hash(&row["sha256"])
                            && row["bytes"].as_u64().is_some_and(|n| {
                                n <= c["requirement"]["max_bytes"].as_u64().unwrap_or(0)
                            })
                            && (c["requirement"]["expected_sha256"].is_null()
                                || c["requirement"]["expected_sha256"] == row["sha256"])))
                    && !rows[..i].iter().any(|old| {
                        if input {
                            old["contract"]["requirement"]["artifact_id"]
                                == c["requirement"]["artifact_id"]
                        } else {
                            old["requirement"]["artifact_id"] == c["requirement"]["artifact_id"]
                        }
                    })
            })
        {
            return false;
        }
    }
    let companion = &b["companion"];
    let start = &b["start_contract"];
    exact_fields(
        companion,
        &[
            "executable",
            "executable_sha256",
            "argument_template",
            "credential_transport",
            "global_daemon_token_available",
        ],
    ) && companion["executable"]
        .as_str()
        .is_some_and(|p| p.starts_with('/') && !p.contains('\0'))
        && graph_hash(&companion["executable_sha256"])
        && companion["argument_template"] == json!(["factory-mcp", "${SESSION_HANDLE_FILE}"])
        && companion["credential_transport"] == "private_0600_file_reference"
        && companion["global_daemon_token_available"] == false
        && exact_fields(
            &b["guarantees"],
            &[
                "filesystem_sandbox",
                "network_sandbox",
                "account_isolation",
                "output_quota",
                "continuous_input_immutability",
                "complete_process_tree_containment",
            ],
        )
        && b["guarantees"]
            .as_object()
            .is_some_and(|v| v.values().all(|v| v == false))
        && exact_fields(
            start,
            &[
                "mode",
                "root_task_request_digest",
                "context_continuation",
                "implicit_start_on_approval",
                "new_workspace_authority",
            ],
        )
        && start["mode"] == "single_scope_single_run"
        && graph_hash(&start["root_task_request_digest"])
        && start["context_continuation"] == "new_attempt_explicit_handoff_only"
        && start["implicit_start_on_approval"] == false
        && start["new_workspace_authority"] == false
}
fn factory_refused(message: &str) -> Result<Value, String> {
    Ok(
        json!({"request_id":null,"error":{"code":"INVALID_APPROVAL_PROPOSAL","message":message,"retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":0,"warnings":[]}),
    )
}
pub fn confirm_factory_and_finalize(
    response: Value,
    selection: &Value,
    mut confirm_native: impl FnMut(&str) -> bool,
    finalize: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    if let Some(error) = graph_server_error(&response) {
        return error;
    }
    let proposal = response["result"].clone();
    if serde_json::to_vec(&response).map_or(true, |v| v.len() > GRAPH_REVIEW_SNAPSHOT_BYTES)
        || proposal["kind"] != "factory"
        || !graph_id(&proposal["id"])
        || !graph_revision(&proposal["revision"])
        || !proposal["id"].as_str().is_some_and(|id| id.len() <= 128)
        || !factory_snapshot_matches(&proposal["snapshot"], selection)
    {
        return factory_refused("Factory proposal does not match the exact independent task/grant/recipient/workspace/input/budget and companion boundaries");
    }
    let pages = match graph_review_pages(&proposal["snapshot"]) {
        Ok(pages) => pages,
        Err(_) => {
            return factory_refused("Complete Factory JSON exceeds the bounded Native review limit")
        }
    };
    let id = proposal["id"].as_str().unwrap();
    let revision = proposal["revision"].as_u64().unwrap();
    let total = pages.iter().map(String::len).sum::<usize>();
    let mut offset = 0;
    let action="Review INDEPENDENT bounded Factory task-delegation authority. Existing Grant execution authority and Factory task-splitting limits are distinct and both displayed. Approval does NOT start a run. Parent continuation is a NEW attempt with explicit handoff, not restored native context. No child planning/Goal execution/workspace creation, OS sandbox, network/account isolation, output quota or process-tree containment is granted. / 独立审查有界委派；Grant执行权限与Factory任务拆分上限分别核对；批准不启动，父任务以新尝试交接，不恢复原生上下文。";
    for (index, page) in pages.iter().enumerate() {
        let end = offset + page.len();
        let message=format!("{action}\n\nFactory proposal {id} / revision {revision}. Page {}/{}; complete JSON UTF-8 bytes {}..{} of {}. OK only reviews this page; final confirmation is still required. Cancel stops review without a rejection receipt.\n\n{page}",index+1,pages.len(),offset,end,total);
        if message.len() > 16384 {
            return factory_refused("Factory Native review page exceeds display limit");
        }
        if !confirm_native(&message) {
            return graph_declined();
        }
        offset = end;
    }
    if !confirm_native(&format!("{action}\n\nFINAL independent Factory approval — proposal {id} / revision {revision}. All {} complete JSON pages ({total} UTF-8 bytes) reviewed. OK records only this exact proposal. Changed inputs require a fresh proposal; unknown replies require reconciliation. Cancel records no rejection.",pages.len())){return graph_declined();}
    let result = finalize(
        json!({"method":"approvals.finalize","params":{"proposal_id":id},"expected_revision":revision,"idempotency_key":format!("proposal:{id}")}),
    )?;
    if let Some(error) = result.get("error") {
        if error.is_object()
            && error["code"].is_string()
            && error["message"].is_string()
            && error["retryable"].is_boolean()
            && error["side_effect_state"].is_string()
        {
            return Ok(result);
        }
        return Ok(
            json!({"request_id":null,"error":{"code":"OUTCOME_UNKNOWN","message":"Factory private finalization returned a malformed error; reconcile the exact original request","retryable":false,"side_effect_state":"unknown","details_ref":null},"revision":0,"warnings":[]}),
        );
    }
    let scope = &result["result"];
    if scope["id"] == proposal["id"]
        && scope["approval_id"] == proposal["id"]
        && scope["root_task_id"] == selection["root_task_id"]
        && scope["project_id"] == proposal["snapshot"]["bindings"]["root_task"]["project_id"]
        && scope["root_grant_id"] == selection["grant_id"]
        && scope["root_grant_revision"] == selection["expected_grant_revision"]
        && scope["selection"] == *selection
        && scope["bindings"] == proposal["snapshot"]["bindings"]
        && scope["contract_digest"] == proposal["snapshot"]["contract_digest"]
        && graph_revision(&scope["revision"])
        && scope["revoked"].is_boolean()
    {
        return Ok(result);
    }
    Ok(
        json!({"request_id":null,"error":{"code":"OUTCOME_UNKNOWN","message":"Factory finalization returned an unbound receipt; reconcile the exact original request before any action","retryable":false,"side_effect_state":"unknown","details_ref":null},"revision":0,"warnings":[]}),
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
    fn graph_native_result_fixture(proposal: &Value) -> Value {
        let snapshot = &proposal["snapshot"];
        let scope = &snapshot["scope"];
        json!({"graph_contract_version":1,"run":{"id":proposal["id"],"scope_id":scope["id"],"project_id":scope["project_id"],"task_id":scope["task_id"],"root_grant_id":scope["root_grant_id"],"plan_revision":snapshot["intent"]["expected_plan_revision"],"plan_digest":snapshot["plan"]["digest"],"revision":1,"state":"queued","cancel_requested":false,"task_accepted":false,"created_at":"2026-10-28","updated_at":"2026-10-28"},"ledger_revision":snapshot["intent"]["expected_ledger_revision"].as_u64().unwrap()+2,"task_accepted":false})
    }
    #[test]
    fn actual_core12_generated_graph_proposal_is_reviewed_completely_with_original_private_reference(
    ) {
        // Actual authenticated Unix-frame Core12 propose fixture SHA256
        // 6074f58ca57c370ed789b92c8602c2dc5e37e5cf29963f83ee4e76297362f36a.
        // This callback probe is NOT Core finalization, worker dispatch or an
        // operating-system owner dialog.
        let original: Value = serde_json::from_str(include_str!(
            "../../src/goal-graph-actual-proposal-fixture.json"
        ))
        .unwrap();
        let selection = original["snapshot"]["intent"].clone();
        let snapshot = original["snapshot"].clone();
        let mut pages = Vec::new();
        let result=confirm_goal_graph_and_finalize(json!({"result":original}),&selection,|message|{assert!(message.len()<=16_384);if !message.contains("FINAL native decision"){pages.push(message.rsplit("\n\n").next().unwrap().to_owned());}true},|frame|{assert_eq!(frame,json!({"method":"approvals.finalize","params":{"proposal_id":original["id"]},"expected_revision":original["revision"],"idempotency_key":format!("proposal:{}",original["id"].as_str().unwrap())}));Ok(json!({"result":graph_native_result_fixture(&original)}))}).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&pages.concat()).unwrap(),
            snapshot
        );
        assert_eq!(result["result"]["run"]["id"], original["id"]);
        let declined = confirm_goal_graph_and_finalize(
            json!({"result":original}),
            &selection,
            |_| false,
            |_| panic!("decline finalized actual proposal"),
        )
        .unwrap();
        assert_eq!(declined["error"]["code"], "PERMISSION_DENIED");
    }
    #[test]
    fn actual_core_node_proposal_binds_real_capture_origin_and_complete_private_owner_decision() {
        // Actual Core Work/owned-stop/capture/propose fixture8fe6. This tests
        // Native callbacks only, not an OS dialog or actual Owner finalization.
        let proposal: Value = serde_json::from_str(include_str!(
            "../../src/goal-graph-actual-node-proposal-fixture.json"
        ))
        .unwrap();
        let selection = proposal["snapshot"]["intent"].clone();
        let revision = proposal["snapshot"]["expected_run_revision"]
            .as_u64()
            .unwrap();
        let expected = proposal["snapshot"].clone();
        let mut pages = Vec::new();
        let response=confirm_goal_node_and_finalize(json!({"result":proposal}),&selection,revision,|message|{assert!(message.len()<=16_384);if !message.contains("FINAL native decision"){pages.push(message.rsplit("\n\n").next().unwrap().to_owned());}true},|frame|{assert_eq!(frame,json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":format!("proposal:{}",proposal["id"].as_str().unwrap())}));Ok(json!({"result":{"graph_contract_version":1,"approval_id":proposal["id"],"run_id":selection["run_id"],"node_id":selection["node_id"],"plan_revision":selection["expected_plan_revision"],"decision":"accepted","owner_decision_id":"callback-only-not-Core-receipt","run_revision":revision+1,"task_accepted":false}}))}).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&pages.concat()).unwrap(),
            expected
        );
        assert_eq!(response["result"]["approval_id"], proposal["id"]);
        let denied = confirm_goal_node_rejection_and_decline(
            json!({"result":proposal}),
            &selection,
            revision,
            |_| false,
            |_| panic!("Cancel wrote a rejection"),
        )
        .unwrap();
        assert_eq!(denied["error"]["code"], "PERMISSION_DENIED");
    }
    #[test]
    fn actual_integrity_validator_observation_roundtrips_every_native_review_byte() {
        // Actual Core packet, including nonempty validation_refs. Callbacks
        // exercise the host bridge only; no OS dialog or Core write runs here.
        let packet: Value = serde_json::from_str(include_str!(
            "../../src/goal-graph-actual-integrity-task-fixture.json"
        ))
        .unwrap();
        let proposal = &packet["owner_proposal"];
        let selection = proposal["snapshot"]["intent"].clone();
        let revision = proposal["snapshot"]["expected_run_revision"]
            .as_u64()
            .unwrap();
        let expected = proposal["snapshot"].clone();
        let validation = &expected["validation_refs"][0];
        assert!(!expected["validation_refs"].as_array().unwrap().is_empty());
        assert_eq!(validation["scope"], "artifact_integrity_only");
        assert_eq!(validation["correctness"], "not_established");
        assert_eq!(validation["task_acceptance"], "not_established");
        assert_eq!(validation["stop_confirmed"], true);
        assert_eq!(validation["io_completion"], "worker_joined");
        assert!(validation["observation"]["goal_io"].is_object());
        let mut pages = Vec::new();
        let writes = Cell::new(0);
        let result = confirm_goal_node_and_finalize(
            json!({"result":proposal}),
            &selection,
            revision,
            |message| {
                assert!(message.len() <= 16_384);
                if !message.contains("FINAL native decision") {
                    pages.push(message.rsplit("\n\n").next().unwrap().to_owned());
                }
                true
            },
            |frame| {
                writes.set(writes.get() + 1);
                assert_eq!(
                    frame,
                    json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":format!("proposal:{}",proposal["id"].as_str().unwrap())})
                );
                // The preserved matching actual first decision is returned
                // by this callback; this callback is not a new Core run.
                Ok(json!({"result":packet["first_decision"]}))
            },
        )
        .unwrap();
        assert_eq!(writes.get(), 1);
        let reviewed: Value = serde_json::from_str(&pages.concat()).unwrap();
        assert_eq!(reviewed, expected);
        assert_eq!(
            reviewed["validation_refs"][0]["observation"],
            validation["observation"]
        );
        assert_eq!(result["result"]["approval_id"], proposal["id"]);
        assert_eq!(result["result"], packet["first_decision"]);
    }

    #[test]
    fn actual_integrity_native_cancel_on_each_page_and_final_never_finalizes_or_rejects() {
        let packet: Value = serde_json::from_str(include_str!(
            "../../src/goal-graph-actual-integrity-task-fixture.json"
        ))
        .unwrap();
        let proposal = &packet["owner_proposal"];
        let selection = &proposal["snapshot"]["intent"];
        let revision = proposal["snapshot"]["expected_run_revision"]
            .as_u64()
            .unwrap();
        let prompt_count = graph_review_pages(&proposal["snapshot"]).unwrap().len() + 1;
        for explicit_rejection in [false, true] {
            for cancel_at in 0..prompt_count {
                let prompts = Cell::new(0);
                let writes = Cell::new(0);
                let confirm = |_message: &str| {
                    let current = prompts.get();
                    prompts.set(current + 1);
                    current != cancel_at
                };
                let finalize = |_frame: Value| -> Result<Value, String> {
                    writes.set(writes.get() + 1);
                    panic!("Cancelled native review wrote a decision")
                };
                let result = if explicit_rejection {
                    confirm_goal_node_rejection_and_decline(
                        json!({"result":proposal}),
                        selection,
                        revision,
                        confirm,
                        finalize,
                    )
                } else {
                    confirm_goal_node_and_finalize(
                        json!({"result":proposal}),
                        selection,
                        revision,
                        confirm,
                        finalize,
                    )
                }
                .unwrap();
                assert_eq!(prompts.get(), cancel_at + 1);
                assert_eq!(writes.get(), 0);
                assert_eq!(result["error"]["code"], "PERMISSION_DENIED");
                assert_eq!(result["error"]["side_effect_state"], "not_started");
                assert!(result.get("result").is_none());
            }
        }
    }

    #[test]
    fn actual_integrity_missing_observation_or_unjoined_pass_never_opens_native_review() {
        let packet: Value = serde_json::from_str(include_str!(
            "../../src/goal-graph-actual-integrity-task-fixture.json"
        ))
        .unwrap();
        let original = &packet["owner_proposal"];
        let selection = &original["snapshot"]["intent"];
        let revision = original["snapshot"]["expected_run_revision"]
            .as_u64()
            .unwrap();
        for mode in [
            "missing",
            "null",
            "not_object",
            "missing_goal_io",
            "not_stopped",
            "not_joined",
        ] {
            let mut proposal = original.clone();
            let validation = &mut proposal["snapshot"]["validation_refs"][0];
            match mode {
                "missing" => {
                    validation.as_object_mut().unwrap().remove("observation");
                }
                "null" => validation["observation"] = Value::Null,
                "not_object" => validation["observation"] = json!("opaque-details-only"),
                "missing_goal_io" => {
                    validation["observation"]
                        .as_object_mut()
                        .unwrap()
                        .remove("goal_io");
                }
                "not_stopped" => validation["stop_confirmed"] = json!(false),
                "not_joined" => validation["io_completion"] = json!("not_started"),
                _ => unreachable!(),
            }
            let prompts = Cell::new(0);
            let writes = Cell::new(0);
            let result = confirm_goal_node_and_finalize(
                json!({"result":proposal}),
                selection,
                revision,
                |_| {
                    prompts.set(prompts.get() + 1);
                    true
                },
                |_| {
                    writes.set(writes.get() + 1);
                    panic!("Invalid receipt finalized")
                },
            )
            .unwrap();
            assert_eq!(prompts.get(), 0, "{mode}");
            assert_eq!(writes.get(), 0, "{mode}");
            assert_eq!(
                result["error"]["side_effect_state"], "not_started",
                "{mode}"
            );
        }
    }

    #[test]
    fn actual_data_and_command_native_review_preserves_complete_observations_and_private_receipts()
    {
        // Real Core packets are transported unchanged. Callback execution is
        // not a fresh Core finalization and is not an actual macOS dialog.
        for (raw, scope, correctness) in [
            (
                include_str!("../../src/goal-graph-actual-data-fixture.json"),
                "data_contract_only",
                "approved_schema_ranges_metadata_only",
            ),
            (
                include_str!("../../src/goal-graph-actual-command-fixture.json"),
                "approved_deterministic_command_only",
                "approved_fixed_command_observations_only",
            ),
        ] {
            let packet: Value = serde_json::from_str(raw).unwrap();
            let proposal = &packet["owner_proposal"];
            let expected = &proposal["snapshot"];
            let selection = &expected["intent"];
            let revision = expected["expected_run_revision"].as_u64().unwrap();
            let validation = &expected["validation_refs"][0];
            assert!(!expected["validation_refs"].as_array().unwrap().is_empty());
            assert_eq!(validation["scope"], scope);
            assert_eq!(validation["correctness"], correctness);
            assert_eq!(validation["task_acceptance"], "not_established");
            assert_eq!(validation["stop_confirmed"], true);
            assert_eq!(validation["io_completion"], "worker_joined");
            assert!(validation["observation"]["goal_io"].is_object());
            let mut pages = Vec::new();
            let writes = Cell::new(0);
            let result = confirm_goal_node_and_finalize(
                json!({"result":proposal}), selection, revision,
                |message| {
                    assert!(message.len() <= 16_384);
                    if !message.contains("FINAL native decision") {
                        pages.push(message.rsplit("\n\n").next().unwrap().to_owned());
                    }
                    true
                },
                |frame| {
                    writes.set(writes.get() + 1);
                    assert_eq!(
                        frame,
                        json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":format!("proposal:{}",proposal["id"].as_str().unwrap())})
                    );
                    Ok(json!({"result":packet["first_decision"]}))
                },
            ).unwrap();
            assert_eq!(writes.get(), 1);
            let reviewed: Value = serde_json::from_str(&pages.concat()).unwrap();
            assert_eq!(&reviewed, expected);
            assert_eq!(
                reviewed["validation_refs"][0]["observation"],
                validation["observation"]
            );
            assert_eq!(result["result"], packet["first_decision"]);
            let denied = confirm_goal_node_rejection_and_decline(
                json!({"result":proposal}),
                selection,
                revision,
                |_| false,
                |_| panic!("Cancel wrote a private rejection"),
            )
            .unwrap();
            assert_eq!(denied["error"]["code"], "PERMISSION_DENIED");
            assert_eq!(denied["error"]["side_effect_state"], "not_started");
            assert!(denied.get("result").is_none());
        }
    }

    fn graph_native_proposal_fixture() -> (Value, Value) {
        let (mut response, selection) = goal_execution_proposal();
        response["result"]["kind"] = json!("goal_graph_execution");
        let snapshot = &mut response["result"]["snapshot"];
        snapshot["scope"]["outputs"] = json!({"artifact-1":"output:artifact-1"});
        snapshot["scope"]["rules"] = json!({"rule-1":"d".repeat(64)});
        snapshot["plan"]["state"] = json!("validated_proposal");
        snapshot["graph_contract_version"] = json!(1);
        snapshot["execution"] = json!("goal_graph_v1_existing_workspace");
        snapshot["definition_reuse"] = json!([{"rule_id":"rule-1","source":{"kind":"integrity","policy_id":"policy-1","revision":2},"definition_digest":"d".repeat(64),"definition":{"complete":"definition"},"allowed_actions":["artifact_integrity_only"],"artifact_ids":["artifact-1"],"goal_acceptance":false}]);
        snapshot["subject_rebinding"] = json!({"mode":"this_run_current_plan_owned_producer_only","project_id":"project-1","workspace_id":"workspace-1","root_grant_id":"grant-1","artifact_ids":["artifact-1"],"requires_current_scope_and_inputs":true,"requires_owned_stop_and_capture":true,"requires_current_definition_digest":true,"allows_legacy_candidate_backfill":false,"allows_old_policy_execution_reuse":false,"allows_new_workspace":false});
        snapshot["budgets"] = json!({"root":snapshot["scope"]["root_budget"],"project":snapshot["scope"]["project_budget"],"usage":{"root":{"attempts":1,"tokens":null},"project":{"attempts":1,"tokens":null}}});
        snapshot["io_limits"] = json!({"max_total_artifact_bytes":16_777_216,"max_artifact_refs":64,"worker_within_attempt_runtime_reservation":true,"cancel_requires_actual_join":true,"timeout_without_join_is_unknown":true});
        (response, selection)
    }
    #[test]
    fn graph_fixed_native_snapshot_checks_full_scope_and_new_authority_before_any_prompt() {
        let (proposal, selection) = graph_native_proposal_fixture();
        let expected = proposal["result"]["snapshot"].clone();
        let mut pages = Vec::new();
        let result=confirm_goal_graph_and_finalize(proposal.clone(),&selection,|message|{if !message.contains("FINAL native decision"){pages.push(message.rsplit("\n\n").next().unwrap().to_owned());}true},|command|{assert_eq!(command,json!({"method":"approvals.finalize","params":{"proposal_id":"execution-proposal"},"expected_revision":4,"idempotency_key":"proposal:execution-proposal"}));Ok(json!({"result":graph_native_result_fixture(&proposal["result"])}))}).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&pages.concat()).unwrap(),
            expected
        );
        assert_eq!(result["result"]["run"]["id"], proposal["result"]["id"]);
        for pointer in [
            "/intent/scope_id",
            "/scope/root_grant_revision",
            "/existing_connection/identity_ref",
            "/definition_reuse/0/goal_acceptance",
            "/definition_reuse/0/allowed_actions/0",
            "/subject_rebinding/allows_new_workspace",
            "/subject_rebinding/requires_owned_stop_and_capture",
            "/enforcement/output_quota",
            "/io_limits/max_artifact_refs",
            "/budgets/root/attempts",
        ] {
            let mut changed = proposal.clone();
            let target = changed["result"]["snapshot"].pointer_mut(pointer).unwrap();
            *target = match target {
                Value::Bool(value) => json!(!*value),
                Value::Number(_) => json!(999),
                _ => json!("foreign-authority"),
            };
            let refused = confirm_goal_graph_and_finalize(
                changed,
                &selection,
                |_| panic!("foreign graph authority prompted"),
                |_| panic!("foreign graph authority finalized"),
            )
            .unwrap();
            assert_eq!(refused["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
        }
    }
    #[test]
    fn graph_postfinalize_wrong_run_id_is_unknown_instead_of_a_new_execution_receipt() {
        let (proposal, selection) = graph_native_proposal_fixture();
        let mut receipt = graph_native_result_fixture(&proposal["result"]);
        receipt["run"]["id"] = json!("another-run");
        let result = confirm_goal_graph_and_finalize(
            proposal,
            &selection,
            |_| true,
            |_| Ok(json!({"result":receipt})),
        )
        .unwrap();
        assert_eq!(result["error"]["code"], "OUTCOME_UNKNOWN");
        assert_eq!(result["error"]["side_effect_state"], "unknown");
        assert!(result.get("result").is_none());
    }
    fn graph_node_proposal_fixture() -> (Value, Value) {
        let selection = json!({"run_id":"run-1","node_id":"owner-1","expected_plan_revision":3,"expected_ledger_revision":17});
        let workspace = json!({"canonical_root":"/generated/workspace","device":1,"inode":2,"kind":{"kind":"directory"}});
        let reference = json!({"candidate_id":"candidate-1","artifact_id":"artifact-1","contract_id":"output:artifact-1","producer_node_id":"work-1","producer_attempt_id":"attempt-1","sha256":"a".repeat(64),"bytes":12,"relative_path":"outputs/report.txt","root":workspace,"file_pin":{"device":1,"inode":3,"bytes":12,"uid":1000,"gid":1000,"mode":33152,"mtime":1,"mtime_nsec":0,"ctime":1,"ctime_nsec":0},"writer_epoch":12});
        let snapshot = json!({"graph_contract_version":1,"intent":selection,"run_binding":{"run_id":"run-1","scope_id":"scope-1","task_id":"task-1","project_id":"project-1","root_grant_id":"grant-1","plan_revision":3,"plan_digest":"b".repeat(64),"scope_digest":"c".repeat(64),"spec_revision":1,"input_revision":"input-1","workspace_id":"workspace-1","workspace":workspace,"capability_digest":"d".repeat(64)},"node":{"id":"owner-1","kind":"owner_approval","goal":"Review exact output","workspace_id":"workspace-1","executor":{"kind":"owner","rule_id":"owner-rule"}},"criterion":{"rule_id":"owner-rule","criterion_text":"Compare the complete report with the approved requirement","criterion_digest":"e".repeat(64),"artifact_ids":["artifact-1"]},"candidate_refs":[reference],"validation_refs":[],"input_bindings":[],"capability_digest":"d".repeat(64),"expected_run_revision":9,"expected_ledger_revision":17});
        let mut snapshot = snapshot;
        snapshot["candidate_receipts"] = json!([{"id":"candidate-1","graph_contract_version":1,"run_id":"run-1","scope_id":"scope-1","plan_revision":3,"plan_digest":"b".repeat(64),"node_id":"work-1","attempt_id":"attempt-1","session_id":"session-1","task_id":"task-1","spec_revision":1,"input_revision":"input-1","workspace_id":"workspace-1","capability_digest":"d".repeat(64),"consumed_inputs":[],"artifacts":snapshot["candidate_refs"],"stop_receipt_sequence":11,"capture_job_id":"capture-1","capture_receipt_sequence":12,"created_at":"2026-10-28","source":"baton_goal_evidence","integrity":"verified","task_acceptance":"not_established"}]);
        (
            json!({"result":{"id":"owner-proposal","kind":"goal_node_acceptance","revision":2,"snapshot":snapshot}}),
            selection,
        )
    }
    #[test]
    fn owner_fixed_native_accept_and_reject_bind_exact_refs_and_separate_private_actions() {
        let (proposal, selection) = graph_node_proposal_fixture();
        confirm_goal_node_and_finalize(proposal.clone(),&selection,9,|message|{assert!(message.contains("APPROVE"));true},|frame|{assert_eq!(frame,json!({"method":"approvals.finalize","params":{"proposal_id":"owner-proposal"},"expected_revision":2,"idempotency_key":"proposal:owner-proposal"}));Ok(json!({"result":{"graph_contract_version":1,"approval_id":"owner-proposal","run_id":"run-1","node_id":"owner-1","plan_revision":3,"decision":"accepted","owner_decision_id":"decision-1","run_revision":10,"task_accepted":false}}))}).unwrap();
        confirm_goal_node_rejection_and_decline(proposal.clone(),&selection,9,|message|{assert!(message.contains("REJECT"));true},|frame|{assert_eq!(frame,json!({"method":"approvals.decline","params":{"proposal_id":"owner-proposal"},"expected_revision":2,"idempotency_key":"proposal-decline:owner-proposal"}));Ok(json!({"result":{"graph_contract_version":1,"approval_id":"owner-proposal","run_id":"run-1","node_id":"owner-1","plan_revision":3,"decision":"rejected","owner_decision_id":"decision-1","run_revision":10,"task_accepted":false}}))}).unwrap();
        let decline = confirm_goal_node_rejection_and_decline(
            proposal,
            &selection,
            9,
            |_| false,
            |_| panic!("Cancel created rejection"),
        )
        .unwrap();
        assert_eq!(decline["error"]["code"], "PERMISSION_DENIED");
    }
    #[test]
    fn owner_postfinalize_unbound_success_is_unknown_and_never_a_not_started_receipt() {
        let (proposal, selection) = graph_node_proposal_fixture();
        let result=confirm_goal_node_and_finalize(proposal,&selection,9,|_|true,|_|Ok(json!({"result":{"graph_contract_version":1,"approval_id":"another-proposal","run_id":"run-1","node_id":"owner-1","plan_revision":3,"decision":"accepted","owner_decision_id":"other-decision","run_revision":10,"task_accepted":true}}))).unwrap();
        assert_eq!(result["error"]["code"], "OUTCOME_UNKNOWN");
        assert_eq!(result["error"]["side_effect_state"], "unknown");
        assert!(result.get("result").is_none());
    }
    #[test]
    fn owner_foreign_snapshot_cas_input_artifact_and_scope_are_refused_without_prompt() {
        let (proposal, selection) = graph_node_proposal_fixture();
        for pointer in [
            "/intent/node_id",
            "/run_binding/run_id",
            "/run_binding/plan_revision",
            "/run_binding/workspace_id",
            "/capability_digest",
            "/expected_run_revision",
            "/expected_ledger_revision",
            "/criterion/rule_id",
            "/criterion/artifact_ids/0",
            "/candidate_refs/0/producer_attempt_id",
            "/candidate_refs/0/root/inode",
            "/candidate_refs/0/relative_path",
            "/candidate_refs/0/file_pin/bytes",
            "/candidate_refs/0/sha256",
            "/candidate_receipts/0/input_revision",
            "/candidate_receipts/0/plan_digest",
            "/candidate_receipts/0/stop_receipt_sequence",
            "/candidate_receipts/0/capture_receipt_sequence",
            "/candidate_receipts/0/artifacts/0/sha256",
            "/candidate_receipts/0/source",
        ] {
            let mut changed = proposal.clone();
            let value = changed["result"]["snapshot"].pointer_mut(pointer).unwrap();
            *value = if value.is_number() {
                json!(999)
            } else if pointer.ends_with("sha256") || pointer.ends_with("plan_digest") {
                json!("f".repeat(64))
            } else if pointer.ends_with("producer_attempt_id")
                || pointer.ends_with("input_revision")
            {
                json!("foreign-valid-reference")
            } else {
                json!("../foreign")
            };
            let result = confirm_goal_node_and_finalize(
                changed,
                &selection,
                9,
                |_| panic!("foreign Owner snapshot prompted"),
                |_| panic!("foreign Owner snapshot finalized"),
            )
            .unwrap();
            assert_eq!(result["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
        }
    }
}

#[cfg(test)]
mod graph_review_tests {
    use super::*;
    use std::cell::Cell;
    fn graph_proposal(kind: GraphApprovalKind) -> Value {
        json!({"result":{"id":"graph-proposal","revision":7,"kind":kind.kind(),"snapshot":{"nested":{"retained":true},"text":"完整😀".repeat(5000)}}})
    }
    #[test]
    fn graph_private_finalize_only_after_every_page_and_final_confirmation() {
        for kind in [
            GraphApprovalKind::Execution,
            GraphApprovalKind::NodeAcceptance,
        ] {
            let response = graph_proposal(kind);
            let expected = response["result"]["snapshot"].clone();
            let mut payloads = Vec::new();
            let confirmations = Cell::new(0);
            let result=confirm_paged_graph_and_finalize(response,kind,GraphDecision::Approve,|message| {
                assert!(message.len()<=16_384);confirmations.set(confirmations.get()+1);
                if !message.contains("FINAL native decision") {payloads.push(message.rsplit("\n\n").next().unwrap().to_owned());}
                true
            },|command| {
                assert_eq!(command,json!({"method":"approvals.finalize","params":{"proposal_id":"graph-proposal"},"expected_revision":7,"idempotency_key":"proposal:graph-proposal"}));
                Ok(json!({"result":{"receipt":"exact"}}))
            }).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&payloads.concat()).unwrap(),
                expected
            );
            assert_eq!(confirmations.get(), payloads.len() + 1);
            assert_eq!(result["result"]["receipt"], "exact");
        }
    }
    #[test]
    fn graph_cancel_on_any_page_or_final_never_creates_a_decision_or_finalizes() {
        let response = graph_proposal(GraphApprovalKind::NodeAcceptance);
        let page_count = graph_review_pages(&response["result"]["snapshot"])
            .unwrap()
            .len();
        for decline in 0..=page_count {
            let prompted = Cell::new(0);
            let finalized = Cell::new(false);
            let result = confirm_paged_graph_and_finalize(
                response.clone(),
                GraphApprovalKind::NodeAcceptance,
                GraphDecision::Approve,
                |_| {
                    let current = prompted.get();
                    prompted.set(current + 1);
                    current != decline
                },
                |_| {
                    finalized.set(true);
                    Ok(json!({}))
                },
            )
            .unwrap();
            assert!(!finalized.get());
            assert_eq!(prompted.get(), decline + 1);
            assert_eq!(result["error"]["code"], "PERMISSION_DENIED");
            assert_eq!(result["error"]["side_effect_state"], "not_started");
            assert!(result.get("result").is_none());
        }
    }
    #[test]
    fn graph_wrong_kind_bad_reference_and_oversize_fail_before_prompt() {
        let original = graph_proposal(GraphApprovalKind::Execution);
        for value in [
            json!(null),
            json!({"error":null,"result":original["result"]}),
            json!({"result":{"id":"foreign\nreference","revision":1,"kind":"goal_graph_execution","snapshot":{}}}),
            json!({"result":{"id":"valid","revision":1,"kind":"goal_execution","snapshot":{}}}),
            json!({"result":{"id":"valid","revision":1,"kind":"goal_graph_execution","snapshot":{"large":"a".repeat(GRAPH_REVIEW_SNAPSHOT_BYTES)}}}),
        ] {
            let result = confirm_paged_graph_and_finalize(
                value,
                GraphApprovalKind::Execution,
                GraphDecision::Approve,
                |_| panic!("invalid proposal prompted"),
                |_| panic!("invalid proposal finalized"),
            )
            .unwrap();
            assert_eq!(result["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
        }
    }
    #[test]
    fn graph_postfinalize_unknown_or_transport_error_is_never_downgraded_or_retried() {
        let unknown = json!({"error":{"code":"OUTCOME_UNKNOWN","message":"reconcile","retryable":false,"side_effect_state":"unknown"}});
        assert_eq!(
            confirm_paged_graph_and_finalize(
                graph_proposal(GraphApprovalKind::Execution),
                GraphApprovalKind::Execution,
                GraphDecision::Approve,
                |_| true,
                |_| Ok(unknown.clone())
            )
            .unwrap(),
            unknown
        );
        assert_eq!(
            confirm_paged_graph_and_finalize(
                graph_proposal(GraphApprovalKind::Execution),
                GraphApprovalKind::Execution,
                GraphDecision::Approve,
                |_| true,
                |_| Err("transport lost after finalize".into())
            )
            .unwrap_err(),
            "transport lost after finalize"
        );
        assert_eq!(
            confirm_paged_graph_and_finalize(
                unknown.clone(),
                GraphApprovalKind::Execution,
                GraphDecision::Approve,
                |_| panic!("Core error prompted"),
                |_| panic!("Core error finalized")
            )
            .unwrap(),
            unknown
        );
    }

    #[test]
    fn explicit_owner_rejection_is_separate_from_cancel_and_graph_execution() {
        let rejected=confirm_paged_graph_and_finalize(graph_proposal(GraphApprovalKind::NodeAcceptance),GraphApprovalKind::NodeAcceptance,GraphDecision::Reject,|message| {assert!(message.contains("REJECT"));true},|command| {
            assert_eq!(command,json!({"method":"approvals.decline","params":{"proposal_id":"graph-proposal"},"expected_revision":7,"idempotency_key":"proposal-decline:graph-proposal"}));
            Ok(json!({"result":{"decision":"rejected"}}))
        }).unwrap();
        assert_eq!(rejected["result"]["decision"], "rejected");
        let invalid = confirm_paged_graph_and_finalize(
            graph_proposal(GraphApprovalKind::Execution),
            GraphApprovalKind::Execution,
            GraphDecision::Reject,
            |_| panic!("execution cannot reject"),
            |_| panic!("execution cannot decline"),
        )
        .unwrap();
        assert_eq!(invalid["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
    }
    #[test]
    fn graph_pages_preserve_complete_values_and_utf8_with_bounded_payloads() {
        let snapshot = json!({"untrusted_text":"审查😀\n\"".repeat(4000),"all_fields":{"null":null,"array":[true,false,1,{"field":"retained"}]},"unknown_future_field":{"not_omitted":"完整"}});
        let pages = graph_review_pages(&snapshot).unwrap();
        assert!(pages.len() > 1);
        assert!(pages
            .iter()
            .all(|page| page.len() <= GRAPH_REVIEW_PAGE_BYTES));
        assert_eq!(
            serde_json::from_str::<Value>(&pages.concat()).unwrap(),
            snapshot
        );
        assert_eq!(pages.concat(), serde_json::to_string(&snapshot).unwrap());
    }
    #[test]
    fn graph_pages_refuse_oversize_or_nonobject_without_truncation() {
        for snapshot in [
            json!(null),
            json!([]),
            json!("not object"),
            json!({"value":"a".repeat(GRAPH_REVIEW_SNAPSHOT_BYTES)}),
        ] {
            assert!(graph_review_pages(&snapshot).is_err());
        }
        let snapshot = json!({"value":"a".repeat(GRAPH_REVIEW_SNAPSHOT_BYTES-12)});
        assert_eq!(
            serde_json::to_string(&snapshot).unwrap().len(),
            GRAPH_REVIEW_SNAPSHOT_BYTES
        );
        assert_eq!(
            graph_review_pages(&snapshot).unwrap().concat(),
            serde_json::to_string(&snapshot).unwrap()
        );
    }
}

#[cfg(test)]
mod factory_tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    fn fixture() -> (Value, Value) {
        let budget = json!({"attempts":3,"parallel":1,"runtime_ms":120000,"output_bytes":8388608,"children":1,"reworks":0,"tokens":null,"cost_microunits":null});
        let input = json!({"artifact_id":"input","relative_path":"input.txt","max_bytes":1024,"expected_sha256":null});
        let output = json!({"artifact_id":"output","relative_path":"report.txt","max_bytes":1024,"expected_sha256":null});
        let selection = json!({"root_task_id":"task","expected_task_revision":1,"grant_id":"grant","expected_grant_revision":2,"connection_ids":["connection"],"workspace_ids":["workspace"],"inputs":[input],"outputs":[output],"recipients":[],"max_depth":1,"max_children":1,"max_total_nodes":2,"root_budget":budget,"project_budget":budget,"ttl_ms":300000,"capabilities":{"delegate_task":true,"resume_parent_as_new_attempt":true,"child_planning":false,"child_goal_execution":false,"create_workspace":false}});
        let contract = |requirement: Value| json!({"canonical_root":"/managed/factory","root_device":1,"root_inode":9223372036854775809u64,"requirement":requirement});
        let bindings = json!({"root_task":{"id":"task","project_id":"project","revision":1,"spec_revision":1,"goal":"Bounded local Factory","state":"draft","created_at":"2026-10-28T00:00:00Z","updated_at":"2026-10-28T00:00:00Z"},"approved_grant":{"id":"grant","root_grant_id":"grant","parent_id":null,"project_id":"project","revision":2,"revoked":false,"scope":{"connection_ids":["connection"],"workspace_ids":["workspace"],"max_children":0,"max_depth":0,"max_attempts":8,"max_concurrency":2,"max_duration_ms":120000}},"connections":[{"id":"connection","revision":2,"executable_sha256":"a".repeat(64),"environment_id":"environment","identity_ref":"identity-unknown","host_id":"local","config_revision":"config","capability_snapshot_id":"capability"}],"workspaces":[{"id":"workspace","project_id":"project","revision":1,"root":"/managed/factory","state":"ready"}],"inputs":[{"contract":contract(input),"sha256":"b".repeat(64),"bytes":3}],"outputs":[contract(output)],"companion":{"executable":"/host/baton","executable_sha256":"c".repeat(64),"argument_template":["factory-mcp","${SESSION_HANDLE_FILE}"],"credential_transport":"private_0600_file_reference","global_daemon_token_available":false},"guarantees":{"filesystem_sandbox":false,"network_sandbox":false,"account_isolation":false,"output_quota":false,"continuous_input_immutability":false,"complete_process_tree_containment":false},"start_contract":{"mode":"single_scope_single_run","root_task_request_digest":"d".repeat(64),"context_continuation":"new_attempt_explicit_handoff_only","implicit_start_on_approval":false,"new_workspace_authority":false}});
        (
            selection.clone(),
            json!({"result":{"id":"factory-proposal","kind":"factory","revision":3,"snapshot":{"schema_version":1,"selection":selection,"bindings":bindings,"contract_digest":"e".repeat(64)}},"revision":8,"warnings":[]}),
        )
    }
    fn scope_result(proposal: &Value) -> Value {
        let snapshot = &proposal["result"]["snapshot"];
        json!({"result":{"id":"factory-proposal","approval_id":"factory-proposal","project_id":"project","root_task_id":"task","root_grant_id":"grant","root_grant_revision":2,"revision":1,"revoked":false,"expires_at_ms":1793145600000i64,"selection":snapshot["selection"],"bindings":snapshot["bindings"],"contract_digest":snapshot["contract_digest"],"created_at":"2026-10-28T00:00:00Z"},"revision":9,"warnings":[]})
    }
    #[test]
    fn factory_full_u64_snapshot_pages_and_original_private_frame_are_preserved() {
        let (selection, proposal) = fixture();
        let result = scope_result(&proposal);
        let messages = RefCell::new(Vec::new());
        let calls = Cell::new(0);
        let returned=confirm_factory_and_finalize(proposal.clone(),&selection,|message|{messages.borrow_mut().push(message.to_owned());true},|frame|{calls.set(calls.get()+1);assert_eq!(frame,json!({"method":"approvals.finalize","params":{"proposal_id":"factory-proposal"},"expected_revision":3,"idempotency_key":"proposal:factory-proposal"}));Ok(result.clone())}).unwrap();
        assert_eq!(returned, result);
        assert_eq!(calls.get(), 1);
        assert!(messages.borrow().len() >= 2);
        assert!(messages
            .borrow()
            .iter()
            .any(|m| m.contains("9223372036854775809")));
        assert!(messages
            .borrow()
            .last()
            .unwrap()
            .contains("FINAL independent Factory"));
        assert_eq!(
            graph_review_pages(&proposal["result"]["snapshot"])
                .unwrap()
                .concat(),
            serde_json::to_string(&proposal["result"]["snapshot"]).unwrap()
        );
    }
    #[test]
    fn factory_cancel_each_page_or_final_is_zero_finalize_not_rejection() {
        let (selection, mut proposal) = fixture();
        proposal["result"]["snapshot"]["bindings"]["root_task"]["goal"] =
            json!("委派".repeat(2000));
        let pages = graph_review_pages(&proposal["result"]["snapshot"])
            .unwrap()
            .len();
        assert!(pages > 1);
        for decline in 0..=pages {
            let prompts = Cell::new(0);
            let calls = Cell::new(0);
            let value = confirm_factory_and_finalize(
                proposal.clone(),
                &selection,
                |_| {
                    let index = prompts.get();
                    prompts.set(index + 1);
                    index != decline
                },
                |_| {
                    calls.set(calls.get() + 1);
                    Ok(json!({}))
                },
            )
            .unwrap();
            assert_eq!(value["error"]["code"], "PERMISSION_DENIED");
            assert_eq!(calls.get(), 0);
        }
    }
    #[test]
    fn factory_foreign_task_grant_unsupported_or_companion_authority_never_prompts() {
        let (selection, proposal) = fixture();
        let mut variants = Vec::new();
        for path in [
            "task",
            "grant",
            "workspace",
            "companion",
            "token",
            "start",
            "capability",
        ] {
            let mut p = proposal.clone();
            let b = &mut p["result"]["snapshot"]["bindings"];
            match path {
                "task" => b["root_task"]["id"] = json!("foreign"),
                "grant" => b["approved_grant"]["revision"] = json!(3),
                "workspace" => b["workspaces"][0]["id"] = json!("foreign"),
                "companion" => b["companion"]["argument_template"] = json!(["--different"]),
                "token" => b["companion"]["global_daemon_token_available"] = json!(true),
                "start" => b["start_contract"]["implicit_start_on_approval"] = json!(true),
                _ => {
                    p["result"]["snapshot"]["selection"]["capabilities"]["create_workspace"] =
                        json!(true)
                }
            };
            variants.push(p);
        }
        for p in variants {
            let prompts = Cell::new(0);
            let calls = Cell::new(0);
            let value = confirm_factory_and_finalize(
                p,
                &selection,
                |_| {
                    prompts.set(prompts.get() + 1);
                    true
                },
                |_| {
                    calls.set(calls.get() + 1);
                    Ok(json!({}))
                },
            )
            .unwrap();
            assert_eq!(value["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
            assert_eq!(prompts.get(), 0);
            assert_eq!(calls.get(), 0);
        }
    }
    #[test]
    fn factory_wrong_postfinalize_id_bindings_or_digest_retains_unknown() {
        let (selection, proposal) = fixture();
        for field in ["id", "approval_id", "contract_digest", "bindings"] {
            let mut result = scope_result(&proposal);
            result["result"][field] = json!("foreign");
            let value = confirm_factory_and_finalize(
                proposal.clone(),
                &selection,
                |_| true,
                |_| Ok(result),
            )
            .unwrap();
            assert_eq!(value["error"]["code"], "OUTCOME_UNKNOWN");
            assert_eq!(value["error"]["side_effect_state"], "unknown");
        }
        for malformed in [json!({"error":true}), json!({"error":{"code":"BROKEN"}})] {
            let value = confirm_factory_and_finalize(
                proposal.clone(),
                &selection,
                |_| true,
                |_| Ok(malformed),
            )
            .unwrap();
            assert_eq!(value["error"]["code"], "OUTCOME_UNKNOWN");
            assert_eq!(value["error"]["side_effect_state"], "unknown");
        }
        let value = confirm_factory_and_finalize(
            proposal.clone(),
            &selection,
            |_| true,
            |_| Err("transport reply lost".into()),
        );
        assert_eq!(value.unwrap_err(), "transport reply lost");
    }
    #[test]
    fn factory_kind_and_envelope_cap_are_independent_from_old_kinds() {
        let (selection, proposal) = fixture();
        let mut wrong = proposal.clone();
        wrong["result"]["kind"] = json!("goal_execution");
        let denied = confirm_factory_and_finalize(
            wrong,
            &selection,
            |_| panic!("wrong kind prompt"),
            |_| panic!("wrong kind finalize"),
        )
        .unwrap();
        assert_eq!(denied["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
        let mut huge = proposal;
        huge["result"]["snapshot"]["bindings"]["root_task"]["goal"] = json!("a".repeat(1_048_576));
        let denied = confirm_factory_and_finalize(
            huge,
            &selection,
            |_| panic!("oversize prompt"),
            |_| panic!("oversize finalize"),
        )
        .unwrap();
        assert_eq!(denied["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
    }
    #[test]
    fn factory_root_unknown_or_running_is_not_an_initial_new_authority() {
        let (selection, proposal) = fixture();
        for state in ["running", "outcome_unknown", "accepted"] {
            let mut p = proposal.clone();
            p["result"]["snapshot"]["bindings"]["root_task"]["state"] = json!(state);
            let denied = confirm_factory_and_finalize(
                p,
                &selection,
                |_| panic!("unsafe root prompt"),
                |_| panic!("unsafe root finalize"),
            )
            .unwrap();
            assert_eq!(denied["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
        }
    }
}
