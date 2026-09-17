use serde_json::json;
use systemprompt_api::services::proxy::audit::jsonrpc::{
    extract_sse_data, parse_response_frame, parse_tool_call,
};

#[test]
fn parse_tool_call_extracts_name_and_arguments() {
    let body = br#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"lookup","arguments":{"q":"acme"}}}"#;
    let call = parse_tool_call(body).expect("tools/call parses");
    let (id, name, args) = (call.id, call.tool_name, call.arguments);
    assert_eq!(id, json!(7));
    assert_eq!(name, "lookup");
    assert_eq!(args, json!({"q": "acme"}));
}

#[test]
fn parse_tool_call_ignores_non_tool_call_methods() {
    let body = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    assert!(parse_tool_call(body).is_none());
}

#[test]
fn parse_tool_call_rejects_malformed_body() {
    assert!(parse_tool_call(b"not json").is_none());
}

#[test]
fn parse_response_frame_matches_id_and_extracts_structured_output() {
    let data =
        r#"{"jsonrpc":"2.0","id":7,"result":{"structuredContent":{"rows":3},"isError":false}}"#;
    let outcome = parse_response_frame(data, &json!(7)).expect("result parses");
    let (output, error) = (outcome.output, outcome.error_message);
    assert_eq!(output, Some(json!({"rows": 3})));
    assert!(error.is_none());
}

#[test]
fn parse_response_frame_reports_tool_error() {
    let data = r#"{"jsonrpc":"2.0","id":7,"result":{"content":[{"type":"text","text":"boom"}],"isError":true}}"#;
    let error = parse_response_frame(data, &json!(7))
        .expect("result parses")
        .error_message;
    assert_eq!(error.as_deref(), Some("MCP tool call returned isError"));
}

#[test]
fn parse_response_frame_surfaces_jsonrpc_error() {
    let data = r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32000,"message":"denied"}}"#;
    let error = parse_response_frame(data, &json!(7))
        .expect("error frame parses")
        .error_message;
    let error = error.expect("jsonrpc error surfaces as error_message");
    assert!(
        error.contains("denied"),
        "error should surface upstream message, got {error}"
    );
}

#[test]
fn parse_response_frame_ignores_mismatched_id() {
    let data = r#"{"jsonrpc":"2.0","id":9,"result":{"isError":false}}"#;
    assert!(parse_response_frame(data, &json!(7)).is_none());
}

#[test]
fn extract_sse_data_joins_multiline_data_fields() {
    let frame = "event: message\ndata: {\"a\":1}\ndata: {\"b\":2}\n\n";
    assert_eq!(
        extract_sse_data(frame).as_deref(),
        Some("{\"a\":1}\n{\"b\":2}")
    );
}

#[test]
fn extract_sse_data_none_when_no_data_lines() {
    assert!(extract_sse_data("event: ping\n\n").is_none());
}

#[test]
fn parse_response_frame_keeps_the_whole_result_for_ingest() {
    let data = r#"{"jsonrpc":"2.0","id":7,"result":{"content":[{"type":"text","text":"hi"}],"structuredContent":{"rows":3},"_meta":{"x":1}}}"#;
    let outcome = parse_response_frame(data, &json!(7)).expect("result parses");
    let result = outcome.result.expect("result object retained");
    assert_eq!(result["structuredContent"], json!({"rows": 3}));
    assert_eq!(result["content"][0]["text"], json!("hi"));
    assert_eq!(result["_meta"]["x"], json!(1));
}

#[test]
fn stamp_execution_adds_the_server_key_under_the_systemprompt_meta() {
    use systemprompt_api::services::proxy::audit::jsonrpc::stamp_execution;
    let data = r#"{"jsonrpc":"2.0","id":7,"result":{"content":[]}}"#;
    let stamped = stamp_execution(data, "exec-1").expect("stamped");
    let frame: serde_json::Value = serde_json::from_str(&stamped).unwrap();
    assert_eq!(
        frame["result"]["_meta"]["io.systemprompt/execution"]["mcp_execution_id"],
        json!("exec-1")
    );
}

#[test]
fn stamp_execution_never_overwrites_an_in_process_key() {
    use systemprompt_api::services::proxy::audit::jsonrpc::stamp_execution;
    let data = r#"{"jsonrpc":"2.0","id":7,"result":{"content":[],"_meta":{"io.systemprompt/execution":{"mcp_execution_id":"orig"}}}}"#;
    let stamped = stamp_execution(data, "exec-1").expect("stamped");
    let frame: serde_json::Value = serde_json::from_str(&stamped).unwrap();
    assert_eq!(
        frame["result"]["_meta"]["io.systemprompt/execution"]["mcp_execution_id"],
        json!("orig")
    );
}

#[test]
fn stamp_execution_leaves_error_frames_alone() {
    use systemprompt_api::services::proxy::audit::jsonrpc::stamp_execution;
    let data = r#"{"jsonrpc":"2.0","id":7,"error":{"code":-1,"message":"no"}}"#;
    assert!(stamp_execution(data, "exec-1").is_none());
}

#[test]
fn replace_sse_data_swaps_only_the_data_lines() {
    use systemprompt_api::services::proxy::audit::jsonrpc::replace_sse_data;
    let frame = "event: message\nid: 3\ndata: {\"a\":1}\ndata: more\n\n";
    let out = replace_sse_data(frame, "{\"b\":2}");
    assert_eq!(out, "event: message\nid: 3\ndata: {\"b\":2}\n\n");
}
