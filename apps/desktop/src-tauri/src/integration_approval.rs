//! Independent Native integration review. Public Core DTOs are display/request
//! data, never a deserialized host authority or proof of an accepted candidate.
use agentbaton_core::integration::{
    IntegrationPolicy, IntegrationPolicySelection, IntegrationPolicySnapshot,
    IntegrationStaticBindings, IntegrationWorkspaceBinding,
};
use serde_json::{json, Value};

pub(super) const TITLE: &str = "AgentBaton — Independent local integration approval";
const ENVELOPE_BYTES: usize = 1_048_576;
const PAGE_BYTES: usize = 12_288;
const ACTION: &str = "Approve INDEPENDENT local branch integration for this exact Task/spec/Goal scope, existing unchecked-out target ref and original shared budgets. Review ALL server-resolved source/target filesystem pins, immutable Grant lineage/limits and as-of usage, output/validator contracts, complete definition_json, executable/script identities, maximum rounds and target-advance rule. Approval can precede Task acceptance: it arms a bounded watcher; no candidate has passed merely because this policy exists. Only a fresh accepted receipt plus NEW correctness validation on the actual merged commit/tree can authorize Core's final local CAS. Original execution Grant, Goal acceptance and this branch permission are distinct. No remote delivery, production release, OS filesystem/network sandbox or whole-filesystem output quota is granted. Unknown effects require reconciliation, not replay or a new key.\n\n独立本地合入批准：完整核对固定任务/规格/Goal范围、未检出目标分支、源与目标身份、原Grant沿袭及共享预算、输出与完整检查规则/程序脚本固定身份、重做轮数和目标推进规则。策略可先于任务验收，不代表已有候选通过；实际合并提交仍须新验证及Core精确CAS。不得扩张原执行权限，不授远程推送/生产发布/系统沙箱或全文件系统配额；未知结果只对账，不自动重放。";

