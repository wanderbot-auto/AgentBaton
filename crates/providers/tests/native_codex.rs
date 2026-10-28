#![cfg(unix)]
//! Generated local protocol processes only; no vendor executable/login/model/network.
use agentbaton_providers::connections::ProtocolRoute;
use agentbaton_providers::driver::ManagedProviderProcess;
use agentbaton_providers::transport::{ApprovedInstallation, FailureKind, SideEffectState};
use agentbaton_providers::{TurnState, UpdateKind, MAX_FRAME_BYTES};
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const FIXTURE: &str = r#"
import os,sys,json,time
mode=sys.argv[1];cwd=os.getcwd();initialized=False;cancel_count=0;turn_count=0
with open('fixture.pid','w') as f:f.write(str(os.getpid()))
def record(direction,value):
 with open('wire.ndjson','a') as f:f.write(json.dumps({'direction':direction,'frame':value})+'\n')
def send(v):
 record('provider',v);print(json.dumps(v),flush=True)
def result(r,v): send({'id':r['id'],'result':v})
def turn(status='inProgress',id=None): return {'id':id or 'turn-'+str(turn_count),'items':[],'status':status}
def complete(status='completed',thread='thread-1',id=None):send({'method':'turn/completed','params':{'threadId':thread,'turn':turn(status,id)}})
def delta(value='native text'):send({'method':'item/agentMessage/delta','params':{'threadId':'thread-1','turnId':'turn-'+str(turn_count),'itemId':'message-1','delta':value}})
for line in sys.stdin:
 r=json.loads(line);record('client',r);m=r.get('method')
 assert 'jsonrpc' not in r
 if m=='initialize':
  assert r['params']['capabilities']=={'experimentalApi':False}
  assert 'protocolVersion' not in r['params']
  if mode=='acp_init':result(r,{'protocolVersion':1});continue
  result(r,{'codexHome':'/unapproved-home' if mode=='wrong_home' else os.environ.get('CODEX_HOME',cwd),'platformFamily':'unix','platformOs':'fixture','userAgent':'generated native contract fixture'})
 elif m=='initialized':
  assert r=={'method':'initialized'};initialized=True
 elif m=='thread/start':
  assert initialized and r['params']=={'cwd':cwd,'approvalPolicy':'on-request','sandbox':'workspace-write','ephemeral':True}
  t={'id':'thread-1','cliVersion':'fixture','createdAt':1,'updatedAt':1,'cwd':cwd,'ephemeral':True,'modelProvider':'fixture','preview':'','projectId':None,'sessionId':'native-session','source':'appServer','status':{'type':'idle'},'turns':[]}
  sandbox={'type':'workspaceWrite','networkAccess':False,'writableRoots':[cwd]}
  if mode=='expanded_scope':sandbox['networkAccess']=True
  if mode=='invalid_scope':sandbox['networkAccess']='false'
  if mode=='extra_root':sandbox['writableRoots'].append('/unapproved-root')
  if mode=='malformed_session':result(r,{'thread':False});continue
  result(r,{'thread':t,'cwd':cwd,'approvalPolicy':'on-request','approvalsReviewer':'user','model':'fixture','modelProvider':'fixture','sandbox':sandbox})
  if mode=='blocked_input':time.sleep(60)
 elif m=='turn/start':
  turn_count+=1
  if mode=='stale_terminal' and turn_count>1:complete(id='turn-1');continue
  assert r['params']['threadId']=='thread-1'
  assert r['params']['input'][0]['type']=='text'
  assert r['params']['sandboxPolicy']=={'type':'workspaceWrite','writableRoots':[cwd],'networkAccess':False,'excludeSlashTmp':True,'excludeTmpdirEnvVar':True}
  assert not any(k in r['params'] for k in ('model','modelProvider','serviceTier'))
  if mode=='error':send({'id':r['id'],'error':{'code':-32000,'message':'secret fixture error'}});continue
  if mode=='early_complete':complete();result(r,{'turn':turn()});continue
  if mode=='wrong_turn':complete(id='wrong-turn');result(r,{'turn':turn()});continue
  result(r,{'turn':turn('completed' if mode=='ack_only' else 'inProgress')})
  if mode in ('hang','cancel','interrupt_ack_only','cancel_race') or (mode=='late_interrupt_ack' and turn_count==1):continue
  if mode=='eof':open('side-effect','w').write('unknown effect');sys.exit(0)
  if mode=='malformed':print('invalid JSON',flush=True);continue
  if mode=='duplicate':print('{"method":"turn/completed","params":{"threadId":"thread-1","threadId":"forged","turn":{"id":"turn-1","status":"completed","items":[]}}}',flush=True);continue
  if mode=='oversized':sys.stdout.write('x'*2097152);sys.stdout.flush();continue
  if mode=='ack_only':continue
  if mode=='stderr':
   import fcntl
   if hasattr(fcntl,'F_SETPIPE_SZ'):fcntl.fcntl(2,fcntl.F_SETPIPE_SZ,4096)
   for _ in range(1024):os.write(2,b'x'*4096)
  if mode in ('permission','permission_flood'):
   if mode=='permission_flood':
    for n in range(400):send({'method':'vendor/future','params':{'threadId':'thread-1','turnId':'turn-1','n':n}})
   for method in ('item/commandExecution/requestApproval','item/fileChange/requestApproval','item/permissions/requestApproval'):
    send({'id':'native-request','method':method,'params':{'threadId':'thread-1','turnId':'turn-1','itemId':'item-1','cwd':cwd,'startedAtMs':1,'permissions':{'network':{'enabled':True}}}})
    reply=json.loads(sys.stdin.readline());assert 'jsonrpc' not in reply
    if method=='item/permissions/requestApproval':assert reply['result']=={'permissions':{},'scope':'turn','strictAutoReview':True}
    else:assert reply['result']=={'decision':'decline'}
   send({'id':'native-auth','method':'account/chatgptAuthTokens/refresh','params':{}})
   assert json.loads(sys.stdin.readline())['error']['code']==-32601
  if mode=='wrong_approval':
   send({'id':999,'method':'item/fileChange/requestApproval','params':{'threadId':'other-thread','turnId':'turn-1','itemId':'item-1','startedAtMs':1}});continue
  if mode in ('flood','permission_flood'):
   for i in range(400):send({'method':'vendor/future','params':{'threadId':'thread-1','turnId':'turn-1','value':'x'*5000,'n':i}})
  delta(os.environ.get('FIXTURE_ACCOUNT','missing') if mode=='env' else '$(touch must-not-exist); 你好')
  complete('interrupted' if mode=='interrupted' else 'failed' if mode=='failed' else 'completed')
  if mode=='late_metadata':send({'method':'thread/tokenUsage/updated','params':{'threadId':'thread-1','turnId':'turn-1','tokenUsage':{}}})
 elif m=='turn/interrupt':
  assert r['params']=={'threadId':'thread-1','turnId':'turn-'+str(turn_count)}
  cancel_count+=1;open('cancel-count','w').write(str(cancel_count))
  if mode=='late_interrupt_ack':complete('interrupted');time.sleep(.03);result(r,{});continue
  result(r,{})
  if mode!='interrupt_ack_only':complete('completed' if mode=='cancel_race' else 'interrupted')
