use std::collections::HashSet;
use systemprompt_identifiers::{AiToolCallId, McpExecutionId, McpServerId, DbValue, ToDbValue};

#[test]
fn mcp_server_id_valid_value() {
    let id = McpServerId::try_new("content-manager").unwrap();
    assert_eq!(id.as_str(), "content-manager");
}

#[test]
fn mcp_server_id_rejects_empty() {
    let err = McpServerId::try_new("").unwrap_err();
    assert_eq!(err.to_string(), "McpServerId cannot be empty");
}

#[test]
#[should_panic(expected = "McpServerId cannot be empty")]
fn mcp_server_id_new_panics_on_empty() {
    let _ = McpServerId::new("");
}

#[test]
fn mcp_server_id_try_from_str() {
    let id: McpServerId = "test-server".try_into().unwrap();
    assert_eq!(id.as_str(), "test-server");
}

#[test]
fn mcp_server_id_try_from_string() {
    let id: McpServerId = String::from("test-server").try_into().unwrap();
    assert_eq!(id.as_str(), "test-server");
}

#[test]
fn mcp_server_id_from_str_parse() {
    let id: McpServerId = "test-server".parse().unwrap();
    assert_eq!(id.as_str(), "test-server");
}

#[test]
fn mcp_server_id_serde_roundtrip() {
    let id = McpServerId::new("serde-server");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serde-server\"");
    let deserialized: McpServerId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn mcp_server_id_serde_rejects_empty_on_deserialize() {
    let result: Result<McpServerId, _> = serde_json::from_str("\"\"");
    assert!(result.is_err());
}

#[test]
fn mcp_server_id_to_db_value() {
    let id = McpServerId::new("db-server");
    let db_val = id.to_db_value();
    assert!(matches!(db_val, DbValue::String(ref s) if s == "db-server"));
}

#[test]
fn mcp_server_id_equality_across_construction() {
    let from_new = McpServerId::new("test");
    let from_try: McpServerId = "test".try_into().unwrap();
    assert_eq!(from_new, from_try);
}

#[test]
fn mcp_server_id_from_env_missing() {
    unsafe { std::env::remove_var("MCP_SERVICE_ID") };
    let err = McpServerId::from_env().unwrap_err();
    assert!(err.to_string().contains("not set"));
}

#[test]
#[allow(unsafe_code)]
fn mcp_server_id_from_env_empty() {
    unsafe { std::env::set_var("MCP_SERVICE_ID", "") };
    let err = McpServerId::from_env().unwrap_err();
    assert!(err.to_string().contains("empty"));
    unsafe { std::env::remove_var("MCP_SERVICE_ID") };
}

#[test]
#[allow(unsafe_code)]
fn mcp_server_id_from_env_valid() {
    unsafe { std::env::set_var("MCP_SERVICE_ID", "test-mcp-server") };
    let id = McpServerId::from_env().unwrap();
    assert_eq!(id.as_str(), "test-mcp-server");
    unsafe { std::env::remove_var("MCP_SERVICE_ID") };
}

#[test]
fn mcp_execution_id_generate_uuid_format() {
    let id = McpExecutionId::generate();
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn mcp_execution_id_generate_unique() {
    let ids: HashSet<String> = (0..10).map(|_| McpExecutionId::generate().as_str().to_string()).collect();
    assert_eq!(ids.len(), 10);
}

#[test]
fn mcp_execution_id_serde_transparent() {
    let id = McpExecutionId::new("exec-test");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"exec-test\"");
}

#[test]
fn ai_tool_call_id_anthropic_format() {
    let id = AiToolCallId::new("toolu_01D7XQ2V9K3J8N5M4P6R7T8W9Y");
    assert!(id.as_str().starts_with("toolu_"));
    assert_eq!(id.as_str().len(), 32);
}

#[test]
fn ai_tool_call_id_serde_transparent() {
    let id = AiToolCallId::new("toolu_test");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"toolu_test\"");
    let deserialized: AiToolCallId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn ai_tool_call_id_to_db_value() {
    let id = AiToolCallId::new("toolu_db");
    let db_val = id.to_db_value();
    assert!(matches!(db_val, DbValue::String(ref s) if s == "toolu_db"));
}
