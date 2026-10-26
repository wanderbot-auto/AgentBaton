#![cfg(unix)]
use agentbaton_providers::transport::{
    ApprovedInstallation, FailureKind, ProviderProcess, SideEffectState,
};
use agentbaton_providers::{ProtocolError, TurnState, MAX_FRAME_BYTES};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;

const FIXTURE: &str = r#"
import json, os, sys, time
mode=sys.argv[1]
workspace=os.getcwd()
with open('fixture.pid','w') as f: f.write(str(os.getpid()))
def emit(value): print(json.dumps(value),flush=True)
def result(request,value): emit({'jsonrpc':'2.0','id':request['id'],'result':value})
prompt_id=None
cancel_count=0
for line in sys.stdin:
    request=json.loads(line)
    method=request.get('method')
    if method=='initialize':
        if mode=='bad_init': print('broken-json',flush=True); continue
        result(request,{'protocolVersion':1,'agentCapabilities':{'loadSession':False,'_fixture':{'parent_credentials_leaked':any(k in os.environ for k in ('GH_TOKEN','GITHUB_TOKEN'))}}})
    elif method=='session/new':
        assert request['params']=={'cwd':workspace,'mcpServers':[]}
        if mode=='bad_session': result(request,{'sessionId':False}); continue
        result(request,{'sessionId':'fixture-native'})
        if mode=='blocked_input': time.sleep(60)
    elif method=='session/prompt':
        prompt_id=request['id']
        if mode=='hang': time.sleep(60)
        if mode=='eof':
            with open('side-effect.txt','w') as f: f.write('effect before unexpected exit')
            sys.exit(0)
        if mode=='partial': sys.stdout.write('{"jsonrpc":'); sys.stdout.flush(); sys.exit(0)
        if mode=='oversized': sys.stdout.write('x'*2097152); sys.stdout.flush(); continue
        if mode=='malformed': print('not JSON',flush=True); continue
        if mode=='cancel' or mode=='ignore_cancel': continue
        if mode=='stream_cancel' and cancel_count==0:
            emit({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'fixture-native','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'waiting for cancellation'}}}})
            continue
        if mode=='delayed_stream':
            emit({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'fixture-native','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'first chunk'}}}})
            time.sleep(0.15)
        if mode=='split_frame':
            wire=json.dumps({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'fixture-native','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':'split chunk'}}}})+'\n'
            middle=len(wire)//2
            sys.stdout.write(wire[:middle]);sys.stdout.flush();time.sleep(0.1)
            sys.stdout.write(wire[middle:]);sys.stdout.flush()
        if mode=='account_env':
            emit({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'fixture-native','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':os.environ.get('FIXTURE_ACCOUNT','missing')}}}})
        if mode=='stderr':
            import fcntl
            if hasattr(fcntl,'F_SETPIPE_SZ'):
                fcntl.fcntl(2,fcntl.F_SETPIPE_SZ,4096)
                with open('stderr-pipe-size.txt','w') as f: f.write(str(fcntl.fcntl(2,fcntl.F_GETPIPE_SZ)))
            for _ in range(1024): os.write(2,b'x'*4096)
        if mode in ('flood','permission_flood'):
            for n in range(400): emit({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'fixture-native','update':{'sessionUpdate':'vendor/future','sequence':n,'body':'x'*5000}}})
        if mode in ('permission','permission_flood'):
            emit({'jsonrpc':'2.0','id':'permission-1','method':'session/request_permission','params':{'sessionId':'fixture-native','toolCall':{'toolCallId':'tool-1'},'options':[{'optionId':'once','kind':'allow_once','name':'Allow'}]}})
            response=json.loads(sys.stdin.readline())
            assert response['result']['outcome']=={'outcome':'cancelled'}
        emit({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'fixture-native','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':request['params']['prompt'][0]['text']}}}})
        result(request,{'stopReason':'end_turn'})
    elif method=='session/cancel':
        cancel_count+=1
        with open('cancel-count.txt','w') as f: f.write(str(cancel_count))
        if mode=='ignore_cancel': continue
        emit({'jsonrpc':'2.0','id':prompt_id,'result':{'stopReason':'cancelled'}})
