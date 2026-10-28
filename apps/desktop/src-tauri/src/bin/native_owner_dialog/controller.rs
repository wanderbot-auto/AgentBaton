//! Pure validation of observations. These tests are NOT OS-dialog evidence.
use serde_json::Value;

pub fn validate(observed: &Value, expected: &Value, phase: &str) -> Result<(), String> {
    if !matches!(phase, "cancel" | "approve")
        || observed["phase"] != phase
        || observed["proposal_id"] != expected["proposal"]["id"]
        || observed["proposal_revision"] != expected["proposal"]["revision"]
        || observed["message"] != expected["message"]
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
        let expected = json!({"proposal":{"id":"exact-grant", "revision":1}, "message":"complete snapshot\n你好"});
        let observed = json!({"phase":"cancel", "proposal_id":"exact-grant", "proposal_revision":1,
            "message":"complete snapshot\n你好", "title":"AgentBaton — Native owner approval",
            "parent_matches":true,"visible":true,"complete_text_visible":true,"window_number":7,
            "buttons":[{"title":"OK","enabled":true,"visible":true},{"title":"Cancel","enabled":true,"visible":true}]});
        (expected, observed)
    }
    #[test]
    fn complete_owned_sheet_observation_is_eligible_for_a_real_click_only() {
        let (expected, observed) = fixture();
        assert!(validate(&observed, &expected, "cancel").is_ok());
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
            assert!(validate(&value, &expected, "cancel").is_err());
        }
    }
    #[test]
    fn clipped_or_partial_message_is_not_reviewed() {
        let (expected, observed) = fixture();
        let mut value = observed.clone();
        value["complete_text_visible"] = json!(false);
        assert!(validate(&value, &expected, "cancel").is_err());
        let mut value = observed;
        value["message"] = json!("complete snapshot");
        assert!(validate(&value, &expected, "cancel").is_err());
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
            assert!(validate(&value, &expected, "cancel").is_err());
        }
        assert!(validate(&observed, &expected, "approve").is_err());
    }
}
