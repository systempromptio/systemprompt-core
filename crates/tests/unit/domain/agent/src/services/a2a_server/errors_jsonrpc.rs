use systemprompt_agent::models::a2a::jsonrpc::NumberOrString;
use systemprompt_agent::services::a2a_server::errors::jsonrpc::{
    JsonRpcErrorBuilder, classify_database_error, forbidden_response, unauthorized_response,
};
use systemprompt_logging::LogLevel;
use systemprompt_traits::RepositoryError;

fn string_request_id() -> NumberOrString {
    NumberOrString::String("req-1".to_string())
}

fn number_request_id() -> NumberOrString {
    NumberOrString::Number(42)
}

#[test]
fn classify_database_error_foreign_key() {
    let error = RepositoryError::ConstraintViolation(
        "FOREIGN KEY constraint failed: tasks.agent_id".to_string(),
    );
    let result = classify_database_error(&error);
    assert!(result.contains("Referenced entity does not exist"));
    assert!(result.contains("FOREIGN KEY constraint failed"));
}

#[test]
fn classify_database_error_unique_constraint() {
    let error =
        RepositoryError::ConstraintViolation("UNIQUE constraint failed: users.email".to_string());
    let result = classify_database_error(&error);
    assert!(result.contains("Duplicate entry"));
    assert!(result.contains("UNIQUE constraint failed"));
}

#[test]
fn classify_database_error_not_null_constraint() {
    let error = RepositoryError::ConstraintViolation(
        "NOT NULL constraint failed: tasks.name".to_string(),
    );
    let result = classify_database_error(&error);
    assert!(result.contains("Required field missing"));
    assert!(result.contains("NOT NULL constraint failed"));
}

#[test]
fn classify_database_error_generic() {
    let error = RepositoryError::Database(Box::new(std::io::Error::new(
        std::io::ErrorKind::ConnectionRefused,
        "connection refused",
    )));
    let result = classify_database_error(&error);
    assert!(result.starts_with("Database error:"));
}

#[test]
fn classify_database_error_not_found() {
    let error = RepositoryError::NotFound("task xyz".to_string());
    let result = classify_database_error(&error);
    assert!(result.starts_with("Database error:"));
}

#[test]
fn classify_database_error_invalid_data() {
    let error = RepositoryError::InvalidData("bad format".to_string());
    let result = classify_database_error(&error);
    assert!(result.starts_with("Database error:"));
}

#[test]
fn builder_new_sets_code_and_message() {
    let builder = JsonRpcErrorBuilder::new(-32000, "Custom error");
    let result = builder.build(&string_request_id());
    assert_eq!(result["error"]["code"], -32000);
    assert_eq!(result["error"]["message"], "Custom error");
    assert_eq!(result["jsonrpc"], "2.0");
    assert_eq!(result["id"], "req-1");
}

#[test]
fn builder_with_data_includes_data_field() {
    let data = serde_json::json!({"detail": "something"});
    let result = JsonRpcErrorBuilder::new(-32000, "err")
        .with_data(data.clone())
        .build(&string_request_id());
    assert_eq!(result["error"]["data"], data);
}

#[test]
fn builder_without_data_omits_data_field() {
    let result = JsonRpcErrorBuilder::new(-32000, "err").build(&string_request_id());
    assert!(result["error"]["data"].is_null());
}

#[test]
fn builder_with_log_sets_message_and_level() {
    let builder = JsonRpcErrorBuilder::new(-32000, "err").with_log("log msg", LogLevel::Info);
    let result = builder.build(&string_request_id());
    assert_eq!(result["error"]["code"], -32000);
}

#[test]
fn builder_log_error_sets_error_level() {
    let builder = JsonRpcErrorBuilder::new(-32000, "err").log_error("an error");
    let result = builder.build(&string_request_id());
    assert_eq!(result["error"]["code"], -32000);
}

#[test]
fn builder_log_warn_sets_warn_level() {
    let builder = JsonRpcErrorBuilder::new(-32000, "err").log_warn("a warning");
    let result = builder.build(&string_request_id());
    assert_eq!(result["error"]["code"], -32000);
}

#[test]
fn builder_build_with_numeric_id() {
    let result = JsonRpcErrorBuilder::new(-32000, "err").build(&number_request_id());
    assert_eq!(result["id"], 42);
    assert_eq!(result["jsonrpc"], "2.0");
}

