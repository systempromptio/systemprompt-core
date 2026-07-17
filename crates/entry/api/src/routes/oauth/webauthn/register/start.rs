//! `WebAuthn` registration-start endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::sync::Arc;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::webauthn::WebAuthnRegistry;
use tracing::instrument;

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

#[derive(Debug, Deserialize)]
pub struct StartRegisterQuery {
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
}

impl StartRegisterQuery {
    fn validate(&self) -> Result<(), &'static str> {
        if self.username.trim().is_empty() {
            return Err("Username is required and cannot be empty");
        }
        if self.username.len() > 50 {
            return Err("Username must be less than 50 characters");
        }
        if !self
            .username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err("Username can only contain letters, numbers, underscores, and hyphens");
        }
        if !crate::services::validation::is_valid_email(&self.email) {
            return Err("Email must be a valid email address");
        }
        Ok(())
    }
}

#[instrument(skip(state, oauth_repo, params), fields(username = %params.username, email = %params.email))]
pub async fn start_register(
    Query(params): Query<StartRegisterQuery>,
    State(state): State<OAuthState>,
    OAuthRepo(oauth_repo): OAuthRepo,
) -> Result<Response, OAuthHttpError> {
    params.validate().map_err(OAuthHttpError::invalid_request)?;

    let user_provider = Arc::clone(state.user_provider());

    let webauthn_service =
        WebAuthnRegistry::get_or_create_service(oauth_repo, user_provider).await?;

    let (challenge, challenge_id) = webauthn_service
        .start_registration(&params.username, &params.email, params.full_name.as_deref())
        .await
        .map_err(|e| OAuthHttpError::registration_failed(e.to_string()))?;

    let mut challenge_json = serde_json::to_value(&challenge)
        .map_err(|e| OAuthHttpError::server_error(format!("Failed to serialize challenge: {e}")))?;

    if let Some(public_key) = challenge_json.get_mut("publicKey")
        && let Some(authenticator_selection) = public_key.get_mut("authenticatorSelection")
        && let Some(obj) = authenticator_selection.as_object_mut()
    {
        obj.remove("authenticatorAttachment");
    }

    let header_value = HeaderValue::from_str(&challenge_id)
        .map_err(|e| OAuthHttpError::server_error(format!("Invalid challenge ID format: {e}")))?;

    let mut headers = HeaderMap::new();
    headers.insert(HeaderName::from_static("x-challenge-id"), header_value);
    Ok((StatusCode::OK, headers, Json(challenge_json)).into_response())
}
