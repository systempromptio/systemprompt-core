use anyhow::Result;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde::Serialize;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::validate_jwt_token;

#[derive(Debug, Serialize)]

pub struct UserinfoResponse {
    pub sub: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]

pub struct UserinfoError {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
}

#[allow(clippy::unused_async)]
pub async fn handle_userinfo(
    State(_state): State<OAuthState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let Some(token) = extract_bearer_token(&headers) else {
        let error = UserinfoError {
            error: "invalid_request".to_string(),
            error_description: Some("Missing or invalid Authorization header".to_string()),
        };
        return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
    };

    get_userinfo(&token).map_or_else(
        |_| {
            let error = UserinfoError {
                error: "invalid_token".to_string(),
                error_description: Some(
                    "The access token provided is expired, revoked, malformed, or invalid"
                        .to_string(),
                ),
            };
            (StatusCode::UNAUTHORIZED, Json(error)).into_response()
        },
        |userinfo| (StatusCode::OK, Json(userinfo)).into_response(),
    )
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth_header = headers.get("authorization")?;
    let auth_str = auth_header
        .to_str()
        .map_err(|e| {
            tracing::debug!(error = %e, "Invalid UTF-8 in Authorization header");
            e
        })
        .ok()?;

    auth_str.strip_prefix("Bearer ").map(ToString::to_string)
}

fn get_userinfo(token: &str) -> Result<UserinfoResponse> {
    let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()?;
    let config = systemprompt_models::Config::get()?;
    let claims = validate_jwt_token(token, jwt_secret, &config.jwt_issuer, &config.jwt_audiences)?;

    Ok(UserinfoResponse {
        sub: claims.sub.clone(),
        username: Some(claims.username.clone()),
        email: Some(claims.email.clone()),
        user_type: Some(claims.user_type.to_string()),
        roles: Some(claims.get_scopes()),
    })
}
