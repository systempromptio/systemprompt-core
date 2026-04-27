use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcError, RequestId};

#[test]
fn test_request_id_string() {
    let id = RequestId::String("test-id".to_string());
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"test-id\"");
}

#[test]
fn test_request_id_number() {
    let id = RequestId::Number(42);
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "42");
}

#[test]
fn test_request_id_deserialize_string() {
    let json = "\"my-id\"";
    let id: RequestId = serde_json::from_str(json).unwrap();
    match id {
        RequestId::String(s) => assert_eq!(s, "my-id"),
        _ => panic!("Expected String variant"),
    }
}

#[test]
fn test_jsonrpc_error_new() {
    let error = JsonRpcError::new(-32600, "Invalid Request");
    assert_eq!(error.code, -32600);
    assert_eq!(error.message, "Invalid Request");
    assert!(error.data.is_none());
}

#[test]
fn test_jsonrpc_error_with_data() {
    let error = JsonRpcError::with_data(
        -32000,
        "Custom error",
        serde_json::json!({"details": "test"}),
    );
    assert_eq!(error.code, -32000);
    assert_eq!(error.message, "Custom error");
    let data = error.data.expect("error with_data should have data field");
    assert_eq!(data["details"], "test");
}

#[test]
fn test_jsonrpc_error_parse_error() {
    let error = JsonRpcError::parse_error();
    assert_eq!(error.code, -32700);
    assert_eq!(error.message, "Parse error");
}

#[test]
fn test_jsonrpc_error_invalid_request() {
    let error = JsonRpcError::invalid_request();
    assert_eq!(error.code, -32600);
    assert_eq!(error.message, "Invalid Request");
}

#[test]
fn test_jsonrpc_error_method_not_found() {
    let error = JsonRpcError::method_not_found();
    assert_eq!(error.code, -32601);
    assert_eq!(error.message, "Method not found");
}

#[test]
fn test_jsonrpc_error_invalid_params() {
    let error = JsonRpcError::invalid_params();
    assert_eq!(error.code, -32602);
    assert_eq!(error.message, "Invalid params");
}

#[test]
fn test_jsonrpc_error_internal_error() {
    let error = JsonRpcError::internal_error();
    assert_eq!(error.code, -32603);
    assert_eq!(error.message, "Internal error");
}

#[test]
fn test_jsonrpc_error_serialize() {
    let error = JsonRpcError::new(-32600, "Test");
    let json = serde_json::to_string(&error).unwrap();
    assert!(json.contains("-32600"));
    assert!(json.contains("Test"));
}

#[test]
fn test_jsonrpc_error_debug() {
    let error = JsonRpcError::new(-32600, "Test");
    let debug = format!("{:?}", error);
    assert!(debug.contains("JsonRpcError"));
}
