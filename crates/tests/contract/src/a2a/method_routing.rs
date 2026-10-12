use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::protocol::{
    A2aJsonRpcRequest, A2aParseError, A2aRequestParams,
};

fn make_request(method: &str, params: serde_json::Value) -> A2aJsonRpcRequest {
    A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: method.to_string(),
        params,
        id: RequestId::Number(1),
    }
}

fn minimal_message_params() -> serde_json::Value {
    serde_json::json!({
        "message": {
            "role": "ROLE_USER",
            "parts": [{"text": "hello"}],
            "messageId": "msg-1",
            "contextId": "00000000-0000-4000-8000-000000000001"
        }
    })
}

fn task_id_params(id: &str) -> serde_json::Value {
    serde_json::json!({"id": id})
}

fn task_query_params(id: &str) -> serde_json::Value {
    serde_json::json!({"id": id})
}

#[test]
fn send_message_routes_correctly() {
    let req = make_request("SendMessage", minimal_message_params());
    let result = req.parse_request();
    assert!(matches!(result, Ok(A2aRequestParams::SendMessage(_))));
}

#[test]
fn send_streaming_message_routes_correctly() {
    let req = make_request("SendStreamingMessage", minimal_message_params());
    let result = req.parse_request();
    assert!(matches!(
        result,
        Ok(A2aRequestParams::SendStreamingMessage(_))
    ));
}

#[test]
fn get_task_routes_correctly() {
    let req = make_request("GetTask", task_query_params("task-1"));
    let result = req.parse_request();
    assert!(matches!(result, Ok(A2aRequestParams::GetTask(_))));
}

#[test]
fn cancel_task_routes_correctly() {
    let req = make_request("CancelTask", task_id_params("task-1"));
    let result = req.parse_request();
    assert!(matches!(result, Ok(A2aRequestParams::CancelTask(_))));
}

#[test]
fn get_extended_agent_card_routes_correctly() {
    let req = make_request("GetExtendedAgentCard", serde_json::json!({}));
    let result = req.parse_request();
    assert!(matches!(
        result,
        Ok(A2aRequestParams::GetExtendedAgentCard(_))
    ));
}

#[test]
fn list_tasks_routes_correctly() {
    let req = make_request("ListTasks", serde_json::json!({}));
    assert!(matches!(
        req.parse_request(),
        Ok(A2aRequestParams::ListTasks(_))
    ));
}

#[test]
fn list_tasks_accepts_null_params() {
    let req = make_request("ListTasks", serde_json::Value::Null);
    assert!(matches!(
        req.parse_request(),
        Ok(A2aRequestParams::ListTasks(_))
    ));
}

#[test]
fn subscribe_to_task_routes_by_id() {
    let req = make_request("SubscribeToTask", task_id_params("task-1"));
    let Ok(A2aRequestParams::SubscribeToTask(params)) = req.parse_request() else {
        panic!("SubscribeToTask must route");
    };
    assert_eq!(params.id.as_str(), "task-1");
}

#[test]
fn legacy_v0_3_method_names_are_not_served() {
    for method in [
        "message/send",
        "message/stream",
        "tasks/get",
        "tasks/cancel",
        "tasks/resubscribe",
    ] {
        let req = make_request(method, minimal_message_params());
        assert!(
            matches!(
                req.parse_request(),
                Err(A2aParseError::UnsupportedMethod { .. })
            ),
            "{method} must be an unsupported method"
        );
    }
}

#[test]
fn unsupported_methods_answer_32601_with_method_not_found_info() {
    use systemprompt_agent::services::a2a_server::errors::JsonRpcErrorBuilder;
    let json = serde_json::to_value(
        JsonRpcErrorBuilder::method_not_found()
            .with_metadata("method", "CreateTaskPushNotificationConfig")
            .build(&RequestId::Number(1)),
    )
    .unwrap();
    super::schema::assert_error_info_envelope(&json, -32601, "METHOD_NOT_FOUND");
}

#[test]
fn push_notification_config_methods_are_not_served() {
    for method in [
        "CreateTaskPushNotificationConfig",
        "GetTaskPushNotificationConfig",
        "ListTaskPushNotificationConfigs",
        "DeleteTaskPushNotificationConfig",
    ] {
        let req = make_request(method, serde_json::json!({"task_id": "task-1"}));
        assert!(
            matches!(
                req.parse_request(),
                Err(A2aParseError::UnsupportedMethod { method: m }) if m == method
            ),
            "{method} must be an unsupported method"
        );
    }
}

#[test]
fn unknown_method_returns_unsupported() {
    let req = make_request("NonExistentMethod", serde_json::json!({}));
    let result = req.parse_request();
    assert!(matches!(
        result,
        Err(A2aParseError::UnsupportedMethod { .. })
    ));
}

#[test]
fn empty_method_returns_unsupported() {
    let req = make_request("", serde_json::json!({}));
    let result = req.parse_request();
    assert!(matches!(
        result,
        Err(A2aParseError::UnsupportedMethod { .. })
    ));
}

#[test]
fn invalid_params_returns_error() {
    let req = make_request("SendMessage", serde_json::json!({"wrong": "params"}));
    let result = req.parse_request();
    assert!(matches!(result, Err(A2aParseError::InvalidParams { .. })));
}

#[test]
fn method_names_are_case_sensitive() {
    let lowercase = make_request("sendmessage", minimal_message_params());
    assert!(matches!(
        lowercase.parse_request(),
        Err(A2aParseError::UnsupportedMethod { .. })
    ));

    let uppercase = make_request("SENDMESSAGE", minimal_message_params());
    assert!(matches!(
        uppercase.parse_request(),
        Err(A2aParseError::UnsupportedMethod { .. })
    ));
}
