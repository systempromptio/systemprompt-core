use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;

pub fn error_response(status: StatusCode, error: &str, description: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "error": error,
            "error_description": description
        })),
    )
        .into_response()
}

pub fn internal_error(message: &str) -> Response {
    error_response(StatusCode::INTERNAL_SERVER_ERROR, "server_error", message)
}

pub fn not_found(message: &str) -> Response {
    error_response(StatusCode::NOT_FOUND, "not_found", message)
}

pub fn bad_request(message: &str) -> Response {
    error_response(StatusCode::BAD_REQUEST, "bad_request", message)
}

pub fn single_response<T: Serialize>(data: T) -> Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "data": data
        })),
    )
        .into_response()
}

pub fn init_error(e: impl std::fmt::Display) -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "server_error",
        &format!("Repository initialization failed: {e}"),
    )
}

pub fn created_response(body: serde_json::Value, location: String) -> Response {
    (StatusCode::CREATED, [("Location", location)], Json(body)).into_response()
}
