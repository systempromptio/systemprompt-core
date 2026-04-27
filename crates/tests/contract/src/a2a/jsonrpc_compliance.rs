use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcError, JsonRpcResponse, RequestId};
use systemprompt_models::a2a::Task;

#[test]
fn response_always_has_jsonrpc_2_0() {
    let response: JsonRpcResponse<Task> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: None,
        id: RequestId::Number(1),
    };
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
}

#[test]
fn request_id_string_roundtrips() {
    let id = RequestId::String("abc-123".to_string());
    let json = serde_json::to_value(&id).unwrap();
    assert!(json.is_string());
    let back: RequestId = serde_json::from_value(json).unwrap();
    assert_eq!(back, id);
}

#[test]
fn request_id_number_roundtrips() {
    let id = RequestId::Number(42);
    let json = serde_json::to_value(&id).unwrap();
    assert!(json.is_number());
    let back: RequestId = serde_json::from_value(json).unwrap();
    assert_eq!(back, id);
}

#[test]
fn error_response_has_required_fields() {
    let error = JsonRpcError::new(-32600, "Invalid Request");
    let json = serde_json::to_value(&error).unwrap();

    assert!(json["code"].is_number(), "Error must have numeric code");
    assert!(
        json["message"].is_string(),
        "Error must have string message"
    );
}

#[test]
fn error_response_data_is_optional() {
    let without_data = JsonRpcError::new(-32600, "Invalid Request");
    let json = serde_json::to_value(&without_data).unwrap();
    assert!(json.get("data").is_none() || json["data"].is_null());

    let with_data = JsonRpcError::with_data(
        -32600,
        "Invalid Request",
        serde_json::json!({"detail": "missing field"}),
    );
    let json = serde_json::to_value(&with_data).unwrap();
    assert!(json["data"].is_object());
}

#[test]
fn success_response_has_result_no_error() {
    let task = Task::default();
    let response: JsonRpcResponse<Task> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(task),
        error: None,
        id: RequestId::Number(1),
    };
    let json = serde_json::to_value(&response).unwrap();
    assert!(json.get("result").is_some());
    assert!(json.get("error").is_none());
}

#[test]
fn error_response_has_error_no_result() {
    let response: JsonRpcResponse<Task> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(JsonRpcError::internal_error()),
        id: RequestId::Number(1),
    };
    let json = serde_json::to_value(&response).unwrap();
    assert!(json.get("result").is_none());
    assert!(json.get("error").is_some());
}
