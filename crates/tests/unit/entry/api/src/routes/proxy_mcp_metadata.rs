//! Serialization of the MCP reverse-proxy tool-execution response payload
//! (`routes::proxy::mcp::ToolExecutionResponse`).
//!
//! Pure `Serialize` DTO; the handlers that build it need an `AppContext`, so
//! only the wire shape is exercised here.

use serde_json::Value;
use systemprompt_api::routes::proxy::mcp::ToolExecutionResponse;
use systemprompt_identifiers::McpExecutionId;

#[test]
fn tool_execution_response_serializes_with_output() {
    let resp = ToolExecutionResponse {
        id: McpExecutionId::generate(),
        tool_name: "search".to_owned(),
        server_name: "sharepoint".to_owned(),
        server_endpoint: "http://localhost:9000/mcp".to_owned(),
        input: serde_json::json!({"q": "rust"}),
        output: Some(serde_json::json!({"hits": 3})),
        status: "completed".to_owned(),
    };
    let v: Value = serde_json::to_value(&resp).expect("serialize");
    assert_eq!(v["tool_name"], "search");
    assert_eq!(v["server_name"], "sharepoint");
    assert_eq!(v["server_endpoint"], "http://localhost:9000/mcp");
    assert_eq!(v["input"]["q"], "rust");
    assert_eq!(v["output"]["hits"], 3);
    assert_eq!(v["status"], "completed");
    assert!(v["id"].is_string());
}

#[test]
fn tool_execution_response_omits_null_output() {
    let resp = ToolExecutionResponse {
        id: McpExecutionId::generate(),
        tool_name: "noop".to_owned(),
        server_name: "srv".to_owned(),
        server_endpoint: "http://localhost:1/mcp".to_owned(),
        input: serde_json::json!({}),
        output: None,
        status: "pending".to_owned(),
    };
    let v: Value = serde_json::to_value(&resp).expect("serialize");
    assert!(v.get("output").is_none());
    assert_eq!(v["status"], "pending");
}
