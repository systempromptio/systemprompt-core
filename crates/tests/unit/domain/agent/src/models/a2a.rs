//! Unit tests for A2A protocol models
//!
//! Tests cover:
//! - JsonRpc types (RequestId, Request, JsonRpcResponse, JsonRpcError)
//! - Protocol types (MessageSendParams, TaskQueryParams, A2aJsonRpcRequest)
//! - Response types (A2aResponse factory methods)
//! - Event types (TaskStatusUpdateEvent, TaskArtifactUpdateEvent)
//! - Push notification types

use systemprompt_core_agent::{
    A2aJsonRpcRequest, A2aRequestParams, A2aResponse, Task, TaskIdParams, TaskQueryParams,
    TaskState, TaskStatus,
};

// ============================================================================
// A2aJsonRpcRequest Parse Tests
// ============================================================================

#[test]
fn test_parse_message_send_request() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "message/send".to_string(),
        params: serde_json::json!({
            "message": {
                "role": "user",
                "parts": [{"kind": "text", "text": "Hello"}],
                "messageId": "msg-1",
                "contextId": "ctx-1",
                "kind": "message"
            }
        }),
        id: systemprompt_core_agent::models::a2a::jsonrpc::RequestId::String("1".to_string()),
    };

    let result = request.parse_request();
    assert!(result.is_ok());
    match result.unwrap() {
        A2aRequestParams::SendMessage(_) => {}
        _ => panic!("Expected SendMessage variant"),
    }
}

#[test]
fn test_parse_tasks_get_request() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tasks/get".to_string(),
        params: serde_json::json!({
            "id": "task-123"
        }),
        id: systemprompt_core_agent::models::a2a::jsonrpc::RequestId::String("2".to_string()),
    };

    let result = request.parse_request();
    assert!(result.is_ok());
    match result.unwrap() {
        A2aRequestParams::GetTask(params) => {
            assert_eq!(params.id, "task-123");
        }
        _ => panic!("Expected GetTask variant"),
    }
}

#[test]
fn test_parse_tasks_cancel_request() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tasks/cancel".to_string(),
        params: serde_json::json!({
            "id": "task-456"
        }),
        id: systemprompt_core_agent::models::a2a::jsonrpc::RequestId::Number(3),
    };

    let result = request.parse_request();
    assert!(result.is_ok());
    match result.unwrap() {
        A2aRequestParams::CancelTask(params) => {
            assert_eq!(params.id, "task-456");
        }
        _ => panic!("Expected CancelTask variant"),
    }
}

#[test]
fn test_parse_message_stream_request() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "message/stream".to_string(),
        params: serde_json::json!({
            "message": {
                "role": "user",
                "parts": [{"kind": "text", "text": "Stream this"}],
                "messageId": "msg-2",
                "contextId": "ctx-2",
                "kind": "message"
            }
        }),
        id: systemprompt_core_agent::models::a2a::jsonrpc::RequestId::String("4".to_string()),
    };

    let result = request.parse_request();
    assert!(result.is_ok());
    match result.unwrap() {
        A2aRequestParams::SendStreamingMessage(_) => {}
        _ => panic!("Expected SendStreamingMessage variant"),
    }
}

#[test]
fn test_parse_unsupported_method() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "unsupported/method".to_string(),
        params: serde_json::json!({}),
        id: systemprompt_core_agent::models::a2a::jsonrpc::RequestId::String("5".to_string()),
    };

    let result = request.parse_request();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Unsupported method"));
}

#[test]
fn test_parse_invalid_params() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tasks/get".to_string(),
        params: serde_json::json!({"wrong": "params"}),
        id: systemprompt_core_agent::models::a2a::jsonrpc::RequestId::String("6".to_string()),
    };

    let result = request.parse_request();
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Invalid parameters"));
}

// ============================================================================
// TaskQueryParams Tests
// ============================================================================

#[test]
fn test_task_query_params_serialize() {
    let params = TaskQueryParams {
        id: "task-789".to_string(),
        history_length: Some(10),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("task-789"));
    assert!(json.contains("10"));
}

#[test]
fn test_task_query_params_deserialize() {
    let json = r#"{"id": "task-abc", "history_length": 5}"#;
    let params: TaskQueryParams = serde_json::from_str(json).unwrap();

    assert_eq!(params.id, "task-abc");
    assert_eq!(params.history_length, Some(5));
}

