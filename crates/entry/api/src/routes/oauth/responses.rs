use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;

pub fn single_response<T: Serialize>(data: T) -> Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "data": data
        })),
    )
        .into_response()
}

pub fn created_response(body: serde_json::Value, location: String) -> Response {
    (StatusCode::CREATED, [("Location", location)], Json(body)).into_response()
}
