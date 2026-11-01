use serde_json::json;
use systemprompt_models::mcp::{McpServerType, ToolsetRef};
use systemprompt_runtime::{collect_manifest_errors, collect_toolset_errors};

use super::mcp_manifest_seam::{deployment, services_with};

fn toolset(id: &str, enabled: bool) -> ToolsetRef {
    serde_json::from_value(json!({ "id": id, "enabled": enabled })).expect("toolset ref")
}

#[test]
fn unknown_enabled_toolsets_are_reported_in_server_order() {
    let mut beta = deployment(McpServerType::Internal, true, false, "bin_b");
    beta.toolsets = vec![toolset("known", true), toolset("ghost", true)];
    let mut alpha = deployment(McpServerType::Internal, true, false, "bin_a");
    alpha.toolsets = vec![toolset("phantom", true), toolset("dormant", false)];
    let mut off = deployment(McpServerType::Internal, false, false, "bin_c");
    off.toolsets = vec![toolset("never-checked", true)];
    let cfg = services_with(vec![("beta", beta), ("alpha", alpha), ("off", off)]);

    let issues = collect_toolset_errors(&cfg, &["known", "other"]);

    let fields: Vec<&str> = issues.iter().map(|i| i.field.as_str()).collect();
    assert_eq!(
        fields,
        vec!["mcp_servers.alpha.toolsets", "mcp_servers.beta.toolsets"]
    );
    assert_eq!(
        issues[0].message,
        "Toolset 'phantom' is not registered by any linked crate"
    );
    assert_eq!(
        issues[1].message,
        "Toolset 'ghost' is not registered by any linked crate"
    );
    assert!(
        issues[0]
            .suggestion
            .as_deref()
            .is_some_and(|s| s.starts_with("Registered toolsets: known, other."))
    );
}

#[test]
fn every_known_toolset_yields_no_issue() {
    let mut server = deployment(McpServerType::Internal, true, false, "bin");
    server.toolsets = vec![toolset("known", true)];
    let cfg = services_with(vec![("srv", server)]);

    assert!(collect_toolset_errors(&cfg, &["known"]).is_empty());
}

#[test]
fn an_internal_server_without_a_binary_is_reported() {
    let mut server = deployment(McpServerType::Internal, true, false, "unused");
    server.binary = None;
    let cfg = services_with(vec![("nobin", server)]);

    let errors = collect_manifest_errors(&cfg, false, |_| Ok::<(), String>(()));

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "mcp_servers.nobin.binary");
    assert_eq!(errors[0].message, "Internal server declares no binary");
    assert_eq!(
        errors[0].suggestion.as_deref(),
        Some("Set binary: <extension binary> in services/mcp/nobin.yaml")
    );
}
