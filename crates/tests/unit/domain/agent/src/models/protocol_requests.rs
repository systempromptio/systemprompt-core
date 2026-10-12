use serde_json::json;
use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::protocol::{
    A2aJsonRpcRequest, A2aParseError, A2aRequestParams, A2aResponse, CancelTaskParams,
    GetTaskParams, ListTasksParams, ListTasksResult, SendMessageConfiguration, SendMessageParams,
    SendMessageResult, TaskNotCancelableError, TaskNotFoundError, UnsupportedOperationError,
};
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::a2a::methods;

fn string_id(s: &str) -> RequestId {
    RequestId::String(s.to_string())
}

fn minimal_message() -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::text("hello")],
        message_id: MessageId::new("msg-test"),
        context_id: ContextId::try_new("00000000-0000-4000-8000-000000000001")
            .expect("valid ContextId"),
        task_id: None,
        reference_task_ids: None,
        metadata: None,
        extensions: None,
    }
}

fn send_message_request(id: RequestId) -> A2aJsonRpcRequest {
    let params = SendMessageParams {
        tenant: None,
        message: minimal_message(),
        configuration: None,
        metadata: None,
    };
    A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::SEND_MESSAGE.to_string(),
        params: serde_json::to_value(&params).unwrap(),
        id,
    }
}

#[test]
fn parse_request_send_message_success() {
    let req = send_message_request(string_id("req-1"));
    let parsed = req.parse_request().expect("should parse");
    assert!(matches!(parsed, A2aRequestParams::SendMessage(_)));
}

#[test]
fn parse_request_get_task_success() {
    let params = GetTaskParams {
        tenant: None,
        id: TaskId::new("task-1"),
        history_length: None,
    };
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::GET_TASK.to_string(),
        params: serde_json::to_value(&params).unwrap(),
        id: string_id("req-2"),
    };
    let parsed = req.parse_request().expect("should parse");
    assert!(matches!(parsed, A2aRequestParams::GetTask(_)));
}

#[test]
fn parse_request_cancel_task_success() {
    let params = CancelTaskParams {
        tenant: None,
        id: TaskId::new("task-2"),
        metadata: None,
    };
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::CANCEL_TASK.to_string(),
        params: serde_json::to_value(&params).unwrap(),
        id: string_id("req-3"),
    };
    let parsed = req.parse_request().expect("should parse");
    assert!(matches!(parsed, A2aRequestParams::CancelTask(_)));
}

#[test]
fn parse_request_get_extended_card_success() {
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::GET_EXTENDED_AGENT_CARD.to_string(),
        params: json!({}),
        id: string_id("req-4"),
    };
    let parsed = req.parse_request().expect("should parse");
    assert!(matches!(parsed, A2aRequestParams::GetExtendedAgentCard(_)));
}

#[test]
fn parse_request_send_streaming_message_success() {
    let params = SendMessageParams {
        tenant: None,
        message: minimal_message(),
        configuration: None,
        metadata: None,
    };
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::SEND_STREAMING_MESSAGE.to_string(),
        params: serde_json::to_value(&params).unwrap(),
        id: string_id("req-5"),
    };
    let parsed = req.parse_request().expect("should parse");
    assert!(matches!(parsed, A2aRequestParams::SendStreamingMessage(_)));
}

#[test]
fn parse_request_unsupported_method_error() {
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "unknown/method".to_string(),
        params: json!({}),
        id: string_id("req-99"),
    };
    let err = req.parse_request().unwrap_err();
    assert!(matches!(err, A2aParseError::UnsupportedMethod { .. }));
    assert!(err.to_string().contains("unknown/method"));
}

#[test]
fn parse_request_invalid_params_error() {
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::GET_TASK.to_string(),
        params: json!({"wrong_field": "value"}),
        id: string_id("req-bad"),
    };
    let err = req.parse_request().unwrap_err();
    assert!(matches!(err, A2aParseError::InvalidParams { .. }));
}

#[test]
fn a2a_parse_error_unsupported_method_display() {
    let err = A2aParseError::UnsupportedMethod {
        method: "foo/bar".to_string(),
    };
    assert!(err.to_string().contains("Unsupported method"));
    assert!(err.to_string().contains("foo/bar"));
}

