//! Pure validation of observations. These tests are NOT OS-dialog evidence.
use serde_json::Value;
use sha2::{Digest, Sha256};

// Evidence limits only: an over-limit observation fails; it is never truncated
// and used as an eligible title/message. This helper grants no click authority.
pub const DIAGNOSTIC_TEXT_BYTES: usize = 32_768;
pub const DIAGNOSTIC_TOTAL_TEXT_BYTES: usize = 65_536;
pub fn diagnostic_text(text: &str, total: &mut usize) -> Result<Value, String> {
    let next = total
        .checked_add(text.len())
        .ok_or("Diagnostic text size overflow")?;
    if text.len() > DIAGNOSTIC_TEXT_BYTES || next > DIAGNOSTIC_TOTAL_TEXT_BYTES {
        return Err("Own sheet diagnostic text byte limit".into());
    }
    *total = next;
    Ok(
        serde_json::json!({"text":text,"utf8_bytes":text.len(),"sha256":format!("{:x}", Sha256::digest(text.as_bytes()))}),
    )
}

fn opaque_id(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|id| !id.trim().is_empty() && id.len() <= 512)
}
/// Test-driver response check only. The Grant approval ID is an independent
/// Core-generated opaque ID, not the Owner proposal ID. The host subsequently
/// binds this entire result to readonly DB/proposal result and both real events.
pub fn validate_grant_response(response: &Value, expected: &Value) -> Result<(), String> {
    let proposal = &expected["proposal"];
    let intent = &proposal["snapshot"]["intent"];
    let grant = &response["result"];
    if response.get("error").is_some()
        || proposal["kind"] != "grant"
        || !opaque_id(&proposal["id"])
        || !opaque_id(&grant["id"])
        || !opaque_id(&grant["approval_id"])
        || grant["project_id"] != intent["project_id"]
        || grant["root_grant_id"] != grant["id"]
        || !grant["parent_id"].is_null()
        || grant["depth"].as_u64() != Some(0)
        || grant["revision"].as_u64() != Some(1)
        || grant["revoked"] != false
    {
        return Err("Grant result identity or immutable root shape mismatch".into());
    }
    let workspace = intent["workspace_id"]
        .as_str()
        .ok_or("Missing workspace intent")?;
    let connection = intent["connection_id"]
        .as_str()
        .ok_or("Missing connection intent")?;
    let scope = serde_json::json!({"workspace_ids":[workspace],"connection_ids":[connection],"host_ids":["local"],
        "max_attempts":intent["max_attempts"],"max_concurrency":intent["max_concurrency"],"max_duration_ms":intent["max_duration_ms"],
        "recipients":[],"network":{"mode":"deny"},"max_depth":0,"max_children":0});
    let enforcement = serde_json::json!({"baton_mediated_scope":true,"os_filesystem_sandbox":false,"os_network_sandbox":false,"native_identity_isolation":false});
    if grant["scope"] != scope
        || grant["enforcement"] != enforcement
        || !proposal["snapshot"]["refs"]["workspace_identity"].is_object()
        || !proposal["snapshot"]["refs"]["connection"].is_object()
        || grant["workspace_bindings"]
            .as_object()
            .is_none_or(|bindings| bindings.len() != 1)
        || grant["connection_bindings"]
            .as_object()
            .is_none_or(|bindings| bindings.len() != 1)
        || grant["workspace_bindings"][workspace]
            != proposal["snapshot"]["refs"]["workspace_identity"]
        || grant["connection_bindings"][connection] != proposal["snapshot"]["refs"]["connection"]
    {
        return Err("Grant result scope, enforcement or exact approved bindings mismatch".into());
    }
    Ok(())
}

