//! Pure validation of observations. These tests are NOT OS-dialog evidence.
use agentbaton_desktop::native_review_test_limits::{
    REVIEW_MESSAGE_BYTES, REVIEW_ROWS_PER_PAGE, REVIEW_ROW_ENCODED_BYTES,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
#[cfg(target_os = "macos")]
#[path = "integration_database.rs"]
mod integration_database;
#[cfg(target_os = "macos")]
pub(crate) use integration_database::checkpoint as integration_database_checkpoint;

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

pub fn fixture_arguments(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(std::ffi::OsString, bool, u64), String> {
    let first = args.next().ok_or("Expected generated host fixture file")?;
    let integration = first == "--integration";
    let file = if integration {
        args.next().ok_or("Expected integration fixture file")?
    } else {
        first
    };
    if args.next().is_some() {
        return Err("Unexpected OS fixture argument".into());
    }
    Ok((
        file,
        integration,
        if integration { 1_048_576 } else { 65536 },
    ))
}
pub const INTEGRATION_KIND: &str = "native_integration_os_dialog_fixture";
pub fn is_integration(expected: &Value) -> bool {
    expected["kind"] == INTEGRATION_KIND
}
pub fn is_independent(expected: &Value) -> bool {
    is_factory(expected) || is_integration(expected)
}
pub const FACTORY_KIND: &str = "native_factory_planning_os_dialog_fixture";
pub fn is_factory(expected: &Value) -> bool {
    expected["kind"] == FACTORY_KIND
}
pub fn dialog_title(expected: &Value) -> &'static str {
    if is_integration(expected) {
        agentbaton_desktop::native_integration_test_view::TITLE
    } else if is_factory(expected) {
        "AgentBaton — Independent Factory approval"
    } else {
        "AgentBaton — Native owner approval"
    }
}
fn hex(value: &Value, len: usize) -> bool {
    value.as_str().is_some_and(|s| {
        s.len() == len
            && s.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    })
}
fn digest(value: &Value) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(|e| e.to_string())?)
    ))
}
/// Test-only reconstruction of each COMPLETE production Factory callback.
/// Python independently reconstructs all snapshot bytes and validates every
/// actual OS text. This mirror does not replace the shipping formatter/bridge.
pub fn factory_messages(proposal: &Value) -> Result<Vec<String>, String> {
    let snapshot = &proposal["snapshot"];
    let selection = &snapshot["selection"];
    if proposal["kind"] != "factory"
        || !opaque_id(&proposal["id"])
        || proposal["revision"].as_u64().is_none_or(|n| n == 0)
        || proposal["consumed"] != false
        || selection["capabilities"]["child_planning"] != true
        || selection["capabilities"]["child_goal_execution"] != false
        || selection["capabilities"]["create_workspace"] != false
        || !selection["child_planning_policy"].is_object()
        || snapshot["bindings"]["child_planning"]["policy"] != selection["child_planning_policy"]
    {
        return Err("Only a complete exact Factory planning proposal is supported".into());
    }
    let complete = serde_json::to_string(snapshot).map_err(|e| e.to_string())?;
    if !snapshot.is_object() || complete.len() > 1_048_576 {
        return Err("Complete Factory snapshot exceeds original 1MiB".into());
    }
    let mut pages = Vec::new();
    let mut start = 0;
    while start < complete.len() {
        let mut end = (start + 12_288).min(complete.len());
        while !complete.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            return Err("UTF8 Factory page boundary".into());
        }
        pages.push(complete[start..end].to_owned());
        start = end;
    }
    if pages.concat() != complete {
        return Err("Incomplete Factory snapshot".into());
    }
    let id = proposal["id"].as_str().unwrap();
    let revision = proposal["revision"].as_u64().unwrap();
    let total = complete.len();
    let mut offset = 0;
    let action = "Review INDEPENDENT bounded Factory delegation AND versioned child PLAN STORAGE ONLY. Review every planning limit, output/validator definition, writable path and Owner acceptance criterion in the complete server snapshot. Approval does NOT start a run or execute a plan/validator. Child Goal execution, workspace creation, Task acceptance, new Grant authority, OS sandbox and native-context restoration are NOT granted. Parent continuation remains a NEW attempt. / 独立批准有界委派与子计划版本存储；完整核对规划限额、输出、检查定义、可写路径及负责人验收标准。批准不启动，不执行计划或检查，不授予子Goal执行、工作区创建、任务验收、新Grant权限或系统隔离；父任务仍以新尝试交接。";
    let mut messages = Vec::new();
    for (index, page) in pages.iter().enumerate() {
        let end = offset + page.len();
        let message=format!("{action}\n\nFactory proposal {id} / revision {revision}. Page {}/{}; complete JSON UTF-8 bytes {}..{} of {}. OK only reviews this page; final confirmation is still required. Cancel stops review without a rejection receipt.\n\n{page}",index+1,pages.len(),offset,end,total);
        if message.len() > 16384 {
            return Err("Factory outer display bound".into());
        }
        messages.push(message);
        offset = end;
    }
    messages.push(format!("{action}\n\nFINAL independent Factory approval — proposal {id} / revision {revision}. All {} complete JSON pages ({total} UTF-8 bytes) reviewed. OK records only this exact proposal. Changed inputs require a fresh proposal; unknown replies require reconciliation. Cancel records no rejection.",pages.len()));
    Ok(messages)
}

