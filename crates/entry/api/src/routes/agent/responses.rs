use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;
use systemprompt_models::{ApiError, CollectionResponse, SingleResponse};

pub fn api_error_response(error: ApiError) -> Response {
    let status = error.code.status_code();
    (status, Json(error)).into_response()
}

pub fn single_response<T: Serialize>(data: T) -> Response {
    (StatusCode::OK, Json(SingleResponse::new(data))).into_response()
}

pub fn single_response_created<T: Serialize>(data: T) -> Response {
    (StatusCode::CREATED, Json(SingleResponse::new(data))).into_response()
}

pub fn collection_response<T: Serialize>(items: Vec<T>) -> Response {
    (StatusCode::OK, Json(CollectionResponse::new(items))).into_response()
}
