//! Native-only approval bridge. No finalize method is registered with Tauri.
use serde_json::{json, Value};

pub enum ApprovalKind {
    Connection,
    Grant,
    Validation,
    DataValidation,
}
impl ApprovalKind {
    pub fn proposal_method(&self) -> &'static str {
        match self {
            Self::Connection => "connections.propose",
            Self::Grant => "grants.propose",
            Self::Validation => "validation_policies.propose",
            Self::DataValidation => "data_validation_policies.propose",
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::Connection => "connection",
            Self::Grant => "grant",
            Self::Validation => "validation",
            Self::DataValidation => "data_validation",
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
        }
    }
}

pub fn confirm_and_finalize(
    response: Value,
    kind: ApprovalKind,
    confirm_native: impl FnOnce(&str) -> bool,
    finalize: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    if response.get("error").is_some() {
        return Ok(response);
    }
    let proposal = response
        .get("result")
        .ok_or("Server returned no approval proposal")?;
    if proposal.get("kind").and_then(Value::as_str) != Some(kind.kind()) {
        return Err("Server proposal kind does not match this native action".into());
    }
    let id = proposal
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or("Invalid server proposal ID")?;
    let revision = proposal
        .get("revision")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or("Invalid server proposal revision")?;
    let snapshot = proposal
        .get("snapshot")
        .filter(|value| value.is_object())
        .ok_or("Server returned no approval snapshot")?;
    // Native dialogs must show the entire exact snapshot. Oversized proposals
    // are rejected rather than silently truncating the executable, argv or scope.
    let snapshot = serde_json::to_string_pretty(snapshot).map_err(|e| e.to_string())?;
    if snapshot.len() > 16_384 {
        return Err(
            "Approval snapshot exceeds the native review limit; nothing was approved".into(),
        );
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
    #[test]
    fn wrong_kind_and_oversized_snapshot_cannot_request_confirmation() {
        let confirm = |_: &str| -> bool { panic!("invalid proposal must not show approval") };
        assert!(confirm_and_finalize(
            proposal("connection"),
            ApprovalKind::Grant,
            confirm,
            |_| unreachable!()
        )
        .is_err());
        let mut large = proposal("grant");
        large["result"]["snapshot"]["arguments"] = json!("x".repeat(20_000));
        assert!(
            confirm_and_finalize(large, ApprovalKind::Grant, confirm, |_| unreachable!()).is_err()
        );
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
        assert!(confirm_and_finalize(
            proposal("validation"),
            ApprovalKind::DataValidation,
            |_| panic!("integrity approval cannot be elevated to a data contract"),
            |_| unreachable!()
        )
        .is_err());
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
}
