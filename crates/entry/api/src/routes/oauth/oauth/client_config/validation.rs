use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};

pub fn validate_registration_token(
    headers: &HeaderMap,
) -> Result<String, Box<axum::response::Response>> {
    let auth_header = match headers.get("authorization") {
        Some(header) => match header.to_str() {
            Ok(value) => value,
            Err(_) => {
                return Err(Box::new(
                    (
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({
                            "error": "invalid_token",
                            "error_description": "Invalid authorization header format"
                        })),
                    )
                        .into_response(),
                ));
            },
        },
        None => {
            return Err(Box::new(
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": "invalid_token",
                        "error_description": "Missing authorization header"
                    })),
                )
                    .into_response(),
            ));
        },
    };

    let Some(token) = auth_header.strip_prefix("Bearer ") else {
        return Err(Box::new(
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "invalid_token",
                    "error_description": "Authorization header must use Bearer scheme"
                })),
            )
                .into_response(),
        ));
    };

    if !token.starts_with("reg_") {
        return Err(Box::new(
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "invalid_token",
                    "error_description": "Invalid registration access token format"
                })),
            )
                .into_response(),
        ));
    }

    Ok(token.to_string())
}