fn opaque(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn decimal(value: &str) -> bool {
    value.parse::<u64>().is_ok_and(|n| n.to_string() == value)
}
fn invalid(message: &str) -> Value {
    json!({"request_id":null,"error":{"code":"INVALID_APPROVAL_PROPOSAL","message":message,"retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":0,"warnings":[]})
}
fn unknown() -> Value {
    json!({"request_id":null,"error":{"code":"OUTCOME_UNKNOWN","message":"Integration finalization returned an unbound receipt; reconcile the original request before any action","retryable":false,"side_effect_state":"unknown","details_ref":null},"revision":0,"warnings":[]})
}
fn valid_error(value: &Value) -> bool {
    value.is_object()
        && value["code"].is_string()
        && value["message"].is_string()
        && value["retryable"].is_boolean()
        && value["side_effect_state"].is_string()
}
fn pin(value: &agentbaton_core::integration::IntegrationGitPin) -> bool {
    value.canonical_root.is_absolute()
        && value.common_dir.is_absolute()
        && decimal(&value.device)
        && decimal(&value.inode)
        && decimal(&value.common_device)
        && decimal(&value.common_inode)
        && oid(&value.head)
}
fn selection_shape(selection: &IntegrationPolicySelection) -> bool {
    opaque(&selection.task_id)
        && opaque(&selection.goal_scope_id)
        && selection.host_id == "local"
        && [
            selection.expected_task_revision,
            selection.expected_spec_revision,
            selection.expected_scope_revision,
        ]
        .iter()
        .all(|n| *n > 0 && *n <= 9_007_199_254_740_991)
        && (1..=8).contains(&selection.max_rounds)
        && selection.ttl_ms > 0
        && selection.target_ref.starts_with("refs/heads/")
        && selection.target_ref.len() <= 1024
        && !selection.target_ref.bytes().any(|b| b < 32 || b == 127)
        && oid(&selection.expected_target_head)
        && selection.stage_reservations.preparation.runtime_ms > 0
        && selection.stage_reservations.cas.runtime_ms > 0
        && !selection.stage_reservations.validators.is_empty()
        && selection.stage_reservations.validators.len() <= 64
        && selection
            .stage_reservations
            .validators
            .iter()
            .enumerate()
            .all(|(i, v)| {
                v.reservation.runtime_ms > 0
                    && opaque(&v.validator.policy_id)
                    && v.validator.revision > 0
                    && !selection.stage_reservations.validators[..i]
                        .iter()
                        .any(|old| old.validator == v.validator)
            })
}
fn workspace_binding_matches(
    b: &IntegrationStaticBindings,
    selection: &IntegrationPolicySelection,
) -> bool {
    match &b.workspace_binding {
        IntegrationWorkspaceBinding::TaskManaged { receipt } => {
            receipt.task_id == selection.task_id
                && receipt.workspace_id == b.workspace_id
                && receipt.project_id == b.project_id
                && receipt.host_id == selection.host_id
                && receipt.spec_revision == b.spec_revision
                && opaque(&receipt.workspace_id)
                && matches!(
                    std::path::Path::new(&receipt.workspace_id)
                        .components()
                        .collect::<Vec<_>>()
                        .as_slice(),
                    [std::path::Component::Normal(_)]
                )
                && receipt.managed_root.is_absolute()
                && !receipt.managed_root.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::CurDir | std::path::Component::ParentDir
                    )
                })
                && receipt.managed_root.join(&receipt.workspace_id) == b.source.canonical_root
        }
        IntegrationWorkspaceBinding::FactoryDelegated {
            factory_scope_id,
            factory_scope_digest,
            factory_run_id,
            child_spec_revision,
            lineage,
            delegation_event_sequence,
            delegation_document_digest,
            original_request,
            original_handle,
            workspace_id,
            registered_pin,
            source_base,
            managed_creation_ref,
        } => {
            b.factory_scope_id.as_deref() == Some(factory_scope_id.as_str())
                && opaque(factory_scope_id)
                && hash(factory_scope_digest)
                && opaque(factory_run_id)
                && *child_spec_revision == b.spec_revision
                && *delegation_event_sequence > 0
                && hash(delegation_document_digest)
                && oid(source_base)
                && !managed_creation_ref.is_empty()
                && managed_creation_ref.len() <= 1024
                && workspace_id == &b.workspace_id
                && registered_pin == &b.source
                && pin(registered_pin)
                && original_request.workspace_id == b.workspace_id
                && lineage.task_id == b.task_id
                && lineage.root_task_id == b.root_task_id
                && lineage.factory_scope_id == *factory_scope_id
                && original_handle.task_id == b.task_id
                && original_handle.root_task_id == b.root_task_id
                && original_handle.factory_scope_id == *factory_scope_id
                && lineage.parent_task_id == original_handle.parent_task_id
                && lineage.delegation_id.as_deref() == Some(original_handle.delegation_id.as_str())
        }
    }
}
fn snapshot_matches(
    snapshot: &IntegrationPolicySnapshot,
    selection: &IntegrationPolicySelection,
) -> bool {
    let b = &snapshot.bindings;
    let g = &snapshot.guarantees;
    selection_shape(selection)
        && snapshot.intent == *selection
        && snapshot.authority_kind == "authorized_local_integration"
        && !snapshot.warning.trim().is_empty()
        && snapshot.warning.len() <= 2048
        && b.task_id == selection.task_id
        && b.spec_revision == selection.expected_spec_revision
        && b.goal_scope_id == selection.goal_scope_id
        && b.goal_scope_revision == selection.expected_scope_revision
        && opaque(&b.project_id)
        && b.project_revision > 0
        && opaque(&b.workspace_id)
        && b.workspace_revision > 0
        && opaque(&b.root_task_id)
        && opaque(&b.root_grant_id)
        && hash(&b.goal_scope_digest)
        && hash(&b.repository_digest)
        && hash(&b.binding_digest)
        && pin(&b.project_source)
        && pin(&b.source)
        && workspace_binding_matches(b, selection)
        && b.goal_scope_expires_at_ms > 0
        && !b.grant_lineage.is_empty()
        && b.grant_lineage.iter().enumerate().all(|(i, grant)| {
            !b.grant_lineage[..i].iter().any(|old| old.id == grant.id)
                && opaque(&grant.id)
                && grant.revision > 0
                && opaque(&grant.approval_id)
                && grant.root_grant_id == b.root_grant_id
                && hash(&grant.original_document_sha256)
                && grant.workspace_bindings.values().all(pin)
        })
        && b.grant_lineage
            .iter()
            .any(|grant| grant.id == b.root_grant_id && grant.parent_id.is_none())
        && !b.outputs.is_empty()
        && b.outputs.len() <= 64
        && b.inputs.len() <= 64
        && b.validators.len() == selection.stage_reservations.validators.len()
        && b.validators.iter().enumerate().all(|(i, validator)| {
            !b.validators[..i]
                .iter()
                .any(|old| old.reference == validator.reference)
                && hash(&validator.definition_digest)
                && !validator.definition_json.is_empty()
                && validator.definition_json.len() <= ENVELOPE_BYTES
                && selection
                    .stage_reservations
                    .validators
                    .iter()
                    .any(|v| v.validator == validator.reference)
                && [&validator.program, &validator.script].iter().all(|a| {
                    a.as_ref().is_none_or(|a| {
                        a.canonical_root.is_absolute()
                            && decimal(&a.root_device)
                            && decimal(&a.root_inode)
                    })
                })
        })
        && b.validators.iter().any(|v| {
            matches!(
                v.reference.kind,
                agentbaton_core::goal_store::GoalValidatorKind::Data
                    | agentbaton_core::goal_store::GoalValidatorKind::Command
            )
        })
        && b.outputs.iter().all(|output| {
            output
                .validators
                .iter()
                .all(|required| b.validators.iter().any(|v| &v.reference == required))
        })
        && b.system_git.canonical_path.is_absolute()
        && hash(&b.system_git.sha256)
        && decimal(&b.system_git.device)
        && decimal(&b.system_git.inode)
        && decimal(&b.system_git.links)
        && b.system_git.bytes > 0
        && g.local_unchecked_out_ref_only
        && g.merged_candidate_revalidation_required
        && g.original_index_and_working_files_unchanged
        && g.output_subject_rebinding_same_paths_only
        && g.fixed_inputs_and_scripts_unchanged
        && !g.os_filesystem_sandbox
        && !g.os_network_sandbox
        && !g.whole_filesystem_output_quota
        && !g.remote_delivery_authorized
}

