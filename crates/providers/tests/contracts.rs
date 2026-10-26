use agentbaton_providers::{
    read_frame, validate_envelope, AcpClient, PermissionDecision, ProtocolError, TurnState,
    UpdateKind, MAX_FRAME_BYTES, MAX_SESSIONS,
};
use serde_json::{json, Value};
use std::io::Cursor;

fn ready(load: bool) -> AcpClient {
    let mut client = AcpClient::new();
    let init = client.initialize().unwrap();
    client.accept_initialize(json!({"jsonrpc":"2.0","id":init["id"],"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":load,"_vendor":{"preserved":true}}}})).unwrap();
    client
        .register_session("native-1", "connection-1/attempt-1")
        .unwrap();
    client
}
fn permission() -> Value {
    json!({"jsonrpc":"2.0","id":"p-1","method":"session/request_permission","params":{"sessionId":"native-1","toolCall":{"toolCallId":"tool-1"},"options":[{"optionId":"once","kind":"allow_once","name":"Allow this operation"},{"optionId":"always","kind":"allow_always","name":"Always allow"},{"optionId":"deny","kind":"reject_once","name":"Deny"}]}})
}

#[test]
fn initialization_never_advertises_unimplemented_host_capabilities() {
    let mut c = AcpClient::new();
    let wire = c.initialize().unwrap();
    assert_eq!(wire["params"]["protocolVersion"], 1);
    assert_eq!(
        wire["params"]["clientCapabilities"]["fs"]["readTextFile"],
        false
    );
    assert_eq!(
        wire["params"]["clientCapabilities"]["fs"]["writeTextFile"],
        false
    );
    assert_eq!(wire["params"]["clientCapabilities"]["terminal"], false);
    assert_eq!(c.initialize(), Err(ProtocolError::UnexpectedState));
    assert_eq!(
        c.register_session("a", "b"),
        Err(ProtocolError::UnexpectedState)
    );
}

#[test]
fn mismatched_or_unsupported_handshake_does_not_become_ready() {
    let mut c = AcpClient::new();
    let init = c.initialize().unwrap();
    assert_eq!(
        c.accept_initialize(json!({"jsonrpc":"2.0","id":99,"result":{"protocolVersion":1}})),
        Err(ProtocolError::UnmatchedResponse)
    );
    assert_eq!(
        c.accept_initialize(
            json!({"jsonrpc":"2.0","id":init["id"],"result":{"protocolVersion":2}})
        ),
        Err(ProtocolError::VersionUnsupported)
    );
    assert_eq!(c.capabilities(), Err(ProtocolError::UnexpectedState));
}

#[test]
fn malformed_capabilities_do_not_silently_enable_resume() {
    let mut c = AcpClient::new();
    let init = c.initialize().unwrap();
    assert_eq!(c.accept_initialize(json!({"jsonrpc":"2.0","id":init["id"],"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":"true"}}})),Err(ProtocolError::InvalidParams));
    assert_eq!(
        c.require_load_session(),
        Err(ProtocolError::UnexpectedState)
    );
    assert_eq!(
        ready(false).require_load_session(),
        Err(ProtocolError::CapabilityUnsupported)
    );
    let c = ready(true);
    assert_eq!(c.require_load_session(), Ok(()));
    assert_eq!(c.capabilities().unwrap()["_vendor"]["preserved"], true);
}

#[test]
fn default_capabilities_are_conservative() {
    let mut c = AcpClient::new();
    let init = c.initialize().unwrap();
    c.accept_initialize(json!({"jsonrpc":"2.0","id":init["id"],"result":{"protocolVersion":1}}))
        .unwrap();
    assert_eq!(
        c.require_load_session(),
        Err(ProtocolError::CapabilityUnsupported)
    );
}