"#;
fn installation(mode: &str) -> (TempDir, ApprovedInstallation) {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fixture.py");
    std::fs::write(&script, FIXTURE).unwrap();
    // Test-only discovery of the fixture interpreter; the production API accepts
    // only an explicitly selected absolute installation path.
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let executable = std::fs::canonicalize(PathBuf::from(
        String::from_utf8(python.stdout.clone()).unwrap().trim(),
    ))
    .unwrap();
    let descriptor = ApprovedInstallation {
        expected_sha256: agentbaton_providers::installation::executable_digest(&executable)
            .unwrap(),
        executable,
        arguments: vec!["-u".into(), script.into(), mode.into()],
        workspace: dir.path().canonicalize().unwrap(),
        approval_reference: "test-fixture-approval".into(),
        environment: BTreeMap::new(),
    };
    (dir, descriptor)
}
fn start(mode: &str) -> (TempDir, ProviderProcess) {
    let (dir, descriptor) = installation(mode);
    let process = ProviderProcess::start(descriptor, Duration::from_millis(500)).unwrap();
    (dir, process)
}
fn reaped(pid: u32) {
    // SAFETY: signal 0 checks existence only; no process is signalled.
    assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}

#[test]
fn real_stdio_handshake_session_and_prompt_roundtrip_without_shell_or_parent_credentials() {
    let (dir, mut process) = start("normal");
    let native = process.new_session("trusted-connection/attempt").unwrap();
    let marker = dir.path().join("must-not-exist");
    let text = format!("$(touch {}) ; 你好", marker.display());
    process.begin_prompt(&native, &text).unwrap();
    assert_eq!(process.wait_prompt(&native).unwrap(), TurnState::Completed);
    let records = process.take_evidence();
    assert_eq!(
        records[0]["result"]["agentCapabilities"]["_fixture"]["parent_credentials_leaked"],
        false
    );
    assert!(records.iter().any(|r| r
        .pointer("/params/update/content/text")
        .and_then(serde_json::Value::as_str)
        == Some(text.as_str())));
    assert!(!marker.exists());
    let pid = process.process_id();
    process.terminate();
    assert!(process.diagnostics().process_reaped);
    reaped(pid);
}

