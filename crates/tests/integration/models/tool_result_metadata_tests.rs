use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::mcp::McpToolResultMetadata;

#[test]
fn test_create_and_validate() {
    let mcp_execution_id = McpExecutionId::generate();
    let metadata = McpToolResultMetadata::new(mcp_execution_id);
    assert!(metadata.validate().is_ok());
}

#[test]
fn test_to_meta_and_back() {
    let mcp_execution_id = McpExecutionId::generate();
    let original = McpToolResultMetadata::new(mcp_execution_id)
        .with_execution_time(150)
        .with_server_version("1.0.0");

    let meta = original.to_meta().unwrap();
    let parsed = McpToolResultMetadata::from_meta(&meta).unwrap();

    assert_eq!(original, parsed);
}

#[test]
fn test_missing_meta_fails() {
    let result = rmcp::model::CallToolResult {
        content: vec![],
        structured_content: None,
        is_error: None,
        meta: None,
    };

    assert!(McpToolResultMetadata::from_call_tool_result(&result).is_err());
}
