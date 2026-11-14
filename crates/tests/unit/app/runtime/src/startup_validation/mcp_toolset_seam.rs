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
        "Toolset 'phantom' is not registered by any crate linked into this binary"
    );
    assert_eq!(
        issues[1].message,
        "Toolset 'ghost' is not registered by any crate linked into this binary"
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
fn an_internal_server_without_a_binary_or_toolsets_is_reported() {
    let mut server = deployment(McpServerType::Internal, true, false, "unused");
    server.binary = None;
    let cfg = services_with(vec![("nobin", server)]);

    let errors = collect_manifest_errors(&cfg, false, &["known"], |_| Ok::<(), String>(()));

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "mcp_servers.nobin.binary");
    assert_eq!(
        errors[0].message,
        "Internal server declares neither a binary nor toolsets"
    );
}

#[test]
fn a_hosted_server_with_registered_toolsets_passes() {
    let mut server = deployment(McpServerType::Internal, true, false, "unused");
    server.binary = None;
    server.toolsets = vec![toolset("known", true)];
    let cfg = services_with(vec![("hosted", server)]);

    let errors = collect_manifest_errors(&cfg, false, &["known"], |_| {
        Err::<(), String>("a hosted server resolves no binary".to_owned())
    });

    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn a_hosted_server_naming_an_unregistered_toolset_fails() {
    let mut server = deployment(McpServerType::Internal, true, false, "unused");
    server.binary = None;
    server.toolsets = vec![toolset("known", true), toolset("ghost", true)];
    let cfg = services_with(vec![("hosted", server)]);

    let errors = collect_manifest_errors(&cfg, false, &["known"], |_| Ok::<(), String>(()));

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "mcp_servers.hosted.toolsets");
    assert!(errors[0].message.contains("'ghost'"), "{}", errors[0].message);
}

#[test]
fn an_empty_inventory_fails_every_hosted_toolset_closed() {
    let mut server = deployment(McpServerType::Internal, true, false, "unused");
    server.binary = None;
    server.toolsets = vec![toolset("issues", true), toolset("documents", true)];
    let cfg = services_with(vec![("hosted", server)]);

    let errors = collect_manifest_errors(&cfg, false, &[], |_| Ok::<(), String>(()));

    assert_eq!(errors.len(), 2);
    assert!(errors.iter().all(|e| {
        e.suggestion
            .as_deref()
            .is_some_and(|s| s.contains("none (this binary links no toolsets)"))
    }));
}

#[test]
fn a_binary_server_keeps_its_toolsets_unchecked_here() {
    let mut server = deployment(McpServerType::Internal, true, false, "bin");
    server.toolsets = vec![toolset("linked-into-its-own-binary", true)];
    let cfg = services_with(vec![("ext", server)]);

    let errors = collect_manifest_errors(&cfg, false, &[], |_| Ok::<(), String>(()));

    assert!(errors.is_empty(), "{errors:?}");
}
