use systemprompt_agent::models::a2a::jsonrpc::NumberOrString;
use systemprompt_agent::services::a2a_server::errors::jsonrpc::{
    JsonRpcErrorBuilder, JsonRpcErrorResponse, forbidden_response, unauthorized_response,
};
use systemprompt_agent::services::a2a_server::errors::{A2aErrorInfo, A2aErrorReason};
use systemprompt_logging::LogLevel;

fn string_request_id() -> NumberOrString {
    NumberOrString::String("req-1".to_string())
}

fn to_json(response: JsonRpcErrorResponse) -> serde_json::Value {
    serde_json::to_value(response).expect("serialize JSON-RPC response")
}

fn number_request_id() -> NumberOrString {
    NumberOrString::Number(42)
}

#[test]
fn builder_new_sets_code_and_message() {
    let builder = JsonRpcErrorBuilder::new(-32000, "Custom error", A2aErrorReason::Internal);
    let result = to_json(builder.build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32000);
    assert_eq!(result["error"]["message"], "Custom error");
    assert_eq!(result["jsonrpc"], "2.0");
    assert_eq!(result["id"], "req-1");
}

#[test]
fn builder_with_metadata_lands_in_the_error_info() {
    let result = to_json(
        JsonRpcErrorBuilder::new(-32000, "err", A2aErrorReason::Internal)
            .with_metadata("detail", "something")
            .build(&string_request_id()),
    );
    assert_eq!(
        result["error"]["data"][0]["metadata"]["detail"],
        "something"
    );
}

#[test]
fn every_error_carries_exactly_one_error_info() {
    let result = to_json(
        JsonRpcErrorBuilder::new(-32000, "err", A2aErrorReason::Internal)
            .build(&string_request_id()),
    );
    let data = result["error"]["data"]
        .as_array()
        .expect("data is an array");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["@type"], "type.googleapis.com/google.rpc.ErrorInfo");
    assert_eq!(data[0]["reason"], "INTERNAL");
    assert_eq!(data[0]["domain"], "a2a-protocol.org");
    assert!(
        data[0].get("metadata").is_none(),
        "empty metadata is omitted"
    );
}

#[test]
fn with_error_info_replaces_the_reason() {
    let result = to_json(
        JsonRpcErrorBuilder::internal_error()
            .with_error_info(A2aErrorInfo::new(A2aErrorReason::InvalidAgentResponse))
            .build(&string_request_id()),
    );
    assert_eq!(
        result["error"]["data"][0]["reason"],
        "INVALID_AGENT_RESPONSE"
    );
}

#[test]
fn builder_with_log_sets_message_and_level() {
    let builder = JsonRpcErrorBuilder::new(-32000, "err", A2aErrorReason::Internal)
        .with_log("log msg", LogLevel::Info);
    let result = to_json(builder.build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32000);
}

#[test]
fn builder_log_error_sets_error_level() {
    let builder =
        JsonRpcErrorBuilder::new(-32000, "err", A2aErrorReason::Internal).log_error("an error");
    let result = to_json(builder.build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32000);
}

#[test]
fn builder_log_warn_sets_warn_level() {
    let builder =
        JsonRpcErrorBuilder::new(-32000, "err", A2aErrorReason::Internal).log_warn("a warning");
    let result = to_json(builder.build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32000);
}

#[test]
fn builder_build_with_numeric_id() {
    let result = to_json(
        JsonRpcErrorBuilder::new(-32000, "err", A2aErrorReason::Internal)
            .build(&number_request_id()),
    );
    assert_eq!(result["id"], 42);
    assert_eq!(result["jsonrpc"], "2.0");
}

#[test]
fn factory_invalid_request_code_and_message() {
    let result = to_json(JsonRpcErrorBuilder::invalid_request().build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32600);
    assert_eq!(result["error"]["message"], "Invalid Request");
}

#[test]
fn factory_method_not_found_code_and_message() {
    let result = to_json(JsonRpcErrorBuilder::method_not_found().build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32601);
    assert_eq!(result["error"]["message"], "Method not found");
}

#[test]
fn factory_invalid_params_code_and_message() {
    let result = to_json(JsonRpcErrorBuilder::invalid_params().build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32602);
    assert_eq!(result["error"]["message"], "Invalid params");
}

#[test]
fn factory_internal_error_code_and_message() {
    let result = to_json(JsonRpcErrorBuilder::internal_error().build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32603);
    assert_eq!(result["error"]["message"], "Internal error");
}

