//! Claude Code `permissions` rules derived from the manifest's managed MCP
//! servers, and the splice that keeps a person's own rules intact.

use std::collections::BTreeMap;

use serde_json::json;
use systemprompt_bridge::gateway::manifest::{
    ManagedMcpServer, SignedManifestBuilder, ValidatedUrl,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::{ManagedMcpServerName, ToolName, ToolPolicy};
use systemprompt_bridge::install::mdm::claude_code_settings::permissions::{
    PermissionRules, merged_permissions, rules_for,
};
use systemprompt_test_fixtures::fixture_user_id;

fn server(name: &str, policy: Option<BTreeMap<ToolName, ToolPolicy>>) -> ManagedMcpServer {
    ManagedMcpServer {
        id: systemprompt_identifiers::McpServerId::new(name),
        name: ManagedMcpServerName::try_new(name).unwrap(),
        url: ValidatedUrl::new(format!("https://gw.example.com/api/v1/mcp/{name}/mcp")),
        transport: Some("http".into()),
        headers: None,
        oauth: None,
        tool_policy: policy,
    }
}

fn wildcard(policy: ToolPolicy) -> BTreeMap<ToolName, ToolPolicy> {
    BTreeMap::from([(
        ToolName::try_new(ManagedMcpServer::TOOL_POLICY_WILDCARD).unwrap(),
        policy,
    )])
}

fn manifest(
    servers: Vec<ManagedMcpServer>,
) -> systemprompt_bridge::gateway::manifest::SignedManifest {
    SignedManifestBuilder::new(
        ManifestVersion::try_new("2026-09-11T00:00:00Z-00000000").unwrap(),
        "2026-09-11T00:00:00Z",
        "2026-09-11T00:00:00Z",
        fixture_user_id(),
    )
    .with_managed_mcp_servers(servers)
    .build()
}

#[test]
fn a_wildcard_allow_becomes_the_bare_server_rule_and_one_per_mirroring_plugin() {
    let m = manifest(vec![server("atlassian", Some(wildcard(ToolPolicy::Allow)))]);
    let by_plugin = BTreeMap::from([
        (
            "astound-super-admin".to_owned(),
            vec!["atlassian".to_owned()],
        ),
        ("astound-dev".to_owned(), vec![]),
    ]);
    let rules = rules_for(&m, &by_plugin);
    assert_eq!(
        rules.allow,
        vec![
            "mcp__atlassian".to_owned(),
            "mcp__plugin_astound-super-admin_atlassian".to_owned()
        ]
    );
    assert!(rules.deny.is_empty());
}

#[test]
fn a_named_tool_rule_is_scoped_to_that_tool_and_deny_lands_in_the_deny_list() {
    let policy = BTreeMap::from([
        (ToolName::try_new("*").unwrap(), ToolPolicy::Allow),
        (ToolName::try_new("delete_issue").unwrap(), ToolPolicy::Deny),
    ]);
    let m = manifest(vec![server("atlassian", Some(policy))]);
    let rules = rules_for(&m, &BTreeMap::new());
    assert_eq!(rules.allow, vec!["mcp__atlassian".to_owned()]);
    assert_eq!(rules.deny, vec!["mcp__atlassian__delete_issue".to_owned()]);
}

#[test]
fn prompt_and_absent_policies_write_nothing() {
    let m = manifest(vec![
        server("systemprompt", Some(wildcard(ToolPolicy::Prompt))),
        server("github", None),
    ]);
    assert!(rules_for(&m, &BTreeMap::new()).is_empty());
}

#[test]
fn merging_keeps_the_users_rules_and_replaces_only_what_the_bridge_wrote_before() {
    let existing = json!({
        "defaultMode": "acceptEdits",
        "allow": ["Bash(git status)", "mcp__old", "mcp__atlassian"],
        "deny": ["Bash(rm -rf)"]
    });
    let previously = PermissionRules {
        allow: vec!["mcp__old".into()],
        deny: vec![],
    };
    let next = PermissionRules {
        allow: vec!["mcp__atlassian".into(), "mcp__systemprompt".into()],
        deny: vec![],
    };
    let merged = merged_permissions(Some(&existing), &previously, &next).unwrap();
    assert_eq!(merged["defaultMode"], "acceptEdits");
    assert_eq!(
        merged["allow"],
        json!(["Bash(git status)", "mcp__atlassian", "mcp__systemprompt"])
    );
    assert_eq!(merged["deny"], json!(["Bash(rm -rf)"]));
}

#[test]
fn taking_every_bridge_rule_out_of_an_otherwise_empty_object_removes_the_key() {
    let existing = json!({ "allow": ["mcp__atlassian"] });
    let previously = PermissionRules {
        allow: vec!["mcp__atlassian".into()],
        deny: vec![],
    };
    assert!(
        merged_permissions(Some(&existing), &previously, &PermissionRules::default()).is_none()
    );
}

#[test]
fn a_wildcard_deny_drops_named_allows_that_claude_code_would_never_honour() {
    let policy = BTreeMap::from([
        (ToolName::try_new("*").unwrap(), ToolPolicy::Deny),
        (ToolName::try_new("read_issue").unwrap(), ToolPolicy::Allow),
        (ToolName::try_new("delete_issue").unwrap(), ToolPolicy::Deny),
    ]);
    let m = manifest(vec![server("atlassian", Some(policy))]);
    let rules = rules_for(&m, &BTreeMap::new());
    assert!(
        rules.allow.is_empty(),
        "deny beats allow in Claude Code, so a named allow under a wildcard deny is dead: {:?}",
        rules.allow
    );
    assert_eq!(
        rules.deny,
        vec![
            "mcp__atlassian".to_owned(),
            "mcp__atlassian__delete_issue".to_owned()
        ]
    );
}

fn upstream(
    policy: BTreeMap<String, ToolPolicy>,
) -> systemprompt_bridge::mcp_registry::McpUpstream {
    systemprompt_bridge::mcp_registry::McpUpstream {
        url: ValidatedUrl::new("https://gw.example.com/api/v1/mcp/atlassian/mcp"),
        headers: BTreeMap::new(),
        display_name: "Atlassian".to_owned(),
        transport: None,
        tool_policy: policy,
    }
}

#[test]
fn a_desktop_wildcard_deny_withholds_the_server_rather_than_prompting_for_unknown_tools() {
    use systemprompt_bridge::install::mdm::desktop_tool_policy::{
        denied_outright, desktop_tool_policy_map,
    };
    let denied = upstream(BTreeMap::from([("*".to_owned(), ToolPolicy::Deny)]));
    assert!(denied_outright(&denied));
    let allowed = upstream(BTreeMap::from([
        ("*".to_owned(), ToolPolicy::Allow),
        ("delete_issue".to_owned(), ToolPolicy::Deny),
    ]));
    assert!(!denied_outright(&allowed));
    let map = desktop_tool_policy_map(
        &allowed,
        &["read_issue".to_owned(), "delete_issue".to_owned()],
    );
    assert_eq!(map.get("read_issue").map(String::as_str), Some("allow"));
    assert_eq!(
        map.get("delete_issue").map(String::as_str),
        Some("blocked"),
        "a named entry wins over the wildcard"
    );
}