"#;
fn installation(mode: &str) -> (tempfile::TempDir, ApprovedInstallation) {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("native-fixture.py");
    std::fs::write(&script, FIXTURE).unwrap();
    let python = Command::new("python3")
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let executable = std::fs::canonicalize(PathBuf::from(
        String::from_utf8(python.stdout).unwrap().trim(),
    ))
    .unwrap();
    let installation = ApprovedInstallation {
        expected_sha256: agentbaton_providers::installation::executable_digest(&executable)
            .unwrap(),
        executable,
        arguments: vec!["-u".into(), script.into(), mode.into()],
        workspace: dir.path().canonicalize().unwrap(),
        approval_reference: "generated-fixture-owner-grant".into(),
        environment: BTreeMap::new(),
    };
    (dir, installation)
}
fn start(mode: &str) -> (tempfile::TempDir, ManagedProviderProcess) {
    let preparing = Instant::now();
    let (d, i) = installation(mode);
    let preparation_elapsed = preparing.elapsed();
    let starting = Instant::now();
    let process =
        ManagedProviderProcess::start(ProtocolRoute::CodexAppServer, i, Duration::from_millis(500))
            .unwrap_or_else(|failure| {
                let start_elapsed = starting.elapsed();
                panic!(
                    "native fixture startup failed: {}",
                    startup_failure_diagnostic(
                        mode,
                        d.path(),
                        preparation_elapsed,
                        start_elapsed,
                        failure,
                    )
                )
            });
    (d, process)
}

