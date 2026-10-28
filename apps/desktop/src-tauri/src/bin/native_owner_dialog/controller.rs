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
