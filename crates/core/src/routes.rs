//! Explicit protocol routes. Provider selection never tries a second engine,
//! login mechanism, endpoint or billing mode when the selected route fails.
use crate::{CoreError, Result};
use agentbaton_providers::connections::ProtocolRoute;

pub(crate) fn route_for_provider(provider: &str) -> Result<ProtocolRoute> {
    match provider {
        "custom_acp" => Ok(ProtocolRoute::CustomAcp),
        "codex" => Ok(ProtocolRoute::CodexAppServer),
        "qoder" => Ok(ProtocolRoute::QoderNativeAcp),
        "claude_code" => Ok(ProtocolRoute::ClaudeAgentAcp),
        _ => Err(CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "Select an explicit supported Provider route; automatic fallback is disabled",
        )),
    }
}

pub(crate) fn default_route() -> ProtocolRoute {
    ProtocolRoute::CustomAcp
}

pub(crate) fn validate_route_arguments(
    provider: &str,
    arguments: &[String],
) -> Result<ProtocolRoute> {
    let route = route_for_provider(provider)?;
    if route != ProtocolRoute::CustomAcp && !arguments.is_empty() {
        return Err(CoreError::new(
            "INVALID_REQUEST",
            "Native route arguments are generated from the approved configuration; arbitrary native CLI flags are not supported",
        ));
    }
    Ok(route)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_are_distinct_and_never_guess_a_fallback() {
        assert_eq!(
            route_for_provider("codex").unwrap(),
            ProtocolRoute::CodexAppServer
        );
        assert_eq!(
            route_for_provider("qoder").unwrap(),
            ProtocolRoute::QoderNativeAcp
        );
        assert_eq!(
            route_for_provider("claude_code").unwrap(),
            ProtocolRoute::ClaudeAgentAcp
        );
        assert_eq!(
            route_for_provider("custom_acp").unwrap(),
            ProtocolRoute::CustomAcp
        );
        for unsupported in ["claude --acp", "auto", "codex_acp", "", "CODEX"] {
            assert_eq!(
                route_for_provider(unsupported).unwrap_err().code,
                "CAPABILITY_UNSUPPORTED"
            );
        }
    }

    #[test]
    fn native_cli_flags_cannot_widen_the_selected_configuration() {
        let arguments = vec!["--dangerously-bypass-approvals-and-sandbox".into()];
        for provider in ["codex", "qoder", "claude_code"] {
            assert_eq!(
                validate_route_arguments(provider, &arguments)
                    .unwrap_err()
                    .code,
                "INVALID_REQUEST"
            );
            assert!(validate_route_arguments(provider, &[]).is_ok());
        }
        // A custom executable's literal argv is separately displayed in the
        // native-owner proposal, not interpreted as a native engine's flags.
        assert_eq!(
            validate_route_arguments("custom_acp", &["--fixture-mode".into()]).unwrap(),
            ProtocolRoute::CustomAcp
        );
    }
}
