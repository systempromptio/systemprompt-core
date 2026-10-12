use systemprompt_agent::{
    A2aJsonRpcRequest, A2aRequestParams, A2aResponse, CancelTaskParams, GetTaskParams,
    SendMessageResult, Task, TaskState, TaskStatus,
};
use systemprompt_identifiers::TaskId;
use systemprompt_models::errors::ParseEnumError;

#[test]
fn test_parse_message_send_request() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "SendMessage".to_string(),
        params: serde_json::json!({
            "message": {
                "role": "ROLE_USER",
                "parts": [{"text": "Hello"}],
                "messageId": "msg-1",
                "contextId": "00000000-0000-4000-8000-000000000001"
            }
        }),
        id: systemprompt_agent::models::a2a::jsonrpc::RequestId::String("1".to_string()),
    };

    let parsed = request
        .parse_request()
        .expect("should parse SendMessage request");
    match parsed {
        A2aRequestParams::SendMessage(_) => {},
        _ => panic!("Expected SendMessage variant"),
    }
}

#[test]
fn test_parse_tasks_get_request() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "GetTask".to_string(),
        params: serde_json::json!({
            "id": "task-123"
        }),
        id: systemprompt_agent::models::a2a::jsonrpc::RequestId::String("2".to_string()),
    };

    let parsed = request
        .parse_request()
        .expect("should parse GetTask request");
    match parsed {
        A2aRequestParams::GetTask(params) => {
            assert_eq!(params.id, "task-123");
        },
        _ => panic!("Expected GetTask variant"),
    }
}

#[test]
fn test_parse_tasks_cancel_request() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "CancelTask".to_string(),
        params: serde_json::json!({
            "id": "task-456"
        }),
        id: systemprompt_agent::models::a2a::jsonrpc::RequestId::Number(3),
    };

    let parsed = request
        .parse_request()
        .expect("should parse CancelTask request");
    match parsed {
        A2aRequestParams::CancelTask(params) => {
            assert_eq!(params.id, "task-456");
        },
        _ => panic!("Expected CancelTask variant"),
    }
}

#[test]
fn test_parse_message_stream_request() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "SendStreamingMessage".to_string(),
        params: serde_json::json!({
            "message": {
                "role": "ROLE_USER",
                "parts": [{"text": "Stream this"}],
                "messageId": "msg-2",
                "contextId": "00000000-0000-4000-8000-000000000001"
            }
        }),
        id: systemprompt_agent::models::a2a::jsonrpc::RequestId::String("4".to_string()),
    };

    let parsed = request
        .parse_request()
        .expect("should parse SendStreamingMessage request");
    match parsed {
        A2aRequestParams::SendStreamingMessage(_) => {},
        _ => panic!("Expected SendStreamingMessage variant"),
    }
}

#[test]
fn test_parse_unsupported_method() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "unsupported/method".to_string(),
        params: serde_json::json!({}),
        id: systemprompt_agent::models::a2a::jsonrpc::RequestId::String("5".to_string()),
    };

    let error = request.parse_request().unwrap_err();
    assert!(error.to_string().contains("Unsupported method"));
}

#[test]
fn test_parse_invalid_params() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "GetTask".to_string(),
        params: serde_json::json!({"wrong": "params"}),
        id: systemprompt_agent::models::a2a::jsonrpc::RequestId::String("6".to_string()),
    };

    let error = request.parse_request().unwrap_err();
    assert!(error.to_string().contains("Invalid parameters"));
}

#[test]
fn test_get_task_params_serialize() {
    let params = GetTaskParams {
        tenant: None,
        id: TaskId::new("task-789"),
        history_length: Some(10),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("task-789"));
    assert!(json.contains("10"));
}

#[test]
fn test_get_task_params_optional_history_length() {
    let json = r#"{"id": "task-def"}"#;
    let params: GetTaskParams = serde_json::from_str(json).unwrap();

    assert_eq!(params.id, "task-def");
    assert_eq!(params.history_length, None);
}

#[test]
fn test_cancel_task_params_serialize() {
    let params = CancelTaskParams {
        tenant: None,
        id: TaskId::new("task-123"),
        metadata: None,
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("task-123"));
}

#[test]
fn test_a2a_response_send_message() {
    use systemprompt_agent::models::a2a::jsonrpc::RequestId;

    let task = Task::default();
    let request_id = RequestId::String("req-1".to_string());
    let response = A2aResponse::send_message(SendMessageResult::Task(task), request_id);

    match response {
        A2aResponse::SendMessage(res) => {
            assert_eq!(res.jsonrpc, "2.0");
            res.result
                .expect("send_message response should have result");
            assert!(res.error.is_none());
        },
        _ => panic!("Expected SendMessage variant"),
    }
}

#[test]
fn test_a2a_response_get_task() {
    use systemprompt_agent::models::a2a::jsonrpc::RequestId;

    let task = Task::default();
    let request_id = RequestId::Number(42);
    let response = A2aResponse::get_task(task, request_id);

    match response {
        A2aResponse::GetTask(res) => {
            assert_eq!(res.jsonrpc, "2.0");
            res.result.expect("get_task response should have result");
            assert!(res.error.is_none());
        },
        _ => panic!("Expected GetTask variant"),
    }
}

#[test]
fn test_a2a_response_cancel_task() {
    use systemprompt_agent::models::a2a::jsonrpc::RequestId;

    let task = Task::default();
    let request_id = RequestId::String("cancel-1".to_string());
    let response = A2aResponse::cancel_task(task, request_id);

    match response {
        A2aResponse::CancelTask(res) => {
            assert_eq!(res.jsonrpc, "2.0");
            res.result.expect("cancel_task response should have result");
        },
        _ => panic!("Expected CancelTask variant"),
    }
}

#[test]
fn test_task_default_values() {
    let task = Task::default();

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
        ("TASK_STATE_SUBMITTED", TaskState::Submitted),
        ("TASK_STATE_WORKING", TaskState::Working),
        ("TASK_STATE_COMPLETED", TaskState::Completed),
        ("TASK_STATE_FAILED", TaskState::Failed),
        ("TASK_STATE_CANCELED", TaskState::Canceled),
        ("TASK_STATE_REJECTED", TaskState::Rejected),
        ("TASK_STATE_INPUT_REQUIRED", TaskState::InputRequired),
        ("TASK_STATE_AUTH_REQUIRED", TaskState::AuthRequired),
    ];

    for (input, expected) in states {
        let parsed: TaskState = input.parse().unwrap();
        assert_eq!(parsed, expected, "Failed for input: {input}");
    }
}

#[test]
fn test_task_state_invalid_parsing() {
    let result: Result<TaskState, ParseEnumError> = "completely_invalid_state".parse();
    let error = result.unwrap_err();
    assert_eq!(error.value, "completely_invalid_state");
}