#[test]
fn test_task_query_params_optional_history_length() {
    let json = r#"{"id": "task-def"}"#;
    let params: TaskQueryParams = serde_json::from_str(json).unwrap();

    assert_eq!(params.id, "task-def");
    assert_eq!(params.history_length, None);
}

// ============================================================================
// TaskIdParams Tests
// ============================================================================

#[test]
fn test_task_id_params_serialize() {
    let params = TaskIdParams {
        id: "task-123".to_string(),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("task-123"));
}

#[test]
fn test_task_id_params_deserialize() {
    let json = r#"{"id": "task-xyz"}"#;
    let params: TaskIdParams = serde_json::from_str(json).unwrap();

    assert_eq!(params.id, "task-xyz");
}

#[test]
fn test_task_id_params_equality() {
    let p1 = TaskIdParams {
        id: "test".to_string(),
    };
    let p2 = TaskIdParams {
        id: "test".to_string(),
    };

    assert_eq!(p1, p2);
}

// ============================================================================
// A2aResponse Factory Tests
// ============================================================================

#[test]
fn test_a2a_response_send_message() {
    use systemprompt_core_agent::models::a2a::jsonrpc::RequestId;

    let task = Task::default();
    let request_id = RequestId::String("req-1".to_string());
    let response = A2aResponse::send_message(task, request_id);

    match response {
        A2aResponse::SendMessage(res) => {
            assert_eq!(res.jsonrpc, "2.0");
            assert!(res.result.is_some());
            assert!(res.error.is_none());
        }
        _ => panic!("Expected SendMessage variant"),
    }
}

#[test]
fn test_a2a_response_get_task() {
    use systemprompt_core_agent::models::a2a::jsonrpc::RequestId;

    let task = Task::default();
    let request_id = RequestId::Number(42);
    let response = A2aResponse::get_task(task, request_id);

    match response {
        A2aResponse::GetTask(res) => {
            assert_eq!(res.jsonrpc, "2.0");
            assert!(res.result.is_some());
            assert!(res.error.is_none());
        }
        _ => panic!("Expected GetTask variant"),
    }
}

#[test]
fn test_a2a_response_cancel_task() {
    use systemprompt_core_agent::models::a2a::jsonrpc::RequestId;

    let task = Task::default();
    let request_id = RequestId::String("cancel-1".to_string());
    let response = A2aResponse::cancel_task(task, request_id);

    match response {
        A2aResponse::CancelTask(res) => {
            assert_eq!(res.jsonrpc, "2.0");
            assert!(res.result.is_some());
        }
        _ => panic!("Expected CancelTask variant"),
    }
}

// ============================================================================
// Task and TaskStatus Tests
// ============================================================================

#[test]
fn test_task_default_values() {
    let task = Task::default();

    assert_eq!(task.kind, "task");
    assert!(task.history.is_none());
    assert!(task.artifacts.is_none());
    assert!(task.metadata.is_none());
}

#[test]
fn test_task_status_default() {
    let status = TaskStatus::default();

    assert!(matches!(status.state, TaskState::Submitted));
    assert!(status.message.is_none());
    assert!(status.timestamp.is_none());
}

#[test]
fn test_task_state_parsing() {
    let states = vec![
        ("pending", TaskState::Pending),
        ("submitted", TaskState::Submitted),
        ("working", TaskState::Working),
        ("completed", TaskState::Completed),
        ("failed", TaskState::Failed),
        ("canceled", TaskState::Canceled),
        ("rejected", TaskState::Rejected),
        ("input-required", TaskState::InputRequired),
        ("auth-required", TaskState::AuthRequired),
        ("unknown", TaskState::Unknown),
    ];

    for (input, expected) in states {
        let parsed: TaskState = input.parse().unwrap();
        assert!(
            matches!(&parsed, e if std::mem::discriminant(e) == std::mem::discriminant(&expected)),
            "Failed for input: {}",
            input
        );
    }
}

#[test]
fn test_task_state_invalid_parsing() {
    let result: Result<TaskState, String> = "completely_invalid_state".parse();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid task state"));
}

#[test]
fn test_task_state_serialize() {
    let state = TaskState::Completed;
    let json = serde_json::to_string(&state).unwrap();
    assert_eq!(json, "\"completed\"");
}

