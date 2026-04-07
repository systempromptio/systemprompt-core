use proptest::prelude::*;
use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcError, JsonRpcResponse, RequestId};
use systemprompt_agent::models::a2a::protocol::A2aJsonRpcRequest;
use systemprompt_models::a2a::Task;

use super::a2a::arb_task;

pub fn arb_request_id() -> impl Strategy<Value = RequestId> {
    prop_oneof![
        "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}"
            .prop_map(RequestId::String),
        (1i64..100_000).prop_map(RequestId::Number),
    ]
}

pub fn arb_a2a_method() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("SendMessage".to_string()),
        Just("SendStreamingMessage".to_string()),
        Just("GetTask".to_string()),
        Just("CancelTask".to_string()),
        Just("GetExtendedAgentCard".to_string()),
        Just("SubscribeToTask".to_string()),
        Just("CreateTaskPushNotificationConfig".to_string()),
        Just("GetTaskPushNotificationConfig".to_string()),
        Just("ListTaskPushNotificationConfigs".to_string()),
        Just("DeleteTaskPushNotificationConfig".to_string()),
    ]
}

pub fn arb_jsonrpc_error() -> impl Strategy<Value = JsonRpcError> {
    prop_oneof![
        Just(JsonRpcError::parse_error()),
        Just(JsonRpcError::invalid_request()),
        Just(JsonRpcError::method_not_found()),
        Just(JsonRpcError::invalid_params()),
        Just(JsonRpcError::internal_error()),
        (-32099i32..-32000, "[a-zA-Z ]{1,30}").prop_map(|(code, message)| {
            JsonRpcError::new(code, message)
        }),
    ]
}

pub fn arb_jsonrpc_request() -> impl Strategy<Value = A2aJsonRpcRequest> {
    (arb_a2a_method(), arb_request_id()).prop_map(|(method, id)| A2aJsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method,
        params: serde_json::Value::Object(serde_json::Map::new()),
        id,
    })
}

pub fn arb_jsonrpc_response_task() -> impl Strategy<Value = JsonRpcResponse<Task>> {
    (arb_request_id(), proptest::option::of(arb_task())).prop_map(|(id, result)| {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result,
            error: None,
            id,
        }
    })
}

pub fn arb_jsonrpc_error_response() -> impl Strategy<Value = JsonRpcResponse<Task>> {
    (arb_request_id(), arb_jsonrpc_error()).prop_map(|(id, error)| JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(error),
        id,
    })
}