pub fn validate(
    observed: &Value,
    expected: &Value,
    phase: &str,
    index: usize,
) -> Result<(), String> {
    if !matches!(phase, "cancel" | "approve")
        || observed["phase"] != phase
        || observed["proposal_id"] != expected["proposal"]["id"]
        || observed["proposal_revision"] != expected["proposal"]["revision"]
        || expected["presentation"]
            .as_array()
            .is_none_or(|pages| pages.is_empty() || pages.len() > 1024 || index >= pages.len())
        || observed["presentation_index"] != index
        || observed["presentation_count"]
            != expected["presentation"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0)
        || observed["message"] != expected["presentation"][index]
        || observed["title"] != "AgentBaton — Native owner approval"
        || observed["parent_matches"] != true
        || observed["visible"] != true
        || observed["complete_text_visible"] != true
        || observed["window_number"].as_i64().is_none_or(|n| n <= 0)
    {
        return Err(
            "Own sheet, exact proposal/message or complete visible layout did not match".into(),
        );
    }
    let buttons = observed["buttons"]
        .as_array()
        .ok_or("No button observations")?;
    if buttons.len() != 2
        || ["OK", "Cancel"].iter().any(|title| {
            buttons
                .iter()
                .filter(|b| b["title"] == *title && b["enabled"] == true && b["visible"] == true)
                .count()
                != 1
        })
    {
        return Err("Expected precisely the real enabled visible OK/Cancel controls".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> (Value, Value) {
        let expected = json!({"proposal":{"id":"exact-grant", "revision":1}, "message":"complete snapshot\n你好", "presentation":["exact encoded page","FINAL display confirmation"]});
        let observed = json!({"phase":"cancel", "proposal_id":"exact-grant", "proposal_revision":1,
            "message":"exact encoded page", "presentation_index":0,"presentation_count":2, "title":"AgentBaton — Native owner approval",
            "parent_matches":true,"visible":true,"complete_text_visible":true,"window_number":7,
            "buttons":[{"title":"OK","enabled":true,"visible":true},{"title":"Cancel","enabled":true,"visible":true}]});
        (expected, observed)
    }
    fn grant_response_fixture() -> (Value, Value) {
        let expected = json!({"proposal":{"id":"owner-proposal","kind":"grant","snapshot":{"intent":{"project_id":"project","workspace_id":"workspace","connection_id":"connection","max_attempts":1,"max_concurrency":1,"max_duration_ms":1000},"refs":{"workspace_identity":{"root":"fixed"},"connection":{"revision":2}}}}});
        let response = json!({"result":{"id":"grant","approval_id":"independent-grant-approval","project_id":"project","root_grant_id":"grant","parent_id":null,"depth":0,"revision":1,"revoked":false,
            "scope":{"workspace_ids":["workspace"],"connection_ids":["connection"],"host_ids":["local"],"max_attempts":1,"max_concurrency":1,"max_duration_ms":1000,"recipients":[],"network":{"mode":"deny"},"max_depth":0,"max_children":0},
            "enforcement":{"baton_mediated_scope":true,"os_filesystem_sandbox":false,"os_network_sandbox":false,"native_identity_isolation":false},"workspace_bindings":{"workspace":{"root":"fixed"}},"connection_bindings":{"connection":{"revision":2}}}});
        (expected, response)
    }
    #[test]
    fn grant_response_uses_independent_opaque_approval_identity_and_exact_scope() {
        let (expected, response) = grant_response_fixture();
        assert_ne!(
            response["result"]["approval_id"],
            expected["proposal"]["id"]
        );
        assert!(validate_grant_response(&response, &expected).is_ok());
        for field in ["id", "approval_id"] {
            for bad in [json!(null), json!(""), json!(" "), json!("x".repeat(513))] {
                let mut changed = response.clone();
                changed["result"][field] = bad;
                assert!(validate_grant_response(&changed, &expected).is_err());
            }
        }
    }
    #[test]
    fn foreign_or_expanded_grant_response_and_error_never_complete_os_fixture() {
        let (expected, response) = grant_response_fixture();
        for path in [
            "/result/project_id",
            "/result/root_grant_id",
            "/result/scope/host_ids",
            "/result/scope/network",
            "/result/scope/max_attempts",
            "/result/workspace_bindings/workspace",
            "/result/connection_bindings/connection",
            "/result/enforcement/os_filesystem_sandbox",
        ] {
            let mut changed = response.clone();
            *changed.pointer_mut(path).unwrap() = json!("foreign");
            assert!(
                validate_grant_response(&changed, &expected).is_err(),
                "{path}"
            );
        }
        let mut changed = response;
        changed["error"] = json!({"code":"OUTCOME_UNKNOWN"});
        assert!(validate_grant_response(&changed, &expected).is_err());
    }
    #[test]
    fn complete_owned_sheet_observation_is_eligible_for_a_real_click_only() {
        let (expected, observed) = fixture();
        assert!(validate(&observed, &expected, "cancel", 0).is_ok());
    }
    #[test]
    fn exact_page_order_count_and_final_text_cannot_be_replaced_with_original_or_partial() {
        let (expected, observed) = fixture();
        for (field, wrong) in [
            ("presentation_index", json!(1)),
            ("presentation_count", json!(1)),
            ("message", expected["message"].clone()),
            ("message", json!("exact encoded")),
        ] {
            let mut value = observed.clone();
            value[field] = wrong;
            assert!(validate(&value, &expected, "cancel", 0).is_err());
        }
        assert!(validate(&observed, &expected, "cancel", 1).is_err());
        let mut final_page = observed;
        final_page["presentation_index"] = json!(1);
        final_page["message"] = expected["presentation"][1].clone();
        assert!(validate(&final_page, &expected, "cancel", 1).is_ok());
        final_page["complete_text_visible"] = json!(false);
        assert!(validate(&final_page, &expected, "cancel", 1).is_err());
    }
    #[test]
    fn missing_or_out_of_bound_page_set_never_grants_a_click() {
        let (mut expected, observed) = fixture();
        for pages in [json!(null), json!([]), json!(vec!["x"; 1025])] {
            expected["presentation"] = pages;
            assert!(validate(&observed, &expected, "cancel", 0).is_err());
        }
    }
    #[test]
    fn foreign_parent_and_wrong_proposal_or_revision_never_click() {
        let (expected, observed) = fixture();
        for (field, wrong) in [
            ("parent_matches", json!(false)),
            ("proposal_id", json!("foreign")),
            ("proposal_revision", json!(2)),
        ] {
            let mut value = observed.clone();
            value[field] = wrong;
            assert!(validate(&value, &expected, "cancel", 0).is_err());
        }
    }
    #[test]
    fn clipped_or_partial_message_is_not_reviewed() {
        let (expected, observed) = fixture();
        let mut value = observed.clone();
        value["complete_text_visible"] = json!(false);
        assert!(validate(&value, &expected, "cancel", 0).is_err());
        let mut value = observed;
        value["message"] = json!("complete snapshot");
        assert!(validate(&value, &expected, "cancel", 0).is_err());
    }
    #[test]
    fn missing_disabled_ambiguous_buttons_and_wrong_phase_are_rejected() {
        let (expected, observed) = fixture();
        for buttons in [
            json!([]),
            json!([{"title":"OK","enabled":true,"visible":true}]),
            json!([{"title":"OK","enabled":false,"visible":true},{"title":"Cancel","enabled":true,"visible":true}]),
            json!([{"title":"OK","enabled":true,"visible":true},{"title":"OK","enabled":true,"visible":true}]),
        ] {
            let mut value = observed.clone();
            value["buttons"] = buttons;
            assert!(validate(&value, &expected, "cancel", 0).is_err());
        }
        assert!(validate(&observed, &expected, "approve", 0).is_err());
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    fn raw_utf8_text_hash_and_length_are_not_a_partial_match_or_click_permit() {
        let mut total = 0;
        let value = diagnostic_text("abc", &mut total).unwrap();
        assert_eq!(value["text"], "abc");
        assert_eq!(value["utf8_bytes"], 3);
        assert_eq!(
            value["sha256"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let value = diagnostic_text("你好🔒\n", &mut total).unwrap();
        assert_eq!(value["text"], "你好🔒\n");
        assert_eq!(value["utf8_bytes"], 11);
        assert_eq!(total, 14);
        assert!(validate(&value, &serde_json::json!({}), "cancel", 0).is_err());
    }
    #[test]
    fn per_field_and_aggregate_limits_reject_without_truncation_or_budget_mutation() {
        let mut total = 0;
        let field = "x".repeat(DIAGNOSTIC_TEXT_BYTES);
        assert!(diagnostic_text(&field, &mut total).is_ok());
        assert!(diagnostic_text(&field, &mut total).is_ok());
        assert_eq!(total, DIAGNOSTIC_TOTAL_TEXT_BYTES);
        assert!(diagnostic_text("x", &mut total).is_err());
        assert_eq!(total, DIAGNOSTIC_TOTAL_TEXT_BYTES);
        let mut zero = 0;
        assert!(diagnostic_text(&(field + "x"), &mut zero).is_err());
        assert_eq!(zero, 0);
        let mut overflow = usize::MAX;
        assert!(diagnostic_text("x", &mut overflow).is_err());
        assert_eq!(overflow, usize::MAX);
    }
}
