//! JWT extraction and validation helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod authentication;
pub mod authorization;

use std::future::Future;

use systemprompt_identifiers::ServiceName;
use systemprompt_models::auth::{AuthRequestError, AuthenticatedUser};
use systemprompt_security::keys::SigningKeys;

pub use authentication::AuthenticationService;
pub use authorization::AuthorizationService;

pub trait TokenValidator: Send + Sync {
    fn validate_token(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<AuthenticatedUser, AuthRequestError>> + Send;
}

pub fn extract_bearer_token(headers: &http::HeaderMap) -> Result<String, AuthRequestError> {
    systemprompt_security::TokenExtractor::standard()
        .extract(headers)
        .map_err(|_e| AuthRequestError::AuthenticationFailed {
            message: "Authorization header missing or invalid".to_owned(),
        })
}

pub fn extract_cookie_token(headers: &http::HeaderMap) -> Result<String, AuthRequestError> {
    headers
        .get("cookie")
        .ok_or_else(|| AuthRequestError::AuthenticationFailed {
            message: "Cookie header missing".to_owned(),
        })?
        .to_str()
        .map_err(|_e| AuthRequestError::InvalidTokenFormat)?
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            if cookie.starts_with("access_token=") {
                cookie.strip_prefix("access_token=").map(str::to_owned)
            } else {
                None
            }
        })
        .ok_or_else(|| AuthRequestError::AuthenticationFailed {
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

    pub fn authenticate(
        keys: &SigningKeys,
        headers: &http::HeaderMap,
    ) -> Result<AuthenticatedUser, http::StatusCode> {
        AuthenticationService::authenticate(keys, headers)
    }

    pub fn authorize_service_access(
        keys: &SigningKeys,
        headers: &http::HeaderMap,
        service_name: &ServiceName,
    ) -> Result<AuthenticatedUser, http::StatusCode> {
        AuthorizationService::authorize_service_access(keys, headers, service_name)
    }

    pub fn authorize_required_audience(
        keys: &SigningKeys,
        headers: &http::HeaderMap,
        required_audience: &str,
    ) -> Result<AuthenticatedUser, http::StatusCode> {
        AuthorizationService::authorize_required_audience(keys, headers, required_audience)
    }
}
