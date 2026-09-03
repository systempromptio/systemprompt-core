use systemprompt_bridge::install::mdm_snippet;
use systemprompt_bridge::schedule::Os;

#[test]
fn windows_snippet_disables_local_dev_mcp() {
    let text = mdm_snippet(Os::Windows, Some("https://gateway.example"));
    assert!(
        text.contains(r#""isLocalDevMcpEnabled"="false""#),
        "windows snippet must disable local dev MCP: {text}"
    );
}

/// Egress is unrestricted by default: pinning the allowlist to loopback left
/// agents with no internet access at all, so the key is now an opt-in that the
/// snippet only shows commented out.
#[test]
fn windows_snippet_leaves_cowork_egress_unrestricted() {
    let text = mdm_snippet(Os::Windows, Some("https://gateway.example"));
    for line in text.lines() {
        assert!(
            !line.starts_with(r#""coworkEgressAllowedHosts""#),
            "windows snippet must not set an active egress allowlist: {line}"
        );
    }
    assert!(
        text.contains(r#"; "coworkEgressAllowedHosts"="[\"127.0.0.1\"]""#),
        "the loopback lockdown must stay documented as a commented-out opt-in: {text}"
    );
}

#[test]
fn windows_snippet_embeds_brand_default_workspace_folder() {
    let text = mdm_snippet(Os::Windows, Some("https://gateway.example"));
    assert!(
        text.contains(r#""path":\"~/Systemprompt\""#) || text.contains("~/Systemprompt"),
        "windows snippet must pre-trust the brand default workspace folder: {text}"
    );
    assert!(
        !text.contains("{workspace}"),
        "the {{workspace}} placeholder must be substituted, not left literal: {text}"
    );
}

#[test]
fn linux_snippet_uses_env_vars_and_no_workspace_placeholder() {
    let text = mdm_snippet(Os::Linux, Some("https://gateway.example"));
    assert!(
        text.contains("ANTHROPIC_BASE_URL=https://gateway.example"),
        "linux snippet must interpolate the gateway URL: {text}"
    );
    assert!(
        !text.contains("CLAUDE_INFERENCE_GATEWAY"),
        "only the variable pair proven end-to-end is advertised; the gateway-prefixed keys \
         were dropped because nothing consumed them: {text}"
    );
    assert!(
        !text.contains("allowedWorkspaceFolders"),
        "linux has no MDM policy surface for workspace folders: {text}"
    );
}

// Why: Cowork refuses to start any task when `inferenceProvider=gateway` is
// present without a base URL and credential, which is what an earlier sync
// wrote on a fresh policy key. The gateway block must be one unit and the
// snippet must never seed the provider on its own.
#[test]
fn windows_snippet_never_seeds_a_gateway_provider_without_its_url() {
    let text = mdm_snippet(Os::Windows, Some("https://gateway.example"));
    for line in text.lines() {
        assert!(
            !line.starts_with(r#""inferenceProvider""#)
                && !line.starts_with(r#""inferenceGatewayAuthScheme""#),
            "the snippet must not pin half of the gateway block: {line}"
        );
    }
}

#[test]
fn inference_policy_values_is_a_complete_gateway_block() {
    let values = systemprompt_bridge::install::inference_policy_values(
        "http://127.0.0.1:5010",
        "secret-123",
        None,
    );
    let get = |k: &str| {
        values
            .iter()
            .find(|(n, _, _)| *n == k)
            .map(|(_, _, v)| v.as_str())
    };
    assert_eq!(get("inferenceProvider"), Some("gateway"));
    assert_eq!(
        get("inferenceGatewayBaseUrl"),
        Some("http://127.0.0.1:5010")
    );
    assert_eq!(get("inferenceGatewayApiKey"), Some("secret-123"));
    assert_eq!(get("inferenceGatewayAuthScheme"), Some("bearer"));
    let models: Vec<String> =
        serde_json::from_str(get("inferenceModels").expect("models present")).unwrap();
    assert!(!models.is_empty(), "default model list must not be empty");
    assert!(values.iter().all(|(_, kind, _)| *kind == "REG_SZ"));
}

#[test]
fn inference_policy_values_keeps_the_installed_model_list() {
    let installed = r#"["claude-opus-5"]"#.to_owned();
    let values = systemprompt_bridge::install::inference_policy_values(
        "http://127.0.0.1:5010",
        "s",
        Some(installed.clone()),
    );
    let models = values
        .iter()
        .find(|(n, _, _)| *n == "inferenceModels")
        .map(|(_, _, v)| v.as_str());
    assert_eq!(models, Some(installed.as_str()));

    let blank = systemprompt_bridge::install::inference_policy_values(
        "http://127.0.0.1:5010",
        "s",
        Some("  ".to_owned()),
    );
    let models = blank
        .iter()
        .find(|(n, _, _)| *n == "inferenceModels")
        .map(|(_, _, v)| v.as_str());
    assert_ne!(
        models,
        Some("  "),
        "a blank installed list falls back to the default"
    );
}
