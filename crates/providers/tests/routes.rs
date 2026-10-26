#![cfg(unix)]
//! Explicit route contracts use a generated ACP child, never a vendor executable.
use agentbaton_providers::connections::ProtocolRoute;
use agentbaton_providers::driver::ManagedProviderProcess;
use agentbaton_providers::transport::ApprovedInstallation;
use agentbaton_providers::TurnState;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
#[test]
fn named_acp_routes_negotiate_actual_child_and_claude_disables_ambient_setting_sources() {
    for route in [
        ProtocolRoute::CustomAcp,
        ProtocolRoute::QoderNativeAcp,
        ProtocolRoute::ClaudeAgentAcp,
        ProtocolRoute::CodexAcpAdapter,
    ] {
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("acp-route-fixture.py");
        std::fs::write(
            &script,
            r#"
import sys,json,os
claude=sys.argv[1]=='claude'
for line in sys.stdin:
 r=json.loads(line);assert r['jsonrpc']=='2.0';m=r['method']
 if m=='initialize':
  assert r['params']['protocolVersion']==1
  v={'protocolVersion':1,'agentCapabilities':{'loadSession':False}}
 elif m=='session/new':
  expected={'cwd':os.getcwd(),'mcpServers':[]}
  if claude:expected['_meta']={'claudeCode':{'options':{'settingSources':[],'settings':{'enableAllProjectMcpServers':False,'enabledMcpjsonServers':[],'disabledMcpjsonServers':['blocked-project-server']},'allowDangerouslySkipPermissions':False,'strictMcpConfig':True}}}
  assert r['params']==expected
  v={'sessionId':'selected-acp-route'}
 elif m=='session/prompt':v={'stopReason':'end_turn'}
 else:raise AssertionError(m)
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':v}),flush=True)
"#,
        )
        .unwrap();
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
            workspace: root.path().canonicalize().unwrap(),
            arguments: vec![
                "-u".into(),
                script.into(),
                if route == ProtocolRoute::ClaudeAgentAcp {
                    "claude".into()
                } else {
                    "generic".into()
                },
            ],
            approval_reference: "explicit-fixture-route".into(),
            environment: BTreeMap::new(),
        };
        let mut p =
            ManagedProviderProcess::start(route, installation, Duration::from_millis(500)).unwrap();
        let mut configuration = serde_json::json!({"mcpServers":[]});
        if route == ProtocolRoute::ClaudeAgentAcp {
            configuration["_meta"] = serde_json::json!({"claudeCode":{"options":{"settingSources":[],"settings":{"enableAllProjectMcpServers":false,"enabledMcpjsonServers":[],"disabledMcpjsonServers":["blocked-project-server"]},"allowDangerouslySkipPermissions":false,"strictMcpConfig":true}}});
        }
        p.configure_session_configuration(&configuration).unwrap();
        let mut malformed = configuration.clone();
        malformed["unexpected"] = serde_json::json!("cannot silently ignore");
        assert!(p.configure_session_configuration(&malformed).is_err());
        let native = p.new_session("fixed-local-binding").unwrap();
        assert!(p.configure_session_configuration(&configuration).is_err());
        assert_eq!(native, "selected-acp-route");
        p.begin_prompt(&native, "fixture").unwrap();
        assert_eq!(p.wait_prompt(&native).unwrap(), TurnState::Completed);
        let evidence = p.take_evidence();
        assert_eq!(
            evidence[0]["result"]["agentCapabilities"]["loadSession"],
            false
        );
    }
}
