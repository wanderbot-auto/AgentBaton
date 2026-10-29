//! Deliberately narrow server-side ACP configuration projection.
//! Data shape is not authority: the caller must resolve the approved runtime snapshot.
use crate::transport::TransportFailure;
use crate::ProtocolError;
use serde_json::{json, Value};
fn invalid() -> TransportFailure {
    TransportFailure::protocol(ProtocolError::InvalidParams)
}
fn unsupported() -> TransportFailure {
    TransportFailure::protocol(ProtocolError::CapabilityUnsupported)
}
fn exact_keys(value: &Value, keys: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == keys.len() && keys.iter().all(|k| o.contains_key(*k)))
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && !name.starts_with(".baton-")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
}
pub(crate) fn default_configuration(claude: bool) -> Value {
    let mut config = json!({"mcpServers":[]});
    if claude {
        config["_meta"] = json!({"claudeCode":{"options":{
         "settingSources":[],"settings":{"enableAllProjectMcpServers":false,"enabledMcpjsonServers":[],"disabledMcpjsonServers":[]},
         "allowDangerouslySkipPermissions":false,"strictMcpConfig":true
        }}});
    }
    config
}
pub(crate) fn validate(value: &Value, claude: bool) -> Result<Value, TransportFailure> {
    if serde_json::to_vec(value).map_err(|_| invalid())?.len() > 64 * 1024 {
        return Err(invalid());
    }
    if !exact_keys(
        value,
        if claude {
            &["mcpServers", "_meta"]
        } else {
            &["mcpServers"]
        },
    ) {
        return Err(invalid());
    }
    let servers = value["mcpServers"].as_array().ok_or_else(invalid)?;
    // MCP connection/invocation is not granted by a config record alone. Until
    // scoped server execution/recipient checks exist, reject rather than ignore.
    if !servers.is_empty() {
        return Err(unsupported());
    }
    if claude {
        let meta = &value["_meta"];
        if !exact_keys(meta, &["claudeCode"]) || !exact_keys(&meta["claudeCode"], &["options"]) {
            return Err(invalid());
        }
        let options = &meta["claudeCode"]["options"];
        if !exact_keys(
            options,
            &[
                "settingSources",
                "settings",
                "allowDangerouslySkipPermissions",
                "strictMcpConfig",
            ],
        ) || options["settingSources"] != json!([])
            || options["allowDangerouslySkipPermissions"] != false
            || options["strictMcpConfig"] != true
        {
            return Err(invalid());
        }
        let settings = &options["settings"];
        if !exact_keys(
            settings,
            &[
                "enableAllProjectMcpServers",
                "enabledMcpjsonServers",
                "disabledMcpjsonServers",
            ],
        ) || settings["enableAllProjectMcpServers"] != false
        {
            return Err(invalid());
        }
        let enabled = settings["enabledMcpjsonServers"]
            .as_array()
            .ok_or_else(invalid)?;
        if !enabled.is_empty() {
            return Err(unsupported());
        }
        let disabled = settings["disabledMcpjsonServers"]
            .as_array()
            .ok_or_else(invalid)?;
        if disabled.len() > 128
            || disabled
                .iter()
                .any(|v| v.as_str().is_none_or(|s| !valid_name(s)))
            || disabled
                .iter()
                .map(Value::as_str)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != disabled.len()
        {
            return Err(invalid());
        }
    }
    Ok(value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn claude_projection_cannot_enable_dangerous_or_ambient_options() {
        let baseline = default_configuration(true);
        for (pointer, value) in [
            ("/settingSources", json!(["user", "project"])),
            ("/allowDangerouslySkipPermissions", json!(true)),
            ("/strictMcpConfig", json!(false)),
            ("/settings/enableAllProjectMcpServers", json!(true)),
            ("/env", json!({"TOKEN":"must-never-be-forwarded"})),
            ("/settings/permissions", json!({"allow":["Bash(*)"]})),
        ] {
            let mut config = baseline.clone();
            let options = &mut config["_meta"]["claudeCode"]["options"];
            if let Some((parent, key)) = pointer[1..].rsplit_once('/') {
                options[parent][key] = value;
            } else {
                options[&pointer[1..]] = value;
            }
            assert!(
                validate(&config, true).is_err(),
                "unsafe projection accepted"
            );
        }
        let mut config = baseline;
        config["_meta"]["vendor"] = json!({"execute":"forbidden"});
        assert!(validate(&config, true).is_err());
    }
    #[test]
    fn mcp_connections_and_enabling_project_servers_remain_explicitly_unsupported() {
        let config = json!({"mcpServers":[{"name":"server","command":"/unapproved/executable","args":[],"env":[]}]});
        assert_eq!(
            validate(&config, false).unwrap_err().protocol_error,
            Some(ProtocolError::CapabilityUnsupported)
        );
        let mut config = default_configuration(true);
        config["_meta"]["claudeCode"]["options"]["settings"]["enabledMcpjsonServers"] =
            json!(["project-server"]);
        assert_eq!(
            validate(&config, true).unwrap_err().protocol_error,
            Some(ProtocolError::CapabilityUnsupported)
        );
        assert!(validate(&default_configuration(true), false).is_err());
    }
    #[test]
    fn restricted_disable_list_is_preserved_and_invalid_names_are_rejected() {
        let mut config = default_configuration(true);
        config["_meta"]["claudeCode"]["options"]["settings"]["disabledMcpjsonServers"] =
            json!(["disabled-project-server"]);
        assert_eq!(validate(&config, true).unwrap(), config);
        for names in [
            json!(["duplicate", "duplicate"]),
            json!(["../outside"]),
            json!([".baton-owned"]),
            json!([false]),
        ] {
            config["_meta"]["claudeCode"]["options"]["settings"]["disabledMcpjsonServers"] = names;
            assert!(validate(&config, true).is_err());
        }
    }
}

/// Only trusted host approval resolution may construct this value. No wire or
/// serde conversion grants a server the right to execute. The application must
/// bind the exact installation, Factory approval, session and credential file.
#[derive(Clone)]
pub struct ApprovedFactoryMcp {
    server: Value,
}
impl ApprovedFactoryMcp {
    pub fn from_verified_host(
        executable: &std::path::Path,
        expected: &crate::installation::BinaryDigest,
        credential_file: &std::path::Path,
        approval_reference: &str,
    ) -> Result<Self, TransportFailure> {
        use std::os::unix::fs::MetadataExt;
        if approval_reference.is_empty()
            || approval_reference.len() > 128
            || !executable.is_absolute()
            || !credential_file.is_absolute()
            || credential_file.as_os_str().len() > 4096
        {
            return Err(invalid());
        }
        crate::installation::verify_executable(executable, expected).map_err(|_| unsupported())?;
        let m = std::fs::symlink_metadata(credential_file).map_err(|_| invalid())?;
        let parent = credential_file.parent().ok_or_else(invalid)?;
        let p = std::fs::symlink_metadata(parent).map_err(|_| invalid())?;
        if !m.is_file()
            || m.nlink() != 1
            || m.mode() & 0o7777 != 0o600
            || m.uid() != unsafe { libc::geteuid() }
            || !p.is_dir()
            || p.mode() & 0o7777 != 0o700
            || p.uid() != m.uid()
            || std::fs::canonicalize(credential_file).ok().as_deref() != Some(credential_file)
        {
            return Err(invalid());
        }
        Ok(Self {
            server: json!({"name":"agentbaton-factory","command":executable,"args":["factory-mcp",credential_file],"env":[]}),
        })
    }
}
pub(crate) fn validate_factory(
    configuration: &Value,
    claude: bool,
    approved: &ApprovedFactoryMcp,
) -> Result<Value, TransportFailure> {
    if serde_json::to_vec(configuration)
        .map_err(|_| invalid())?
        .len()
        > 64 * 1024
    {
        return Err(invalid());
    }
    // Preserve all original native configuration restrictions. Only this fixed
    // host-resolved server is added after the baseline has validated.
    validate(configuration, claude)?;
    let mut projected = configuration.clone();
    projected["mcpServers"] = json!([approved.server]);
    Ok(projected)
}