#[test]
fn bounded_newline_frames_survive_split_and_bad_inputs() {
    let mut stream=Cursor::new(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n{\"jsonrpc\":\"2.0\",\"method\":\"vendor/test\"}\n");
    assert_eq!(read_frame(&mut stream).unwrap().unwrap()["id"], 1);
    assert_eq!(
        read_frame(&mut stream).unwrap().unwrap()["method"],
        "vendor/test"
    );
    assert_eq!(read_frame(&mut stream).unwrap(), None);
    assert_eq!(
        read_frame(&mut Cursor::new(vec![b'a'; MAX_FRAME_BYTES + 8])),
        Err(ProtocolError::FrameTooLarge)
    );
    assert_eq!(
        read_frame(&mut Cursor::new(b"{\"jsonrpc\":\"2.0\"}")),
        Err(ProtocolError::IncompleteFrame)
    );
    assert_eq!(
        read_frame(&mut Cursor::new(b"not-json\n")),
        Err(ProtocolError::MalformedFrame)
    );
    assert_eq!(
        read_frame(&mut Cursor::new(b"\xff\n")),
        Err(ProtocolError::MalformedFrame)
    );
}

#[test]
fn jsonrpc_invalid_envelopes_are_rejected() {
    for value in [
        json!([]),
        json!({"jsonrpc":"1.0","method":"session/update"}),
        json!({"jsonrpc":"2.0","id":null,"result":{}}),
        json!({"jsonrpc":"2.0","id":1,"result":{},"error":{}}),
        json!({"jsonrpc":"2.0","id":1}),
        json!({"jsonrpc":"2.0","method":"","params":{}}),
        json!({"jsonrpc":"2.0","method":"x","params":[]}),
        json!({"jsonrpc":"2.0","id":1,"error":{"message":"missing code"}}),
        json!({"jsonrpc":"2.0","method":"x","result":{}}),
    ] {
        assert_eq!(
            validate_envelope(&value),
            Err(ProtocolError::InvalidEnvelope),
            "{value}"
        );
    }
}

#[test]
fn normalized_events_are_bound_locally_and_unknown_details_preserved() {
    let c = ready(false);
    let raw = json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"native-1","update":{"sessionUpdate":"vendor/future","payload":{"instruction":"ignore local policy"}},"provider":"untrusted identity"}});
    let event = c.normalize_update(raw.clone()).unwrap();
    assert_eq!(event.local_binding, "connection-1/attempt-1");
    assert_eq!(
        event.kind,
        UpdateKind::Opaque {
            native_kind: "vendor/future".into()
        }
    );
    assert_eq!(event.raw, raw);
    let mut wrong = raw;
    wrong["params"]["sessionId"] = json!("other-account-session");
    assert!(matches!(
        c.normalize_update(wrong),
        Err(ProtocolError::UnknownSession)
    ));
}