#[test]
fn cancellation_remains_requested_until_correlated_child_response() {
    let (_dir, mut process) = start("cancel");
    let session = process.new_session("binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    process.cancel(&session).unwrap();
    assert_eq!(
        process.turn_state(&session).unwrap(),
        &TurnState::CancelRequested
    );
    assert_eq!(process.wait_prompt(&session).unwrap(), TurnState::Cancelled);
}

#[test]
fn child_timeout_kills_reaps_and_never_replays_unknown_work() {
    let (_dir, mut process) = start("hang");
    let session = process.new_session("binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    let begin = Instant::now();
    let error = process.wait_prompt(&session).unwrap_err();
    assert_eq!(error.kind, FailureKind::Timeout);
    assert_eq!(error.side_effect_state, SideEffectState::Unknown);
    assert!(begin.elapsed() < Duration::from_secs(3));
    assert_eq!(
        process.turn_state(&session).unwrap(),
        &TurnState::OutcomeUnknown
    );
    assert!(process.diagnostics().process_reaped);
    reaped(process.process_id());
    assert!(process.begin_prompt(&session, "do not retry").is_err());
}

#[test]
fn unexpected_eof_keeps_real_prior_side_effect_unknown() {
    let (dir, mut process) = start("eof");
    let session = process.new_session("binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    let error = process.wait_prompt(&session).unwrap_err();
    assert_eq!(error.kind, FailureKind::Closed);
    assert_eq!(error.side_effect_state, SideEffectState::Unknown);
    assert!(dir.path().join("side-effect.txt").exists());
    assert_eq!(
        process.turn_state(&session).unwrap(),
        &TurnState::OutcomeUnknown
    );
    assert!(process.diagnostics().process_reaped);
}

#[test]
fn malformed_partial_and_oversized_stdout_are_isolated_and_reaped() {
    for (mode, kind) in [
        ("malformed", FailureKind::Protocol),
        ("partial", FailureKind::Closed),
        ("oversized", FailureKind::Protocol),
    ] {
        let (_dir, mut process) = start(mode);
        let session = process.new_session("binding").unwrap();
        process.begin_prompt(&session, "work").unwrap();
        let error = process.wait_prompt(&session).unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(error.side_effect_state, SideEffectState::Unknown);
        if mode == "oversized" {
            assert_eq!(error.protocol_error, Some(ProtocolError::FrameTooLarge));
        }
        assert_eq!(
            process.turn_state(&session).unwrap(),
            &TurnState::OutcomeUnknown
        );
        assert!(process.diagnostics().process_reaped);
        reaped(process.process_id());
    }
}

#[test]
fn stderr_flood_cannot_deadlock_or_leak_credential_text_in_diagnostics() {
    let (dir, mut process) = start("stderr");
    let session = process.new_session("binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    let started = Instant::now();
    let state = process.wait_prompt(&session).unwrap_or_else(|failure| {
        panic!(
            "stderr flood failed after {:?}: {failure:?}; numeric-only diagnostics: {:?}",
            started.elapsed(),
            process.diagnostics()
        )
    });
    assert_eq!(state, TurnState::Completed);
    assert!(process.diagnostics().stderr_bytes_observed >= 4 * 1024 * 1024);
    assert!(process.diagnostics().stderr_discarded);
    #[cfg(target_os = "linux")]
    assert_eq!(
        std::fs::read_to_string(dir.path().join("stderr-pipe-size.txt")).unwrap(),
        "4096"
    );
    #[cfg(not(target_os = "linux"))]
    let _ = dir;
}

#[test]
fn protocol_evidence_is_bounded_and_truncation_visible() {
    let (_dir, descriptor) = installation("flood");
    let mut process = ProviderProcess::start(descriptor, Duration::from_secs(3)).unwrap();
    let session = process.new_session("binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    assert_eq!(process.wait_prompt(&session).unwrap(), TurnState::Completed);
    assert!(process.diagnostics().evidence_records_dropped > 0);
    assert!(process.diagnostics().evidence_bytes <= MAX_FRAME_BYTES);
    assert!(process.take_evidence().len() <= 128);
}

#[test]
fn no_policy_grant_means_child_permission_is_cancelled() {
    let (_dir, mut process) = start("permission");
    let session = process.new_session("binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    assert_eq!(process.wait_prompt(&session).unwrap(), TurnState::Completed);
}

#[test]
fn absent_approval_nonabsolute_paths_and_nonexecutable_selection_never_run() {
    let (_dir, mut descriptor) = installation("normal");
    descriptor.approval_reference.clear();
    let error = ProviderProcess::start(descriptor.clone(), Duration::from_secs(1))
        .err()
        .unwrap();
    assert_eq!(error.kind, FailureKind::InvalidInstallation);
    assert_eq!(error.side_effect_state, SideEffectState::None);
    descriptor.approval_reference = "approved".into();
    descriptor.executable = "python3".into();
    assert_eq!(
        ProviderProcess::start(descriptor.clone(), Duration::from_secs(1))
            .err()
            .unwrap()
            .kind,
        FailureKind::InvalidInstallation
    );
    let invalid = descriptor.workspace.join("not-executable");
    std::fs::write(&invalid, "not an executable").unwrap();
    descriptor.expected_sha256 =
        agentbaton_providers::installation::executable_digest(&invalid).unwrap();
    descriptor.executable = invalid;
    let error = ProviderProcess::start(descriptor, Duration::from_secs(1))
        .err()
        .unwrap();
    assert_eq!(error.kind, FailureKind::StartupFailed);
    assert_eq!(error.side_effect_state, SideEffectState::None);
}

#[test]
fn malformed_initialization_fails_startup_and_reaps_owned_child() {
    let (dir, descriptor) = installation("bad_init");
    let error = ProviderProcess::start(descriptor, Duration::from_secs(1))
        .err()
        .unwrap();
    assert_eq!(error.kind, FailureKind::Protocol);
    let pid = std::fs::read_to_string(dir.path().join("fixture.pid"))
        .unwrap()
        .parse()
        .unwrap();
    reaped(pid);
}

#[test]
fn malformed_session_creation_invalidates_transport_and_reaps_child() {
    let (_dir, mut process) = start("bad_session");
    let error = process.new_session("binding").unwrap_err();
    assert_eq!(error.protocol_error, Some(ProtocolError::InvalidParams));
    assert_eq!(error.side_effect_state, SideEffectState::Unknown);
    assert!(process.diagnostics().process_reaped);
    reaped(process.process_id());
}

#[test]
fn quantum_expiration_keeps_process_alive_and_exposes_streamed_typed_updates() {
    let (_dir, descriptor) = installation("delayed_stream");
    let mut process = ProviderProcess::start(descriptor, Duration::from_secs(3)).unwrap();
    let session = process
        .new_session("connection-stream/attempt-stream")
        .unwrap();
    process.begin_prompt(&session, "last chunk").unwrap();
    let mut observed = false;
    for _ in 0..20 {
        let state = process
            .poll_prompt(&session, Duration::from_millis(10))
            .unwrap();
        let events = process.take_updates();
        if events.iter().any(|e|matches!(&e.kind,agentbaton_providers::UpdateKind::AgentText{text} if text=="first chunk")) {
            assert_eq!(state,None);
            assert!(!process.diagnostics().process_reaped);
            assert_eq!(events[0].local_binding,"connection-stream/attempt-stream");
            assert!(serde_json::to_value(&events[0]).unwrap().get("raw").is_none());
            observed=true;
            break;
        }
        assert_eq!(state, None);
    }
    assert!(
        observed,
        "first streamed update must arrive before final turn response"
    );
    assert_eq!(process.wait_prompt(&session).unwrap(), TurnState::Completed);
}

#[test]
fn cancellation_during_poll_is_sent_once_and_never_replays_prompt() {
    let (dir, descriptor) = installation("stream_cancel");
    let mut process = ProviderProcess::start(descriptor, Duration::from_secs(3)).unwrap();
    let session = process.new_session("cancel-binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    assert_eq!(
        process
            .poll_prompt(&session, Duration::from_millis(10))
            .unwrap(),
        None
    );
    process.cancel(&session).unwrap();
    process.cancel(&session).unwrap();
    assert_eq!(
        process.turn_state(&session).unwrap(),
        &TurnState::CancelRequested
    );
    assert_eq!(process.wait_prompt(&session).unwrap(), TurnState::Cancelled);
    // A duplicated cancel would leave an extra old-id response and corrupt this
    // correlated second turn; it must not be sent a second time.
    process
        .begin_prompt(&session, "explicit new turn after confirmed stop")
        .unwrap();
    assert_eq!(process.wait_prompt(&session).unwrap(), TurnState::Completed);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("cancel-count.txt")).unwrap(),
        "1"
    );
}

#[test]
fn repeated_poll_quanta_cannot_extend_original_turn_deadline() {
    let (_dir, mut process) = start("ignore_cancel");
    process.set_io_timeout(Duration::from_millis(160)).unwrap();
    let session = process.new_session("timeout-binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    assert!(process.set_io_timeout(Duration::from_secs(60)).is_err());
    let started = Instant::now();
    process.cancel(&session).unwrap();
    let mut quanta = 0;
    let failure = loop {
        match process.poll_prompt(&session, Duration::from_millis(15)) {
            Ok(None) => {
                quanta += 1;
                assert!(!process.diagnostics().process_reaped);
            }
            Ok(Some(state)) => panic!("ignored cancellation cannot confirm {state:?}"),
            Err(error) => break error,
        }
    };
    assert!(quanta > 1);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(failure.kind, FailureKind::Timeout);
    assert_eq!(failure.side_effect_state, SideEffectState::Unknown);
    assert_eq!(
        process.turn_state(&session).unwrap(),
        &TurnState::OutcomeUnknown
    );
    assert!(process.diagnostics().process_reaped);
    reaped(process.process_id());
    assert!(process
        .begin_prompt(&session, "no automatic replay")
        .is_err());
}

#[test]
fn partial_frames_survive_multiple_poll_quanta() {
    let (_dir, descriptor) = installation("split_frame");
    let mut process = ProviderProcess::start(descriptor, Duration::from_secs(3)).unwrap();
    let session = process.new_session("split-binding").unwrap();
    process.begin_prompt(&session, "normal final text").unwrap();
    assert_eq!(
        process
            .poll_prompt(&session, Duration::from_millis(15))
            .unwrap(),
        None
    );
    assert_eq!(process.wait_prompt(&session).unwrap(), TurnState::Completed);
    assert!(process.take_updates().iter().any(|e|matches!(&e.kind,agentbaton_providers::UpdateKind::AgentText{text} if text=="split chunk")));
}

#[test]
fn independent_process_sessions_keep_account_markers_and_bindings_separate() {
    let (_dir_a, mut descriptor_a) = installation("account_env");
    let (_dir_b, mut descriptor_b) = installation("account_env");
    descriptor_a
        .environment
        .insert("FIXTURE_ACCOUNT".into(), "account-A".into());
    descriptor_b
        .environment
        .insert("FIXTURE_ACCOUNT".into(), "account-B".into());
    let mut a = ProviderProcess::start(descriptor_a, Duration::from_secs(3)).unwrap();
    let mut b = ProviderProcess::start(descriptor_b, Duration::from_secs(3)).unwrap();
    let session_a = a.new_session("connection-A/attempt-A").unwrap();
    let session_b = b.new_session("connection-B/attempt-B").unwrap();
    // Equal provider-native IDs do not imply shared ownership or account identity.
    assert_eq!(session_a, session_b);
    a.begin_prompt(&session_a, "turn A").unwrap();
    b.begin_prompt(&session_b, "turn B").unwrap();
    assert_eq!(a.wait_prompt(&session_a).unwrap(), TurnState::Completed);
    assert_eq!(b.wait_prompt(&session_b).unwrap(), TurnState::Completed);
    for (process, own, other, binding) in [
        (&mut a, "account-A", "account-B", "connection-A/attempt-A"),
        (&mut b, "account-B", "account-A", "connection-B/attempt-B"),
    ] {
        let events = process.take_updates();
        assert!(events.iter().all(|e| e.local_binding == binding));
        assert!(events.iter().any(
            |e| matches!(&e.kind,agentbaton_providers::UpdateKind::AgentText{text} if text==own)
        ));
        assert!(!events.iter().any(
            |e| matches!(&e.kind,agentbaton_providers::UpdateKind::AgentText{text} if text==other)
        ));
        assert_eq!(
            process.take_evidence()[0]["result"]["agentCapabilities"]["_fixture"]
                ["parent_credentials_leaked"],
            false
        );
    }
}

#[test]
fn typed_stream_backpressure_is_visible_and_permission_decision_has_no_raw_fields() {
    let (_dir, descriptor) = installation("flood");
    let mut process = ProviderProcess::start(descriptor, Duration::from_secs(3)).unwrap();
    let session = process.new_session("flood-binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    process.wait_prompt(&session).unwrap();
    assert!(process.diagnostics().update_records_dropped > 0);
    assert!(process.diagnostics().update_bytes <= MAX_FRAME_BYTES);
    assert!(process.take_updates().len() <= 128);
    let (_dir, mut process) = start("permission");
    let session = process.new_session("permission-binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    process.wait_prompt(&session).unwrap();
    let events = process.take_updates();
    assert!(events.iter().any(|e|matches!(&e.kind,agentbaton_providers::UpdateKind::PermissionDenied{tool_call_id} if tool_call_id=="tool-1")));
    assert!(events
        .iter()
        .all(|e| serde_json::to_value(e).unwrap().get("raw").is_none()));
}

#[test]
fn blocked_child_stdin_cannot_block_prompt_or_cancel_worker() {
    let (_dir, mut process) = start("blocked_input");
    let session = process.new_session("blocked-input-binding").unwrap();
    process.set_io_timeout(Duration::from_millis(180)).unwrap();
    let started = Instant::now();
    process
        .begin_prompt(&session, &"\n".repeat(65_536))
        .unwrap();
    process.cancel(&session).unwrap();
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(process.diagnostics().outbound_bytes > 0);
    assert_eq!(
        process
            .poll_prompt(&session, Duration::from_millis(10))
            .unwrap(),
        None
    );
    assert!(!process.diagnostics().process_reaped);
    let error = process.wait_prompt(&session).unwrap_err();
    assert_eq!(error.kind, FailureKind::Timeout);
    assert_eq!(
        process.turn_state(&session).unwrap(),
        &TurnState::OutcomeUnknown
    );
    assert!(process.diagnostics().process_reaped);
    assert_eq!(process.diagnostics().outbound_bytes, 0);
}

#[test]
fn control_decision_backpressure_fails_closed_instead_of_losing_permission_evidence() {
    let (_dir, descriptor) = installation("permission_flood");
    let mut process = ProviderProcess::start(descriptor, Duration::from_secs(3)).unwrap();
    let session = process.new_session("critical-event-binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    let error = process.wait_prompt(&session).unwrap_err();
    assert_eq!(error.protocol_error, Some(ProtocolError::ResourceLimit));
    assert!(process.diagnostics().process_reaped);
    assert!(process.diagnostics().update_records_dropped > 0);
}
