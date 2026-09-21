//! The Claude Desktop managed policy and its per-platform rendering.
//!
//! These assertions were impossible before the policy was extracted: the plist
//! was hand-built inside `#[cfg(target_os = "macos")]` string concatenation, so
//! the macOS profile shipped with no `managedMcpServers` key and with MCP
//! servers pointing at the upstream gateway with no credential, and nothing on
//! any other host could see it.

use std::collections::BTreeMap;

use systemprompt_bridge::claude_policy::audit_workspace_folders;
use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::install::mdm::policy::{
    McpServerEntry, PolicyEntry, PolicyInputs, PolicyValue, claude_desktop_policy, plist_body,
};

fn entry(name: &str) -> McpServerEntry {
    McpServerEntry {
        name: name.to_owned(),
        url: format!("http://127.0.0.1:48217/mcp/{name}"),
        tool_policy: Default::default(),
    }
}

fn host_token() -> HostToken {
    HostToken::new("desktop-host-token")
}

fn policy_with(servers: &[McpServerEntry]) -> Vec<PolicyEntry> {
    let headers = BTreeMap::new();
    let token = host_token();
    claude_desktop_policy(&PolicyInputs {
        base_url: "http://127.0.0.1:48217",
        host_token: &token,
        models: None,
        headers: &headers,
        egress_allowed_hosts: None,
        org_uuid: None,
        mcp_servers: Some(servers),
    })
    .expect("policy renders")
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
    assert_eq!(
        first["headers"]["Authorization"],
        "Bearer desktop-host-token"
    );
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

// Why: the Claude Desktop Code tab treats this list as the only permitted
// workspace roots. A single brand entry blocked every folder outside it
// ("Directory C:\\Users\\x is not within the allowed workspace roots") while
// the presence-only assertion above stayed green.
#[test]
fn the_workspace_folders_pre_trust_the_brand_dir_and_allow_home() {
    let policy = policy_with(&[]);
    let Some(PolicyValue::Json(value)) = value_of(&policy, "allowedWorkspaceFolders") else {
        panic!("allowedWorkspaceFolders must be a JSON value");
    };
    let folders = value.as_array().expect("array");
    assert_ne!(
        folders.len(),
        1,
        "a single brand-only entry locks the Code tab out of home"
    );
    assert_eq!(folders.len(), 2);
    assert_eq!(folders[0]["path"], "~/Systemprompt");
    assert_eq!(folders[0]["isDefaultSelected"], true);
    assert_eq!(folders[1]["path"], "~");
    assert_eq!(folders[1]["isDefaultSelected"], false);
}

#[test]
fn the_plist_renders_arrays_and_dicts_as_native_elements() {
    let servers = vec![entry("odoo")];
    let body = plist_body(&policy_with(&servers), "  ");

    assert!(body.contains("<key>managedMcpServers</key>"));
    assert!(body.contains("<key>Authorization</key>"));
    assert!(body.contains("<string>Bearer desktop-host-token</string>"));
    assert!(body.contains("<key>inferenceProvider</key>\n  <string>gateway</string>"));
}

// Why: Claude Desktop validates each `allowedWorkspaceFolders` entry against
// its schema, drops a malformed one, and an empty resulting list blocks every
// folder ("Your administrator has disabled adding folders").
// `isDefaultSelected` is typed boolean; rendering it as `<string>true</string>`
// dropped both entries on every Mac and locked the Code tab out. The top-level
// `disable*` keys are string booleans by Claude's published encoding and must
// stay that way.
#[test]
fn the_plist_workspace_folders_carry_native_booleans() {
    let body = plist_body(&policy_with(&[]), "  ");
    let start = body
        .find("<key>allowedWorkspaceFolders</key>")
        .expect("workspace key rendered");
    let end = body[start..].find("</array>").expect("array closed") + start;
    let block = &body[start..end];

    assert!(block.contains("<key>isDefaultSelected</key>\n      <true/>"));
    assert!(block.contains("<key>isDefaultSelected</key>\n      <false/>"));
    assert!(!block.contains("<string>true</string>"));
    assert!(!block.contains("<string>false</string>"));
    assert!(body.contains("<key>disableAutoUpdates</key>\n  <string>true</string>"));
    assert!(body.contains("<key>disableNonessentialServices</key>\n  <string>false</string>"));
}

#[test]
fn nested_numbers_render_as_plist_numbers() {
    let entry: PolicyEntry = ("x", PolicyValue::Json(serde_json::json!([1, 2.5])));
    let body = plist_body(&[entry], "");
    assert!(body.contains("<integer>1</integer>"));
    assert!(body.contains("<real>2.5</real>"));
}

#[test]
fn the_audit_drops_string_booleans_and_reports_an_empty_list() {
    let raw = r#"[{"path":"~/Astound","isDefaultSelected":"true"},{"path":"~","isDefaultSelected":"false"}]"#;
    let audit = audit_workspace_folders(raw).expect("a list");
    assert!(audit.kept.is_empty());
    assert_eq!(audit.dropped.len(), 2);
    assert!(audit.dropped[0].0.contains("~/Astound"));
    assert!(audit.blocks_all_folders());
    assert!(!audit.allows_home());
}

#[test]
fn the_audit_keeps_valid_entries_and_plain_paths() {
    let raw = r#"[{"path":"~/Astound","isDefaultSelected":true},"~",{"path":"/x","mode":"ro"},{"path":""},{"path":"/y","extra":1}]"#;
    let audit = audit_workspace_folders(raw).expect("a list");
    let kept: Vec<&str> = audit.kept.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(kept, ["~/Astound", "~", "/x"]);
    assert_eq!(audit.dropped.len(), 2);
    assert_eq!(audit.dropped[0].1, "empty path");
    assert!(audit.dropped[1].1.contains("extra"));
    assert!(!audit.blocks_all_folders());
    assert!(audit.allows_home());
}

#[test]
fn the_audit_of_an_empty_list_blocks_all_folders() {
    let audit = audit_workspace_folders("[]").expect("a list");
    assert!(audit.blocks_all_folders());
    assert!(audit_workspace_folders("{}").is_err());
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
        tool_policy: Default::default(),
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
        Some(&PolicyValue::Str("desktop-host-token".to_owned()))
    );
    assert_eq!(
        value_of(&policy, "inferenceGatewayAuthScheme"),
        Some(&PolicyValue::Str("bearer".to_owned()))
    );
    let PolicyValue::Json(models) = value_of(&policy, "inferenceModels").expect("models present")
    else {
        panic!("inferenceModels must be a JSON value");
    };
    assert_eq!(
        models,
        &serde_json::json!([
            "claude-opus-5",
            "claude-sonnet-5",
            "claude-fable-5-1",
            "claude-haiku-4-5-20251001"
        ])
    );
}