/// Pure production formatter also exposed as a read-only feature-gated test view.
/// It never calls Core finalize and never interprets JS strings as authority.
pub(super) fn review_messages(
    response: &Value,
    selection: &IntegrationPolicySelection,
) -> Result<Vec<String>, String> {
    if response.get("error").is_some()
        || serde_json::to_vec(response).map_or(true, |v| v.len() > ENVELOPE_BYTES)
    {
        return Err(
            "Complete integration proposal exceeds the bounded Native envelope or is an error"
                .into(),
        );
    }
    let proposal = &response["result"];
    let id = proposal["id"]
        .as_str()
        .filter(|id| opaque(id) && id.len() <= 128)
        .ok_or("Invalid integration proposal ID")?;
    let revision = proposal["revision"]
        .as_u64()
        .filter(|v| *v > 0 && *v <= 9_007_199_254_740_991)
        .ok_or("Invalid integration proposal revision")?;
    if proposal["kind"] != "integration" || proposal["consumed"] != false {
        return Err("Independent unconsumed integration proposal required".into());
    }
    let snapshot: IntegrationPolicySnapshot = serde_json::from_value(proposal["snapshot"].clone())
        .map_err(|_| "Complete typed integration snapshot required")?;
    if !snapshot_matches(&snapshot, selection)
        || serde_json::to_value(&snapshot).map_err(|_| "Cannot restore integration snapshot")?
            != proposal["snapshot"]
    {
        return Err("Integration proposal differs from the exact requested policy or complete server bindings".into());
    }
    let complete = serde_json::to_string(&proposal["snapshot"])
        .map_err(|_| "Cannot display full integration snapshot")?;
    format_complete_snapshot(&complete, id, revision)
}

