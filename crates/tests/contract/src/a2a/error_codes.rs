use systemprompt_agent::models::a2a::jsonrpc::JsonRpcError;

#[test]
fn parse_error_code_is_minus_32700() {
    let err = JsonRpcError::parse_error();
    assert_eq!(err.code, -32700);
}

#[test]
fn invalid_request_code_is_minus_32600() {
    let err = JsonRpcError::invalid_request();
    assert_eq!(err.code, -32600);
}

#[test]
fn method_not_found_code_is_minus_32601() {
    let err = JsonRpcError::method_not_found();
    assert_eq!(err.code, -32601);
}

#[test]
fn invalid_params_code_is_minus_32602() {
    let err = JsonRpcError::invalid_params();
    assert_eq!(err.code, -32602);
}

#[test]
fn internal_error_code_is_minus_32603() {
    let err = JsonRpcError::internal_error();
    assert_eq!(err.code, -32603);
}

#[test]
fn standard_error_codes_have_messages() {
    let errors = [
        JsonRpcError::parse_error(),
        JsonRpcError::invalid_request(),
        JsonRpcError::method_not_found(),
        JsonRpcError::invalid_params(),
        JsonRpcError::internal_error(),
    ];
    for err in &errors {
        assert!(!err.message.is_empty(), "Error code {} should have a message", err.code);
    }
}

#[test]
fn custom_error_preserves_code_and_message() {
    let err = JsonRpcError::new(-32001, "Task not found");
    assert_eq!(err.code, -32001);
    assert_eq!(err.message, "Task not found");
}

#[test]
fn error_with_data_preserves_all_fields() {
    let data = serde_json::json!({"taskId": "abc-123"});
    let err = JsonRpcError::with_data(-32001, "Task not found", data.clone());
    assert_eq!(err.code, -32001);
    assert_eq!(err.message, "Task not found");
    assert_eq!(err.data, Some(data));
}
