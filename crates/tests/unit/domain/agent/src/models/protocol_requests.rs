use serde_json::json;
use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::protocol::{
    A2aJsonRpcRequest, A2aParseError, A2aRequestParams, A2aResponse, MessageSendConfiguration,
    MessageSendParams, PushNotificationConfig, TaskIdParams, TaskNotCancelableError,
    TaskNotFoundError, TaskQueryParams, UnsupportedOperationError,
};
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState, TextPart};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::a2a::methods;

fn string_id(s: &str) -> RequestId {
    RequestId::String(s.to_string())
}

fn minimal_message() -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: "hello".to_string(),
        })],
        message_id: MessageId::new("msg-test"),
        context_id: ContextId::new("00000000-0000-4000-8000-000000000001"),
        task_id: None,
        reference_task_ids: None,
        metadata: None,
        extensions: None,
    }
}

fn send_message_request(id: RequestId) -> A2aJsonRpcRequest {
    let params = MessageSendParams {
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
    let params = TaskQueryParams {
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
    let params = TaskIdParams {
        id: TaskId::new("task-2"),
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
    assert!(matches!(
        parsed,
        A2aRequestParams::GetAuthenticatedExtendedCard(_)
    ));
}

#[test]
fn parse_request_send_streaming_message_success() {
    let params = MessageSendParams {
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
    let err = A2aParseError::InvalidParams {
        method: "tasks/get".to_string(),
        error: "missing field `id`".to_string(),
    };
    assert!(err.to_string().contains("Invalid parameters"));
    assert!(err.to_string().contains("tasks/get"));
    assert!(err.to_string().contains("missing field"));
}

#[test]
fn a2a_parse_error_clone_and_eq() {
    let err1 = A2aParseError::UnsupportedMethod {
        method: "a/b".to_string(),
    };
    let err2 = err1.clone();
    assert_eq!(err1, err2);
}

#[test]
fn a2a_response_send_message_constructor() {
    let task = systemprompt_agent::Task::default();
    let resp = A2aResponse::send_message(task, string_id("id-1"));
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
    let config = MessageSendConfiguration {
        accepted_output_modes: Some(vec!["text/plain".to_string()]),
        history_length: Some(10),
        push_notification_config: None,
        blocking: Some(true),
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: MessageSendConfiguration = serde_json::from_str(&json).unwrap();
    assert_eq!(de.history_length, Some(10));
    assert_eq!(de.blocking, Some(true));
    assert!(de.push_notification_config.is_none());
}

#[test]
fn message_send_configuration_all_none_serde() {
    let config = MessageSendConfiguration {
        accepted_output_modes: None,
        history_length: None,
        push_notification_config: None,
        blocking: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: MessageSendConfiguration = serde_json::from_str(&json).unwrap();
    assert!(de.accepted_output_modes.is_none());
    assert!(de.blocking.is_none());
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
fn task_not_found_error_clone_and_eq() {
    let err = TaskNotFoundError {
        task_id: TaskId::new("t-1"),
        message: "not found".to_string(),
        code: -32001,
        data: json!(null),
    };
    let cloned = err.clone();
    assert_eq!(err, cloned);
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
fn task_not_cancelable_error_debug() {
    let err = TaskNotCancelableError {
        task_id: TaskId::new("t-2"),
        state: TaskState::Canceled,
        message: "Already canceled".to_string(),
        code: -32002,
        data: json!({}),
    };
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("TaskNotCancelableError"));
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
    let config = MessageSendConfiguration {
        accepted_output_modes: Some(vec!["text/plain".to_string(), "text/markdown".to_string()]),
        history_length: Some(5),
        push_notification_config: Some(PushNotificationConfig {
            url: "https://hook.example.com".to_string(),
            token: None,
            authentication: None,
            endpoint: String::new(),
            headers: None,
        }),
        blocking: Some(false),
    };
    let params = MessageSendParams {
        message: minimal_message(),
        configuration: Some(config),
        metadata: None,
    };
    let json = serde_json::to_string(&params).unwrap();
    let de: MessageSendParams = serde_json::from_str(&json).unwrap();
    let cfg = de.configuration.unwrap();
    assert_eq!(cfg.history_length, Some(5));
    let push = cfg
        .push_notification_config
        .expect("push config round-trips");
    assert_eq!(push.url, "https://hook.example.com");
}

#[test]
fn task_id_params_serde_roundtrip() {
    let params = TaskIdParams {
        id: TaskId::new("task-cancel"),
    };
    let json = serde_json::to_string(&params).unwrap();
    let de: TaskIdParams = serde_json::from_str(&json).unwrap();
    assert_eq!(de.id.as_str(), "task-cancel");
}

#[test]
fn task_query_params_with_history_length() {
    let params = TaskQueryParams {
        id: TaskId::new("task-query"),
        history_length: Some(20),
    };
    let json = serde_json::to_string(&params).unwrap();
    let de: TaskQueryParams = serde_json::from_str(&json).unwrap();
    assert_eq!(de.id.as_str(), "task-query");
    assert_eq!(de.history_length, Some(20));
}

#[test]
fn a2a_jsonrpc_request_debug_and_clone() {
    let req = send_message_request(string_id("dbg-1"));
    let cloned = req.clone();
    let debug_str = format!("{:?}", req);
    assert!(debug_str.contains("A2aJsonRpcRequest"));
    assert_eq!(cloned.method, req.method);
}

#[test]
fn parse_subscribe_to_task_method() {
    let params = json!({
        "task_id": "task-sub",
        "config": {
            "url": "https://hook.example.com",
            "endpoint": ""
        }
    });
    let req = A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: methods::SUBSCRIBE_TO_TASK.to_string(),
        params,
        id: string_id("req-sub"),
    };
    let parsed = req.parse_request().expect("should parse subscribe");
    assert!(matches!(parsed, A2aRequestParams::TaskResubscription(_)));
}
