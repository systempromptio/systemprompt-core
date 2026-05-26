//! Unit tests for `McpExtension`.

use systemprompt_extension::prelude::*;
use systemprompt_mcp::McpExtension;

#[test]
fn test_metadata_id_and_name() {
    let ext = McpExtension;
    let meta = ext.metadata();
    assert_eq!(meta.id, "mcp");
    assert_eq!(meta.name, "MCP");
    assert!(!meta.version.is_empty());
}

#[test]
fn test_dependencies_contains_users() {
    let ext = McpExtension;
    let deps = ext.dependencies();
    assert!(deps.iter().any(|d| *d == "users"));
}

#[test]
fn test_schemas_three_tables() {
    let ext = McpExtension;
    let schemas = ext.schemas();
    assert_eq!(schemas.len(), 3);
}

#[test]
fn test_schemas_table_names_match_expected() {
    let ext = McpExtension;
    let schemas = ext.schemas();
    let names: Vec<&str> = schemas.iter().map(|s| s.table.as_str()).collect();
    assert!(names.contains(&"mcp_tool_executions"));
    assert!(names.contains(&"mcp_sessions"));
    assert!(names.contains(&"mcp_artifacts"));
}

#[test]
fn test_default_and_clone_copy() {
    let a = McpExtension;
    let b = a;
    assert_eq!(a.metadata().id, b.metadata().id);
    let _c: McpExtension = McpExtension::default();
}