/// Expand a <=64KiB fixed fixture into its independently bound full presentation.
/// Descriptor digests are evidence bindings only; no click/finalize authority.
pub fn prepare_factory(expected: &mut Value) -> Result<(), String> {
    let fields = [
        "kind",
        "runtime",
        "output",
        "proposal",
        "invoke",
        "baseline_sequence",
        "phase",
        "cancel_at",
        "review_nonce",
        "snapshot_sha256",
        "outer_message_sha256",
        "presentation_sha256",
        "presentation_count",
    ];
    if !is_factory(expected)
        || expected
            .as_object()
            .is_none_or(|o| o.len() != fields.len() || fields.iter().any(|k| !o.contains_key(*k)))
        || !matches!(expected["phase"].as_str(), Some("cancel" | "approve"))
        || !hex(&expected["review_nonce"], 32)
        || expected["baseline_sequence"].as_u64().is_none()
        || expected["invoke"].as_object().is_none_or(|v| v.len() != 2)
        || expected["invoke"]["idempotencyKey"] != "owner-os-factory-planning"
        || expected["invoke"]["selection"] != expected["proposal"]["snapshot"]["selection"]
        || expected["snapshot_sha256"] != digest(&expected["proposal"]["snapshot"])?
    {
        return Err("Factory fixture kind/CAS/nonce/full selection mismatch".into());
    }
    let messages = factory_messages(&expected["proposal"])?;
    let outer_hashes: Vec<_> = messages
        .iter()
        .map(|m| format!("{:x}", Sha256::digest(m.as_bytes())))
        .collect();
    if expected["outer_message_sha256"] != serde_json::json!(outer_hashes) {
        return Err("Factory outer messages changed".into());
    }
    let mut presentation = Vec::new();
    let mut contexts = Vec::new();
    for (outer, message) in messages.iter().enumerate() {
        let inner = mirrored_review_prompts(message)?;
        for (index, page) in inner.iter().enumerate() {
            presentation.push(page.clone());
            contexts.push(
                serde_json::json!({"outer_index":outer,"outer_count":messages.len(),
               "inner_index":index,"inner_count":inner.len(),"outer_final":outer+1==messages.len(),
               "inner_final":index+1==inner.len(),"message_sha256":outer_hashes[outer]}),
            );
        }
    }
    if presentation.len() < 2
        || presentation.len() > 1024
        || presentation.iter().any(|p| p.len() > 2048)
        || expected["presentation_count"].as_u64() != Some(presentation.len() as u64)
        || expected["cancel_at"]
            .as_u64()
            .is_none_or(|n| n >= presentation.len() as u64)
        || expected["presentation_sha256"] != digest(&serde_json::json!(presentation))?
    {
        return Err("Factory complete nested presentation/count/cancel boundary mismatch".into());
    }
    expected["presentation"] = serde_json::json!(presentation);
    expected["factory_contexts"] = serde_json::json!(contexts);
    Ok(())
}
pub fn validate_factory_identity(
    value: &Value,
    expected: &Value,
    index: usize,
) -> Result<(), String> {
    if !is_factory(expected) {
        return Err("Wrong Factory fixture kind".into());
    }
    let context = &expected["factory_contexts"][index];
    if value["review_nonce"] != expected["review_nonce"]
        || !hex(&value["review_nonce"], 32)
        || !context.is_object()
        || value["factory_page"] != *context
        || ["outer_index", "outer_count", "inner_index", "inner_count"]
            .iter()
            .any(|k| {
                value["factory_page"][*k].as_u64() != context[*k].as_u64()
                    || context[*k].as_u64().is_none()
            })
    {
        return Err("Factory outer/inner identity or fresh per-process nonce mismatch".into());
    }
    Ok(())
}
pub fn cancel_here(expected: &Value, phase: &str, index: usize) -> Result<bool, String> {
    let count = expected["presentation"]
        .as_array()
        .ok_or("No display pages")?
        .len();
    if !matches!(phase, "cancel" | "approve") || index >= count {
        return Err("Out of order phase/page".into());
    }
    if is_independent(expected) {
        if expected["phase"] != phase {
            return Err("Foreign Factory phase".into());
        }
        let cancel = expected["cancel_at"]
            .as_u64()
            .filter(|n| *n < count as u64)
            .ok_or("Typed bounded cancel index required")?;
        Ok(phase == "cancel" && cancel == index as u64)
    } else {
        Ok(phase == "cancel" && index + 1 == count)
    }
}

