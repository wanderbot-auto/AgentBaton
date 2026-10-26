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
    elif method=='session/prompt':
        prompt_id=request['id']
        if mode=='hang': time.sleep(60)
        if mode=='eof':
            with open('side-effect.txt','w') as f: f.write('effect before unexpected exit')
            sys.exit(0)
        if mode=='partial': sys.stdout.write('{"jsonrpc":'); sys.stdout.flush(); sys.exit(0)
        if mode=='oversized': sys.stdout.write('x'*2097152); sys.stdout.flush(); continue
        if mode=='malformed': print('not JSON',flush=True); continue
        if mode=='cancel': continue
        if mode=='stderr':
            for _ in range(1024): os.write(2,b'x'*4096)
        if mode=='flood':
            for n in range(400): emit({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'fixture-native','update':{'sessionUpdate':'vendor/future','sequence':n,'body':'x'*5000}}})
        if mode=='permission':
            emit({'jsonrpc':'2.0','id':'permission-1','method':'session/request_permission','params':{'sessionId':'fixture-native','toolCall':{'toolCallId':'tool-1'},'options':[{'optionId':'once','kind':'allow_once','name':'Allow'}]}})
            response=json.loads(sys.stdin.readline())
            assert response['result']['outcome']=={'outcome':'cancelled'}
        emit({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':'fixture-native','update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':request['params']['prompt'][0]['text']}}}})
        result(request,{'stopReason':'end_turn'})
    elif method=='session/cancel':
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
    let descriptor = ApprovedInstallation {
        executable: PathBuf::from(String::from_utf8(python.stdout).unwrap().trim()),
        arguments: vec!["-u".into(), script.into(), mode.into()],
        workspace: dir.path().to_path_buf(),
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
    let (_dir, mut process) = start("stderr");
    let session = process.new_session("binding").unwrap();
    process.begin_prompt(&session, "work").unwrap();
    assert_eq!(process.wait_prompt(&session).unwrap(), TurnState::Completed);
    assert!(process.diagnostics().stderr_bytes_observed >= 4 * 1024 * 1024);
    assert!(process.diagnostics().stderr_discarded);
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
