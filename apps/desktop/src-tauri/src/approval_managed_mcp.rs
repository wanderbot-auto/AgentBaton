//! Closed, host-only review of one explicitly selected native MCP configuration.
//! Complete server snapshots, including retained Skill text, are reviewed in
//! lossless pages; a renderer-provided RuntimePlan is never authority.
use super::*;

pub fn confirm_managed_mcp_and_finalize(
    response: Value,
    selection: &Value,
    request_key: &str,
    mut confirm_native: impl FnMut(&str) -> bool,
    finalize: impl FnOnce(Value) -> Result<Value, String>,
) -> Result<Value, String> {
    use agentbaton_core::sessions::{
        ManagedMcpConfirmation, ManagedMcpOperation, ManagedMcpPreview, ManagedMcpSelection,
        ProviderConnection,
    };
    if let Some(error) = graph_server_error(&response) {
        return error;
    }
    let invalid = || {
        graph_refused("Managed MCP review does not match the exact program, source identity, tool list and unchanged base policy; nothing was approved")
    };
    let connection_fields = [
        "id",
        "name",
        "provider",
        "protocol_route",
        "executable",
        "arguments",
        "executable_sha256",
        "environment_id",
        "identity_ref",
        "config_revision",
        "runtime_plan",
        "host_id",
        "approval_id",
        "probe_state",
        "revision",
        "capability_snapshot_id",
        "identity_status",
        "config_status",
        "created_at",
        "version_evidence",
    ];
    let profile_fields = [
        "connection_id",
        "connection_revision",
        "config_revision",
        "runtime_plan_digest",
        "sandbox_mode",
        "approval_policy",
        "has_managed_skill",
        "managed_mcp",
        "runtime_connected",
        "runtime_used",
    ];
    if serde_json::to_vec(&response).map_or(true, |raw| raw.len() > GRAPH_REVIEW_SNAPSHOT_BYTES) {
        return invalid();
    }
    let proposal = &response["result"];
    let snapshot = &proposal["snapshot"];
    let Ok(intent) = serde_json::from_value::<ManagedMcpSelection>(selection.clone()) else {
        return invalid();
    };
    let Ok(preview) = serde_json::from_value::<ManagedMcpPreview>(snapshot["preview"].clone())
    else {
        return invalid();
    };
    let Ok(base) =
        serde_json::from_value::<ProviderConnection>(snapshot["base_connection"].clone())
    else {
        return invalid();
    };
    let Some(mut expected_plan) = base.runtime_plan.clone() else {
        return invalid();
    };
    let mut derived = base.clone();
    derived.runtime_plan = match serde_json::from_value(snapshot["after_plan"].clone()) {
        Ok(p) => Some(p),
        Err(_) => return invalid(),
    };
    let after_plan = derived.runtime_plan.as_ref().expect("constructed plan");
    if !exact_fields(selection, &["request", "expected_preview_digest"])
        || !exact_fields(
            &selection["request"],
            &[
                "base_connection_id",
                "expected_base_revision",
                "new_name",
                "operation",
                "server",
            ],
        )
        || !exact_fields(
            snapshot,
            &[
                "intent",
                "preview",
                "base_connection",
                "base_connection_digest",
                "after_plan",
                "after_plan_digest",
                "source_binding",
                "request_key",
                "scope",
                "warning",
            ],
        )
        || !exact_fields(
            &snapshot["preview"],
            &[
                "base",
                "operation",
                "new_name",
                "server",
                "policy_unchanged",
                "preview_digest",
            ],
        )
        || !exact_fields(&snapshot["preview"]["base"], &profile_fields)
        || !exact_fields(&snapshot["base_connection"], &connection_fields)
        || proposal["kind"] != "managed_mcp"
        || !graph_id(&proposal["id"])
        || !graph_revision(&proposal["revision"])
        || snapshot["intent"] != *selection
        || !graph_id(&json!(request_key))
        || snapshot["request_key"] != request_key
        || snapshot["scope"] != "new_managed_connection_version_probe_only"
        || !snapshot["warning"].as_str().is_some_and(|v| !v.is_empty())
        || !graph_hash(&snapshot["base_connection_digest"])
        || !graph_hash(&snapshot["after_plan_digest"])
        || !graph_hash(&json!(intent.expected_preview_digest))
        || preview.preview_digest != intent.expected_preview_digest
        || preview.new_name != intent.request.new_name
        || preview.operation != intent.request.operation
        || !preview.policy_unchanged
        || preview.new_name.trim().is_empty()
        || preview.new_name.len() > 128
        || preview.new_name.chars().any(char::is_control)
        || base.id != intent.request.base_connection_id
        || base.revision != intent.request.expected_base_revision
        || !graph_revision(&json!(base.revision))
        || base.provider != "codex"
        || snapshot["base_connection"]["protocol_route"] != "codex_app_server"
        || base.host_id != "local"
        || base.probe_state != "ready"
        || !base.executable.is_absolute()
        || !graph_hash(&json!(base.executable_sha256))
        || preview.base.connection_id != base.id
        || preview.base.connection_revision != base.revision
        || preview.base.config_revision != base.config_revision
        || expected_plan.plan_digest().ok().as_ref() != Some(&preview.base.runtime_plan_digest)
        || json!(expected_plan.provider) != "codex"
        || expected_plan.codex.model_name().is_some()
        || json!(expected_plan.codex.sandbox) != preview.base.sandbox_mode
        || json!(expected_plan.codex.approval_policy) != preview.base.approval_policy
        || preview.base.has_managed_skill != !expected_plan.skills.is_empty()
        || !expected_plan.claude.enabled_project_mcp_names.is_empty()
        || !expected_plan.claude.disabled_project_mcp_names.is_empty()
        || expected_plan.mcp_servers.len() != usize::from(preview.base.managed_mcp.is_some())
        || preview.base.runtime_connected.is_some()
        || preview.base.runtime_used.is_some()
        || after_plan.plan_digest().ok().as_deref() != snapshot["after_plan_digest"].as_str()
    {
        return invalid();
    }
    if let Some(before) = &preview.base.managed_mcp {
        let server = serde_json::to_value(&expected_plan.mcp_servers[0])
            .map_err(|_| "Native review serialization failed".to_owned())?;
        if !summary_matches_server(&snapshot["preview"]["base"]["managed_mcp"], &server)
            || before.approval_mode != "prompt"
        {
            return invalid();
        }
    }
    expected_plan.mcp_servers.clear();
    expected_plan
        .source_refs
        .retain(|s| s.source_ref != "baton-managed-codex-mcp");
    match intent.request.operation {
        ManagedMcpOperation::Replace => {
            let (Some(requested), Some(summary)) = (&intent.request.server, &preview.server) else {
                return invalid();
            };
            let mut tools = requested.enabled_tools.clone();
            tools.sort();
            let binding = &snapshot["source_binding"];
            let identity = &binding["identity"];
            if !exact_fields(
                &selection["request"]["server"],
                &["name", "executable", "enabled_tools"],
            ) || requested.name != summary.name
                || requested.executable != summary.executable
                || tools != summary.enabled_tools
                || !valid_names(&json!(tools))
                || summary.approval_mode != "prompt"
                || !summary.executable.starts_with('/')
                || summary.executable.len() > 4096
                || summary.executable.chars().any(char::is_control)
                || !graph_hash(&json!(summary.executable_sha256))
                || !exact_fields(binding, &["executable_sha256", "identity"])
                || binding["executable_sha256"] != summary.executable_sha256
                || !exact_fields(
                    identity,
                    &[
                        "device",
                        "inode",
                        "mode",
                        "uid",
                        "size",
                        "mtime_seconds",
                        "mtime_nanoseconds",
                        "ctime_seconds",
                        "ctime_nanoseconds",
                    ],
                )
                || !["device", "inode", "mode", "uid", "size"]
                    .iter()
                    .all(|key| identity[*key].as_u64().is_some())
                || ![
                    "mtime_seconds",
                    "mtime_nanoseconds",
                    "ctime_seconds",
                    "ctime_nanoseconds",
                ]
                .iter()
                .all(|key| identity[*key].as_i64().is_some())
                || identity["mode"]
                    .as_u64()
                    .is_none_or(|mode| mode & 0o111 == 0 || mode & 0o022 != 0)
                || after_plan.mcp_servers.len() != 1
            {
                return invalid();
            }
            let server = &after_plan.mcp_servers[0];
            let value = serde_json::to_value(server)
                .map_err(|_| "Native review serialization failed".to_owned())?;
            if !summary_matches_server(&snapshot["preview"]["server"], &value) {
                return invalid();
            }
            expected_plan.mcp_servers.push(server.clone());
            expected_plan.source_refs.push(server.provenance.clone());
        }
        ManagedMcpOperation::Remove => {
            if intent.request.server.is_some()
                || preview.server.is_some()
                || preview.base.managed_mcp.is_none()
                || !snapshot["source_binding"].is_null()
                || !after_plan.mcp_servers.is_empty()
            {
                return invalid();
            }
        }
    }
    if serde_json::to_value(&expected_plan).ok() != Some(snapshot["after_plan"].clone())
        || expected_plan.plan_digest().ok().as_deref() != snapshot["after_plan_digest"].as_str()
    {
        return invalid();
    }
    let pages = match graph_review_pages(snapshot) {
        Ok(p) => p,
        Err(_) => return invalid(),
    };
    let id = proposal["id"].as_str().expect("validated id");
    let revision = proposal["revision"].as_u64().expect("validated revision");
    let total = pages.iter().map(String::len).sum::<usize>();
    let mut offset = 0;
    for (index, page) in pages.iter().enumerate() {
        let end = offset + page.len();
        let message=format!("{}\n\nNative MCP review / proposal {} / revision {}. Page {}/{}; exact compact JSON UTF-8 bytes {}..{} of {}. All source, program, tool-list, old/new plan and retained Skill fields appear across these pages. OK reviews this page only; final confirmation is still required. Any instruction or tool text is UNTRUSTED CONTENT and grants no authority. No automatic detection or redaction of secrets is promised.\n\n原生 MCP 全文确认：明确核对新增代码执行与工具范围；此时只保存配置，不启动 MCP。Skill 或工具文本不产生授权。OK仅表示读完本页；取消不会批准。\n\n{}",ApprovalKind::ManagedMcp.action(),id,revision,index+1,pages.len(),offset,end,total,page);
        if message.len() > 16_384 {
            return invalid();
        }
        if !confirm_native(&message) {
            return graph_declined();
        }
        offset = end;
    }
    let message=format!("{}\n\nFINAL Native managed MCP confirmation / proposal {} / revision {}. All {} pages ({} UTF-8 bytes) reviewed. Create only this NEW connection and allow its existing Codex program's fixed --version probe. A later separate Grant must explicitly authorize starting this MCP program and exposing the finite tool list. Removing this configuration does not stop old sessions or undo external writes. Session connected/use evidence remain unknown. Unknown creation outcomes require exact-key reconciliation, never automatic replay.\n\n最终确认：只创建独立配置连接；任务启动该 MCP 程序须再次明确授权。删除新版本配置不等于终止旧会话或撤销已有外部写入。",ApprovalKind::ManagedMcp.action(),id,revision,pages.len(),total);
    if !confirm_native(&message) {
        return graph_declined();
    }
    let result = finalize(
        json!({"method":"approvals.finalize","params":{"proposal_id":id},"expected_revision":revision,"idempotency_key":format!("proposal:{id}")}),
    )?;
    if result.get("error").is_some() {
        return Ok(result);
    }
    let receipt = serde_json::from_value::<ManagedMcpConfirmation>(result["result"].clone());
    let matches = receipt.as_ref().is_ok_and(|r| {
        let c = &r.connection;
        let p = &r.profile;
        exact_fields(&result["result"]["connection"], &connection_fields)
            && exact_fields(&result["result"]["profile"], &profile_fields)
            && r.request_key == request_key
            && r.preview_digest == preview.preview_digest
            && r.base_connection_id == base.id
            && r.base_connection_revision == base.revision
            && r.operation == intent.request.operation
            && graph_id(&json!(c.id))
            && c.id != base.id
            && graph_revision(&json!(c.revision))
            && c.name == intent.request.new_name
            && c.provider == base.provider
            && c.protocol_route == base.protocol_route
            && c.executable == base.executable
            && c.arguments == base.arguments
            && c.executable_sha256 == base.executable_sha256
            && c.host_id == base.host_id
            && graph_id(&json!(c.environment_id))
            && c.environment_id != base.environment_id
            && graph_id(&json!(c.identity_ref))
            && c.identity_ref != base.identity_ref
            && graph_id(&json!(c.approval_id))
            && c.approval_id != base.approval_id
            && c.capability_snapshot_id != base.capability_snapshot_id
            && c.identity_status == base.identity_status
            && c.config_status == base.config_status
            && c.runtime_plan
                .as_ref()
                .and_then(|p| serde_json::to_value(p).ok())
                == Some(snapshot["after_plan"].clone())
            && p.connection_id == c.id
            && p.connection_revision == c.revision
            && p.config_revision == c.config_revision
            && Some(p.runtime_plan_digest.as_str()) == snapshot["after_plan_digest"].as_str()
            && p.sandbox_mode == preview.base.sandbox_mode
            && p.approval_policy == preview.base.approval_policy
            && p.has_managed_skill == preview.base.has_managed_skill
            && json!(p.managed_mcp) == json!(preview.server)
            && p.runtime_connected.is_none()
            && p.runtime_used.is_none()
    });
    if matches {
        Ok(result)
    } else {
        Ok(
            json!({"request_id":null,"error":{"code":"OUTCOME_UNKNOWN","message":"Managed MCP returned an unbound receipt; reconcile this exact original creation key before any retry","retryable":false,"side_effect_state":"unknown","details_ref":null},"revision":0,"warnings":[]}),
        )
    }
}