// Inspect only after a failed start, after the owned child has been dropped.
// Never forward wire values, paths, stderr, argv or environment values. Existing
// fixture records show receipt/recording, not whether stdout was flushed/read.
fn startup_failure_diagnostic(
    mode: &str,
    directory: &Path,
    preparation_elapsed: Duration,
    start_elapsed: Duration,
    failure: agentbaton_providers::transport::TransportFailure,
) -> serde_json::Value {
    const SCAN_LIMIT: u64 = 4096;
    let wire = directory.join("wire.ndjson");
    let wire_bytes = std::fs::metadata(&wire).ok().map(|m| m.len());
    let mut bytes = Vec::new();
    let wire_read_ok = std::fs::File::open(&wire)
        .and_then(|file| file.take(SCAN_LIMIT).read_to_end(&mut bytes))
        .is_ok();
    let mut initialize_received = 0;
    let mut initialize_response_recorded = 0;
    let mut initialized_received = 0;
    let mut invalid_records = 0;
    for record in bytes.split_inclusive(|b| *b == b'\n') {
        if !record.ends_with(b"\n") {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(record) else {
            invalid_records += 1;
            continue;
        };
        let frame = &value["frame"];
        match value["direction"].as_str() {
            Some("client") if frame["method"] == "initialize" => initialize_received += 1,
            Some("provider") if frame["id"] == 1 && frame.get("result").is_some() => {
                initialize_response_recorded += 1;
            }
            Some("client") if frame["method"] == "initialized" => initialized_received += 1,
            _ => {}
        }
    }
    let fixture_pid_recorded = directory.join("fixture.pid").is_file();
    let last_observed_stage = if initialized_received > 0 {
        "initialized_received"
    } else if initialize_response_recorded > 0 {
        "initialize_response_recorded"
    } else if initialize_received > 0 {
        "initialize_received"
    } else if fixture_pid_recorded {
        "fixture_pid_recorded"
    } else {
        "no_fixture_record_observed"
    };
    json!({
        "mode": mode,
        "timeout_ms": 500,
        // Start includes digest verification/spawn; initialize's deadline begins
        // later inside exchange, so this is not initialize-only elapsed time.
        "preparation_elapsed_us": preparation_elapsed.as_micros(),
        "start_elapsed_us": start_elapsed.as_micros(),
        "kind": failure.kind,
        "side_effect_state": failure.side_effect_state,
        "protocol_error": failure.protocol_error,
        "last_observed_stage": last_observed_stage,
        "fixture_pid_recorded": fixture_pid_recorded,
        "wire_bytes": wire_bytes,
        "wire_scanned_bytes": bytes.len(),
        "wire_read_ok": wire_read_ok,
        "wire_scan_truncated": wire_bytes.is_some_and(|n| n > SCAN_LIMIT),
        "invalid_complete_records": invalid_records,
        "initialize_received": initialize_received,
        "initialize_response_recorded": initialize_response_recorded,
        "initialized_received": initialized_received,
    })
}
fn reaped(pid: u32) {
    assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}
#[test]
fn native_startup_failure_diagnostics_are_bounded_and_raw_free() {
    let (dir, installation) = installation("normal");
    // A generated local process that records entry but cannot initialize before
    // the original deadline. No vendor program, environment or identity is used.
    std::fs::write(
        dir.path().join("native-fixture.py"),
        "import os,time\nwith open('fixture.pid','w') as f:f.write(str(os.getpid()))\ntime.sleep(60)\n",
    )
    .unwrap();
    let starting = Instant::now();
    let failure = match ManagedProviderProcess::start(
        ProtocolRoute::CodexAppServer,
        installation,
        Duration::from_millis(500),
    ) {
        Ok(_) => panic!("accepted fixture without initialize response"),
        Err(failure) => failure,
    };
    let start_elapsed = starting.elapsed();
    assert_eq!(failure.kind, FailureKind::Timeout);
    assert_eq!(failure.side_effect_state, SideEffectState::Unknown);
    // A loaded runner may not schedule Python before the deadline. That is
    // itself useful startup evidence; do not add a new availability assumption.
    let fixture_pid = std::fs::read_to_string(dir.path().join("fixture.pid")).ok();
    if let Some(pid) = &fixture_pid {
        reaped(pid.parse().unwrap());
    }
    let diagnostic =
        startup_failure_diagnostic("normal", dir.path(), Duration::ZERO, start_elapsed, failure);
    assert_eq!(
        diagnostic["last_observed_stage"],
        if fixture_pid.is_some() {
            "fixture_pid_recorded"
        } else {
            "no_fixture_record_observed"
        }
    );
    assert_eq!(diagnostic["initialize_received"], 0);
    eprintln!("expected generated delayed-start failure: {diagnostic}");

    // Malicious/raw values in otherwise recognizable records must not escape;
    // scanning remains bounded even with a giant or incomplete trailing frame.
    let mut wire = String::from(
        "{\"direction\":\"client\",\"frame\":{\"method\":\"initialize\",\"params\":\"secret fixture credential\"}}\n\
         {\"direction\":\"provider\",\"frame\":{\"id\":1,\"result\":{\"codexHome\":\"secret fixture credential\"}}}\n\
         secret fixture credential\n",
    );
    wire.push_str(&"secret fixture credential".repeat(1024));
    std::fs::write(dir.path().join("wire.ndjson"), wire).unwrap();
    let diagnostic =
        startup_failure_diagnostic("normal", dir.path(), Duration::ZERO, start_elapsed, failure);
    assert_eq!(
        diagnostic["last_observed_stage"],
        "initialize_response_recorded"
    );
    assert_eq!(diagnostic["initialize_received"], 1);
    assert_eq!(diagnostic["initialize_response_recorded"], 1);
    assert_eq!(diagnostic["invalid_complete_records"], 1);
    assert_eq!(diagnostic["wire_scanned_bytes"], 4096);
    assert_eq!(diagnostic["wire_scan_truncated"], true);
    let rendered = diagnostic.to_string();
    assert!(!rendered.contains("secret fixture"));
    assert!(!rendered.contains(dir.path().to_str().unwrap()));
}
#[test]
fn native_roundtrip_has_distinct_envelope_bound_thread_and_no_shell() {
    let (dir, mut p) = start("normal");
    let t = p.new_session("server-owned-attempt").unwrap();
    p.begin_prompt(&t, "real fixture prompt").unwrap();
    assert_eq!(p.wait_prompt(&t).unwrap(), TurnState::Completed);
    let updates = p.take_updates();
    assert!(updates.iter().any(|e| e.local_binding=="server-owned-attempt" && matches!(&e.kind,UpdateKind::AgentText{text} if text=="$(touch must-not-exist); 你好")));
    assert!(!dir.path().join("must-not-exist").exists());
    if let Some(path) = std::env::var_os("AGENTBATON_TEST_SCHEMA_CAPTURE") {
        std::fs::copy(dir.path().join("wire.ndjson"), path).unwrap();
    }
    let pid = p.process_id();
    p.terminate();
    assert!(p.diagnostics().process_reaped);
    reaped(pid);
}
#[test]
fn submission_ack_even_completed_status_is_not_terminal_notification() {
    let (_dir, mut p) = start("ack_only");
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "work").unwrap();
    assert!(p
        .poll_prompt(&t, Duration::from_millis(25))
        .unwrap()
        .is_none());
    assert_eq!(p.turn_state(&t).unwrap(), &TurnState::Running);
    let error = p.wait_prompt(&t).unwrap_err();
    assert_eq!(error.kind, FailureKind::Timeout);
    assert_eq!(error.side_effect_state, SideEffectState::Unknown);
    assert!(p.diagnostics().process_reaped);
}
#[test]
fn interrupt_ack_is_not_stop_and_cancel_is_idempotent_before_native_turn_id() {
    let (dir, mut p) = start("interrupt_ack_only");
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "work").unwrap();
    p.cancel(&t).unwrap();
    p.cancel(&t).unwrap();
    assert!(p
        .poll_prompt(&t, Duration::from_millis(40))
        .unwrap()
        .is_none());
    assert_eq!(p.turn_state(&t).unwrap(), &TurnState::CancelRequested);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("cancel-count")).unwrap(),
        "1"
    );
    assert_eq!(p.wait_prompt(&t).unwrap_err().kind, FailureKind::Timeout);
    assert_eq!(p.turn_state(&t).unwrap(), &TurnState::OutcomeUnknown);
}
#[test]
fn native_cancel_confirmed_stop_and_completion_race_remain_distinct() {
    for (mode, state) in [
        ("cancel", TurnState::Cancelled),
        ("cancel_race", TurnState::Completed),
    ] {
        let (dir, mut p) = start(mode);
        let t = p.new_session("binding").unwrap();
        p.begin_prompt(&t, "work").unwrap();
        p.cancel(&t).unwrap();
        p.cancel(&t).unwrap();
        assert_eq!(p.wait_prompt(&t).unwrap(), state);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("cancel-count")).unwrap(),
            "1"
        );
    }
}
#[test]
fn native_completion_can_precede_submission_response_but_ids_must_match() {
    let (_dir, mut p) = start("early_complete");
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "work").unwrap();
    assert_eq!(p.wait_prompt(&t).unwrap(), TurnState::Completed);
    let (_dir, mut p) = start("wrong_turn");
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "work").unwrap();
    assert!(p.wait_prompt(&t).is_err());
    assert!(p.diagnostics().process_reaped);
}
#[test]
fn native_permissions_are_denied_and_unknown_auth_callback_never_reads_identity() {
    let (_dir, mut p) = start("permission");
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "work").unwrap();
    assert_eq!(p.wait_prompt(&t).unwrap(), TurnState::Completed);
    assert_eq!(
        p.take_updates()
            .iter()
            .filter(|e| matches!(e.kind, UpdateKind::PermissionDenied { .. }))
            .count(),
        3
    );
    let (_dir, mut p) = start("wrong_approval");
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "work").unwrap();
    assert!(p.wait_prompt(&t).is_err());
    assert!(p.diagnostics().process_reaped);
}
#[test]
fn native_faults_reap_child_without_replay_or_secret_error_text() {
    for mode in ["malformed", "duplicate", "oversized", "error", "eof"] {
        let (dir, mut p) = start(mode);
        let t = p.new_session("binding").unwrap();
        p.begin_prompt(&t, "work").unwrap();
        let error = p.wait_prompt(&t).unwrap_err();
        assert_eq!(error.side_effect_state, SideEffectState::Unknown);
        assert!(!format!("{error:?}").contains("secret fixture"));
        assert!(p.diagnostics().process_reaped);
        reaped(p.process_id());
        assert!(p.begin_prompt(&t, "never replay").is_err());
        if mode == "eof" {
            assert!(dir.path().join("side-effect").exists());
        }
    }
}
#[test]
fn native_small_pipe_stderr_preserves_original_four_mib_five_hundred_ms_contract() {
    let (_dir, mut p) = start("stderr");
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "work").unwrap();
    let now = Instant::now();
    let state = p.wait_prompt(&t).unwrap_or_else(|e| {
        panic!(
            "native flood {e:?} {:?} {:?}",
            now.elapsed(),
            p.diagnostics()
        )
    });
    assert_eq!(state, TurnState::Completed);
    assert!(p.diagnostics().stderr_bytes_observed >= 4 * 1024 * 1024);
    assert!(p.diagnostics().stderr_discarded);
}
#[test]
fn native_event_backpressure_is_visible_without_losing_completion() {
    let (_dir, i) = installation("flood");
    let mut p =
        ManagedProviderProcess::start(ProtocolRoute::CodexAppServer, i, Duration::from_secs(3))
            .unwrap();
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "work").unwrap();
    assert_eq!(p.wait_prompt(&t).unwrap(), TurnState::Completed);
    assert!(p.diagnostics().update_records_dropped > 0);
    assert!(p.diagnostics().evidence_records_dropped > 0);
    assert!(p.diagnostics().update_bytes <= MAX_FRAME_BYTES);
    assert!(p.diagnostics().evidence_bytes <= MAX_FRAME_BYTES);
}
#[test]
fn native_interrupt_and_failed_status_do_not_accept_task_or_invent_cancellation() {
    for (mode, state) in [
        ("interrupted", TurnState::Interrupted),
        ("failed", TurnState::OutcomeUnknown),
    ] {
        let (_d, mut p) = start(mode);
        let t = p.new_session("binding").unwrap();
        p.begin_prompt(&t, "work").unwrap();
        assert_eq!(p.wait_prompt(&t).unwrap(), state);
    }
}
#[test]
fn native_quantum_does_not_extend_turn_deadline_and_blocked_outbound_stays_responsive() {
    for mode in ["hang", "blocked_input"] {
        let (_d, mut p) = start(mode);
        let t = p.new_session("binding").unwrap();
        p.set_io_timeout(Duration::from_millis(150)).unwrap();
        let now = Instant::now();
        p.begin_prompt(&t, &"x".repeat(200 * 1024)).unwrap();
        assert!(now.elapsed() < Duration::from_millis(100));
        loop {
            match p.poll_prompt(&t, Duration::from_millis(5)) {
                Ok(None) => continue,
                Err(e) => {
                    assert_eq!(e.kind, FailureKind::Timeout);
                    break;
                }
                Ok(Some(s)) => panic!("invented stop {s:?}"),
            }
        }
        assert!(now.elapsed() < Duration::from_secs(1));
        assert!(p.diagnostics().process_reaped);
    }
}
#[test]
fn native_wrong_dialect_and_expanded_native_scope_fail_closed() {
    let (_d, i) = installation("acp_init");
    assert!(ManagedProviderProcess::start(
        ProtocolRoute::CodexAppServer,
        i,
        Duration::from_millis(500)
    )
    .is_err());
    let (_d, mut p) = start("expanded_scope");
    assert_eq!(
        p.configure_session_configuration(&json!({"mcpServers":[]}))
            .unwrap_err()
            .protocol_error,
        Some(agentbaton_providers::ProtocolError::CapabilityUnsupported)
    );
    assert!(p.new_session("binding").is_err());
    assert!(p.diagnostics().process_reaped);
    // Native initialize cannot be silently interpreted as an ACP server either.
    let (_d, i) = installation("normal");
    assert!(
        ManagedProviderProcess::start(ProtocolRoute::CustomAcp, i, Duration::from_millis(500))
            .is_err()
    );
}
#[test]
fn native_envelope_rejects_ambiguous_shapes_and_preserves_extensions() {
    use agentbaton_providers::codex::validate_native_envelope;
    assert!(validate_native_envelope(
        &json!({"id":1,"result":{},"vendorExtension":{"opaque":true}})
    )
    .is_ok());
    for value in [
        json!({"id":true,"result":{}}),
        json!({"id":1,"result":{},"error":{"code":1,"message":"no"}}),
        json!({"method":"x","params":false}),
        json!({"jsonrpc":"1.0","method":"x"}),
    ] {
        assert!(validate_native_envelope(&value).is_err());
    }
}

