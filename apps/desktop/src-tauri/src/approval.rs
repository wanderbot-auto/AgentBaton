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
            Self::DataWorkspace => "Approve ONE exact selected-file dataset copy from the server-resolved registered source to its host-managed destination. Review the project, preview revision, every file path, actual SHA-256/bytes, dataset revision and destination in the COMPLETE snapshot. Preview verification alone does not authorize copying. No original source writes, Git initialization, scripts, model calls or execution grant. Selected-file before/after observations are NOT an atomic whole-directory freeze, continuous immutability, DLP or an OS filesystem sandbox. Cancellation is a request; completed or partial copies can remain and must be reconciled, never silently replayed or deleted.\n\n独立数据工作区原生批准：完整核对项目、预览版本、所选文件路径/实际哈希/字节、数据集版本及后台目标。仅批准该精确副本，不修改原源、不初始化 Git、不执行脚本或模型、不授予执行权限；不保证整目录冻结、DLP 或系统隔离。取消请求不等于已停止或删除。",
        }
    }
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
    if snapshot.len() > 16_384 {
        return refused("Approval snapshot exceeds the native review limit; nothing was approved");
    }
    let message = format!("{}\n\nThis confirmation comes from the native application, outside the web renderer. Review the SERVER-resolved snapshot in full. Values below are JSON-escaped.\n\nNo verified OS filesystem/network sandbox or native account isolation is claimed. A selected program may access resources available to this user. Version evidence does not prove authentication or model compatibility.\n\n原生负责人确认：请完整核对以下后台解析的程序、参数或执行范围。程序可能访问当前账号可用资源；不保证系统沙箱、网络隔离或原生账号隔离。取消将阻止本次批准，输入变化后必须重新提议。\n\n{}\n\nCancel denies this action. Approval is bound to this server proposal and its revision; changed inputs must be proposed again.", kind.action(), snapshot);
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
}
