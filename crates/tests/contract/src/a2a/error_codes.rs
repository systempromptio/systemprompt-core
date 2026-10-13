use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcError, RequestId};
use systemprompt_agent::services::a2a_server::errors::{
    A2aErrorInfo, A2aErrorReason, JsonRpcErrorBuilder, forbidden_response, unauthorized_response,
};
use systemprompt_identifiers::TaskId;

use super::schema::assert_error_info_envelope;

fn envelope(builder: JsonRpcErrorBuilder) -> serde_json::Value {
    serde_json::to_value(builder.build(&RequestId::Number(7))).unwrap()
}

#[test]
fn standard_jsonrpc_codes() {
    assert_eq!(JsonRpcError::parse_error().code, -32700);
    assert_eq!(JsonRpcError::invalid_request().code, -32600);
    assert_eq!(JsonRpcError::method_not_found().code, -32601);
    assert_eq!(JsonRpcError::invalid_params().code, -32602);
    assert_eq!(JsonRpcError::internal_error().code, -32603);
}

#[test]
fn every_builder_carries_its_error_info() {
    let task = TaskId::new("task-1");
    let cases = [
        (
            JsonRpcErrorBuilder::parse_error(),
            -32700,
            "INVALID_REQUEST",
        ),
        (
            JsonRpcErrorBuilder::invalid_request(),
            -32600,
            "INVALID_REQUEST",
        ),
        (
            JsonRpcErrorBuilder::method_not_found(),
            -32601,
            "METHOD_NOT_FOUND",
        ),
        (
            JsonRpcErrorBuilder::invalid_params(),
            -32602,
            "INVALID_PARAMS",
        ),
        (JsonRpcErrorBuilder::internal_error(), -32603, "INTERNAL"),
        (
            JsonRpcErrorBuilder::task_not_found(&task),
            -32001,
            "TASK_NOT_FOUND",
        ),
        (
            JsonRpcErrorBuilder::task_not_cancelable(&task),
            -32002,
            "TASK_NOT_CANCELABLE",
        ),
        (
            JsonRpcErrorBuilder::push_notification_not_supported(),
            -32003,
            "PUSH_NOTIFICATION_NOT_SUPPORTED",
        ),
        (
            JsonRpcErrorBuilder::unsupported_operation(),
            -32004,
            "UNSUPPORTED_OPERATION",
        ),
        (
            JsonRpcErrorBuilder::extended_agent_card_not_configured(),
            -32007,
            "EXTENDED_AGENT_CARD_NOT_CONFIGURED",
        ),
        (
            JsonRpcErrorBuilder::version_not_supported("0.3"),
            -32009,
            "VERSION_NOT_SUPPORTED",
        ),
        (
            JsonRpcErrorBuilder::unauthorized("no token"),
            -32600,
            "UNAUTHENTICATED",
        ),
        (
            JsonRpcErrorBuilder::forbidden("wrong audience"),
            -32600,
            "FORBIDDEN",
        ),
    ];
    for (builder, code, reason) in cases {
        let json = envelope(builder);
        assert_error_info_envelope(&json, code, reason);
        assert_eq!(json["id"], 7);
    }
}

#[test]
fn task_errors_name_the_task_in_metadata() {
    let json = envelope(JsonRpcErrorBuilder::task_not_found(&TaskId::new("task-9")));
    assert_eq!(json["error"]["data"][0]["metadata"]["taskId"], "task-9");
}

#[test]
fn a2a_specific_codes_map_to_spec_reasons() {
    let table = [
        (
            -32005,
            A2aErrorReason::ContentTypeNotSupported,
            "CONTENT_TYPE_NOT_SUPPORTED",
        ),
        (
            -32006,
            A2aErrorReason::InvalidAgentResponse,
            "INVALID_AGENT_RESPONSE",
        ),
        (
            -32008,
            A2aErrorReason::ExtensionSupportRequired,
            "EXTENSION_SUPPORT_REQUIRED",
        ),
    ];
    for (code, reason, wire) in table {
        let json = envelope(JsonRpcErrorBuilder::new(code, "error", reason));
        assert_error_info_envelope(&json, i64::from(code), wire);
    }
}

#[test]
fn http_status_follows_the_a2a_binding() {
    use http::StatusCode;
    let task = TaskId::new("t");
    assert_eq!(
        JsonRpcErrorBuilder::task_not_found(&task).http_status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        JsonRpcErrorBuilder::task_not_cancelable(&task).http_status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        JsonRpcErrorBuilder::version_not_supported("").http_status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        JsonRpcErrorBuilder::invalid_params().http_status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        JsonRpcErrorBuilder::internal_error().http_status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn with_error_info_replaces_the_detail() {
    let info = A2aErrorInfo::new(A2aErrorReason::Forbidden).with_metadata("scope", "a2a");
    let json = envelope(JsonRpcErrorBuilder::internal_error().with_error_info(info));
    assert_error_info_envelope(&json, -32603, "FORBIDDEN");
    assert_eq!(json["error"]["data"][0]["metadata"]["scope"], "a2a");
}

#[test]
fn auth_shortcuts_set_status_and_error_info() {
    let (status, body) = unauthorized_response("Bearer token required", &RequestId::Number(1));
    assert_eq!(status, http::StatusCode::UNAUTHORIZED);
    let json = serde_json::to_value(body).unwrap();
    assert_error_info_envelope(&json, -32600, "UNAUTHENTICATED");
    assert_eq!(
        json["error"]["data"][0]["metadata"]["detail"],
        "Bearer token required"
    );

    let (status, body) = forbidden_response("audience", &RequestId::Number(1));
    assert_eq!(status, http::StatusCode::FORBIDDEN);
    assert_error_info_envelope(&serde_json::to_value(body).unwrap(), -32600, "FORBIDDEN");
}

#[test]
fn unidentified_errors_serialise_a_null_id() {
    let json =
        serde_json::to_value(JsonRpcErrorBuilder::parse_error().build_unidentified()).unwrap();
    assert!(json["id"].is_null());
    assert!(json.as_object().unwrap().contains_key("id"));
    assert_error_info_envelope(&json, -32700, "INVALID_REQUEST");
}
