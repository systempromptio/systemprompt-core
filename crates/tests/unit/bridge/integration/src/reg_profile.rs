use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::install::mdm::policy::{
    McpServerEntry, PolicyInputs, claude_desktop_policy, reg_values,
};
use systemprompt_bridge::install::reg_values::{parse_reg_entries, render_reg_values};
use systemprompt_bridge::integration::claude_desktop::reg_profile::{profile_entries, render_reg};
use systemprompt_bridge::integration::host_app::ProfileGenInputs;

const ORG_UUID: &str = "6f1d2c3a-4b5e-4f60-8a71-9b0c1d2e3f40";

fn inputs() -> ProfileGenInputs {
    ProfileGenInputs {
        gateway_base_url: "https://gateway.example.com".to_string(),
        host_token: HostToken::new("sp-secret-key"),
        models: vec!["claude-opus-4-7".to_string()],
        default_model: None,
        organization_uuid: Some(ORG_UUID.to_string()),
        headers: Default::default(),
        mcp_servers: Some(Vec::new()),
    }
}

fn value_of<'a>(entries: &'a [(String, String)], name: &str) -> &'a str {
    entries
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
        .unwrap_or_else(|| panic!("missing {name} in {entries:?}"))
}

#[test]
fn profile_entries_carry_required_policy_keys() {
    let entries = profile_entries(&inputs()).expect("profile renders");
    let owned: Vec<(String, String)> = entries
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    assert_eq!(value_of(&owned, "inferenceProvider"), "gateway");
    assert_eq!(value_of(&owned, "inferenceGatewayAuthScheme"), "bearer");
    assert_eq!(
        value_of(&owned, "inferenceGatewayBaseUrl"),
        "https://gateway.example.com"
    );
    assert_eq!(
        value_of(&owned, "inferenceGatewayApiKey"),
        "sp-secret-key",
        "the registry profile carries the host token it was handed"
    );
    assert_eq!(value_of(&owned, "inferenceModels"), "[\"claude-opus-4-7\"]");
}

#[test]
fn empty_models_falls_back_to_defaults() {
    let mut probe = inputs();
    probe.models = vec![];
    let entries: Vec<(String, String)> = profile_entries(&probe)
        .expect("profile renders")
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let parsed: Vec<String> = serde_json::from_str(value_of(&entries, "inferenceModels"))
        .expect("models is a json array");
    assert!(
        parsed.len() >= 2,
        "expected default model list, got {parsed:?}"
    );
    assert!(parsed.iter().any(|m| m == "claude-opus-5"));
}

#[test]
fn headers_emit_inference_custom_headers_key() {
    let mut probe = inputs();
    probe
        .headers
        .insert("x-inference-protocol".to_string(), "anthropic".to_string());
    let entries: Vec<(String, String)> = profile_entries(&probe)
        .expect("profile renders")
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let raw = value_of(&entries, "inferenceCustomHeaders");
    let parsed: std::collections::BTreeMap<String, String> =
        serde_json::from_str(raw).expect("headers is a json object");
    assert_eq!(
        parsed.get("x-inference-protocol").map(String::as_str),
        Some("anthropic")
    );
}

#[test]
fn no_headers_key_when_absent() {
    let entries = profile_entries(&inputs()).expect("profile renders");
    assert!(!entries.iter().any(|(k, _)| *k == "inferenceCustomHeaders"));
}

#[test]
fn render_targets_hkcu_unelevated_and_hklm_elevated() {
    assert!(
        render_reg(false, &inputs())
            .expect("profile renders")
            .contains(r"[HKEY_CURRENT_USER\SOFTWARE\Policies\Claude]")
    );
    assert!(
        render_reg(true, &inputs())
            .expect("profile renders")
            .contains(r"[HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Claude]")
    );
}

#[test]
fn hklm_profile_parses_to_the_whole_policy() {
    let rendered = render_reg(true, &inputs()).expect("profile renders");
    assert!(rendered.contains(r"[HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Claude]"));
    let parsed = parse_reg_entries(&rendered).expect("rendered profile parses");
    let names: Vec<&str> = parsed.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "inferenceProvider",
            "inferenceGatewayBaseUrl",
            "inferenceGatewayApiKey",
            "inferenceGatewayAuthScheme",
            "inferenceModels",
            "disableEssentialTelemetry",
            "disableNonessentialTelemetry",
            "disableNonessentialServices",
            "disableAutoUpdates",
            "disableDeploymentModeChooser",
            "isLocalDevMcpEnabled",
            "allowedWorkspaceFolders",
            "deploymentOrganizationUuid",
            "managedMcpServers",
        ]
    );
}

fn mcp_servers() -> Vec<McpServerEntry> {
    vec![
        McpServerEntry {
            name: "atlassian".to_string(),
            url: "http://127.0.0.1:48217/mcp/atlassian".to_string(),
            tool_policy: Default::default(),
        },
        McpServerEntry {
            name: "systemprompt".to_string(),
            url: "http://127.0.0.1:48217/mcp/systemprompt".to_string(),
            tool_policy: Default::default(),
        },
    ]
}

