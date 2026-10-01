use systemprompt_models::api::errors::{ApiError, ErrorCode};

#[test]
fn constructors_carry_their_error_codes() {
    for (error, code) in [
        (ApiError::not_found("user u-1"), ErrorCode::NotFound),
        (ApiError::bad_request("missing field"), ErrorCode::BadRequest),
        (ApiError::unauthorized("expired token"), ErrorCode::Unauthorized),
        (ApiError::forbidden("not admin"), ErrorCode::Forbidden),
        (ApiError::conflict("user exists"), ErrorCode::ConflictError),
        (ApiError::rate_limited("login"), ErrorCode::RateLimited),
        (
            ApiError::validation_error("invalid", Vec::new()),
            ErrorCode::ValidationError,
        ),
        (
            ApiError::service_unavailable("storage"),
            ErrorCode::ServiceUnavailable,
        ),
        (ApiError::internal_error("boom"), ErrorCode::InternalError),
    ] {
        assert_eq!(error.code, code, "{}", error.message);
    }
}

#[test]
fn an_authentication_failure_answers_401() {
    let error = ApiError::unauthorized("bad signature");
    assert_eq!(error.code.status_code(), 401);
}

#[test]
fn only_internal_and_unavailable_are_server_errors() {
    assert!(ErrorCode::InternalError.is_server_error());
    assert!(ErrorCode::ServiceUnavailable.is_server_error());
    for code in [
        ErrorCode::NotFound,
        ErrorCode::BadRequest,
        ErrorCode::Unauthorized,
        ErrorCode::Forbidden,
        ErrorCode::ValidationError,
        ErrorCode::ConflictError,
        ErrorCode::RateLimited,
    ] {
        assert!(!code.is_server_error(), "{code:?}");
    }
}

#[test]
fn a_client_error_body_carries_message_and_details() {
    let error = ApiError::not_found("user u-1").with_details("checked the primary");
    let body = serde_json::to_value(&error).unwrap();
    assert_eq!(body["code"], "not_found");
    assert_eq!(body["message"], "user u-1");
    assert_eq!(body["details"], "checked the primary");
}

#[test]
fn a_server_error_body_is_redacted_to_the_public_message() {
    let json: serde_json::Error = serde_json::from_str::<i32>("not-a-num").unwrap_err();
    let error = ApiError::internal("Decoding failed", json)
        .with_details("pool exhausted at 10.0.0.4")
        .with_error_key("decode_failed");
    let body = serde_json::to_value(&error).unwrap();
    assert_eq!(body["message"], "Internal server error");
    assert!(body.get("details").is_none(), "{body}");
    assert_eq!(body["error_key"], "decode_failed");
    assert!(error.source().is_some(), "the cause stays available for logging");

    let unavailable = serde_json::to_value(ApiError::service_unavailable("vault down")).unwrap();
    assert_eq!(unavailable["message"], "Service temporarily unavailable");
}

#[test]
fn a_server_error_round_trips_through_the_wire_shape() {
    let body = serde_json::to_string(&ApiError::internal_error("context")).unwrap();
    let parsed: ApiError = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed.code, ErrorCode::InternalError);
    assert_eq!(parsed.message, "Internal server error");
    assert!(parsed.source().is_none());
}

#[derive(Debug)]
struct QuotaError(&'static str);

impl std::fmt::Display for QuotaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "quota: {}", self.0)
    }
}

impl std::error::Error for QuotaError {}

impl systemprompt_traits::ExtensionError for QuotaError {
    fn code(&self) -> &'static str {
        "QUOTA"
    }

    fn status(&self) -> http::StatusCode {
        if self.0 == "backend" {
            http::StatusCode::INTERNAL_SERVER_ERROR
        } else {
            http::StatusCode::TOO_MANY_REQUESTS
        }
    }
}

#[test]
fn an_extension_error_renders_its_status_code_and_message() {
    let error = ApiError::from_extension(QuotaError("daily limit"));
    assert_eq!(error.code, ErrorCode::RateLimited);
    assert_eq!(error.error_key.as_deref(), Some("QUOTA"));
    assert_eq!(error.message, "quota: daily limit");
}

#[test]
fn an_extension_server_error_keeps_its_text_out_of_the_body() {
    let error = ApiError::from_extension(QuotaError("backend"));
    assert_eq!(error.code, ErrorCode::InternalError);
    let body = serde_json::to_string(&error).unwrap();
    assert!(!body.contains("backend"), "{body}");
    assert!(error.source().is_some());
}
