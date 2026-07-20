use systemprompt_bridge::install::mdm_snippet;
use systemprompt_bridge::schedule::Os;

#[test]
fn windows_snippet_pins_cowork_egress_and_local_dev_mcp() {
    let text = mdm_snippet(Os::Windows, Some("https://gateway.example"));
    assert!(
        text.contains(r#""isLocalDevMcpEnabled"="false""#),
        "windows snippet must disable local dev MCP: {text}"
    );
    assert!(
        text.contains(r#""coworkEgressAllowedHosts"="[\"127.0.0.1\"]""#),
        "windows snippet must pin Cowork egress to loopback: {text}"
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
        text.contains("CLAUDE_INFERENCE_GATEWAY_BASE_URL=https://gateway.example"),
        "linux snippet must interpolate the gateway URL: {text}"
    );
    assert!(
        !text.contains("allowedWorkspaceFolders"),
        "linux has no MDM policy surface for workspace folders: {text}"
    );
}
