//! The Claude Desktop managed policy and its per-platform rendering.
//!
//! These assertions were impossible before the policy was extracted: the plist
//! was hand-built inside `#[cfg(target_os = "macos")]` string concatenation, so
//! the macOS profile shipped with no `managedMcpServers` key and with MCP
//! servers pointing at the upstream gateway with no credential, and nothing on
//! any other host could see it.

use std::collections::BTreeMap;

use systemprompt_bridge::install::mdm::policy::{
    McpServerEntry, PolicyInputs, PolicyValue, claude_desktop_policy, plist_body,
};

fn entry(name: &str) -> McpServerEntry {
    McpServerEntry {
        name: name.to_owned(),
        url: format!("http://127.0.0.1:48217/mcp/{name}"),
        bearer: "Bearer loopback-secret".to_owned(),
    }
}

fn policy_with(servers: &[McpServerEntry]) -> Vec<(&'static str, PolicyValue)> {
    let headers = BTreeMap::new();
    claude_desktop_policy(&PolicyInputs {
        base_url: "http://127.0.0.1:48217",
        api_key: "loopback-secret",
        models: None,
        headers: &headers,
        egress_allowed_hosts: None,
        org_uuid: None,
        mcp_servers: servers,
    })
}

fn value_of<'a>(policy: &'a [(&'static str, PolicyValue)], key: &str) -> Option<&'a PolicyValue> {
    policy.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

// Why: the defect that broke the 2026-09-03 Mac install. A server published
// with the upstream URL and no headers can never authenticate, and bypasses
// the proxy that stamps the per-user JWT and applies governance.
#[test]
fn managed_mcp_servers_point_at_the_loopback_proxy_and_carry_the_bearer() {
    let servers = vec![entry("knowledge-bank")];
    let policy = policy_with(&servers);

    let PolicyValue::Json(value) = value_of(&policy, "managedMcpServers").expect("key present")
    else {
        panic!("managedMcpServers must be a JSON value");
    };
    let first = &value.as_array().expect("an array")[0];

    assert_eq!(first["url"], "http://127.0.0.1:48217/mcp/knowledge-bank");
    assert_eq!(first["headers"]["Authorization"], "Bearer loopback-secret");
    assert_eq!(first["transport"], "http");
    assert!(
        first.get("oauth").is_none(),
        "an empty oauth dict asks for well-known discovery against a bearer URL"
    );
}

// Why: omitting the key on an empty registry leaves a stale server in place,
// so the policy publishes an empty list instead.
#[test]
fn an_empty_registry_still_publishes_the_key_so_stale_servers_clear() {
    let policy = policy_with(&[]);
    let PolicyValue::Json(value) = value_of(&policy, "managedMcpServers").expect("key present")
    else {
        panic!("managedMcpServers must be a JSON value");
    };
    assert_eq!(value.as_array().map(Vec::len), Some(0));
}

// Why: the GUI profile carried neither key, so its MCP panel stayed empty and
// the `allowedWorkspaceFolders` setup-health row could never go green.
#[test]
fn the_policy_carries_every_key_the_setup_health_checks_require() {
    let servers = vec![entry("odoo")];
    let policy = policy_with(&servers);
    for key in [
        "inferenceProvider",
        "inferenceGatewayBaseUrl",
        "inferenceGatewayApiKey",
        "inferenceModels",
        "allowedWorkspaceFolders",
        "managedMcpServers",
    ] {
        assert!(value_of(&policy, key).is_some(), "missing key: {key}");
    }
}

#[test]
fn the_plist_renders_arrays_and_dicts_as_native_elements() {
    let servers = vec![entry("odoo")];
    let body = plist_body(&policy_with(&servers), "  ");

    assert!(body.contains("<key>managedMcpServers</key>"));
    assert!(body.contains("<key>Authorization</key>"));
    assert!(body.contains("<string>Bearer loopback-secret</string>"));
    assert!(body.contains("<key>inferenceProvider</key>\n  <string>gateway</string>"));
}

// Why: `macos::apply` compares rendered bytes against what is on disk to decide
// whether to raise an administrator prompt. An unstable key or server order
// would prompt the user on every sync.
#[test]
fn rendering_is_byte_stable_across_runs() {
    let servers = vec![entry("odoo"), entry("knowledge-bank")];
    assert_eq!(
        plist_body(&policy_with(&servers), "  "),
        plist_body(&policy_with(&servers), "  ")
    );
}

#[test]
fn xml_special_characters_in_a_server_name_are_escaped() {
    let servers = vec![McpServerEntry {
        name: "a&b<c".to_owned(),
        url: "http://127.0.0.1:48217/mcp/a".to_owned(),
        bearer: "Bearer x".to_owned(),
    }];
    let body = plist_body(&policy_with(&servers), "  ");
    assert!(body.contains("a&amp;b&lt;c"));
    assert!(!body.contains("a&b<c"));
}

// Why: Cowork treats `inferenceProvider=gateway` without a base URL and a
// credential as unusable and refuses to start any task, so a partial gateway
// block is worse than none.
#[test]
fn the_gateway_block_is_written_as_one_complete_unit() {
    let policy = policy_with(&[]);
    assert_eq!(
        value_of(&policy, "inferenceProvider"),
        Some(&PolicyValue::Str("gateway".to_owned()))
    );
    assert_eq!(
        value_of(&policy, "inferenceGatewayBaseUrl"),
        Some(&PolicyValue::Str("http://127.0.0.1:48217".to_owned()))
    );
    assert_eq!(
        value_of(&policy, "inferenceGatewayApiKey"),
        Some(&PolicyValue::Str("loopback-secret".to_owned()))
    );
    assert_eq!(
        value_of(&policy, "inferenceGatewayAuthScheme"),
        Some(&PolicyValue::Str("bearer".to_owned()))
    );
    let PolicyValue::Json(models) = value_of(&policy, "inferenceModels").expect("models present")
    else {
        panic!("inferenceModels must be a JSON value");
    };
    assert!(
        models.as_array().is_some_and(|m| !m.is_empty()),
        "the default model list must not be empty"
    );
}

// Why: the gateway supplies a compatible model list through the host profile,
// and a re-apply must not overwrite it with the built-in default.
#[test]
fn an_installed_model_list_wins_over_the_default() {
    let headers = BTreeMap::new();
    let policy = claude_desktop_policy(&PolicyInputs {
        base_url: "http://127.0.0.1:48217",
        api_key: "s",
        models: Some(r#"["claude-opus-5"]"#.to_owned()),
        headers: &headers,
        egress_allowed_hosts: None,
        org_uuid: None,
        mcp_servers: &[],
    });
    let PolicyValue::Json(models) = value_of(&policy, "inferenceModels").expect("models present")
    else {
        panic!("inferenceModels must be a JSON value");
    };
    assert_eq!(models, &serde_json::json!(["claude-opus-5"]));
}
