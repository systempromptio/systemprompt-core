//! JWT extraction and validation helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod authentication;
pub mod authorization;

use std::future::Future;

use systemprompt_models::auth::{AuthError, AuthenticatedUser};

pub use authentication::AuthenticationService;
pub use authorization::AuthorizationService;

pub trait TokenValidator: Send + Sync {
    fn validate_token(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<AuthenticatedUser, AuthError>> + Send;
}

pub fn extract_bearer_token(headers: &http::HeaderMap) -> Result<String, AuthError> {
    systemprompt_security::TokenExtractor::standard()
        .extract(headers)
        .map_err(|_e| AuthError::AuthenticationFailed {
            message: "Authorization header missing or invalid".to_owned(),
        })
}

pub fn extract_cookie_token(headers: &http::HeaderMap) -> Result<String, AuthError> {
    headers
        .get("cookie")
        .ok_or_else(|| AuthError::AuthenticationFailed {
            message: "Cookie header missing".to_owned(),
        })?
        .to_str()
        .map_err(|_e| AuthError::InvalidTokenFormat)?
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            if cookie.starts_with("access_token=") {
                cookie.strip_prefix("access_token=").map(str::to_owned)
            } else {
                None
            }
        })
        .ok_or_else(|| AuthError::AuthenticationFailed {
            message: "Access token not found in cookies".to_owned(),
        })
}

#[derive(Debug, Copy, Clone)]
pub struct AuthService;

impl AuthService {
    pub fn extract_bearer_token(headers: &http::HeaderMap) -> Result<String, http::StatusCode> {
        systemprompt_security::TokenExtractor::standard()
            .extract(headers)
            .map_err(|_e| http::StatusCode::UNAUTHORIZED)
    }

    pub fn authenticate(headers: &http::HeaderMap) -> Result<AuthenticatedUser, http::StatusCode> {
        AuthenticationService::authenticate(headers)
    }

    pub fn authorize_service_access(
        headers: &http::HeaderMap,
        service_name: &str,
    ) -> Result<AuthenticatedUser, http::StatusCode> {
        AuthorizationService::authorize_service_access(headers, service_name)
    }

    pub fn authorize_required_audience(
        headers: &http::HeaderMap,
        required_audience: &str,
    ) -> Result<AuthenticatedUser, http::StatusCode> {
        AuthorizationService::authorize_required_audience(headers, required_audience)
    }
}
