use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::protocol::{A2aJsonRpcRequest, A2aParseError, A2aRequestParams};

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
            "contextId": "ctx-1"
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
        Ok(A2aRequestParams::GetAuthenticatedExtendedCard(_))
    ));
}

#[test]
fn subscribe_to_task_routes_correctly() {
    let req = make_request(
        "SubscribeToTask",
        serde_json::json!({
            "task_id": "task-1",
            "config": {
                "url": "https://example.com/webhook"
            }
        }),
    );
    let result = req.parse_request();
    assert!(matches!(
        result,
        Ok(A2aRequestParams::TaskResubscription(_))
    ));
}

#[test]
fn create_push_notification_config_routes_correctly() {
    let req = make_request(
        "CreateTaskPushNotificationConfig",
        serde_json::json!({
            "task_id": "task-1",
            "config": {
                "url": "https://example.com/webhook"
            }
        }),
    );
    let result = req.parse_request();
    assert!(matches!(
        result,
        Ok(A2aRequestParams::SetTaskPushNotificationConfig(_))
    ));
}

#[test]
fn get_push_notification_config_routes_correctly() {
    let req = make_request(
        "GetTaskPushNotificationConfig",
        serde_json::json!({"task_id": "task-1"}),
    );
    let result = req.parse_request();
    assert!(matches!(
        result,
        Ok(A2aRequestParams::GetTaskPushNotificationConfig(_))
    ));
}

#[test]
fn list_push_notification_configs_routes_correctly() {
    let req = make_request(
        "ListTaskPushNotificationConfigs",
        serde_json::json!({"task_id": "task-1"}),
    );
    let result = req.parse_request();
    assert!(matches!(
        result,
        Ok(A2aRequestParams::ListTaskPushNotificationConfig(_))
    ));
}

#[test]
fn delete_push_notification_config_routes_correctly() {
    let req = make_request(
        "DeleteTaskPushNotificationConfig",
        serde_json::json!({"task_id": "task-1"}),
    );
    let result = req.parse_request();
    assert!(matches!(
        result,
        Ok(A2aRequestParams::DeleteTaskPushNotificationConfig(_))
    ));
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
    assert!(matches!(
        result,
        Err(A2aParseError::InvalidParams { .. })
    ));
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