// The Repair button writes the profile and the startup sync enforces the
// policy; a key the sync writes that the profile omits reopens the
// repair → "already holds this policy" → sync-fails loop.
#[test]
fn profile_key_set_equals_the_enforced_policy_key_set() {
    let mut probe = inputs();
    probe.mcp_servers = Some(mcp_servers());
    probe
        .headers
        .insert("x-inference-protocol".to_string(), "anthropic".to_string());
    let profile: Vec<(&str, String)> = profile_entries(&probe).expect("profile renders");

    let policy = claude_desktop_policy(&PolicyInputs {
        base_url: &probe.gateway_base_url,
        host_token: &probe.host_token,
        models: Some(serde_json::to_string(&probe.models).expect("json")),
        headers: &probe.headers,
        egress_allowed_hosts: None,
        org_uuid: probe.organization_uuid.as_deref(),
        mcp_servers: probe.mcp_servers.as_deref(),
    })
    .expect("policy renders");
    let enforced: Vec<(&str, String)> = reg_values(&policy)
        .into_iter()
        .map(|(k, _, v)| (k, v))
        .collect();

    assert_eq!(profile, enforced);
}

#[test]
fn profile_carries_managed_mcp_servers_with_host_bearer() {
    let mut probe = inputs();
    probe.mcp_servers = Some(mcp_servers());
    let entries: Vec<(String, String)> = profile_entries(&probe)
        .expect("profile renders")
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let servers: Vec<serde_json::Value> =
        serde_json::from_str(value_of(&entries, "managedMcpServers"))
            .expect("managedMcpServers is a json array");
    let names: Vec<&str> = servers.iter().filter_map(|s| s["name"].as_str()).collect();
    assert_eq!(names, vec!["atlassian", "systemprompt"]);
    assert!(
        servers
            .iter()
            .all(|s| s["headers"]["Authorization"] == "Bearer sp-secret-key"),
        "every managed server carries the host token: {servers:?}"
    );
}

#[test]
fn unprojected_connectors_withhold_managed_mcp_servers() {
    let mut probe = inputs();
    probe.mcp_servers = None;
    let entries = profile_entries(&probe).expect("profile renders");
    assert!(!entries.iter().any(|(k, _)| *k == "managedMcpServers"));
}

#[test]
fn non_uuid_organization_is_refused_not_dropped() {
    let mut probe = inputs();
    probe.organization_uuid = Some("org-abc".to_string());
    let err = profile_entries(&probe).expect_err("a non-UUID organisation is an error");
    assert!(
        err.to_string().contains("deploymentOrganizationUuid"),
        "{err}"
    );
}

#[test]
fn rendered_profile_round_trips_through_parser() {
    let probe = inputs();
    let rendered = render_reg(false, &probe).expect("profile renders");
    assert!(rendered.starts_with("Windows Registry Editor Version 5.00"));

    let parsed = parse_reg_entries(&rendered).expect("rendered profile parses");
    let expected: Vec<(String, String)> = profile_entries(&probe)
        .expect("profile renders")
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    assert_eq!(parsed, expected);
}

#[test]
fn round_trip_preserves_backslashes_and_quotes() {
    let body = render_reg_values(
        false,
        &[(
            "customValue",
            r#"key-with-"quote"-and-\back\slash"#.to_string(),
        )],
    );
    let parsed = parse_reg_entries(&body).expect("rendered values parse");
    assert_eq!(
        value_of(&parsed, "customValue"),
        r#"key-with-"quote"-and-\back\slash"#
    );
}

#[test]
fn parser_ignores_header_and_section_lines() {
    let parsed = parse_reg_entries(
        "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\SOFTWARE\\Policies\\Claude]\r\n\"inferenceProvider\"=\"gateway\"\r\n",
    )
    .expect("header and section lines are skipped");
    assert_eq!(
        parsed,
        vec![("inferenceProvider".to_string(), "gateway".to_string())]
    );
}

#[test]
fn render_reg_values_round_trips_a_json_payload() {
    let payload = r#"[{"name":"systemprompt","headers":{"Authorization":"Bearer x"}}]"#;
    let body = render_reg_values(true, &[("managedMcpServers", payload.to_string())]);

    assert!(body.contains("[HKEY_LOCAL_MACHINE\\SOFTWARE\\Policies\\Claude]"));
    let entries = parse_reg_entries(&body).expect("rendered values parse");
    assert_eq!(entries.len(), 1);
    assert_eq!(value_of(&entries, "managedMcpServers"), payload);
}

// A line that is neither header, section, blank, nor `"name"="value"` is a
// parse error naming the line, never a silently skipped entry.
#[test]
fn parser_rejects_a_malformed_value_line_and_names_it() {
    let err = parse_reg_entries(
        "Windows Registry Editor Version 5.00\r\n[HKEY_CURRENT_USER\\SOFTWARE\\Policies\\Claude]\r\n\"inferenceProvider\"=\"gateway\"\r\ngarbage line\r\n",
    )
    .expect_err("a malformed line is an error");
    assert_eq!(err.line, 4);
    assert_eq!(err.text, "garbage line");
}