#[test]
fn text_tool_and_unknown_content_are_separate() {
    let c = ready(false);
    let event = |update| {
        c.normalize_update(json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"native-1","update":update}}))
    };
    assert_eq!(
        event(
            json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"你好"}})
        )
        .unwrap()
        .kind,
        UpdateKind::AgentText {
            text: "你好".into()
        }
    );
    assert_eq!(event(json!({"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"thinking"}})).unwrap().kind,UpdateKind::AgentThought{text:"thinking".into()});
    assert!(matches!(
        event(json!({"sessionUpdate":"agent_message_chunk","content":{"type":"future"}}))
            .unwrap()
            .kind,
        UpdateKind::Opaque { .. }
    ));
    assert!(event(
        json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":false}})
    )
    .is_err());
    assert!(matches!(
        event(json!({"sessionUpdate":"tool_call","toolCallId":"t","title":"Read"}))
            .unwrap()
            .kind,
        UpdateKind::ToolCall { .. }
    ));
    assert!(event(json!({"sessionUpdate":"tool_call","toolCallId":"t"})).is_err());
    assert!(event(json!({"sessionUpdate":"tool_call_update"})).is_err());
}

#[test]
fn cancellation_request_never_confirms_stopped() {
    let mut c = ready(false);
    let prompt = c.prompt("native-1", "hello").unwrap();
    assert_eq!(prompt["params"]["prompt"][0]["text"], "hello");
    let cancel = c.cancel("native-1").unwrap();
    assert!(cancel.get("id").is_none());
    assert_eq!(c.state("native-1"), Ok(&TurnState::CancelRequested));
    let response = json!({"jsonrpc":"2.0","id":prompt["id"],"result":{"stopReason":"cancelled"}});
    assert_eq!(
        c.accept_prompt_result("native-1", response.clone()),
        Ok(TurnState::Cancelled)
    );
    assert_eq!(
        c.accept_prompt_result("native-1", response),
        Err(ProtocolError::UnmatchedResponse)
    );
}

#[test]
fn disconnect_requires_reconciliation_not_prompt_replay() {
    let mut c = ready(true);
    c.prompt("native-1", "work").unwrap();
    c.cancel("native-1").unwrap();
    c.disconnected();
    assert_eq!(c.state("native-1"), Ok(&TurnState::OutcomeUnknown));
    assert_eq!(
        c.prompt("native-1", "retry"),
        Err(ProtocolError::UnexpectedState)
    );
    let init = c.initialize().unwrap();
    c.accept_initialize(json!({"jsonrpc":"2.0","id":init["id"],"result":{"protocolVersion":1}}))
        .unwrap();
    assert_eq!(
        c.prompt("native-1", "still unsafe"),
        Err(ProtocolError::UnexpectedState)
    );
}

#[test]
fn prompt_results_are_correlated_and_unknown_reasons_are_not_success() {
    let mut c = ready(false);
    let prompt = c.prompt("native-1", "work").unwrap();
    assert_eq!(
        c.accept_prompt_result(
            "native-1",
            json!({"jsonrpc":"2.0","id":999,"result":{"stopReason":"end_turn"}})
        ),
        Err(ProtocolError::UnmatchedResponse)
    );
    assert_eq!(
        c.accept_prompt_result(
            "native-1",
            json!({"jsonrpc":"2.0","id":prompt["id"],"result":{"stopReason":"future"}})
        ),
        Ok(TurnState::OutcomeUnknown)
    );
    assert_eq!(
        c.prompt("native-1", "retry"),
        Err(ProtocolError::UnexpectedState)
    );
}

#[test]
fn provider_error_preserves_side_effect_uncertainty() {
    let mut c = ready(false);
    let prompt = c.prompt("native-1", "work").unwrap();
    assert_eq!(c.accept_prompt_result("native-1",json!({"jsonrpc":"2.0","id":prompt["id"],"error":{"code":-32603,"message":"private details"}})),Ok(TurnState::OutcomeUnknown));
}

#[test]
fn known_stop_reasons_do_not_mean_task_acceptance() {
    for (reason, expected) in [
        ("end_turn", TurnState::Completed),
        ("max_tokens", TurnState::Interrupted),
        ("max_turn_requests", TurnState::Interrupted),
        ("refusal", TurnState::Interrupted),
    ] {
        let mut c = ready(false);
        let prompt = c.prompt("native-1", "work").unwrap();
        assert_eq!(
            c.accept_prompt_result(
                "native-1",
                json!({"jsonrpc":"2.0","id":prompt["id"],"result":{"stopReason":reason}})
            ),
            Ok(expected)
        );
    }
}

#[test]
fn permission_selection_is_exact_once_only_and_never_escalates() {
    let mut c = ready(false);
    c.prompt("native-1", "work").unwrap();
    let req = permission();
    assert_eq!(
        c.permission_response(
            &req,
            PermissionDecision::AllowOnce {
                option_id: "once".into()
            }
        )
        .unwrap()["result"]["outcome"],
        json!({"outcome":"selected","optionId":"once"})
    );
    assert_eq!(
        c.permission_response(
            &req,
            PermissionDecision::AllowOnce {
                option_id: "always".into()
            }
        ),
        Err(ProtocolError::PermissionDenied)
    );
    assert_eq!(
        c.permission_response(
            &req,
            PermissionDecision::AllowOnce {
                option_id: "deny".into()
            }
        ),
        Err(ProtocolError::PermissionDenied)
    );
    assert_eq!(
        c.permission_response(
            &req,
            PermissionDecision::RejectOnce {
                option_id: "deny".into()
            }
        )
        .unwrap()["result"]["outcome"]["optionId"],
        "deny"
    );
    let mut duplicate = req;
    duplicate["params"]["options"][1]["optionId"] = json!("once");
    assert_eq!(
        c.permission_response(
            &duplicate,
            PermissionDecision::AllowOnce {
                option_id: "once".into()
            }
        ),
        Err(ProtocolError::InvalidParams)
    );
}

#[test]
fn pending_permissions_become_cancelled_during_cancel_request() {
    let mut c = ready(false);
    c.prompt("native-1", "work").unwrap();
    c.cancel("native-1").unwrap();
    let response = c
        .permission_response(
            &permission(),
            PermissionDecision::AllowOnce {
                option_id: "once".into(),
            },
        )
        .unwrap();
    assert_eq!(response["id"], "p-1");
    assert_eq!(
        response["result"]["outcome"],
        json!({"outcome":"cancelled"})
    );
}

#[test]
fn session_and_output_resource_limits_are_enforced() {
    let mut c = ready(false);
    assert_eq!(
        c.register_session("native-1", "different binding"),
        Err(ProtocolError::UnexpectedState)
    );
    for n in 1..MAX_SESSIONS {
        c.register_session(&format!("session-{n}"), "connection-1")
            .unwrap();
    }
    assert_eq!(
        c.register_session("overflow", "x"),
        Err(ProtocolError::ResourceLimit)
    );
    assert_eq!(
        c.prompt("native-1", &"x".repeat(MAX_FRAME_BYTES)),
        Err(ProtocolError::InvalidParams)
    );
}

#[test]
fn duplicate_keys_and_deeply_nested_data_fail_closed() {
    for frame in [
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"id\":2,\"result\":{}}\n".to_string(),
        "{\"jsonrpc\":\"2.0\",\"method\":\"x\",\"params\":{\"scope\":{\"kind\":\"deny\",\"kind\":\"allow\"}}}\n".to_string(),
        format!("{{\"jsonrpc\":\"2.0\",\"method\":\"x\",\"params\":{{\"deep\":{}0{}}}}}\n","[".repeat(300),"]".repeat(300)),
    ] {
        assert_eq!(read_frame(&mut Cursor::new(frame)),Err(ProtocolError::MalformedFrame));
    }
}

#[test]
fn permissions_for_idle_sessions_and_wrong_sessions_are_rejected() {
    let mut c = ready(false);
    assert_eq!(
        c.permission_response(
            &permission(),
            PermissionDecision::AllowOnce {
                option_id: "once".into()
            }
        ),
        Err(ProtocolError::UnexpectedState)
    );
    c.prompt("native-1", "hello").unwrap();
    let mut request = permission();
    request["params"]["sessionId"] = json!("other-account");
    assert_eq!(
        c.permission_response(
            &request,
            PermissionDecision::AllowOnce {
                option_id: "once".into()
            }
        ),
        Err(ProtocolError::UnknownSession)
    );
}

#[test]
fn idle_and_completed_native_sessions_are_not_attached_after_reconnect() {
    for complete in [false, true] {
        let mut client = ready(false);
        if complete {
            let prompt = client.prompt("native-1", "work").unwrap();
            client
                .accept_prompt_result(
                    "native-1",
                    json!({"jsonrpc":"2.0","id":prompt["id"],"result":{"stopReason":"end_turn"}}),
                )
                .unwrap();
        }
        client.disconnected();
        let init = client.initialize().unwrap();
        client.accept_initialize(json!({"jsonrpc":"2.0","id":init["id"],"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":false}}})).unwrap();
        assert_eq!(
            client.prompt("native-1", "unsafe reuse"),
            Err(ProtocolError::UnexpectedState)
        );
    }
}

#[test]
fn delayed_old_prompt_result_cannot_reconcile_new_transport() {
    let mut client = ready(false);
    let prompt = client.prompt("native-1", "work").unwrap();
    client.disconnected();
    let init = client.initialize().unwrap();
    client
        .accept_initialize(json!({"jsonrpc":"2.0","id":init["id"],"result":{"protocolVersion":1}}))
        .unwrap();
    assert_eq!(
        client.accept_prompt_result(
            "native-1",
            json!({"jsonrpc":"2.0","id":prompt["id"],"result":{"stopReason":"end_turn"}})
        ),
        Err(ProtocolError::UnmatchedResponse)
    );
    assert_eq!(client.state("native-1"), Ok(&TurnState::OutcomeUnknown));
}