fn format_complete_snapshot(
    complete: &str,
    id: &str,
    revision: u64,
) -> Result<Vec<String>, String> {
    if complete.is_empty() || complete.len() > ENVELOPE_BYTES {
        return Err("Incomplete or oversized integration JSON".into());
    }
    let mut pages = Vec::new();
    let mut start = 0;
    while start < complete.len() {
        let mut end = (start + PAGE_BYTES).min(complete.len());
        while !complete.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            return Err("Integration UTF8 page boundary refused".into());
        }
        pages.push(complete[start..end].to_owned());
        start = end;
    }
    if pages.is_empty() || pages.concat() != complete {
        return Err("Incomplete integration snapshot presentation".into());
    }
    let mut messages = Vec::with_capacity(pages.len() + 1);
    let mut offset = 0;
    for (index, page) in pages.iter().enumerate() {
        let end = offset + page.len();
        let message=format!("{ACTION}\n\nIntegration proposal {id} / revision {revision}. Page {}/{}; complete JSON UTF-8 bytes {}..{} of {}. OK reviews this page only; final confirmation remains required. Cancel creates no approval or rejection receipt.\n\n{page}",index+1,pages.len(),offset,end,complete.len());
        if message.len() > 16_384 {
            return Err("Integration Native page exceeds display bound".into());
        }
        messages.push(message);
        offset = end;
    }
    messages.push(format!("{ACTION}\n\nFINAL independent integration approval — proposal {id} / revision {revision}. All {} complete JSON pages / {} UTF-8 bytes reviewed. OK approves only this exact policy; it does not mark Task/merged candidate accepted. Every execution stage still requires current authority and fresh merged correctness receipts. Unknown replies require reconciliation, never automatic replay.",pages.len(),complete.len()));
    Ok(messages)
}

pub(super) fn confirm_and_finalize(
    response: Value,
    selection: &IntegrationPolicySelection,
    mut confirm_native: impl FnMut(&str) -> bool,
    finalize: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    if let Some(error) = response.get("error") {
        return Ok(if valid_error(error) {
            response
        } else {
            invalid("Malformed server error before integration review")
        });
    }
    let messages = match review_messages(&response, selection) {
        Ok(v) => v,
        Err(_) => {
            return Ok(invalid(
                "Integration proposal does not match the exact complete Native review contract",
            ))
        }
    };
    for message in &messages {
        if !confirm_native(message) {
            return Ok(
                json!({"request_id":null,"error":{"code":"PERMISSION_DENIED","message":"Native integration review cancelled; no policy or rejection receipt created","retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":0,"warnings":[]}),
            );
        }
    }
    let proposal = &response["result"];
    let id = proposal["id"].as_str().unwrap();
    let revision = proposal["revision"].as_u64().unwrap();
    let result = finalize(
        json!({"method":"approvals.finalize","params":{"proposal_id":id},"expected_revision":revision,"idempotency_key":format!("proposal:{id}")}),
    )?;
    if let Some(error) = result.get("error") {
        return Ok(if valid_error(error) {
            result
        } else {
            unknown()
        });
    }
    if !receipt_matches(&result, proposal, selection) {
        return Ok(unknown());
    }
    Ok(result)
}

pub(super) fn receipt_matches(
    result: &Value,
    proposal: &Value,
    selection: &IntegrationPolicySelection,
) -> bool {
    if result.get("error").is_some() {
        return false;
    }
    let Ok(policy) = serde_json::from_value::<IntegrationPolicy>(result["result"].clone()) else {
        return false;
    };
    let snapshot = &proposal["snapshot"];
    let Ok(digest) = agentbaton_core::integration::integration_snapshot_digest(snapshot) else {
        return false;
    };
    let Some(ttl_upper) = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| d.as_millis().checked_add(u128::from(selection.ttl_ms)))
        .and_then(|n| i64::try_from(n).ok())
    else {
        return false;
    };
    policy.id == proposal["id"].as_str().unwrap_or("")
        && policy.approval_id == policy.id
        && policy.revision == 1
        && !policy.revoked
        && policy.selection == *selection
        && serde_json::to_value(&policy.bindings).ok().as_ref() == Some(&snapshot["bindings"])
        && policy.snapshot_digest == digest
        && !policy.created_at.trim().is_empty()
        && policy.expires_at_ms > 0
        && policy.expires_at_ms <= ttl_upper
        && policy.expires_at_ms <= policy.bindings.goal_scope_expires_at_ms
        && policy
            .bindings
            .grant_lineage
            .iter()
            .all(|grant| policy.expires_at_ms <= grant.expires_at_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn selection() -> IntegrationPolicySelection {
        serde_json::from_value(json!({"task_id":"task","expected_task_revision":3,"expected_spec_revision":2,"goal_scope_id":"scope","expected_scope_revision":1,"host_id":"local","target_ref":"refs/heads/reviewed","expected_target_head":"a".repeat(40),"max_rounds":2,"on_target_advance":"block","ttl_ms":30000,"stage_reservations":{"preparation":{"runtime_ms":4000,"output_bytes":1024,"tokens":null,"cost_microunits":null},"validators":[{"validator":{"kind":"data","policy_id":"data","revision":1},"reservation":{"runtime_ms":2000,"output_bytes":1024,"tokens":null,"cost_microunits":null}}],"cas":{"runtime_ms":1000,"output_bytes":1024,"tokens":null,"cost_microunits":null}}})).unwrap()
    }
    #[test]
    fn selection_keeps_twelve_typed_fields_and_fixed_no_authority_inputs() {
        let s = selection();
        assert!(selection_shape(&s));
        let value = serde_json::to_value(&s).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 12);
        for (field, bad) in [
            ("caller", json!({})),
            ("approved", json!(true)),
            ("snapshot", json!({})),
            ("source_path", json!("/foreign")),
        ] {
            let mut input = value.clone();
            input[field] = bad;
            assert!(serde_json::from_value::<IntegrationPolicySelection>(input).is_err());
        }
        let mut input = value.clone();
        input["stage_reservations"]["validators"] = json!([
            input["stage_reservations"]["validators"][0],
            input["stage_reservations"]["validators"][0]
        ]);
        assert!(!selection_shape(&serde_json::from_value(input).unwrap()));
        for (field, bad) in [
            ("max_rounds", json!(0)),
            ("max_rounds", json!(9)),
            ("host_id", json!("remote")),
            ("expected_task_revision", json!(0)),
            ("expected_target_head", json!("not-head")),
        ] {
            let mut input = value.clone();
            input[field] = bad;
            assert!(!selection_shape(&serde_json::from_value(input).unwrap()));
        }
        let mut null = value;
        null["stage_reservations"] = Value::Null;
        assert!(serde_json::from_value::<IntegrationPolicySelection>(null).is_err());
    }
    #[test]
    fn malformed_or_oversized_proposal_never_opens_dialog_or_finalizes() {
        for value in [
            json!({}),
            json!({"error":null}),
            json!({"result":{"id":"p","kind":"goal","revision":1,"consumed":false,"snapshot":{}}}),
            json!({"result":{"padding":"x".repeat(ENVELOPE_BYTES)}}),
        ] {
            let response = confirm_and_finalize(
                value,
                &selection(),
                |_| panic!("no review before validation"),
                |_| panic!("no finalize before validation"),
            )
            .unwrap();
            assert_eq!(response["error"]["code"], "INVALID_APPROVAL_PROPOSAL");
            assert_eq!(response["error"]["side_effect_state"], "not_started");
        }
    }
    #[test]
    fn original_core_error_is_preserved_without_retry_or_claiming_approval() {
        let error = json!({"request_id":"req","error":{"code":"INPUT_CHANGED","message":"changed","retryable":false,"side_effect_state":"not_started","details_ref":null},"revision":73,"warnings":[]});
        assert_eq!(
            confirm_and_finalize(
                error.clone(),
                &selection(),
                |_| panic!("no dialog"),
                |_| panic!("no retry")
            )
            .unwrap(),
            error
        );
    }
}