// Why: the gateway supplies a compatible model list through the host profile,
// and a re-apply must not overwrite it with the built-in default.
#[test]
fn an_installed_model_list_wins_over_the_default() {
    let headers = BTreeMap::new();
    let token = host_token();
    let policy = claude_desktop_policy(&PolicyInputs {
        base_url: "http://127.0.0.1:48217",
        host_token: &token,
        models: Some(r#"["claude-opus-5"]"#.to_owned()),
        headers: &headers,
        egress_allowed_hosts: None,
        org_uuid: None,
        mcp_servers: Some(&[]),
    })
    .expect("policy renders");
    let PolicyValue::Json(models) = value_of(&policy, "inferenceModels").expect("models present")
    else {
        panic!("inferenceModels must be a JSON value");
    };
    assert_eq!(models, &serde_json::json!(["claude-opus-5"]));
}

// Why: the pin is the bridge's own supply-chain value, not Claude's, so it
// must never be written into Claude's policy hive.
#[test]
fn the_policy_never_carries_the_manifest_pin() {
    let policy = policy_with(&[]);
    for key in ["manifestPubkey", "inferenceManifestPubkey"] {
        assert!(value_of(&policy, key).is_none(), "pin leaked as {key}");
    }
}

// Why: `disableNonessentialServices=true` blocks the renderer Cowork's MCP
// display extensions load from, so it is written as an explicit `false` and an
// older `true` is corrected on the next sync rather than left standing.
#[test]
fn nonessential_services_stay_enabled_and_are_written_explicitly() {
    let policy = policy_with(&[]);
    assert_eq!(
        value_of(&policy, "disableNonessentialServices"),
        Some(&PolicyValue::Bool(false))
    );
}

#[test]
fn a_valid_org_uuid_is_carried_and_a_malformed_one_is_rejected() {
    let headers = BTreeMap::new();
    let token = host_token();
    let with = |uuid: Option<&str>| {
        claude_desktop_policy(&PolicyInputs {
            base_url: "http://127.0.0.1:48217",
            host_token: &token,
            models: None,
            headers: &headers,
            egress_allowed_hosts: None,
            org_uuid: uuid,
            mcp_servers: Some(&[]),
        })
    };
    assert_eq!(
        value_of(
            &with(Some("f8e4d915-f8ad-5304-ab0d-c1bf895df963")).expect("valid uuid renders"),
            "deploymentOrganizationUuid"
        ),
        Some(&PolicyValue::Str(
            "f8e4d915-f8ad-5304-ab0d-c1bf895df963".to_owned()
        ))
    );
    assert!(
        matches!(
            with(Some("garbage")),
            Err(systemprompt_bridge::install::mdm::MdmError::InvalidConfig { key, .. })
                if key == "deploymentOrganizationUuid"
        ),
        "a malformed org uuid is a config rejection, not a silent drop"
    );
    assert!(
        value_of(
            &with(None).expect("no uuid renders"),
            "deploymentOrganizationUuid"
        )
        .is_none()
    );
}

// Why: Claude Desktop breaks on non-Anthropic model families. The gateway
// serves Gemini for Claude Code's benefit; it must never reach this key.
#[test]
fn desktop_inference_models_never_carry_non_anthropic_ids() {
    let headers = BTreeMap::new();
    let token = host_token();
    let policy = claude_desktop_policy(&PolicyInputs {
        base_url: "http://127.0.0.1:48217",
        host_token: &token,
        models: Some(
            r#"["gemini-2.5-flash", "claude-sonnet-5", "vertex-gemini-2.5-pro"]"#.to_owned(),
        ),
        headers: &headers,
        egress_allowed_hosts: None,
        org_uuid: None,
        mcp_servers: Some(&[]),
    })
    .expect("policy renders");
    let PolicyValue::Json(models) = value_of(&policy, "inferenceModels").expect("models present")
    else {
        panic!("inferenceModels must be a JSON value");
    };
    assert_eq!(models, &serde_json::json!(["claude-sonnet-5"]));
}

#[test]
fn an_all_gemini_list_falls_back_to_the_default_claude_models() {
    let headers = BTreeMap::new();
    let token = host_token();
    let policy = claude_desktop_policy(&PolicyInputs {
        base_url: "http://127.0.0.1:48217",
        host_token: &token,
        models: Some(r#"["gemini-2.5-flash"]"#.to_owned()),
        headers: &headers,
        egress_allowed_hosts: None,
        org_uuid: None,
        mcp_servers: Some(&[]),
    })
    .expect("policy renders");
    let PolicyValue::Json(models) = value_of(&policy, "inferenceModels").expect("models present")
    else {
        panic!("inferenceModels must be a JSON value");
    };
    let ids = models.as_array().expect("array");
    assert!(!ids.is_empty());
    assert!(
        ids.iter()
            .all(|m| m.as_str().is_some_and(|s| s.contains("claude"))),
        "{models}"
    );
}