#[test]
fn a2a_parse_error_invalid_params_display() {
    let source = serde_json::from_value::<GetTaskParams>(json!({})).expect_err("id is required");
    let err = A2aParseError::InvalidParams {
        method: "GetTask".to_string(),
        source: std::sync::Arc::new(source),
    };
    assert!(err.to_string().contains("Invalid parameters"));
    assert!(err.to_string().contains("GetTask"));
    assert!(err.to_string().contains("missing field"));
    assert!(std::error::Error::source(&err).is_some());
}


#[test]
fn a2a_response_send_message_constructor() {
    let task = systemprompt_agent::Task::default();
    let resp =
        A2aResponse::send_message(SendMessageResult::Task(Box::new(task)), string_id("id-1"));
    assert!(matches!(resp, A2aResponse::SendMessage(_)));
}

#[test]
fn a2a_response_get_task_constructor() {
    let task = systemprompt_agent::Task::default();
    let resp = A2aResponse::get_task(task, string_id("id-2"));
    assert!(matches!(resp, A2aResponse::GetTask(_)));
}

#[test]
fn a2a_response_cancel_task_constructor() {
    let task = systemprompt_agent::Task::default();
    let resp = A2aResponse::cancel_task(task, string_id("id-3"));
    assert!(matches!(resp, A2aResponse::CancelTask(_)));
}

#[test]
fn message_send_configuration_serde_roundtrip() {
    let config = SendMessageConfiguration {
        accepted_output_modes: Some(vec!["text/plain".to_string()]),
        history_length: Some(10),
        return_immediately: true,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: SendMessageConfiguration = serde_json::from_str(&json).unwrap();
    assert_eq!(de.history_length, Some(10));
    assert!(de.return_immediately);
}

#[test]
fn message_send_configuration_all_none_serde() {
    let config = SendMessageConfiguration {
        accepted_output_modes: None,
        history_length: None,
        return_immediately: false,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: SendMessageConfiguration = serde_json::from_str(&json).unwrap();
    assert!(de.accepted_output_modes.is_none());
    assert!(!de.return_immediately);
}

#[test]
fn task_not_found_error_serde_roundtrip() {
    let err = TaskNotFoundError {
        task_id: TaskId::new("task-missing"),
        message: "Task not found".to_string(),
        code: -32001,
        data: json!({"detail": "expired"}),
    };
    let json = serde_json::to_string(&err).unwrap();
    let de: TaskNotFoundError = serde_json::from_str(&json).unwrap();
    assert_eq!(de.task_id.as_str(), "task-missing");
    assert_eq!(de.code, -32001);
}


#[test]
fn task_not_cancelable_error_serde_roundtrip() {
    let err = TaskNotCancelableError {
        task_id: TaskId::new("task-done"),
        state: TaskState::Completed,
        message: "Cannot cancel completed task".to_string(),
        code: -32002,
        data: json!(null),
    };
    let json = serde_json::to_string(&err).unwrap();
    let de: TaskNotCancelableError = serde_json::from_str(&json).unwrap();
    assert_eq!(de.task_id.as_str(), "task-done");
    assert_eq!(de.code, -32002);
}


#[test]
fn unsupported_operation_error_serde_roundtrip() {
    let err = UnsupportedOperationError {
        operation: "streaming".to_string(),
        message: "Streaming not supported".to_string(),
        code: -32003,
        data: json!({"reason": "no_streaming"}),
    };
    let json = serde_json::to_string(&err).unwrap();
    let de: UnsupportedOperationError = serde_json::from_str(&json).unwrap();
    assert_eq!(de.operation, "streaming");
    assert_eq!(de.code, -32003);
}

#[test]
fn message_send_params_with_configuration() {
    let config = SendMessageConfiguration {
        accepted_output_modes: Some(vec!["text/plain".to_string(), "text/markdown".to_string()]),
        history_length: Some(5),
        return_immediately: false,
    };
    let params = SendMessageParams {
        tenant: None,
        message: minimal_message(),
        configuration: Some(config),
        metadata: None,
    };
    let json = serde_json::to_string(&params).unwrap();
    let de: SendMessageParams = serde_json::from_str(&json).unwrap();
    let cfg = de.configuration.unwrap();
    assert_eq!(cfg.history_length, Some(5));
    assert!(!cfg.return_immediately);
}

#[test]
fn cancel_task_params_serde_roundtrip() {
    let params = CancelTaskParams {
        tenant: Some("acme".to_string()),
        id: TaskId::new("task-cancel"),
        metadata: None,
    };
    let value = serde_json::to_value(&params).unwrap();
    assert_eq!(value, json!({"tenant": "acme", "id": "task-cancel"}));
    let de: CancelTaskParams = serde_json::from_value(value).unwrap();
    assert_eq!(de.id.as_str(), "task-cancel");
}

#[test]
fn get_task_params_with_history_length() {
    let params = GetTaskParams {
        tenant: None,
        id: TaskId::new("task-query"),
        history_length: Some(20),
    };
    let value = serde_json::to_value(&params).unwrap();
    assert_eq!(value, json!({"id": "task-query", "historyLength": 20}));
    let de: GetTaskParams = serde_json::from_value(value).unwrap();
    assert_eq!(de.history_length, Some(20));
}

#[test]
fn send_message_configuration_wire_names() {
    let value = serde_json::to_value(SendMessageConfiguration {
        accepted_output_modes: Some(vec!["text/plain".to_string()]),
        history_length: Some(3),
        return_immediately: true,
    })
    .unwrap();
    assert_eq!(
        value,
        json!({"acceptedOutputModes": ["text/plain"], "historyLength": 3, "returnImmediately": true})
    );
    let de: SendMessageConfiguration = serde_json::from_value(json!({})).unwrap();
    assert!(!de.return_immediately);
}

#[test]
fn parse_subscribe_to_task_method() {
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::SUBSCRIBE_TO_TASK.to_string(),
        params: json!({"id": "task-sub", "tenant": "acme"}),
        id: string_id("req-sub"),
    };
    let parsed = req.parse_request().expect("should parse subscribe");
    assert_eq!(parsed.tenant(), Some("acme"));
    let A2aRequestParams::SubscribeToTask(params) = parsed else {
        panic!("expected SubscribeToTask");
    };
    assert_eq!(params.id.as_str(), "task-sub");
}

#[test]
fn parse_subscribe_to_task_rejects_legacy_task_id_field() {
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::SUBSCRIBE_TO_TASK.to_string(),
        params: json!({"task_id": "task-sub"}),
        id: string_id("req-sub"),
    };
    assert!(matches!(
        req.parse_request().unwrap_err(),
        A2aParseError::InvalidParams { .. }
    ));
}