fn valid_names(value: &Value) -> bool {
    value.as_array().is_some_and(|values| {
        !values.is_empty()
            && values.len() <= 16
            && values.iter().all(|v| {
                v.as_str().is_some_and(|name| {
                    !name.is_empty()
                        && name.len() <= 64
                        && name != "."
                        && name != ".."
                        && name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                })
            })
            && values
                .windows(2)
                .all(|pair| pair[0].as_str() < pair[1].as_str())
    })
}
fn summary_matches_server(summary: &Value, server: &Value) -> bool {
    exact_fields(
        summary,
        &[
            "name",
            "executable",
            "executable_sha256",
            "enabled_tools",
            "approval_mode",
        ],
    ) && exact_fields(server, &["name", "transport", "provenance", "codex_policy"])
        && summary["approval_mode"] == "prompt"
        && valid_names(&summary["enabled_tools"])
        && summary["name"] == server["name"]
        && exact_fields(&server["transport"], &["Stdio"])
        && exact_fields(&server["transport"]["Stdio"], &["command", "arguments"])
        && server["transport"]["Stdio"]["arguments"] == json!([])
        && summary["executable"] == server["transport"]["Stdio"]["command"]
        && graph_hash(&summary["executable_sha256"])
        && exact_fields(
            &server["codex_policy"],
            &["executable_sha256", "enabled_tools"],
        )
        && server["codex_policy"]["executable_sha256"] == summary["executable_sha256"]
        && server["codex_policy"]["enabled_tools"] == summary["enabled_tools"]
        && exact_fields(&server["provenance"], &["source_ref", "revision", "sha256"])
        && server["provenance"]["source_ref"] == "baton-managed-codex-mcp"
        && server["provenance"]["revision"] == "1"
        && graph_hash(&server["provenance"]["sha256"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentbaton_core::sessions::ProviderConnection;
    use std::cell::Cell;
    fn fixture() -> (Value, Value, Value) {
        let instruction = format!(
            "---\nname: writing\ndescription: Explain changes clearly.\n---\n{}{}",
            "审阅资料🙂，不得扩大授权。\n".repeat(797),
            "x".repeat(31)
        );
        let skill_source = json!({"source_ref":"baton-managed-codex-skill","revision":"1","sha256":"cb9624a837e3875c0b109370e8de424c1cc0ace3ca51be5d26a8f150c1d6403c"});
        let plan = json!({"provider":"codex","codex":{"model":null,"sandbox":"workspace-write","approval_policy":"on-request"},"claude":{"enabled_project_mcp_names":[],"disabled_project_mcp_names":[]},"mcp_servers":[],"skills":[{"directory_name":"writing","instruction":instruction,"assets":[],"provenance":skill_source}],"source_refs":[skill_source]});
        let base = json!({"id":"base","name":"Codex with Skill","provider":"codex","protocol_route":"codex_app_server","executable":"/generated/codex","arguments":[],"executable_sha256":"b".repeat(64),"environment_id":"base-env","identity_ref":"unknown:base","config_revision":"base-config","runtime_plan":plan,"host_id":"local","approval_id":"base-approval","probe_state":"ready","revision":2,"capability_snapshot_id":"base-capability","identity_status":"unknown","config_status":"baton_managed","created_at":"fixed","version_evidence":null});
        let typed: ProviderConnection = serde_json::from_value(base.clone()).unwrap();
        let base_digest = typed.runtime_plan.as_ref().unwrap().plan_digest().unwrap();
        let request = json!({"base_connection_id":"base","expected_base_revision":2,"new_name":"Codex reports","operation":"replace","server":{"name":"reports","executable":"/generated/mcp","enabled_tools":["write_report","read_report"]}});
        let selection = json!({"request":request,"expected_preview_digest":"c".repeat(64)});
        let summary = json!({"name":"reports","executable":"/generated/mcp","executable_sha256":"a".repeat(64),"enabled_tools":["read_report","write_report"],"approval_mode":"prompt"});
        let profile = json!({"connection_id":"base","connection_revision":2,"config_revision":"base-config","runtime_plan_digest":base_digest,"sandbox_mode":"workspace-write","approval_policy":"on-request","has_managed_skill":true,"managed_mcp":null,"runtime_connected":null,"runtime_used":null});
        let preview = json!({"base":profile,"operation":"replace","new_name":"Codex reports","server":summary,"policy_unchanged":true,"preview_digest":"c".repeat(64)});
        let mcp_source = json!({"source_ref":"baton-managed-codex-mcp","revision":"1","sha256":"64ad4eb0b68e94002e2d0220f826548ed08d557e2024611b5cc983f311824464"});
        let server = json!({"name":"reports","transport":{"Stdio":{"command":"/generated/mcp","arguments":[]}},"codex_policy":{"executable_sha256":"a".repeat(64),"enabled_tools":["read_report","write_report"]},"provenance":mcp_source});
        let mut after = plan;
        after["mcp_servers"] = json!([server]);
        after["source_refs"]
            .as_array_mut()
            .unwrap()
            .push(mcp_source);
        let mut connection = base.clone();
        connection["id"] = json!("derived");
        connection["name"] = json!("Codex reports");
        connection["environment_id"] = json!("derived-env");
        connection["identity_ref"] = json!("unknown:derived");
        connection["approval_id"] = json!("new-approval");
        connection["capability_snapshot_id"] = json!("new-capability");
        connection["config_revision"] = json!("new-config");
        connection["runtime_plan"] = after.clone();
        connection["revision"] = json!(1);
        connection["probe_state"] = json!("pending");
        let typed: ProviderConnection = serde_json::from_value(connection.clone()).unwrap();
        let after_digest = typed.runtime_plan.as_ref().unwrap().plan_digest().unwrap();
        let proposal = json!({"result":{"id":"mcp-proposal","kind":"managed_mcp","revision":1,"snapshot":{"intent":selection,"preview":preview,"base_connection":base,"base_connection_digest":"d".repeat(64),"after_plan":after,"after_plan_digest":after_digest,"source_binding":{"executable_sha256":"a".repeat(64),"identity":{"device":1,"inode":2,"mode":0o100700,"uid":1000,"size":100,"mtime_seconds":1,"mtime_nanoseconds":0,"ctime_seconds":1,"ctime_nanoseconds":0}},"request_key":"request","scope":"new_managed_connection_version_probe_only","warning":"Selected code runs only after separate Grant"}}});
        let receipt = json!({"result":{"connection":connection,"profile":{"connection_id":"derived","connection_revision":1,"config_revision":"new-config","runtime_plan_digest":after_digest,"sandbox_mode":"workspace-write","approval_policy":"on-request","has_managed_skill":true,"managed_mcp":summary,"runtime_connected":null,"runtime_used":null},"request_key":"request","preview_digest":"c".repeat(64),"base_connection_id":"base","base_connection_revision":2,"operation":"replace"}});
        (selection, proposal, receipt)
    }
    #[test]
    fn full_mcp_review_keeps_large_skill_and_sends_only_exact_owner_reference() {
        let (selection, proposal, receipt) = fixture();
        let pages = graph_review_pages(&proposal["result"]["snapshot"]).unwrap();
        assert!(pages.len() > 4);
        assert_eq!(
            pages.concat(),
            serde_json::to_string(&proposal["result"]["snapshot"]).unwrap()
        );
        let mut prompts = Vec::new();
        let result=confirm_managed_mcp_and_finalize(proposal,&selection,"request",|m|{prompts.push(m.to_owned());true},|command|{assert_eq!(command,json!({"method":"approvals.finalize","params":{"proposal_id":"mcp-proposal"},"expected_revision":1,"idempotency_key":"proposal:mcp-proposal"}));Ok(receipt.clone())}).unwrap();
        assert_eq!(result, receipt);
        assert_eq!(prompts.len(), pages.len() + 1);
        for (prompt, page) in prompts.iter().zip(pages) {
            assert!(prompt.ends_with(&page));
            assert!(prompt.len() < 16384);
            assert!(prompt.contains("UNTRUSTED CONTENT"));
            assert!(prompt.contains("network-deny"));
        }
        assert!(prompts.last().unwrap().contains("later separate Grant"));
    }
    #[test]
    fn every_mcp_page_and_final_can_be_declined_and_server_error_never_prompts() {
        let (selection, proposal, _) = fixture();
        let count = graph_review_pages(&proposal["result"]["snapshot"])
            .unwrap()
            .len()
            + 1;
        for cancelled in 0..count {
            let index = Cell::new(0);
            let r = confirm_managed_mcp_and_finalize(
                proposal.clone(),
                &selection,
                "request",
                |_| {
                    let i = index.get();
                    index.set(i + 1);
                    i != cancelled
                },
                |_| panic!("cancel cannot finalize"),
            )
            .unwrap();
            assert_eq!(r["error"]["side_effect_state"], "not_started");
            assert_eq!(index.get(), cancelled + 1);
        }
        let mut error = proposal;
        error["error"] = json!({"code":"OUTCOME_UNKNOWN","message":"Uncertain original request","retryable":false,"side_effect_state":"unknown"});
        let result = confirm_managed_mcp_and_finalize(
            error.clone(),
            &selection,
            "request",
            |_| panic!("error cannot prompt"),
            |_| panic!("error cannot finalize"),
        )
        .unwrap();
        assert_eq!(result, error);
    }
    #[test]
    fn mcp_review_refuses_expanded_commands_tools_policies_or_source_before_prompt() {
        let (selection, original, _) = fixture();
        for variant in 0..15 {
            let mut p = original.clone();
            let s = &mut p["result"]["snapshot"];
            match variant {
                0 => s["intent"]["request"]["expected_base_revision"] = json!(3),
                1 => s["after_plan"]["codex"]["sandbox"] = json!("read-only"),
                2 => s["after_plan"]["codex"]["model"] = json!("override"),
                3 => s["after_plan"]["skills"][0]["instruction"] = json!("changed old instruction"),
                4 => {
                    s["after_plan"]["mcp_servers"][0]["transport"]["Stdio"]["arguments"] =
                        json!(["secret"])
                }
                5 => {
                    s["after_plan"]["mcp_servers"][0]["transport"] =
                        json!({"Http":{"endpoint":"https://private.invalid/mcp"}})
                }
                6 => s["preview"]["server"]["enabled_tools"] = json!(["extra_tool"]),
                7 => s["preview"]["server"]["approval_mode"] = json!("auto"),
                8 => s["source_binding"]["executable_sha256"] = json!("f".repeat(64)),
                9 => s["source_binding"]["identity"]["mode"] = json!(0o100777),
                10 => s["preview"]["base"]["runtime_connected"] = json!(true),
                11 => s["scope"] = json!("execute_mcp_now"),
                12 => s["request_key"] = json!("other"),
                13 => {
                    s["after_plan"]["mcp_servers"][0]["codex_policy"]["enabled_tools"] = json!([])
                }
                _ => s["unreviewed_extra_authority"] = json!(true),
            }
            let r = confirm_managed_mcp_and_finalize(
                p,
                &selection,
                "request",
                |_| panic!("invalid proposal cannot prompt"),
                |_| panic!("invalid proposal cannot finalize"),
            )
            .unwrap();
            assert_eq!(
                r["error"]["code"], "INVALID_APPROVAL_PROPOSAL",
                "variant {variant}"
            );
        }
    }
    #[test]
    fn mcp_unbound_postcommit_receipts_are_unknown_and_generic_approval_is_closed() {
        let (selection, proposal, receipt) = fixture();
        for variant in 0..7 {
            let mut r = receipt.clone();
            match variant {
                0 => r["result"]["connection"]["id"] = json!("base"),
                1 => r["result"]["profile"]["runtime_used"] = json!(true),
                2 => r["result"]["connection"]["environment_id"] = json!("base-env"),
                3 => r["result"]["profile"]["managed_mcp"]["approval_mode"] = json!("auto"),
                4 => {
                    r["result"]["connection"]["runtime_plan"]["codex"]["sandbox"] =
                        json!("read-only")
                }
                5 => r["result"]["request_key"] = json!("other"),
                _ => r["result"]["profile"]["has_managed_skill"] = json!(false),
            }
            let result = confirm_managed_mcp_and_finalize(
                proposal.clone(),
                &selection,
                "request",
                |_| true,
                |_| Ok(r),
            )
            .unwrap();
            assert_eq!(
                result["error"]["side_effect_state"], "unknown",
                "variant {variant}"
            );
        }
        let r = confirm_and_finalize(
            proposal,
            ApprovalKind::ManagedMcp,
            |_| panic!("generic route cannot skip typed full review"),
            |_| panic!("generic route cannot finalize"),
        )
        .unwrap();
        assert_eq!(r["error"]["side_effect_state"], "not_started");
    }
    #[test]
    fn removing_mcp_preserves_skill_and_never_claims_old_session_revocation() {
        let (_, old_proposal, old_receipt) = fixture();
        let mut base = old_receipt["result"]["connection"].clone();
        base["revision"] = json!(2);
        base["probe_state"] = json!("ready");
        let mut base_profile = old_receipt["result"]["profile"].clone();
        base_profile["connection_revision"] = json!(2);
        let selection = json!({"request":{"base_connection_id":"derived","expected_base_revision":2,"new_name":"No MCP","operation":"remove","server":null},"expected_preview_digest":"e".repeat(64)});
        let after = old_proposal["result"]["snapshot"]["base_connection"]["runtime_plan"].clone();
        let after_digest =
            old_proposal["result"]["snapshot"]["preview"]["base"]["runtime_plan_digest"].clone();
        let preview = json!({"base":base_profile,"operation":"remove","new_name":"No MCP","server":null,"policy_unchanged":true,"preview_digest":"e".repeat(64)});
        let proposal = json!({"result":{"id":"remove-proposal","kind":"managed_mcp","revision":1,"snapshot":{"intent":selection,"preview":preview,"base_connection":base,"base_connection_digest":"d".repeat(64),"after_plan":after,"after_plan_digest":after_digest,"source_binding":null,"request_key":"request","scope":"new_managed_connection_version_probe_only","warning":"Old sessions stay unchanged"}}});
        let mut c = old_proposal["result"]["snapshot"]["base_connection"].clone();
        c["id"] = json!("removed");
        c["name"] = json!("No MCP");
        c["environment_id"] = json!("removed-env");
        c["identity_ref"] = json!("unknown:removed");
        c["approval_id"] = json!("remove-approval");
        c["capability_snapshot_id"] = json!("removed-capability");
        c["config_revision"] = json!("removed-config");
        c["revision"] = json!(1);
        c["probe_state"] = json!("pending");
        let receipt = json!({"result":{"connection":c,"profile":{"connection_id":"removed","connection_revision":1,"config_revision":"removed-config","runtime_plan_digest":after_digest,"sandbox_mode":"workspace-write","approval_policy":"on-request","has_managed_skill":true,"managed_mcp":null,"runtime_connected":null,"runtime_used":null},"request_key":"request","preview_digest":"e".repeat(64),"base_connection_id":"derived","base_connection_revision":2,"operation":"remove"}});
        let mut messages = Vec::new();
        let result = confirm_managed_mcp_and_finalize(
            proposal,
            &selection,
            "request",
            |m| {
                messages.push(m.to_owned());
                true
            },
            |_| Ok(receipt.clone()),
        )
        .unwrap();
        assert_eq!(result, receipt);
        assert!(messages
            .last()
            .unwrap()
            .contains("does not stop old sessions"));
    }
}