#[test]
fn factory_parse_error_code_and_message() {
    let result = to_json(JsonRpcErrorBuilder::parse_error().build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32700);
    assert_eq!(result["error"]["message"], "Parse error");
}

#[test]
fn factory_unauthorized_includes_reason_data() {
    let result =
        to_json(JsonRpcErrorBuilder::unauthorized("bad token").build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32600);
    assert_eq!(result["error"]["message"], "Unauthorized");
    assert_eq!(result["error"]["data"][0]["reason"], "UNAUTHENTICATED");
    assert_eq!(
        result["error"]["data"][0]["metadata"]["detail"],
        "bad token"
    );
}

#[test]
fn factory_forbidden_includes_reason_data() {
    let result =
        to_json(JsonRpcErrorBuilder::forbidden("access denied").build(&string_request_id()));
    assert_eq!(result["error"]["code"], -32600);
    assert_eq!(result["error"]["message"], "Forbidden");
    assert_eq!(result["error"]["data"][0]["reason"], "FORBIDDEN");
    assert_eq!(
        result["error"]["data"][0]["metadata"]["detail"],
        "access denied"
    );
}

#[test]
fn build_with_status_invalid_request_returns_bad_request() {
    let (status, body) =
        JsonRpcErrorBuilder::invalid_request().build_with_status(&string_request_id());
    let body = to_json(body);
    assert_eq!(status.as_u16(), 400);
    assert_eq!(body["error"]["code"], -32600);
}

#[test]
fn build_with_status_method_not_found_returns_not_found() {
    let (status, _) =
        JsonRpcErrorBuilder::method_not_found().build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 404);
}

#[test]
fn build_with_status_invalid_params_returns_bad_request() {
    let (status, _) = JsonRpcErrorBuilder::invalid_params().build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 400);
}

#[test]
fn build_with_status_internal_error_returns_500() {
    let (status, _) = JsonRpcErrorBuilder::internal_error().build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 500);
}

#[test]
fn build_with_status_parse_error_returns_bad_request() {
    let (status, _) = JsonRpcErrorBuilder::parse_error().build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 400);
}

#[test]
fn build_with_status_task_errors_follow_the_binding() {
    let task = systemprompt_identifiers::TaskId::new("t");
    let (status, _) =
        JsonRpcErrorBuilder::task_not_found(&task).build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 404);
    let (status, _) =
        JsonRpcErrorBuilder::task_not_cancelable(&task).build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 409);
    let (status, body) =
        JsonRpcErrorBuilder::version_not_supported("0.3").build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 400);
    assert_eq!(to_json(body)["error"]["code"], -32009);
}

#[test]
fn build_with_status_unknown_code_returns_500() {
    let (status, _) = JsonRpcErrorBuilder::new(-32099, "custom", A2aErrorReason::Internal)
        .build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 500);
}

#[test]
fn unauthorized_response_returns_401_status() {
    let (status, body) = unauthorized_response("expired token", &string_request_id());
    let body = to_json(body);
    assert_eq!(status.as_u16(), 401);
    assert_eq!(body["error"]["message"], "Unauthorized");
    assert_eq!(
        body["error"]["data"][0]["metadata"]["detail"],
        "expired token"
    );
}

#[test]
fn forbidden_response_returns_403_status() {
    let (status, body) = forbidden_response("not allowed", &string_request_id());
    let body = to_json(body);
    assert_eq!(status.as_u16(), 403);
    assert_eq!(body["error"]["message"], "Forbidden");
    assert_eq!(
        body["error"]["data"][0]["metadata"]["detail"],
        "not allowed"
    );
}

#[test]
fn builder_chaining_metadata_then_log() {
    let result = to_json(
        JsonRpcErrorBuilder::new(-32000, "chained", A2aErrorReason::Internal)
            .with_metadata("extra", "true")
            .log_warn("warn msg")
            .build(&string_request_id()),
    );
    assert_eq!(result["error"]["data"][0]["metadata"]["extra"], "true");
    assert_eq!(result["error"]["message"], "chained");
}

#[test]
fn builder_debug_impl() {
    let builder = JsonRpcErrorBuilder::new(-32600, "test", A2aErrorReason::InvalidRequest);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("JsonRpcErrorBuilder"));
    assert!(debug_str.contains("-32600"));
}

#[test]
fn builder_response_structure_is_valid_jsonrpc() {
    let result = to_json(JsonRpcErrorBuilder::internal_error().build(&string_request_id()));
    assert!(result.get("jsonrpc").is_some());
    assert!(result.get("error").is_some());
    assert!(result.get("id").is_some());
    assert!(result.get("result").is_none());
}