#[cfg(test)]
mod full_presentation_tests {
    use super::*;
    #[test]
    fn complete_definition_json_and_lossless_identity_text_survive_every_page() {
        let value = json!({"definition_json":format!("{}END-EXACT-CRITERION", "目标\\\"规则\n".repeat(10000)),"device":"18446744073709551615","inode":"9007199254740993"});
        let complete = serde_json::to_string(&value).unwrap();
        let messages = format_complete_snapshot(&complete, "proposal", 9).unwrap();
        assert!(messages.len() > 2);
        let restored: String = messages[..messages.len() - 1]
            .iter()
            .map(|m| m.rsplit_once("\n\n").unwrap().1)
            .collect();
        assert_eq!(restored, complete);
        assert_eq!(serde_json::from_str::<Value>(&restored).unwrap(), value);
        assert!(messages
            .last()
            .unwrap()
            .contains("FINAL independent integration approval"));
        assert!(messages.iter().all(|m| m.len() <= 16384));
    }
    #[test]
    fn empty_and_overlimit_full_json_are_never_truncated_into_review() {
        assert!(format_complete_snapshot("", "p", 1).is_err());
        assert!(format_complete_snapshot(&"x".repeat(ENVELOPE_BYTES + 1), "p", 1).is_err());
    }
}

