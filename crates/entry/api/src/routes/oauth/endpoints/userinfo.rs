use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::validate_jwt_token;

use crate::routes::oauth::OAuthHttpError;

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

#[allow(clippy::unused_async)]
pub async fn handle_userinfo(
    State(_state): State<OAuthState>,
    headers: HeaderMap,
) -> Result<Response, OAuthHttpError> {
    let token = extract_bearer_token(&headers).ok_or_else(|| {
        OAuthHttpError::invalid_request("Missing or invalid Authorization header")
    })?;

    let userinfo = get_userinfo(&token).map_err(|_| {
        OAuthHttpError::invalid_token(
            "The access token provided is expired, revoked, malformed, or invalid",
        )
    })?;
    Ok((StatusCode::OK, Json(userinfo)).into_response())
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

fn get_userinfo(token: &str) -> anyhow::Result<UserinfoResponse> {
    let config = systemprompt_models::Config::get()?;
    let claims = validate_jwt_token(token, &config.jwt_issuer, &config.jwt_audiences)?;

    Ok(UserinfoResponse {
        sub: claims.sub.clone(),
        username: Some(claims.username.clone()),
        email: Some(claims.email.clone()),
        user_type: Some(claims.user_type.to_string()),
        roles: Some(claims.get_scopes()),
    })
}
