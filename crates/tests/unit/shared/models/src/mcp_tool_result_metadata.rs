//! Unit tests for [`systemprompt_models::mcp::McpToolResultMetadata`].

use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::mcp::McpToolResultMetadata;

#[test]
fn create_and_validate() {
    let mcp_execution_id = McpExecutionId::generate();
    let metadata = McpToolResultMetadata::new(mcp_execution_id.clone());
    metadata
        .validate()
        .expect("metadata.validate() should succeed");
    assert_eq!(metadata.mcp_execution_id, mcp_execution_id);
}

#[test]
fn to_meta_and_back() {
    let mcp_execution_id = McpExecutionId::generate();
    let original = McpToolResultMetadata::new(mcp_execution_id)
        .with_execution_time(150)
        .with_server_version("1.0.0");

    let meta = original.to_meta().unwrap();
    let parsed = McpToolResultMetadata::from_meta(&meta).unwrap();

    assert_eq!(original, parsed);
}

#[test]
fn missing_meta_fails() {
    let result = rmcp::model::CallToolResult::default();

    let err = McpToolResultMetadata::from_call_tool_result(&result)
        .expect_err("missing meta should produce an error");
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("_meta is missing"),
        "error should mention missing _meta, got: {err_msg}"
    );
}