#[test]
fn factory_invalid_request_code_and_message() {
    let result = JsonRpcErrorBuilder::invalid_request().build(&string_request_id());
    assert_eq!(result["error"]["code"], -32600);
    assert_eq!(result["error"]["message"], "Invalid Request");
}

#[test]
fn factory_method_not_found_code_and_message() {
    let result = JsonRpcErrorBuilder::method_not_found().build(&string_request_id());
    assert_eq!(result["error"]["code"], -32601);
    assert_eq!(result["error"]["message"], "Method not found");
}

#[test]
fn factory_invalid_params_code_and_message() {
    let result = JsonRpcErrorBuilder::invalid_params().build(&string_request_id());
    assert_eq!(result["error"]["code"], -32602);
    assert_eq!(result["error"]["message"], "Invalid params");
}

#[test]
fn factory_internal_error_code_and_message() {
    let result = JsonRpcErrorBuilder::internal_error().build(&string_request_id());
    assert_eq!(result["error"]["code"], -32603);
    assert_eq!(result["error"]["message"], "Internal error");
}

#[test]
fn factory_parse_error_code_and_message() {
    let result = JsonRpcErrorBuilder::parse_error().build(&string_request_id());
    assert_eq!(result["error"]["code"], -32700);
    assert_eq!(result["error"]["message"], "Parse error");
}

#[test]
fn factory_unauthorized_includes_reason_data() {
    let result = JsonRpcErrorBuilder::unauthorized("bad token").build(&string_request_id());
    assert_eq!(result["error"]["code"], -32600);
    assert_eq!(result["error"]["message"], "Unauthorized");
    assert_eq!(result["error"]["data"]["reason"], "bad token");
}

#[test]
fn factory_forbidden_includes_reason_data() {
    let result = JsonRpcErrorBuilder::forbidden("access denied").build(&string_request_id());
    assert_eq!(result["error"]["code"], -32600);
    assert_eq!(result["error"]["message"], "Forbidden");
    assert_eq!(result["error"]["data"]["reason"], "access denied");
}

#[test]
fn build_with_status_invalid_request_returns_bad_request() {
    let (status, body) =
        JsonRpcErrorBuilder::invalid_request().build_with_status(&string_request_id());
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
    let (status, _) =
        JsonRpcErrorBuilder::invalid_params().build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 400);
}

#[test]
fn build_with_status_internal_error_returns_500() {
    let (status, _) =
        JsonRpcErrorBuilder::internal_error().build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 500);
}

#[test]
fn build_with_status_parse_error_returns_bad_request() {
    let (status, _) = JsonRpcErrorBuilder::parse_error().build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 400);
}

#[test]
fn build_with_status_unknown_code_returns_500() {
    let (status, _) =
        JsonRpcErrorBuilder::new(-32001, "custom").build_with_status(&string_request_id());
    assert_eq!(status.as_u16(), 500);
}

#[test]
fn unauthorized_response_returns_401_status() {
    let (status, body) = unauthorized_response("expired token", &string_request_id());
    assert_eq!(status.as_u16(), 401);
    assert_eq!(body["error"]["message"], "Unauthorized");
    assert_eq!(body["error"]["data"]["reason"], "expired token");
}

#[test]
fn forbidden_response_returns_403_status() {
    let (status, body) = forbidden_response("not allowed", &string_request_id());
    assert_eq!(status.as_u16(), 403);
    assert_eq!(body["error"]["message"], "Forbidden");
    assert_eq!(body["error"]["data"]["reason"], "not allowed");
}

#[test]
fn builder_chaining_data_then_log() {
    let data = serde_json::json!({"extra": true});
    let result = JsonRpcErrorBuilder::new(-32000, "chained")
        .with_data(data.clone())
        .log_warn("warn msg")
        .build(&string_request_id());
    assert_eq!(result["error"]["data"], data);
    assert_eq!(result["error"]["message"], "chained");
}

#[test]
fn builder_debug_impl() {
    let builder = JsonRpcErrorBuilder::new(-32600, "test");
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("JsonRpcErrorBuilder"));
    assert!(debug_str.contains("-32600"));
}

#[test]
fn builder_response_structure_is_valid_jsonrpc() {
    let result = JsonRpcErrorBuilder::internal_error().build(&string_request_id());
    assert!(result.get("jsonrpc").is_some());
    assert!(result.get("error").is_some());
    assert!(result.get("id").is_some());
    assert!(result.get("result").is_none());
}