#[cfg(test)]
mod typed_review_tests {
    use super::*;
    // DTO-shape fixture only: this does not mint a Native/Core capability.
    fn proposal() -> Value {
        let s = super::tests::selection();
        let pin = json!({"canonical_root":"/generated/managed/workspace","device":"9007199254740993","inode":"18446744073709551615","common_dir":"/generated/source/.git","common_device":"1","common_inode":"2","head":"a".repeat(40)});
        let original_pin = json!({"canonical_root":"/generated/source","device":"9007199254740993","inode":"2","common_dir":"/generated/source/.git","head":"a".repeat(40)});
        let artifact = json!({"artifact_id":"output","relative_path":"report.json","max_bytes":4096,"expected_sha256":null});
        let validator = json!({"kind":"data","policy_id":"data","revision":1});
        let budget = json!({"attempts":10,"parallel":1,"runtime_ms":30000,"output_bytes":4096,"children":0,"reworks":0,"tokens":null,"cost_microunits":null});
        let scope = json!({"workspace_ids":["workspace"],"connection_ids":["connection"],"host_ids":["local"],"max_attempts":10,"max_concurrency":1,"max_duration_ms":30000,"recipients":[],"network":{"mode":"deny"},"max_depth":0,"max_children":0});
        let receipt = json!({"version":1,"binding_id":"binding","workspace_id":"workspace","task_id":"task","project_id":"project","host_id":"local","original_task_revision":1,"reserved_task_revision":2,"spec_revision":2,"project_revision":1,"source_pin":original_pin,"managed_root":"/generated/managed","preparation_key":"preparation","request_key":"request","request_digest":"b".repeat(64),"created_at":"2026-10-30T00:00:00Z"});
        let bindings = json!({"task_id":"task","task_goal":"reviewed task","spec_revision":2,"project_id":"project","project_revision":1,"project_source":pin,"workspace_id":"workspace","workspace_revision":1,"source":pin,"workspace_binding":{"kind":"task_managed","receipt":receipt},"goal_scope_id":"scope","goal_scope_revision":1,"goal_scope_digest":"b".repeat(64),"goal_scope_expires_at_ms":4102444800000_i64,"root_task_id":"task","root_grant_id":"grant","factory_scope_id":null,"ancestors":[],"grant_lineage":[{"id":"grant","root_grant_id":"grant","parent_id":null,"revision":1,"approval_id":"original-owner","approved_at":"2026-10-30T00:00:00Z","expires_at_ms":4102444800000_i64,"scope":scope,"connection_bindings":{},"workspace_bindings":{"workspace":pin},"original_document_sha256":"c".repeat(64)}],"root_budget":budget,"project_budget":budget,"inputs":[],"outputs":[{"artifact":artifact,"validators":[validator]}],"validators":[{"reference":validator,"rule_id":"rule","definition_digest":"d".repeat(64),"definition_json":" {\"identity\":\"18446744073709551615\",\"rule\":\"全部原文\"} \n","program":null,"script":null}],"system_git":{"canonical_path":"/usr/bin/git","device":"1","inode":"3","sha256":"e".repeat(64),"bytes":8192,"uid":0,"mode":33261,"links":"1"},"repository_digest":"f".repeat(64),"binding_digest":"1".repeat(64)});
        let usage = json!({"attempts":1,"active":0,"children":0,"runtime_ms":null,"output_bytes":null,"tokens":null,"cost_microunits":null});
        let snapshot = json!({"intent":s,"bindings":bindings,"as_of_sequence":42,"shared_usage":{"root":usage,"project":usage},"authority_kind":"authorized_local_integration","guarantees":{"local_unchecked_out_ref_only":true,"merged_candidate_revalidation_required":true,"original_index_and_working_files_unchanged":true,"output_subject_rebinding_same_paths_only":true,"fixed_inputs_and_scripts_unchanged":true,"os_filesystem_sandbox":false,"os_network_sandbox":false,"whole_filesystem_output_quota":false,"remote_delivery_authorized":false},"warning":"All original unenforced boundaries remain explicit."});
        json!({"id":"proposal","revision":1,"kind":"integration","consumed":false,"snapshot":snapshot,"created_at":"2026-10-30T00:00:00Z","expires_at_ms":4102444800000_i64})
    }
    fn result(proposal: &Value) -> Value {
        let expires = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            + 1000;
        json!({"result":{"id":"proposal","revision":1,"approval_id":"proposal","selection":proposal["snapshot"]["intent"],"bindings":proposal["snapshot"]["bindings"],"snapshot_digest":agentbaton_core::integration::integration_snapshot_digest(&proposal["snapshot"]).unwrap(),"created_at":"2026-10-30T00:00:00Z","expires_at_ms":expires,"revoked":false},"revision":43,"warnings":[]})
    }
    #[test]
    fn exact_typed_policy_requires_every_real_native_page_then_private_finalize() {
        let proposal = proposal();
        let selection = super::tests::selection();
        let response = json!({"result":proposal});
        let pages = review_messages(&response, &selection).unwrap();
        let mut seen = Vec::new();
        let expected = result(&proposal);
        let out=confirm_and_finalize(response,&selection,|message|{seen.push(message.to_owned());true},|frame|{
            assert_eq!(frame,json!({"method":"approvals.finalize","params":{"proposal_id":"proposal"},"expected_revision":1,"idempotency_key":"proposal:proposal"}));Ok(expected.clone())
        }).unwrap();
        assert_eq!(seen, pages);
        assert_eq!(out, expected);
    }
    #[test]
    fn cancel_any_outer_page_or_final_review_never_finalizes_or_declines() {
        let response = json!({"result":proposal()});
        let selection = super::tests::selection();
        let pages = review_messages(&response, &selection).unwrap();
        for stop in 0..pages.len() {
            let mut i = 0;
            let out = confirm_and_finalize(
                response.clone(),
                &selection,
                |_| {
                    let yes = i != stop;
                    i += 1;
                    yes
                },
                |_| panic!("no finalize or decline after cancellation"),
            )
            .unwrap();
            assert_eq!(out["error"]["code"], "PERMISSION_DENIED");
            assert_eq!(out["error"]["side_effect_state"], "not_started");
            assert_eq!(i, stop + 1);
        }
    }
    #[test]
    fn malformed_server_bindings_fail_before_dialog_and_do_not_upgrade_old_grants() {
        let proposal = proposal();
        let selection = super::tests::selection();
        for (path, bad) in [
            ("/snapshot/authority_kind", json!("goal")),
            (
                "/snapshot/guarantees/remote_delivery_authorized",
                json!(true),
            ),
            (
                "/snapshot/bindings/source/device",
                json!(9007199254740993_u64),
            ),
            ("/snapshot/bindings/source/inode", json!("01")),
            (
                "/snapshot/bindings/workspace_binding/receipt/task_id",
                json!("other-task"),
            ),
            ("/snapshot/bindings/goal_scope_expires_at_ms", json!(0)),
            ("/snapshot/bindings/validators/0/definition_json", json!("")),
            ("/snapshot/intent/max_rounds", json!(3)),
        ] {
            let mut p = proposal.clone();
            *p.pointer_mut(path).unwrap() = bad;
            let out = confirm_and_finalize(
                json!({"result":p}),
                &selection,
                |_| panic!("no native review for mismatched snapshot"),
                |_| panic!("no finalize"),
            )
            .unwrap();
            assert_eq!(out["error"]["side_effect_state"], "not_started");
        }
        let mut duplicate = proposal.clone();
        duplicate["snapshot"]["bindings"]["validators"] = json!([
            duplicate["snapshot"]["bindings"]["validators"][0],
            duplicate["snapshot"]["bindings"]["validators"][0]
        ]);
        assert!(review_messages(&json!({"result":duplicate}), &selection).is_err());
    }
    #[test]
    fn task_managed_receipt_binds_exact_safe_child_instead_of_managed_parent_or_prefix() {
        let p = proposal();
        let selection = super::tests::selection();
        assert_eq!(
            p["snapshot"]["bindings"]["workspace_binding"]["receipt"]["managed_root"],
            "/generated/managed"
        );
        assert_eq!(
            p["snapshot"]["bindings"]["source"]["canonical_root"],
            "/generated/managed/workspace"
        );
        assert!(review_messages(&json!({"result":p}), &selection).is_ok());
        for root in [
            "/generated/managed",
            "/generated/managed-other/workspace",
            "/generated/managed/foreign",
            "/foreign/workspace",
        ] {
            let mut bad = p.clone();
            bad["snapshot"]["bindings"]["source"]["canonical_root"] = json!(root);
            assert!(review_messages(&json!({"result":bad}), &selection).is_err());
        }
        for id in [
            ".",
            "..",
            "../outside",
            "/outside",
            "workspace/child",
            "workspace\\child",
        ] {
            let mut bad = p.clone();
            bad["snapshot"]["bindings"]["workspace_id"] = json!(id);
            bad["snapshot"]["bindings"]["workspace_binding"]["receipt"]["workspace_id"] = json!(id);
            let derived = std::path::Path::new("/generated/managed").join(id);
            bad["snapshot"]["bindings"]["source"]["canonical_root"] = json!(derived);
            assert!(review_messages(&json!({"result":bad}), &selection).is_err());
        }
    }
    #[test]
    fn factory_delegated_workspace_keeps_original_lineage_without_fake_task_receipt() {
        let mut p = proposal();
        p["snapshot"]["bindings"]["factory_scope_id"] = json!("factory");
        p["snapshot"]["bindings"]["root_task_id"] = json!("root-task");
        p["snapshot"]["bindings"]["ancestors"] = json!(["root-task"]);
        let binding = json!({"kind":"factory_delegated","factory_scope_id":"factory","factory_scope_digest":"2".repeat(64),"factory_run_id":"factory-run","child_spec_revision":2,"lineage":{"task_id":"task","root_task_id":"root-task","parent_task_id":"root-task","factory_scope_id":"factory","delegation_id":"delegation","depth":1},"delegation_event_sequence":40,"delegation_document_digest":"3".repeat(64),"original_request":{"goal":"reviewed task","connection_id":"connection","workspace_id":"workspace","input_artifact_ids":[],"output_artifact_ids":["output"],"budget":p["snapshot"]["bindings"]["root_budget"],"wait_for_child":true},"original_handle":{"task_id":"task","parent_task_id":"root-task","root_task_id":"root-task","delegation_id":"delegation","factory_scope_id":"factory","revision":1,"state":"queued","attempt_id":null,"cancel_requested":false,"confirmed_stop":false,"result_available":false},"workspace_id":"workspace","registered_pin":p["snapshot"]["bindings"]["source"],"source_base":"a".repeat(40),"managed_creation_ref":"managed-creation"});
        p["snapshot"]["bindings"]["workspace_binding"] = binding;
        let selection = super::tests::selection();
        assert!(review_messages(&json!({"result":p}), &selection).is_ok());
        for path in [
            "/snapshot/bindings/workspace_binding/original_handle/task_id",
            "/snapshot/bindings/workspace_binding/original_request/workspace_id",
            "/snapshot/bindings/workspace_binding/lineage/delegation_id",
        ] {
            let mut bad = p.clone();
            *bad.pointer_mut(path).unwrap() = json!("foreign");
            assert!(review_messages(&json!({"result":bad}), &selection).is_err());
        }
        assert!(p["snapshot"]["bindings"]["workspace_binding"]
            .get("receipt")
            .is_none());
    }
    #[test]
    fn final_receipt_hash_binding_expiry_and_scope_are_not_renderer_claims() {
        let proposal = proposal();
        let selection = super::tests::selection();
        let good = result(&proposal);
        assert!(receipt_matches(&good, &proposal, &selection));
        for (path, bad) in [
            ("/result/id", json!("foreign")),
            ("/result/approval_id", json!("grant")),
            ("/result/revision", json!(2)),
            ("/result/revoked", json!(true)),
            ("/result/snapshot_digest", json!("0".repeat(64))),
            ("/result/bindings/task_id", json!("other")),
            ("/result/expires_at_ms", json!(i64::MAX)),
            ("/result/expires_at_ms", json!(0)),
        ] {
            let mut r = good.clone();
            *r.pointer_mut(path).unwrap() = bad;
            assert!(!receipt_matches(&r, &proposal, &selection));
            let out =
                confirm_and_finalize(json!({"result":proposal}), &selection, |_| true, |_| Ok(r))
                    .unwrap();
            assert_eq!(out["error"]["side_effect_state"], "unknown");
        }
    }
}
