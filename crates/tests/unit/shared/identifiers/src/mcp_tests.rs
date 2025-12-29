//! Unit tests for MCP-related identifier types.

use std::collections::HashSet;
use systemprompt_identifiers::{AiToolCallId, McpExecutionId, McpServerId, ToDbValue, DbValue};

// ============================================================================
// AiToolCallId Tests
// ============================================================================

#[test]
fn test_ai_tool_call_id_new() {
    let id = AiToolCallId::new("toolu_01D7XQ2V9K3J8N5M4P6R7T8W9Y");
    assert_eq!(id.as_str(), "toolu_01D7XQ2V9K3J8N5M4P6R7T8W9Y");
}

#[test]
fn test_ai_tool_call_id_display() {
    let id = AiToolCallId::new("display-tool");
    assert_eq!(format!("{}", id), "display-tool");
}

#[test]
fn test_ai_tool_call_id_from_string() {
    let id: AiToolCallId = String::from("from-string-tool").into();
    assert_eq!(id.as_str(), "from-string-tool");
}

#[test]
fn test_ai_tool_call_id_from_str() {
    let id: AiToolCallId = "from-str-tool".into();
    assert_eq!(id.as_str(), "from-str-tool");
}

#[test]
fn test_ai_tool_call_id_as_ref() {
    let id = AiToolCallId::new("as-ref-tool");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-tool");
}

#[test]
fn test_ai_tool_call_id_clone_and_eq() {
    let id1 = AiToolCallId::new("clone-tool");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_ai_tool_call_id_hash() {
    let id1 = AiToolCallId::new("hash-tool");
    let id2 = AiToolCallId::new("hash-tool");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_ai_tool_call_id_serialize_json() {
    let id = AiToolCallId::new("serialize-tool");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-tool\"");
}

#[test]
fn test_ai_tool_call_id_deserialize_json() {
    let id: AiToolCallId = serde_json::from_str("\"deserialize-tool\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-tool");
}

#[test]
fn test_ai_tool_call_id_to_db_value() {
    let id = AiToolCallId::new("db-value-tool");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-tool"));
}

#[test]
fn test_ai_tool_call_id_ref_to_db_value() {
    let id = AiToolCallId::new("db-value-ref-tool");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-tool"));
}

#[test]
fn test_ai_tool_call_id_anthropic_format() {
    let id = AiToolCallId::new("toolu_01D7XQ2V9K3J8N5M4P6R7T8W9Y");
    assert!(id.as_str().starts_with("toolu_"));
}

// ============================================================================
// McpExecutionId Tests
// ============================================================================

#[test]
fn test_mcp_execution_id_new() {
    let id = McpExecutionId::new("exec-123");
    assert_eq!(id.as_str(), "exec-123");
}

#[test]
fn test_mcp_execution_id_generate() {
    let id = McpExecutionId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_mcp_execution_id_generate_unique() {
    let id1 = McpExecutionId::generate();
    let id2 = McpExecutionId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_mcp_execution_id_display() {
    let id = McpExecutionId::new("display-exec");
    assert_eq!(format!("{}", id), "display-exec");
}

#[test]
fn test_mcp_execution_id_from_string() {
    let id: McpExecutionId = String::from("from-string-exec").into();
    assert_eq!(id.as_str(), "from-string-exec");
}

#[test]
fn test_mcp_execution_id_from_str() {
    let id: McpExecutionId = "from-str-exec".into();
    assert_eq!(id.as_str(), "from-str-exec");
}

#[test]
fn test_mcp_execution_id_as_ref() {
    let id = McpExecutionId::new("as-ref-exec");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-exec");
}

#[test]
fn test_mcp_execution_id_clone_and_eq() {
    let id1 = McpExecutionId::new("clone-exec");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_mcp_execution_id_hash() {
    let id1 = McpExecutionId::new("hash-exec");
    let id2 = McpExecutionId::new("hash-exec");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_mcp_execution_id_serialize_json() {
    let id = McpExecutionId::new("serialize-exec");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-exec\"");
}

#[test]
fn test_mcp_execution_id_deserialize_json() {
    let id: McpExecutionId = serde_json::from_str("\"deserialize-exec\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-exec");
}

#[test]
fn test_mcp_execution_id_to_db_value() {
    let id = McpExecutionId::new("db-value-exec");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-exec"));
}

#[test]
fn test_mcp_execution_id_ref_to_db_value() {
    let id = McpExecutionId::new("db-value-ref-exec");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-exec"));
}

// ============================================================================
// McpServerId Tests
// ============================================================================

#[test]
fn test_mcp_server_id_new() {
    let id = McpServerId::new("content-manager");
    assert_eq!(id.as_str(), "content-manager");
}

#[test]
fn test_mcp_server_id_display() {
    let id = McpServerId::new("display-server");
    assert_eq!(format!("{}", id), "display-server");
}

#[test]
fn test_mcp_server_id_from_string() {
    let id: McpServerId = String::from("from-string-server").into();
    assert_eq!(id.as_str(), "from-string-server");
}

#[test]
fn test_mcp_server_id_from_str() {
    let id: McpServerId = "from-str-server".into();
    assert_eq!(id.as_str(), "from-str-server");
}

#[test]
fn test_mcp_server_id_as_ref() {
    let id = McpServerId::new("as-ref-server");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-server");
}

#[test]
fn test_mcp_server_id_clone_and_eq() {
    let id1 = McpServerId::new("clone-server");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_mcp_server_id_hash() {
    let id1 = McpServerId::new("hash-server");
    let id2 = McpServerId::new("hash-server");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_mcp_server_id_serialize_json() {
    let id = McpServerId::new("serialize-server");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-server\"");
}

#[test]
fn test_mcp_server_id_deserialize_json() {
    let id: McpServerId = serde_json::from_str("\"deserialize-server\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-server");
}

#[test]
fn test_mcp_server_id_to_db_value() {
    let id = McpServerId::new("db-value-server");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-server"));
}

#[test]
fn test_mcp_server_id_ref_to_db_value() {
    let id = McpServerId::new("db-value-ref-server");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-server"));
}

#[test]
#[should_panic(expected = "MCP server ID cannot be empty")]
fn test_mcp_server_id_empty_panics() {
    let _ = McpServerId::new("");
}

#[test]
fn test_mcp_server_id_from_env_missing() {
    // Ensure the env var is not set
    std::env::remove_var("MCP_SERVICE_ID");
    let result = McpServerId::from_env();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not set"));
}

#[test]
fn test_mcp_server_id_from_env_empty() {
    std::env::set_var("MCP_SERVICE_ID", "");
    let result = McpServerId::from_env();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("empty"));
    std::env::remove_var("MCP_SERVICE_ID");
}

#[test]
fn test_mcp_server_id_from_env_valid() {
    std::env::set_var("MCP_SERVICE_ID", "test-mcp-server");
    let result = McpServerId::from_env();
    assert!(result.is_ok());
    assert_eq!(result.unwrap().as_str(), "test-mcp-server");
    std::env::remove_var("MCP_SERVICE_ID");
}

#[test]
fn test_mcp_server_id_hyphenated_name() {
    let id = McpServerId::new("content-research");
    assert_eq!(id.as_str(), "content-research");
}

#[test]
fn test_mcp_server_id_systemprompt_admin() {
    let id = McpServerId::new("systemprompt-admin");
    assert_eq!(id.as_str(), "systemprompt-admin");
}
