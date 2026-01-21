use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;
use systemprompt_models::{ApiError, CollectionResponse, SingleResponse};

pub fn api_error_response(error: ApiError) -> Response {
    let status = match error.status_code {
        400 => StatusCode::BAD_REQUEST,
        401 => StatusCode::UNAUTHORIZED,
        403 => StatusCode::FORBIDDEN,
        404 => StatusCode::NOT_FOUND,
        409 => StatusCode::CONFLICT,
        422 => StatusCode::UNPROCESSABLE_ENTITY,
        500 => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

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
