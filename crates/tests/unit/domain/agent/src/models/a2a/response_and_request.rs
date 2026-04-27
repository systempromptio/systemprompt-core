use systemprompt_agent::models::a2a::jsonrpc::{
    JSON_RPC_VERSION_2_0, JsonRpcError, JsonRpcResponse, Request, RequestId,
};

#[test]
fn test_jsonrpc_response_with_result() {
    let response: JsonRpcResponse<String> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some("success".to_string()),
        error: None,
        id: RequestId::String("1".to_string()),
    };

    assert_eq!(response.jsonrpc, "2.0");
    response
        .result
        .expect("response with result should have result");
    assert!(response.error.is_none());
}

#[test]
fn test_jsonrpc_response_with_error() {
    let response: JsonRpcResponse<String> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(JsonRpcError::invalid_request()),
        id: RequestId::Number(1),
    };

    assert!(response.result.is_none());
    let error = response
        .error
        .expect("response with error should have error");
    assert_eq!(error.code, -32600);
}

#[test]
fn test_jsonrpc_response_serialize_result() {
    let response: JsonRpcResponse<String> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some("test".to_string()),
        error: None,
        id: RequestId::String("req-1".to_string()),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("2.0"));
    assert!(json.contains("test"));
    assert!(json.contains("req-1"));
    assert!(!json.contains("error"));
}

#[test]
fn test_jsonrpc_response_serialize_error() {
    let response: JsonRpcResponse<String> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(JsonRpcError::parse_error()),
        id: RequestId::Number(42),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("-32700"));
    assert!(json.contains("Parse error"));
    assert!(!json.contains("result"));
}

#[test]
fn test_jsonrpc_response_debug() {
    let response: JsonRpcResponse<String> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some("test".to_string()),
        error: None,
        id: RequestId::String("1".to_string()),
    };

    let debug = format!("{:?}", response);
    assert!(debug.contains("JsonRpcResponse"));
}

#[test]
fn test_request_serialize() {
    let request: Request<serde_json::Value> = Request {
        jsonrpc: "2.0".to_string(),
        method: "test/method".to_string(),
        params: serde_json::json!({"key": "value"}),
        id: RequestId::String("req-1".to_string()),
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("2.0"));
    assert!(json.contains("test/method"));
    assert!(json.contains("key"));
    assert!(json.contains("value"));
}

#[test]
fn test_request_debug() {
    let request: Request<serde_json::Value> = Request {
        jsonrpc: "2.0".to_string(),
        method: "test".to_string(),
        params: serde_json::json!({}),
        id: RequestId::Number(1),
    };

    let debug = format!("{:?}", request);
    assert!(debug.contains("Request"));
}

#[test]
fn test_json_rpc_version_constant() {
    assert_eq!(JSON_RPC_VERSION_2_0, "2.0");
}
