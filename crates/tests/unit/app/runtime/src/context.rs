//! Unit tests for AppContext and AppContextBuilder
//!
//! Tests cover:
//! - AppContextBuilder creation and configuration
//! - Audience accessor methods
//! - Debug trait implementations

use systemprompt_extension::ExtensionRegistry;
use systemprompt_runtime::{AppContext, AppContextBuilder};

// ============================================================================
// AppContextBuilder Tests
// ============================================================================

#[test]
fn test_context_builder_new() {
    let builder = AppContextBuilder::new();
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("AppContextBuilder"));
}

#[test]
fn test_context_builder_default() {
    let builder = AppContextBuilder::default();
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("AppContextBuilder"));
}

#[test]
fn test_context_builder_with_extensions() {
    let registry = ExtensionRegistry::new();
    let builder = AppContextBuilder::new().with_extensions(registry);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("AppContextBuilder"));
}

#[test]
fn test_context_builder_chaining() {
    let registry = ExtensionRegistry::new();
    let builder = AppContextBuilder::new().with_extensions(registry);
    let _ = builder;
}

#[test]
fn test_context_builder_with_startup_warnings_true() {
    let builder = AppContextBuilder::new().with_startup_warnings(true);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("show_startup_warnings: true"));
}

#[test]
fn test_context_builder_with_startup_warnings_false() {
    let builder = AppContextBuilder::new().with_startup_warnings(false);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("show_startup_warnings: false"));
}

#[test]
fn test_context_builder_full_chain() {
    let registry = ExtensionRegistry::new();
    let builder = AppContextBuilder::new()
        .with_extensions(registry)
        .with_startup_warnings(true);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("AppContextBuilder"));
    assert!(debug_str.contains("show_startup_warnings: true"));
}

#[test]
fn test_context_builder_default_startup_warnings() {
    let builder = AppContextBuilder::default();
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("show_startup_warnings: false"));
}

// ============================================================================
// AppContext Static Methods Tests
// ============================================================================

#[test]
fn test_get_provided_audiences() {
    let audiences = AppContext::get_provided_audiences();
    assert!(audiences.contains(&"a2a".to_string()));
    assert!(audiences.contains(&"api".to_string()));
    assert!(audiences.contains(&"mcp".to_string()));
    assert_eq!(audiences.len(), 3);
}

#[test]
fn test_get_provided_audiences_order() {
    let audiences = AppContext::get_provided_audiences();
    // Verify the audiences are returned in expected order
    assert_eq!(audiences[0], "a2a");
    assert_eq!(audiences[1], "api");
    assert_eq!(audiences[2], "mcp");
}

#[test]
fn test_get_valid_audiences_returns_provided() {
    let audiences = AppContext::get_valid_audiences("test-module");
    let provided = AppContext::get_provided_audiences();
    assert_eq!(audiences, provided);
}

#[test]
fn test_get_valid_audiences_different_modules() {
    let auth_audiences = AppContext::get_valid_audiences("auth");
    let mcp_audiences = AppContext::get_valid_audiences("mcp-server");
    let agent_audiences = AppContext::get_valid_audiences("agent");

    // All modules should get the same audiences
    assert_eq!(auth_audiences, mcp_audiences);
    assert_eq!(mcp_audiences, agent_audiences);
}

#[test]
fn test_get_valid_audiences_empty_module_name() {
    let audiences = AppContext::get_valid_audiences("");
    assert!(audiences.contains(&"a2a".to_string()));
    assert!(audiences.contains(&"api".to_string()));
    assert!(audiences.contains(&"mcp".to_string()));
}

#[test]
fn test_get_server_audiences_returns_provided() {
    let audiences = AppContext::get_server_audiences("test-server", 8080);
    let provided = AppContext::get_provided_audiences();
    assert_eq!(audiences, provided);
}

#[test]
fn test_get_server_audiences_different_ports() {
    let port_8080 = AppContext::get_server_audiences("server", 8080);
    let port_3000 = AppContext::get_server_audiences("server", 3000);
    let port_443 = AppContext::get_server_audiences("server", 443);

    // All ports should get the same audiences
    assert_eq!(port_8080, port_3000);
    assert_eq!(port_3000, port_443);
}

#[test]
fn test_get_server_audiences_different_servers() {
    let api = AppContext::get_server_audiences("api-server", 8080);
    let mcp = AppContext::get_server_audiences("mcp-server", 3000);
    let agent = AppContext::get_server_audiences("agent-server", 4000);

    // All servers should get the same audiences
    assert_eq!(api, mcp);
    assert_eq!(mcp, agent);
}

#[test]
fn test_get_server_audiences_boundary_ports() {
    let min_port = AppContext::get_server_audiences("server", 0);
    let max_port = AppContext::get_server_audiences("server", 65535);

    assert_eq!(min_port.len(), 3);
    assert_eq!(max_port.len(), 3);
}

// ============================================================================
// Audience Content Tests
// ============================================================================

#[test]
fn test_audiences_contain_expected_values() {
    let audiences = AppContext::get_provided_audiences();

    // a2a - Agent-to-Agent protocol
    assert!(audiences.iter().any(|a| a == "a2a"));

    // api - Standard API access
    assert!(audiences.iter().any(|a| a == "api"));

    // mcp - Model Context Protocol
    assert!(audiences.iter().any(|a| a == "mcp"));
}

#[test]
fn test_audiences_are_lowercase() {
    let audiences = AppContext::get_provided_audiences();

    for audience in &audiences {
        assert_eq!(audience, &audience.to_lowercase());
    }
}

#[test]
fn test_audiences_no_duplicates() {
    let audiences = AppContext::get_provided_audiences();
    let mut unique = audiences.clone();
    unique.sort();
    unique.dedup();

    assert_eq!(audiences.len(), unique.len());
}