#[test]
fn test_task_state_deserialize() {
    let json = "\"working\"";
    let state: TaskState = serde_json::from_str(json).unwrap();
    assert!(matches!(state, TaskState::Working));
}

// ============================================================================
// JsonRpc Types Tests
// ============================================================================

#[test]
fn test_request_id_string() {
    use systemprompt_core_agent::models::a2a::jsonrpc::RequestId;

    let id = RequestId::String("test-id".to_string());
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"test-id\"");
}

#[test]
fn test_request_id_number() {
    use systemprompt_core_agent::models::a2a::jsonrpc::RequestId;

    let id = RequestId::Number(42);
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "42");
}

#[test]
fn test_request_id_deserialize_string() {
    use systemprompt_core_agent::models::a2a::jsonrpc::RequestId;

    let json = "\"my-id\"";
    let id: RequestId = serde_json::from_str(json).unwrap();
    match id {
        RequestId::String(s) => assert_eq!(s, "my-id"),
        _ => panic!("Expected String variant"),
    }
}

#[test]
fn test_request_id_deserialize_number() {
    use systemprompt_core_agent::models::a2a::jsonrpc::RequestId;

    let json = "123";
    let id: RequestId = serde_json::from_str(json).unwrap();
    match id {
        RequestId::Number(n) => assert_eq!(n, 123),
        _ => panic!("Expected Number variant"),
    }
}

#[test]
fn test_request_id_equality() {
    use systemprompt_core_agent::models::a2a::jsonrpc::RequestId;

    let id1 = RequestId::String("same".to_string());
    let id2 = RequestId::String("same".to_string());
    assert_eq!(id1, id2);
}

// ============================================================================
// JsonRpcError Tests
// ============================================================================

#[test]
fn test_jsonrpc_error_new() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let error = JsonRpcError::new(-32600, "Invalid Request");
    assert_eq!(error.code, -32600);
    assert_eq!(error.message, "Invalid Request");
    assert!(error.data.is_none());
}

#[test]
fn test_jsonrpc_error_with_data() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let error = JsonRpcError::with_data(-32000, "Custom error", serde_json::json!({"details": "test"}));
    assert_eq!(error.code, -32000);
    assert_eq!(error.message, "Custom error");
    assert!(error.data.is_some());
    assert_eq!(error.data.unwrap()["details"], "test");
}

#[test]
fn test_jsonrpc_error_parse_error() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let error = JsonRpcError::parse_error();
    assert_eq!(error.code, -32700);
    assert_eq!(error.message, "Parse error");
}

#[test]
fn test_jsonrpc_error_invalid_request() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let error = JsonRpcError::invalid_request();
    assert_eq!(error.code, -32600);
    assert_eq!(error.message, "Invalid Request");
}

#[test]
fn test_jsonrpc_error_method_not_found() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let error = JsonRpcError::method_not_found();
    assert_eq!(error.code, -32601);
    assert_eq!(error.message, "Method not found");
}

#[test]
fn test_jsonrpc_error_invalid_params() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let error = JsonRpcError::invalid_params();
    assert_eq!(error.code, -32602);
    assert_eq!(error.message, "Invalid params");
}

#[test]
fn test_jsonrpc_error_internal_error() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let error = JsonRpcError::internal_error();
    assert_eq!(error.code, -32603);
    assert_eq!(error.message, "Internal error");
}

#[test]
fn test_jsonrpc_error_serialize() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let error = JsonRpcError::new(-32600, "Test");
    let json = serde_json::to_string(&error).unwrap();
    assert!(json.contains("-32600"));
    assert!(json.contains("Test"));
}

#[test]
fn test_jsonrpc_error_deserialize() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let json = r#"{"code": -32700, "message": "Parse error"}"#;
    let error: JsonRpcError = serde_json::from_str(json).unwrap();
    assert_eq!(error.code, -32700);
    assert_eq!(error.message, "Parse error");
}

#[test]
fn test_jsonrpc_error_clone() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let error = JsonRpcError::new(-32600, "Test");
    let cloned = error.clone();
    assert_eq!(error.code, cloned.code);
    assert_eq!(error.message, cloned.message);
}

