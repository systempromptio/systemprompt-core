//! Finish step of the `WebAuthn` registration ceremony.
//!
//! Validates the submitted credential, creates the user, optionally migrates an
//! anonymous session onto the new identity, and issues a short-lived
//! verified-authentication token for the [`webauthn_complete`] bridge.
//!
//! [`webauthn_complete`]: crate::routes::oauth::endpoints::webauthn_complete
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::{ChallengeId, SessionId, UserId};
use systemprompt_manifest::Config;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::webauthn::FinishRegistrationParams;
use tracing::instrument;
use webauthn_rs::prelude::*;

use crate::routes::oauth::OAuthHttpError;

#[derive(Debug, Deserialize)]
pub struct FinishRegisterRequest {
    pub challenge_id: ChallengeId,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub credential: RegisterPublicKeyCredential,
    #[serde(default)]
    pub session_id: Option<SessionId>,
}

impl FinishRegisterRequest {
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

#[derive(Debug, Serialize)]
pub(super) struct FinishRegisterResponse {
    pub user_id: UserId,
    pub success: bool,
    pub auth_token: Option<String>,
}

#[instrument(skip(state, config, body), fields(challenge_id = tracing::field::Empty, username = tracing::field::Empty))]
pub async fn finish_register(
    State(state): State<OAuthState>,
    Extension(config): Extension<Arc<Config>>,
    body: Result<Json<FinishRegisterRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, OAuthHttpError> {
    super::ensure_registration_enabled(&config)?;
    let request = crate::routes::oauth::webauthn::json_body(body)?;
    tracing::Span::current().record(
        "challenge_id",
        tracing::field::display(&request.challenge_id),
    );
    tracing::Span::current().record("username", tracing::field::display(&request.username));
    request
        .validate()
        .map_err(OAuthHttpError::invalid_request)?;

    let webauthn_service = state.webauthn()?;

    let mut builder = FinishRegistrationParams::builder(
        &request.challenge_id,
        &request.username,
        &request.email,
        &request.credential,
    );
    if let Some(ref name) = request.full_name {
        builder = builder.with_full_name(name);
    }

    let user_id = webauthn_service
        .finish_registration(builder.build())
        .await?;

    if let Some(session_id) = &request.session_id {
        migrate_session_user(&state, session_id, &user_id).await;
    }

    let auth_token = systemprompt_oauth::services::generate_secure_token("webauthn_verified");
    webauthn_service
        .store_verified_authentication(auth_token.clone(), user_id.clone())
        .await?;

    Ok((
        StatusCode::OK,
        Json(FinishRegisterResponse {
            user_id,
            success: true,
            auth_token: Some(auth_token),
        }),
    )
        .into_response())
}

async fn migrate_session_user(state: &OAuthState, session_id: &SessionId, new_user_id: &UserId) {
    let analytics_provider = state.session_provider();

    match analytics_provider
        .find_active_session_by_id(session_id)
        .await
    {
        Ok(Some(session)) => {
            let Some(old_user_id) = session.user_id else {
                return;
            };
            if old_user_id == *new_user_id {
                return;
            }
            match state
                .user_provider()
                .promote_anonymous(&old_user_id, new_user_id)
                .await
            {
                Ok(rows_transferred) => {
                    tracing::info!(
                        session_id = %session_id,
                        old_user_id = %old_user_id,
                        new_user_id = %new_user_id,
                        rows_transferred,
                        "Promoted anonymous user history onto registered account"
                    );
                },
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        session_id = %session_id,
                        old_user_id = %old_user_id,
                        new_user_id = %new_user_id,
                        "Failed to promote anonymous user"
                    );
                },
            }
        },
        Ok(None) => {
            tracing::warn!(session_id = %session_id, "Session not found for migration");
        },
        Err(e) => {
            tracing::error!(
                error = %e,
                session_id = %session_id,
                "Failed to retrieve session for migration"
            );
        },
    }
}