pub fn validate_factory_response(response: &Value, expected: &Value) -> Result<(), String> {
    let p = &expected["proposal"];
    let s = &response["result"];
    if !is_factory(expected)
        || response.get("error").is_some()
        || p["kind"] != "factory"
        || s["id"] != p["id"]
        || s["approval_id"] != p["id"]
        || s["root_task_id"] != p["snapshot"]["selection"]["root_task_id"]
        || s["root_grant_id"] != p["snapshot"]["selection"]["grant_id"]
        || s["root_grant_revision"].as_u64()
            != p["snapshot"]["selection"]["expected_grant_revision"].as_u64()
        || s["project_id"] != p["snapshot"]["bindings"]["root_task"]["project_id"]
        || s["selection"] != p["snapshot"]["selection"]
        || s["bindings"] != p["snapshot"]["bindings"]
        || s["contract_digest"] != p["snapshot"]["contract_digest"]
        || s["revision"].as_u64() != Some(1)
        || s["revoked"] != false
    {
        return Err("Factory response does not bind exact complete approved scope".into());
    }
    Ok(())
}

/// This test-only descriptor delegates to the real production formatter.
/// It is not a source of Core approval or a substitute for the final DB check.
pub fn prepare_integration(expected: &mut Value) -> Result<(), String> {
    let fields = [
        "kind",
        "runtime",
        "output",
        "proposal",
        "invoke",
        "baseline_sequence",
        "phase",
        "cancel_at",
        "review_nonce",
        "snapshot_sha256",
        "outer_message_sha256",
        "presentation_sha256",
        "presentation_count",
    ];
    if !is_integration(expected)
        || expected
            .as_object()
            .is_none_or(|o| o.len() != fields.len() || fields.iter().any(|k| !o.contains_key(*k)))
        || !matches!(expected["phase"].as_str(), Some("cancel" | "approve"))
        || !hex(&expected["review_nonce"], 32)
        || expected["baseline_sequence"].as_u64().is_none()
        || expected["invoke"].as_object().is_none_or(|o| o.len() != 2)
        || expected["invoke"]["idempotencyKey"] != "owner-os-integration"
        || expected["invoke"]["selection"] != expected["proposal"]["snapshot"]["intent"]
        || expected["snapshot_sha256"] != digest(&expected["proposal"]["snapshot"])?
    {
        return Err("Integration fixture kind/CAS/nonce/complete intent mismatch".into());
    }
    let selection = serde_json::from_value(expected["invoke"]["selection"].clone())
        .map_err(|_| "Invalid typed integration selection")?;
    let messages = agentbaton_desktop::native_integration_test_view::review_messages(
        &expected["proposal"],
        &selection,
    )?;
    let hashes: Vec<_> = messages
        .iter()
        .map(|m| format!("{:x}", Sha256::digest(m.as_bytes())))
        .collect();
    if expected["outer_message_sha256"] != serde_json::json!(hashes) {
        return Err("Integration outer messages changed".into());
    }
    let mut presentation = Vec::new();
    let mut contexts = Vec::new();
    for (outer, message) in messages.iter().enumerate() {
        let inner = mirrored_review_prompts(message)?;
        for (index, page) in inner.iter().enumerate() {
            presentation.push(page.clone());
            contexts.push(serde_json::json!({"outer_index":outer,"outer_count":messages.len(),"inner_index":index,"inner_count":inner.len(),"outer_final":outer+1==messages.len(),"inner_final":index+1==inner.len(),"message_sha256":hashes[outer]}));
        }
    }
    if presentation.len() < 2
        || presentation.len() > 1024
        || presentation.iter().any(|p| p.len() > 2048)
        || expected["presentation_count"].as_u64() != Some(presentation.len() as u64)
        || expected["cancel_at"]
            .as_u64()
            .is_none_or(|n| n >= presentation.len() as u64)
        || expected["presentation_sha256"] != digest(&serde_json::json!(presentation))?
    {
        return Err("Integration full nested display/cancel boundary mismatch".into());
    }
    expected["presentation"] = serde_json::json!(presentation);
    expected["integration_contexts"] = serde_json::json!(contexts);
    Ok(())
}
pub fn validate_integration_identity(
    value: &Value,
    expected: &Value,
    index: usize,
) -> Result<(), String> {
    let c = &expected["integration_contexts"][index];
    if !is_integration(expected)
        || value["review_nonce"] != expected["review_nonce"]
        || !hex(&value["review_nonce"], 32)
        || !c.is_object()
        || value["integration_page"] != *c
        || ["outer_index", "outer_count", "inner_index", "inner_count"]
            .iter()
            .any(|k| {
                c[*k].as_u64().is_none() || value["integration_page"][*k].as_u64() != c[*k].as_u64()
            })
    {
        return Err("Integration outer/inner/nonce identity mismatch".into());
    }
    Ok(())
}
fn integration_counts(value: &Value, policies: u64) -> bool {
    value.as_object().is_some_and(|v| v.len() == 6)
        && value["schema_version"].as_u64() == Some(17)
        && value["sequence"].as_u64().is_some()
        && value["policies"].as_u64() == Some(policies)
        && ["runs", "attempts", "ref_fences"]
            .iter()
            .all(|k| value[*k].as_u64() == Some(0))
}
/// Builds only from two actual readonly query results; no caller pass flag.
pub fn integration_database_evidence(
    before: &Value,
    after: &Value,
    expected: &Value,
) -> Result<Value, String> {
    let flags = [
        "proposal_matches",
        "proposal_result_matches",
        "policy_matches",
        "owner_event_matches",
        "policy_event_matches",
    ];
    let complete = |v: &Value| {
        v.as_object().is_some_and(|o| {
            o.len() == 8
                && o.contains_key("counts")
                && o.contains_key("owner_event_sequence")
                && o.contains_key("policy_event_sequence")
                && flags.iter().all(|k| v[*k] == true)
        })
    };
    let phase = expected["phase"]
        .as_str()
        .filter(|s| matches!(*s, "approve" | "cancel"))
        .ok_or("Invalid integration evidence phase")?;
    if !is_integration(expected)
        || !complete(before)
        || !complete(after)
        || !integration_counts(&before["counts"], 0)
        || before["counts"]["sequence"] != expected["baseline_sequence"]
        || !before["owner_event_sequence"].is_null()
        || !before["policy_event_sequence"].is_null()
        || !integration_counts(&after["counts"], if phase == "approve" { 1 } else { 0 })
    {
        return Err("Incomplete independent integration database witness".into());
    }
    if phase == "approve" {
        let old = before["counts"]["sequence"].as_u64().unwrap();
        let owner = after["owner_event_sequence"]
            .as_u64()
            .ok_or("No real Owner event sequence")?;
        let policy = after["policy_event_sequence"]
            .as_u64()
            .ok_or("No real policy event sequence")?;
        let current = after["counts"]["sequence"].as_u64().unwrap();
        if !(old < owner && owner < policy && policy <= current) {
            return Err("Integration Owner/policy event order mismatch".into());
        }
    } else if after["counts"] != before["counts"]
        || !after["owner_event_sequence"].is_null()
        || !after["policy_event_sequence"].is_null()
    {
        return Err("Cancelled integration review changed authority or events".into());
    }
    Ok(
        serde_json::json!({"read_only":true,"phase":phase,"before":before["counts"],"after":after["counts"],"proposal_matches":true,"proposal_result_matches":true,"policy_matches":true,"owner_event_matches":true,"policy_event_matches":true,"owner_event_sequence":after["owner_event_sequence"],"policy_event_sequence":after["policy_event_sequence"]}),
    )
}
pub fn validate_integration_response(response: &Value, expected: &Value) -> Result<(), String> {
    if !is_integration(expected) {
        return Err("Wrong integration fixture".into());
    }
    let selection = serde_json::from_value(expected["invoke"]["selection"].clone())
        .map_err(|_| "Invalid integration intent")?;
    if !agentbaton_desktop::native_integration_test_view::receipt_matches(
        response,
        &expected["proposal"],
        &selection,
    ) {
        return Err("Integration response does not bind complete approved policy".into());
    }
    Ok(())
}