#[test]
fn parse_list_tasks_method() {
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::LIST_TASKS.to_string(),
        params: json!({
            "contextId": "00000000-0000-4000-8000-000000000001",
            "status": "TASK_STATE_WORKING",
            "pageSize": 10,
            "pageToken": "abc",
            "includeArtifacts": true
        }),
        id: string_id("req-list"),
    };
    let A2aRequestParams::ListTasks(params) = req.parse_request().expect("should parse list")
    else {
        panic!("expected ListTasks");
    };
    assert_eq!(params.status, Some(TaskState::Working));
    assert_eq!(params.page_size, Some(10));
    assert_eq!(params.page_token.as_deref(), Some("abc"));
    assert_eq!(params.include_artifacts, Some(true));
}

#[test]
fn parse_list_tasks_accepts_absent_params() {
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::LIST_TASKS.to_string(),
        params: serde_json::Value::Null,
        id: string_id("req-list"),
    };
    assert_eq!(
        req.parse_request().expect("should parse"),
        A2aRequestParams::ListTasks(ListTasksParams::default())
    );
}

#[test]
fn list_tasks_response_constructor_serialises_spec_fields() {
    let resp = A2aResponse::list_tasks(
        ListTasksResult {
            tasks: vec![],
            next_page_token: String::new(),
            page_size: 50,
            total_size: 0,
        },
        string_id("id-list"),
    );
    let value = serde_json::to_value(resp).unwrap();
    assert_eq!(value["jsonrpc"], "2.0");
    assert_eq!(
        value["result"],
        json!({"tasks": [], "nextPageToken": "", "pageSize": 50, "totalSize": 0})
    );
}

#[test]
fn send_message_response_wraps_result_oneof() {
    let task = systemprompt_agent::Task::default();
    let value = serde_json::to_value(A2aResponse::send_message(
        SendMessageResult::Task(Box::new(task)),
        string_id("id-sm"),
    ))
    .unwrap();
    assert!(value["result"]["task"].is_object());
}