#[test]
fn native_stale_terminal_cannot_complete_a_new_submission() {
    let (_d, mut p) = start("stale_terminal");
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "first").unwrap();
    assert_eq!(p.wait_prompt(&t).unwrap(), TurnState::Completed);
    p.begin_prompt(&t, "second").unwrap();
    let error = p.wait_prompt(&t).unwrap_err();
    assert_eq!(
        error.protocol_error,
        Some(agentbaton_providers::ProtocolError::UnmatchedResponse)
    );
    assert!(p.diagnostics().process_reaped);
}
#[test]
fn native_malformed_or_expanded_session_response_reaps_owned_child() {
    for mode in ["malformed_session", "invalid_scope", "extra_root"] {
        let (_d, mut p) = start(mode);
        assert!(p.new_session("binding").is_err());
        assert!(p.diagnostics().process_reaped);
        reaped(p.process_id());
    }
}
#[test]
fn native_home_mismatch_is_not_silently_treated_as_selected_namespace() {
    let (dir, mut installation) = installation("wrong_home");
    installation.environment.insert(
        "CODEX_HOME".into(),
        dir.path().join("private-native-home").into(),
    );
    let error = match ManagedProviderProcess::start(
        ProtocolRoute::CodexAppServer,
        installation,
        Duration::from_millis(500),
    ) {
        Ok(_) => panic!("accepted wrong namespace"),
        Err(e) => e,
    };
    assert_eq!(error.side_effect_state, SideEffectState::Unknown);
    let pid = std::fs::read_to_string(dir.path().join("fixture.pid"))
        .unwrap()
        .parse()
        .unwrap();
    reaped(pid);
}

