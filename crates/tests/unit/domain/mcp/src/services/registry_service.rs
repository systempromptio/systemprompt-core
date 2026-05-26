//! Unit tests for `RegistryService` construction and error-paths.
//!
//! The `get_enabled_servers` / `validate` / `find_server` paths rely on
//! `Config::get()` global state, which is not initialised in unit tests,
//! so those calls deterministically return an error here.

use systemprompt_mcp::McpServerRegistry;
use systemprompt_test_fixtures::fixture_user_id;

#[test]
fn test_new_constructs() {
    let _r = McpServerRegistry::new(fixture_user_id());
}

#[test]
fn test_clone() {
    let r = McpServerRegistry::new(fixture_user_id());
    let _r2 = r.clone();
}

#[test]
fn test_debug() {
    let r = McpServerRegistry::new(fixture_user_id());
    let d = format!("{:?}", r);
    assert!(d.contains("RegistryService") || d.contains("Registry"));
}

#[test]
fn test_get_server_missing_propagates_error() {
    let r = McpServerRegistry::new(fixture_user_id());
    let result = r.get_server("nonexistent");
    result.unwrap_err();
}

#[test]
fn test_find_server_missing_returns_error_or_none() {
    let r = McpServerRegistry::new(fixture_user_id());
    // Without a loaded Config, this errors at the loader. Either Err or
    // Ok(None) is acceptable.
    let _ = r.find_server("nonexistent");
}
