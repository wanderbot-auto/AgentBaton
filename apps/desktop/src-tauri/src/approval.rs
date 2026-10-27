//! Native-only approval bridge. No finalize method is registered with Tauri.
use serde_json::{json, Value};

pub enum ApprovalKind {
    Connection,
    Grant,
    Validation,
    DataValidation,
    CommandValidation,
}
impl ApprovalKind {
    pub fn proposal_method(&self) -> &'static str {
        match self {
            Self::Connection => "connections.propose",
            Self::Grant => "grants.propose",
            Self::Validation => "validation_policies.propose",
            Self::DataValidation => "data_validation_policies.propose",
            Self::CommandValidation => "command_validation_policies.propose",
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::Connection => "connection",
            Self::Grant => "grant",
            Self::Validation => "validation",
            Self::DataValidation => "data_validation",
            Self::CommandValidation => "command_validation",
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
        }
    }
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
}