#[test]
fn late_native_metadata_and_interrupt_ack_do_not_break_next_turn_correlation() {
    let (_d, mut p) = start("late_metadata");
    let t = p.new_session("binding").unwrap();
    for input in ["first", "second"] {
        p.begin_prompt(&t, input).unwrap();
        assert_eq!(p.wait_prompt(&t).unwrap(), TurnState::Completed);
    }
    let (_d, mut p) = start("late_interrupt_ack");
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "first").unwrap();
    p.cancel(&t).unwrap();
    assert_eq!(p.wait_prompt(&t).unwrap(), TurnState::Cancelled);
    p.begin_prompt(&t, "second").unwrap();
    assert_eq!(p.wait_prompt(&t).unwrap(), TurnState::Completed);
}
#[test]
fn independent_native_process_environments_do_not_share_fixture_identity_markers() {
    let mut processes = Vec::new();
    let mut dirs = Vec::new();
    for marker in ["account-a", "account-b"] {
        let (dir, mut i) = installation("env");
        let home = dir.path().join("native-home");
        std::fs::create_dir(&home).unwrap();
        i.environment.insert("CODEX_HOME".into(), home.into());
        i.environment
            .insert("FIXTURE_ACCOUNT".into(), marker.into());
        let mut p = ManagedProviderProcess::start(
            ProtocolRoute::CodexAppServer,
            i,
            Duration::from_millis(500),
        )
        .unwrap();
        let t = p.new_session(marker).unwrap();
        processes.push((p, t, marker));
        dirs.push(dir);
    }
    // Start both turns only after both binary verification/handshakes finish;
    // hashing a second interpreter is not part of the first turn's IO contract.
    for (p, t, _) in &mut processes {
        p.begin_prompt(t, "work").unwrap();
    }
    for (p, t, marker) in &mut processes {
        assert_eq!(p.wait_prompt(t).unwrap(), TurnState::Completed);
        assert!(p.take_updates().iter().any(|e| e.local_binding == *marker
            && matches!(&e.kind,UpdateKind::AgentText{text} if text==*marker)));
    }
}

#[test]
fn native_control_backpressure_fails_closed_instead_of_losing_denial_evidence() {
    let (_d, i) = installation("permission_flood");
    let mut p =
        ManagedProviderProcess::start(ProtocolRoute::CodexAppServer, i, Duration::from_secs(3))
            .unwrap();
    let t = p.new_session("binding").unwrap();
    p.begin_prompt(&t, "work").unwrap();
    let error = p.wait_prompt(&t).unwrap_err();
    assert_eq!(
        error.protocol_error,
        Some(agentbaton_providers::ProtocolError::ResourceLimit)
    );
    assert!(p.diagnostics().process_reaped);
    assert!(p.diagnostics().update_records_dropped > 0);
}
