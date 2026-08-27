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
