pub mod authentication;
pub mod authorization;

use async_trait::async_trait;
use systemprompt_models::auth::{AuthError, AuthenticatedUser};

pub use authentication::AuthenticationService;
pub use authorization::AuthorizationService;

#[async_trait]
pub trait TokenValidator: Send + Sync {
    async fn validate_token(&self, token: &str) -> Result<AuthenticatedUser, AuthError>;
}

pub fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Result<String, AuthError> {
    systemprompt_security::TokenExtractor::standard()
        .extract(headers)
        .map_err(|_| AuthError::AuthenticationFailed {
            message: "Authorization header missing or invalid".to_string(),
        })
}

pub fn extract_cookie_token(headers: &axum::http::HeaderMap) -> Result<String, AuthError> {
    headers
        .get("cookie")
        .ok_or(AuthError::AuthenticationFailed {
            message: "Cookie header missing".to_string(),
        })?
        .to_str()
        .map_err(|_| AuthError::InvalidTokenFormat)?
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            if cookie.starts_with("access_token=") {
                cookie
                    .strip_prefix("access_token=")
                    .map(ToString::to_string)
            } else {
                None
            }
        })
        .ok_or(AuthError::AuthenticationFailed {
            message: "Access token not found in cookies".to_string(),
        })
}

#[derive(Debug, Copy, Clone)]
pub struct AuthService;

impl AuthService {
    pub fn extract_bearer_token(
        headers: &axum::http::HeaderMap,
    ) -> Result<String, axum::http::StatusCode> {
        systemprompt_security::TokenExtractor::standard()
            .extract(headers)
            .map_err(|_| axum::http::StatusCode::UNAUTHORIZED)
    }

    pub async fn authenticate(
        headers: &axum::http::HeaderMap,
    ) -> Result<AuthenticatedUser, axum::http::StatusCode> {
        AuthenticationService::authenticate(headers).await
    }

    pub async fn authorize_service_access(
        headers: &axum::http::HeaderMap,
        service_name: &str,
    ) -> Result<AuthenticatedUser, axum::http::StatusCode> {
        AuthorizationService::authorize_service_access(headers, service_name).await
    }

    pub async fn authorize_required_audience(
        headers: &axum::http::HeaderMap,
        required_audience: &str,
    ) -> Result<AuthenticatedUser, axum::http::StatusCode> {
        AuthorizationService::authorize_required_audience(headers, required_audience).await
    }
}
