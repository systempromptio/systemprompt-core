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
fn test_schemas_five_tables() {
    let ext = McpExtension;
    let schemas = ext.schemas();
    assert_eq!(
        schemas
            .iter()
            .filter(|schema| schema.table.is_some())
            .count(),
        5
    );
}

#[test]
fn test_schemas_table_names_match_expected() {
    let ext = McpExtension;
    let schemas = ext.schemas();
    let names: Vec<&str> = schemas.iter().filter_map(|s| s.table.as_deref()).collect();
    assert!(names.contains(&"mcp_tool_executions"));
    assert!(names.contains(&"mcp_sessions"));
    assert!(names.contains(&"mcp_artifacts"));
    assert!(names.contains(&"mcp_proxy_identities"));
    assert!(names.contains(&"mcp_external_sessions"));
}

#[test]
fn owner_capture_and_privacy_contracts_are_registered() {
    let schemas = McpExtension.schemas();
    let capture: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("EXECUTE FUNCTION sp_capture_reporting_change")
        })
        .collect();
    assert_eq!(
        capture.len(),
        1,
        "owner capture SQL must be registered exactly once"
    );
    for view in ["reporting_source_mcp_tool_executions"] {
        assert!(
            capture[0].sql.contains(view),
            "missing reporting view: {view}"
        );
    }
    let privacy: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("CREATE OR REPLACE FUNCTION public.lock_mcp_reporting_sources")
        })
        .collect();
    assert_eq!(
        privacy.len(),
        1,
        "owner privacy SQL must survive capture registration"
    );
}