pub fn mirrored_review_prompts(message: &str) -> Result<Vec<String>, String> {
    if message.len() > REVIEW_MESSAGE_BYTES {
        return Err("Complete native message exceeds the bounded presentation envelope".into());
    }
    let mut rows = Vec::new();
    let mut encoded = String::new();
    let mut start = 0;
    let mut end = 0;
    for (offset, ch) in message.char_indices() {
        let escaped = if ch == '\u{061c}'
            || ch == '\x7f'
            || ('\u{0080}'..='\u{009f}').contains(&ch)
            || ('\u{200b}'..='\u{200f}').contains(&ch)
            || ('\u{2028}'..='\u{202e}').contains(&ch)
            || ('\u{2060}'..='\u{206f}').contains(&ch)
            || ch == '\u{feff}'
        {
            format!("\\u{:04x}", ch as u32)
        } else {
            let quoted = serde_json::to_string(&ch.to_string()).map_err(|e| e.to_string())?;
            quoted[1..quoted.len() - 1].to_owned()
        };
        if !encoded.is_empty() && encoded.len() + escaped.len() > REVIEW_ROW_ENCODED_BYTES {
            rows.push((start, offset, std::mem::take(&mut encoded)));
            start = offset;
        }
        encoded.push_str(&escaped);
        end = offset + ch.len_utf8();
    }
    if !encoded.is_empty() || rows.is_empty() {
        rows.push((start, end, encoded));
    }
    // Verify the exact roundtrip before opening even the first native page.
    let mut restored = String::new();
    for (start, end, encoded) in &rows {
        let decoded: String = serde_json::from_str(&format!("\"{encoded}\""))
            .map_err(|_| "Native review encoding could not be restored")?;
        if message.get(*start..*end) != Some(decoded.as_str()) {
            return Err("Native review encoding changed original bytes".into());
        }
        restored.push_str(&decoded);
    }
    if restored != message {
        return Err("Native review encoding omitted original bytes".into());
    }
    let total = rows.len().div_ceil(REVIEW_ROWS_PER_PAGE);
    let mut prompts = Vec::with_capacity(total + 1);
    for (index, page) in rows.chunks(REVIEW_ROWS_PER_PAGE).enumerate() {
        let body = page
            .iter()
            .enumerate()
            .map(|(row, (_, _, text))| {
                format!(
                    "L{:05} \"{}\"",
                    index * REVIEW_ROWS_PER_PAGE + row + 1,
                    text
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        prompts.push(format!("Native review page {}/{}\nOriginal UTF-8 bytes {}..{} of {}.\nRead JSON strings in L order; escapes preserve ALL text.\nOK reads this page only. Cancel stops this review.\n{}", index + 1, total, page[0].0, page.last().unwrap().1, message.len(), body));
    }
    prompts.push(format!("FINAL display confirmation\nAll {} pages / {} original UTF-8 bytes were reviewed.\nOK confirms the original request exactly as displayed.\nAny further required reviews still apply.\nCancel stops without private approval.", total, message.len()));
    Ok(prompts)
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
        || observed["title"] != dialog_title(expected)
        || observed["parent_matches"] != true
        || observed["visible"] != true
        || observed["complete_text_visible"] != true
        || observed["window_number"].as_i64().is_none_or(|n| n <= 0)
    {
        return Err(
            "Own sheet, exact proposal/message or complete visible layout did not match".into(),
        );
    }
    if is_factory(expected) {
        validate_factory_identity(observed, expected, index)?;
    } else if is_integration(expected) {
        validate_integration_identity(observed, expected, index)?;
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

#[cfg(test)]
mod factory_tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> Value {
        let policy = json!({"max_nodes":1,"max_revisions":2,"max_plan_bytes":1024,"outputs":[{"artifact":{"artifact_id":"report","relative_path":"report.txt","max_bytes":1024,"expected_sha256":null},"validators":[]}],"writable_prefixes":["report.txt"],"owner_acceptance_criteria":"完整原文 中文😀 {page}; no execution"});
        let selection = json!({"root_task_id":"task","grant_id":"grant","expected_grant_revision":1,"capabilities":{"child_planning":true,"child_goal_execution":false,"create_workspace":false},"child_planning_policy":policy});
        let p = json!({"id":"proposal","kind":"factory","revision":1,"consumed":false,"snapshot":{"schema_version":1,"selection":selection,"bindings":{"root_task":{"project_id":"project"},"child_planning":{"policy":policy}},"contract_digest":"c".repeat(64)}});
        let messages = factory_messages(&p).unwrap();
        let hashes: Vec<_> = messages
            .iter()
            .map(|m| format!("{:x}", Sha256::digest(m.as_bytes())))
            .collect();
        let pages: Vec<_> = messages
            .iter()
            .flat_map(|m| mirrored_review_prompts(m).unwrap())
            .collect();
        json!({"kind":FACTORY_KIND,"proposal":p,"phase":"cancel","cancel_at":pages.len()-1,
            "invoke":{"selection":selection,"idempotencyKey":"owner-os-factory-planning"},
            "runtime":"/generated","output":"/evidence","baseline_sequence":10,"review_nonce":"a".repeat(32),"snapshot_sha256":digest(&p["snapshot"]).unwrap(),
            "outer_message_sha256":hashes,"presentation_sha256":digest(&json!(pages)).unwrap(),"presentation_count":pages.len()})
    }
    #[test]
    fn factory_compact_fixture_reconstructs_all_callbacks_without_changing_original_snapshot() {
        let mut e = fixture();
        let before = e["proposal"].clone();
        assert!(serde_json::to_vec(&e).unwrap().len() < 65536);
        prepare_factory(&mut e).unwrap();
        assert_eq!(e["proposal"], before);
        assert!(e["presentation"].as_array().unwrap().len() > 2);
        let contexts = e["factory_contexts"].as_array().unwrap();
        assert_eq!(contexts.last().unwrap()["outer_final"], true);
        assert_eq!(contexts.last().unwrap()["inner_final"], true);
    }
    #[test]
    fn factory_wrong_kind_digest_selection_nonce_or_typed_cas_never_becomes_a_grant() {
        for (field, value) in [
            ("kind", json!("grant")),
            ("snapshot_sha256", json!("0".repeat(64))),
            ("presentation_sha256", json!("0".repeat(64))),
            ("review_nonce", json!("")),
            ("baseline_sequence", json!(true)),
            ("cancel_at", json!(1.0)),
            ("presentation_count", json!(true)),
        ] {
            let mut e = fixture();
            e[field] = value;
            assert!(prepare_factory(&mut e).is_err(), "{field}");
        }
        let mut e = fixture();
        e["invoke"]["selection"]["capabilities"]["create_workspace"] = json!(true);
        assert!(prepare_factory(&mut e).is_err());
        let mut e = fixture();
        e["proposal"]["snapshot"]["selection"]["child_planning_policy"] = Value::Null;
        assert!(prepare_factory(&mut e).is_err());
    }
    #[test]
    fn factory_each_outer_and_inner_context_has_exact_nonce_and_order_with_no_cross_page_permit() {
        let mut e = fixture();
        prepare_factory(&mut e).unwrap();
        for index in 0..e["presentation"].as_array().unwrap().len() {
            let value = json!({"review_nonce":e["review_nonce"],"factory_page":e["factory_contexts"][index]});
            assert!(validate_factory_identity(&value, &e, index).is_ok());
            let mut wrong = value.clone();
            wrong["review_nonce"] = json!("b".repeat(32));
            assert!(validate_factory_identity(&wrong, &e, index).is_err());
            let mut wrong = value.clone();
            wrong["factory_page"]["outer_index"] = json!(0.0);
            assert!(validate_factory_identity(&wrong, &e, index).is_err());
            let next = (index + 1) % e["presentation"].as_array().unwrap().len();
            assert!(validate_factory_identity(&value, &e, next).is_err());
        }
    }
    #[test]
    fn factory_scope_response_keeps_full_policy_bindings_and_original_grant_revision() {
        let e = fixture();
        let p = &e["proposal"];
        let scope = json!({"id":p["id"],"approval_id":p["id"],"root_task_id":"task","root_grant_id":"grant","root_grant_revision":1,"project_id":"project","selection":p["snapshot"]["selection"],"bindings":p["snapshot"]["bindings"],"contract_digest":p["snapshot"]["contract_digest"],"revision":1,"revoked":false});
        let result = json!({"result":scope});
        assert!(validate_factory_response(&result, &e).is_ok());
        for (key, value) in [
            ("approval_id", json!("foreign")),
            ("bindings", json!({})),
            ("revision", json!(1.0)),
            ("root_grant_revision", json!(true)),
            ("revoked", json!(0)),
            ("selection", json!({})),
        ] {
            let mut wrong = result.clone();
            wrong["result"][key] = value;
            assert!(validate_factory_response(&wrong, &e).is_err(), "{key}");
        }
    }
    #[test]
    fn every_factory_outer_inner_page_and_final_can_cancel_without_advancing_past_the_cancel() {
        let mut e = fixture();
        prepare_factory(&mut e).unwrap();
        let count = e["presentation"].as_array().unwrap().len();
        for decline in 0..count {
            e["cancel_at"] = json!(decline);
            for page in 0..count {
                assert_eq!(cancel_here(&e, "cancel", page).unwrap(), page == decline);
            }
            assert!(cancel_here(&e, "approve", decline).is_err());
        }
        e["phase"] = json!("approve");
        for page in 0..count {
            assert!(!cancel_here(&e, "approve", page).unwrap());
        }
        assert!(cancel_here(&e, "approve", count).is_err());
    }
    #[test]
    fn factory_closed_foreign_parent_partial_text_or_unknown_reply_never_qualifies() {
        let mut e = fixture();
        prepare_factory(&mut e).unwrap();
        let observed = json!({"phase":"cancel","proposal_id":e["proposal"]["id"],"proposal_revision":1,
           "presentation_index":0,"presentation_count":e["presentation"].as_array().unwrap().len(),
           "message":e["presentation"][0],"title":dialog_title(&e),"parent_matches":true,
           "visible":true,"complete_text_visible":true,"window_number":1,
           "review_nonce":e["review_nonce"],"factory_page":e["factory_contexts"][0],
           "buttons":[{"title":"OK","enabled":true,"visible":true},{"title":"Cancel","enabled":true,"visible":true}]});
        assert!(validate(&observed, &e, "cancel", 0).is_ok());
        for (key, value) in [
            ("parent_matches", json!(false)),
            ("visible", json!(false)),
            ("complete_text_visible", json!(false)),
            ("message", json!("partial")),
            ("window_number", json!(0)),
            ("title", json!("AgentBaton — Native owner approval")),
            ("proposal_revision", json!(true)),
        ] {
            let mut wrong = observed.clone();
            wrong[key] = value;
            assert!(validate(&wrong, &e, "cancel", 0).is_err(), "{key}");
        }
        assert!(validate(&observed, &e, "unknown", 0).is_err());
        for reply in [
            json!({}),
            json!({"error":{"code":"OUTCOME_UNKNOWN"}}),
            json!({"result":{"id":"foreign"}}),
        ] {
            assert!(validate_factory_response(&reply, &e).is_err());
        }
    }
}

#[cfg(test)]
mod integration_controls {
    use super::*;
    use serde_json::json;
    fn expected() -> Value {
        json!({"kind":INTEGRATION_KIND,"phase":"approve","review_nonce":"a".repeat(32),"presentation":["one","final"],"cancel_at":1,"integration_contexts":[{"outer_index":0,"outer_count":2,"inner_index":0,"inner_count":2,"outer_final":false,"inner_final":false,"message_sha256":"b".repeat(64)}]})
    }
    #[test]
    fn integration_fixture_requires_explicit_mode_and_keeps_old_input_bound() {
        let old = fixture_arguments(["fixture".into()].into_iter()).unwrap();
        assert!(!old.1);
        assert_eq!(old.2, 65536);
        let new =
            fixture_arguments(["--integration".into(), "fixture".into()].into_iter()).unwrap();
        assert!(new.1);
        assert_eq!(new.2, 1_048_576);
        assert!(fixture_arguments(["--integration".into()].into_iter()).is_err());
        assert!(fixture_arguments(
            ["--integration".into(), "fixture".into(), "extra".into()].into_iter()
        )
        .is_err());
    }
    #[test]
    fn integration_nonce_and_outer_inner_identity_cannot_be_swapped_or_retyped() {
        let expected = expected();
        let value = json!({"review_nonce":expected["review_nonce"],"integration_page":expected["integration_contexts"][0]});
        assert!(validate_integration_identity(&value, &expected, 0).is_ok());
        for (field, bad) in [
            ("outer_index", json!(1)),
            ("inner_index", json!(true)),
            ("outer_count", Value::Null),
            ("message_sha256", json!("c".repeat(64))),
        ] {
            let mut bad_value = value.clone();
            bad_value["integration_page"][field] = bad;
            assert!(validate_integration_identity(&bad_value, &expected, 0).is_err());
        }
        let mut bad = value.clone();
        bad["review_nonce"] = json!("c".repeat(32));
        assert!(validate_integration_identity(&bad, &expected, 0).is_err());
        assert!(validate_integration_identity(&value, &expected, 1).is_err());
    }
    #[test]
    fn integration_cancel_phase_is_separate_and_cannot_approve_early() {
        let mut expected = expected();
        assert!(!cancel_here(&expected, "approve", 0).unwrap());
        assert!(cancel_here(&expected, "cancel", 0).is_err());
        expected["phase"] = json!("cancel");
        assert!(!cancel_here(&expected, "cancel", 0).unwrap());
        assert!(cancel_here(&expected, "cancel", 1).unwrap());
        assert!(cancel_here(&expected, "cancel", 2).is_err());
        expected["cancel_at"] = json!(true);
        assert!(cancel_here(&expected, "cancel", 1).is_err());
    }
    #[test]
    fn integration_shape_or_renderer_claims_never_substitute_for_typed_core_snapshot() {
        let mut expected = expected();
        expected["invoke"] = json!({"selection":{},"idempotencyKey":"owner-os-integration"});
        assert!(prepare_integration(&mut expected).is_err());
        assert!(
            validate_integration_response(&json!({"result":{"approved":true}}), &expected).is_err()
        );
        assert_eq!(
            dialog_title(&expected),
            agentbaton_desktop::native_integration_test_view::TITLE
        );
        assert!(!is_factory(&expected));
    }
}

#[cfg(test)]
mod integration_database_controls {
    use super::*;
    use serde_json::json;
    fn witness(policies: u64, sequence: u64, owner: Value, policy: Value) -> Value {
        json!({"counts":{"schema_version":17,"sequence":sequence,"policies":policies,"runs":0,"attempts":0,"ref_fences":0},"proposal_matches":true,"proposal_result_matches":true,"policy_matches":true,"owner_event_matches":true,"policy_event_matches":true,"owner_event_sequence":owner,"policy_event_sequence":policy})
    }
    #[test]
    fn integration_mutation_witness_is_distinct_from_unchanged_readonly_group() {
        let before = witness(0, 10, Value::Null, Value::Null);
        let after = witness(1, 12, json!(11), json!(12));
        let expected = json!({"kind":INTEGRATION_KIND,"phase":"approve","baseline_sequence":10});
        let group = integration_database_evidence(&before, &after, &expected).unwrap();
        assert_eq!(group["before"]["policies"], 0);
        assert_eq!(group["after"]["policies"], 1);
        assert!(group.get("unchanged").is_none());
        assert_eq!(group["read_only"], true);
        assert!(integration_database_evidence(&before, &before, &expected).is_err());
        assert!(integration_database_evidence(
            &json!({"read_only":true,"unchanged":true}),
            &after,
            &expected
        )
        .is_err());
    }
    #[test]
    fn integration_approval_db_rejects_partial_types_changed_counts_or_false_checks() {
        let before = witness(0, 10, Value::Null, Value::Null);
        let after = witness(1, 12, json!(11), json!(12));
        let expected = json!({"kind":INTEGRATION_KIND,"phase":"approve","baseline_sequence":10});
        for (field, bad) in [
            ("schema_version", json!(16)),
            ("schema_version", json!(18)),
            ("schema_version", json!(17.0)),
            ("policies", json!(true)),
            ("runs", json!(1)),
            ("attempts", json!(1)),
            ("ref_fences", json!(1)),
        ] {
            let mut bad_after = after.clone();
            bad_after["counts"][field] = bad;
            assert!(integration_database_evidence(&before, &bad_after, &expected).is_err());
        }
        for field in [
            "proposal_matches",
            "proposal_result_matches",
            "policy_matches",
            "owner_event_matches",
            "policy_event_matches",
        ] {
            let mut bad = after.clone();
            bad[field] = json!(false);
            assert!(integration_database_evidence(&before, &bad, &expected).is_err());
            bad.as_object_mut().unwrap().remove(field);
            assert!(integration_database_evidence(&before, &bad, &expected).is_err());
        }
        for (owner, policy) in [
            (json!(12), json!(11)),
            (json!(10), json!(12)),
            (json!(11), json!(13)),
            (json!(true), json!(12)),
            (Value::Null, json!(12)),
        ] {
            let bad = witness(1, 12, owner, policy);
            assert!(integration_database_evidence(&before, &bad, &expected).is_err());
        }
    }
    #[test]
    fn integration_cancel_must_preserve_real_zero_counts_unconsumed_proposal_and_sequence() {
        let before = witness(0, 10, Value::Null, Value::Null);
        let expected = json!({"kind":INTEGRATION_KIND,"phase":"cancel","baseline_sequence":10});
        let group = integration_database_evidence(&before, &before, &expected).unwrap();
        assert_eq!(group["before"], group["after"]);
        assert!(integration_database_evidence(
            &before,
            &witness(1, 12, json!(11), json!(12)),
            &expected
        )
        .is_err());
        assert!(integration_database_evidence(
            &before,
            &witness(0, 11, Value::Null, Value::Null),
            &expected
        )
        .is_err());
    }
}