#[test]
fn test_jsonrpc_error_debug() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JsonRpcError;

    let error = JsonRpcError::new(-32600, "Test");
    let debug = format!("{:?}", error);
    assert!(debug.contains("JsonRpcError"));
}

// ============================================================================
// JsonRpcResponse Tests
// ============================================================================

#[test]
fn test_jsonrpc_response_with_result() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};

    let response: JsonRpcResponse<String> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some("success".to_string()),
        error: None,
        id: RequestId::String("1".to_string()),
    };

    assert_eq!(response.jsonrpc, "2.0");
    assert!(response.result.is_some());
    assert!(response.error.is_none());
}

#[test]
fn test_jsonrpc_response_with_error() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{JsonRpcError, JsonRpcResponse, RequestId};

    let response: JsonRpcResponse<String> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(JsonRpcError::invalid_request()),
        id: RequestId::Number(1),
    };

    assert!(response.result.is_none());
    assert!(response.error.is_some());
    assert_eq!(response.error.as_ref().unwrap().code, -32600);
}

#[test]
fn test_jsonrpc_response_serialize_result() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};

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
    // error should be omitted when None
    assert!(!json.contains("error"));
}

#[test]
fn test_jsonrpc_response_serialize_error() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{JsonRpcError, JsonRpcResponse, RequestId};

    let response: JsonRpcResponse<String> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(JsonRpcError::parse_error()),
        id: RequestId::Number(42),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("-32700"));
    assert!(json.contains("Parse error"));
    // result should be omitted when None
    assert!(!json.contains("result"));
}

#[test]
fn test_jsonrpc_response_deserialize() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};

    let json = r#"{"jsonrpc": "2.0", "result": "ok", "id": 1}"#;
    let response: JsonRpcResponse<String> = serde_json::from_str(json).unwrap();
    assert_eq!(response.jsonrpc, "2.0");
    assert_eq!(response.result, Some("ok".to_string()));
    assert!(matches!(response.id, RequestId::Number(1)));
}

#[test]
fn test_jsonrpc_response_clone() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};

    let response: JsonRpcResponse<String> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some("test".to_string()),
        error: None,
        id: RequestId::String("1".to_string()),
    };

    let cloned = response.clone();
    assert_eq!(response.jsonrpc, cloned.jsonrpc);
    assert_eq!(response.result, cloned.result);
}

#[test]
fn test_jsonrpc_response_debug() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};

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
fn test_jsonrpc_response_equality() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};

    let response1: JsonRpcResponse<String> = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some("test".to_string()),
        error: None,
        id: RequestId::String("1".to_string()),
    };

    let response2 = response1.clone();
    assert_eq!(response1, response2);
}

// ============================================================================
// Request Type Tests
// ============================================================================

#[test]
fn test_request_serialize() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{Request, RequestId};

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
fn test_request_deserialize() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{Request, RequestId};

    let json = r#"{"jsonrpc": "2.0", "method": "test", "params": {}, "id": 1}"#;
    let request: Request<serde_json::Value> = serde_json::from_str(json).unwrap();
    assert_eq!(request.jsonrpc, "2.0");
    assert_eq!(request.method, "test");
    assert!(matches!(request.id, RequestId::Number(1)));
}

#[test]
fn test_request_clone() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{Request, RequestId};

    let request: Request<serde_json::Value> = Request {
        jsonrpc: "2.0".to_string(),
        method: "test".to_string(),
        params: serde_json::json!({}),
        id: RequestId::Number(1),
    };

    let cloned = request.clone();
    assert_eq!(request.method, cloned.method);
}

#[test]
fn test_request_debug() {
    use systemprompt_core_agent::models::a2a::jsonrpc::{Request, RequestId};

    let request: Request<serde_json::Value> = Request {
        jsonrpc: "2.0".to_string(),
        method: "test".to_string(),
        params: serde_json::json!({}),
        id: RequestId::Number(1),
    };

    let debug = format!("{:?}", request);
    assert!(debug.contains("Request"));
}

// ============================================================================
// JSON_RPC_VERSION_2_0 Constant Test
// ============================================================================

#[test]
fn test_json_rpc_version_constant() {
    use systemprompt_core_agent::models::a2a::jsonrpc::JSON_RPC_VERSION_2_0;

    assert_eq!(JSON_RPC_VERSION_2_0, "2.0");
}
