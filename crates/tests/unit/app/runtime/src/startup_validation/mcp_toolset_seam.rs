//! The hosted-toolset check `ToolsetSlot::build` runs
//! (`mcp_host::contract::unregistered_toolsets`) and what the startup
//! manifest check still reports for internal servers.

use serde_json::json;
use systemprompt_mcp_host::contract::{UnregisteredToolset, unregistered_toolsets};
use systemprompt_models::mcp::{McpServerType, ToolsetRef};
use systemprompt_runtime::collect_manifest_errors;

use super::mcp_manifest_seam::{deployment, services_with};

fn toolset(id: &str, enabled: bool) -> ToolsetRef {
    serde_json::from_value(json!({ "id": id, "enabled": enabled })).expect("toolset ref")
}

fn missing(server: &str, toolset: &str) -> UnregisteredToolset {
    UnregisteredToolset {
        server: server.to_owned(),
        toolset: toolset.to_owned(),
    }
}

#[test]
fn unknown_enabled_hosted_toolsets_are_reported_in_server_order() {
    let mut beta = deployment(McpServerType::Internal, true, false, "unused");
    beta.binary = None;
    beta.toolsets = vec![toolset("known", true), toolset("ghost", true)];
    let mut alpha = deployment(McpServerType::Internal, true, false, "unused");
    alpha.binary = None;
    alpha.toolsets = vec![toolset("phantom", true), toolset("dormant", false)];
    let mut off = deployment(McpServerType::Internal, false, false, "unused");
    off.binary = None;
    off.toolsets = vec![toolset("never-checked", true)];
    let cfg = services_with(vec![("beta", beta), ("alpha", alpha), ("off", off)]);

    assert_eq!(
        unregistered_toolsets(&cfg, &["known", "other"]),
        [missing("alpha", "phantom"), missing("beta", "ghost")]
    );
}

#[test]
fn every_contributed_toolset_passes() {
    let mut server = deployment(McpServerType::Internal, true, false, "unused");
    server.binary = None;
    server.toolsets = vec![toolset("known", true)];
    let cfg = services_with(vec![("srv", server)]);

    assert!(unregistered_toolsets(&cfg, &["known"]).is_empty());
}

#[test]
fn no_contributed_toolsets_fails_every_hosted_toolset_closed() {
    let mut server = deployment(McpServerType::Internal, true, false, "unused");
    server.binary = None;
    server.toolsets = vec![toolset("issues", true), toolset("documents", true)];
    let cfg = services_with(vec![("hosted", server)]);

    assert_eq!(
        unregistered_toolsets(&cfg, &[]),
        [missing("hosted", "issues"), missing("hosted", "documents")]
    );
}

#[test]
fn a_binary_server_keeps_its_toolsets_unchecked() {
    let mut server = deployment(McpServerType::Internal, true, false, "bin");
    server.toolsets = vec![toolset("linked-into-its-own-binary", true)];
    let cfg = services_with(vec![("ext", server)]);

    assert!(unregistered_toolsets(&cfg, &[]).is_empty());
    assert!(collect_manifest_errors(&cfg, false, |_| Ok::<(), String>(())).is_empty());
}

#[test]
fn an_internal_server_without_a_binary_or_toolsets_is_reported() {
    let mut server = deployment(McpServerType::Internal, true, false, "unused");
    server.binary = None;
    let cfg = services_with(vec![("nobin", server)]);

    let errors = collect_manifest_errors(&cfg, false, |_| Ok::<(), String>(()));

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].field, "mcp_servers.nobin.binary");
    assert_eq!(
        errors[0].message,
        "Internal server declares neither a binary nor toolsets"
    );
}

#[test]
fn a_hosted_server_resolves_no_binary_in_the_manifest_check() {
    let mut server = deployment(McpServerType::Internal, true, false, "unused");
    server.binary = None;
    server.toolsets = vec![toolset("ghost", true)];
    let cfg = services_with(vec![("hosted", server)]);

    let errors = collect_manifest_errors(&cfg, false, |_| {
        Err::<(), String>("a hosted server resolves no binary".to_owned())
    });

    assert!(errors.is_empty(), "{errors:?}");
}
