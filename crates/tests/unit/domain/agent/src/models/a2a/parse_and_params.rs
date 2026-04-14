use systemprompt_agent::{
    A2aJsonRpcRequest, A2aRequestParams, A2aResponse, Task, TaskIdParams, TaskQueryParams,
    TaskState, TaskStatus,
};

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
                "contextId": "ctx-1"
            }
        }),
        id: systemprompt_agent::models::a2a::jsonrpc::RequestId::String("1".to_string()),
    };

    let parsed = request.parse_request().expect("should parse SendMessage request");
    match parsed {
        A2aRequestParams::SendMessage(_) => {}
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

    let parsed = request.parse_request().expect("should parse GetTask request");
    match parsed {
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
        method: "CancelTask".to_string(),
        params: serde_json::json!({
            "id": "task-456"
        }),
        id: systemprompt_agent::models::a2a::jsonrpc::RequestId::Number(3),
    };

    let parsed = request.parse_request().expect("should parse CancelTask request");
    match parsed {
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
        method: "SendStreamingMessage".to_string(),
        params: serde_json::json!({
            "message": {
                "role": "ROLE_USER",
                "parts": [{"text": "Stream this"}],
                "messageId": "msg-2",
                "contextId": "ctx-2"
            }
        }),
        id: systemprompt_agent::models::a2a::jsonrpc::RequestId::String("4".to_string()),
    };

    let parsed = request.parse_request().expect("should parse SendStreamingMessage request");
    match parsed {
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
fn test_task_query_params_optional_history_length() {
    let json = r#"{"id": "task-def"}"#;
    let params: TaskQueryParams = serde_json::from_str(json).unwrap();

    assert_eq!(params.id, "task-def");
    assert_eq!(params.history_length, None);
}

#[test]
fn test_task_id_params_serialize() {
    let params = TaskIdParams {
        id: "task-123".to_string(),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("task-123"));
}

#[test]
fn test_a2a_response_send_message() {
    use systemprompt_agent::models::a2a::jsonrpc::RequestId;

    let task = Task::default();
    let request_id = RequestId::String("req-1".to_string());
    let response = A2aResponse::send_message(task, request_id);

    match response {
        A2aResponse::SendMessage(res) => {
            assert_eq!(res.jsonrpc, "2.0");
            res.result.expect("send_message response should have result");
            assert!(res.error.is_none());
        }
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
        }
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
        }
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
    let error = result.unwrap_err();
    assert!(error.contains("Invalid task state"));
}

