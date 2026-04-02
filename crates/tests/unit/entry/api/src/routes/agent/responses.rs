use axum::body::to_bytes;
use axum::http::StatusCode;
use systemprompt_api::routes::agent::responses::{
    collection_response, single_response, single_response_created,
};
use systemprompt_models::api::ApiError;
use systemprompt_api::routes::agent::responses::api_error_response;

#[tokio::test]
async fn test_single_response_returns_200() {
    let data = serde_json::json!({"name": "test"});
    let response = single_response(data);
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_single_response_body_contains_data() {
    let data = serde_json::json!({"key": "value"});
    let response = single_response(data);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"]["key"], "value");
}

#[tokio::test]
async fn test_single_response_body_contains_meta() {
    let data = serde_json::json!({"key": "value"});
    let response = single_response(data);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["meta"].is_object());
    assert!(json["meta"]["version"].is_string());
}

#[tokio::test]
async fn test_single_response_created_returns_201() {
    let data = serde_json::json!({"id": "new_item"});
    let response = single_response_created(data);
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_single_response_created_body_contains_data() {
    let data = serde_json::json!({"id": "new_item", "name": "Created Item"});
    let response = single_response_created(data);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"]["id"], "new_item");
    assert_eq!(json["data"]["name"], "Created Item");
}

#[tokio::test]
async fn test_collection_response_returns_200() {
    let items: Vec<serde_json::Value> = vec![
        serde_json::json!({"id": 1}),
        serde_json::json!({"id": 2}),
    ];
    let response = collection_response(items);
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_collection_response_body_contains_array() {
    let items = vec![
        serde_json::json!({"id": 1}),
        serde_json::json!({"id": 2}),
        serde_json::json!({"id": 3}),
    ];
    let response = collection_response(items);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let data = json["data"].as_array().unwrap();
    assert_eq!(data.len(), 3);
}

#[tokio::test]
async fn test_collection_response_empty_items() {
    let items: Vec<serde_json::Value> = vec![];
    let response = collection_response(items);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let data = json["data"].as_array().unwrap();
    assert!(data.is_empty());
}

#[tokio::test]
async fn test_api_error_response_not_found_returns_404() {
    let error = ApiError::not_found("Resource not found");
    let response = api_error_response(error);
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_api_error_response_bad_request_returns_400() {
    let error = ApiError::bad_request("Invalid input");
    let response = api_error_response(error);
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_error_response_internal_error_returns_500() {
    let error = ApiError::internal_error("Something broke");
    let response = api_error_response(error);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_api_error_response_body_contains_error_message() {
    let error = ApiError::not_found("Task xyz not found");
    let response = api_error_response(error);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["message"], "Task xyz not found");
    assert_eq!(json["code"], "not_found");
}
