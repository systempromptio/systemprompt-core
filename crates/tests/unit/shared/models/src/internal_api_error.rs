use systemprompt_models::api::errors::{ApiError, ErrorCode, InternalApiError};

#[test]
fn not_found_constructor_and_display() {
    let e = InternalApiError::not_found("User", "u-1");
    assert!(matches!(e.error_code(), ErrorCode::NotFound));
    let s = format!("{e}");
    assert!(s.contains("User"));
    assert!(s.contains("u-1"));
}

#[test]
fn bad_request_constructor_and_code() {
    let e = InternalApiError::bad_request("missing field");
    assert!(matches!(e.error_code(), ErrorCode::BadRequest));
    assert!(e.to_string().contains("missing field"));
}

#[test]
fn unauthorized_constructor_and_code() {
    let e = InternalApiError::unauthorized("expired token");
    assert!(matches!(e.error_code(), ErrorCode::Unauthorized));
    assert!(e.to_string().contains("expired token"));
}

#[test]
fn forbidden_constructor_and_code() {
    let e = InternalApiError::forbidden("/admin", "not admin");
    assert!(matches!(e.error_code(), ErrorCode::Forbidden));
    let s = e.to_string();
    assert!(s.contains("/admin"));
    assert!(s.contains("not admin"));
}

#[test]
fn validation_error_constructor_and_code() {
    let e = InternalApiError::validation_error("email", "invalid format");
    assert!(matches!(e.error_code(), ErrorCode::ValidationError));
    let s = e.to_string();
    assert!(s.contains("email"));
    assert!(s.contains("invalid format"));
}

#[test]
fn conflict_constructor_and_code() {
    let e = InternalApiError::conflict("user");
    assert!(matches!(e.error_code(), ErrorCode::ConflictError));
    assert!(e.to_string().contains("user"));
}

#[test]
fn rate_limited_constructor_and_code() {
    let e = InternalApiError::rate_limited("login");
    assert!(matches!(e.error_code(), ErrorCode::RateLimited));
    assert!(e.to_string().contains("login"));
}

#[test]
fn service_unavailable_constructor_and_code() {
    let e = InternalApiError::service_unavailable("storage");
    assert!(matches!(e.error_code(), ErrorCode::ServiceUnavailable));
    assert!(e.to_string().contains("storage"));
}

#[test]
fn database_error_maps_to_internal_code() {
    let e = InternalApiError::database_error("connection refused");
    assert!(matches!(e.error_code(), ErrorCode::InternalError));
}

#[test]
fn authentication_error_maps_to_internal_code() {
    let e = InternalApiError::authentication_error("bad sig");
    assert!(matches!(e.error_code(), ErrorCode::InternalError));
}

#[test]
fn internal_error_constructor_and_code() {
    let e = InternalApiError::internal_error("boom");
    assert!(matches!(e.error_code(), ErrorCode::InternalError));
    assert!(e.to_string().contains("boom"));
}

#[test]
fn from_json_error_maps_to_internal_code() {
    let json: serde_json::Error = serde_json::from_str::<i32>("not-a-num").unwrap_err();
    let e = InternalApiError::JsonError(json);
    assert!(matches!(e.error_code(), ErrorCode::InternalError));
}

#[test]
fn into_api_error_carries_details_for_not_found() {
    let e = InternalApiError::not_found("User", "u-1");
    let api: ApiError = e.into();
    assert!(matches!(api.code, ErrorCode::NotFound));
    assert!(api.details.is_some());
    assert!(api.details.unwrap().contains("u-1"));
}

#[test]
fn into_api_error_carries_details_for_validation_error() {
    let e = InternalApiError::validation_error("email", "missing @");
    let api: ApiError = e.into();
    assert!(matches!(api.code, ErrorCode::ValidationError));
    assert!(api.details.unwrap().contains("email"));
}

#[test]
fn into_api_error_carries_details_for_forbidden() {
    let e = InternalApiError::forbidden("/x", "no perms");
    let api: ApiError = e.into();
    assert!(matches!(api.code, ErrorCode::Forbidden));
    assert!(api.details.unwrap().contains("/x"));
}

#[test]
fn into_api_error_carries_details_for_database_error() {
    let e = InternalApiError::database_error("pool exhausted");
    let api: ApiError = e.into();
    assert!(matches!(api.code, ErrorCode::InternalError));
    assert!(api.details.unwrap().contains("pool exhausted"));
}

#[test]
fn into_api_error_omits_details_for_bad_request() {
    let e = InternalApiError::bad_request("missing field");
    let api: ApiError = e.into();
    assert!(matches!(api.code, ErrorCode::BadRequest));
    assert!(api.details.is_none());
}

#[test]
fn into_api_error_omits_details_for_internal_error() {
    let e = InternalApiError::internal_error("x");
    let api: ApiError = e.into();
    assert!(api.details.is_none());
}
