use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::{ChallengeId, UserId};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::webauthn::{FinishRegistrationParams, WebAuthnRegistry};
use tracing::instrument;
use webauthn_rs::prelude::*;

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

#[derive(Debug, Deserialize)]
pub struct FinishRegisterRequest {
    pub challenge_id: ChallengeId,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub credential: RegisterPublicKeyCredential,
    #[serde(default)]
    pub session_id: Option<String>,
}

impl FinishRegisterRequest {
    fn validate(&self) -> Result<(), &'static str> {
        if self.challenge_id.as_str().trim().is_empty() {
            return Err("Challenge ID is required");
        }
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
}

#[instrument(skip(state, oauth_repo, request), fields(challenge_id = %request.challenge_id, username = %request.username))]
pub async fn finish_register(
    State(state): State<OAuthState>,
    OAuthRepo(oauth_repo): OAuthRepo,
    Json(request): Json<FinishRegisterRequest>,
) -> Result<Response, OAuthHttpError> {
    request
        .validate()
        .map_err(OAuthHttpError::invalid_request)?;

    let user_provider = Arc::clone(state.user_provider());

    let webauthn_service = WebAuthnRegistry::get_or_create_service(oauth_repo, user_provider)
        .await
        .map_err(|e| OAuthHttpError::server_error(format!("Failed to initialize WebAuthn: {e}")))?;

    let mut builder = FinishRegistrationParams::builder(
        request.challenge_id.as_str(),
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

    if let Some(publisher) = state.event_publisher() {
        publisher.publish_user_event(systemprompt_traits::UserEvent::UserCreated {
            user_id: user_id.clone(),
        });
    }

    if let Some(session_id_str) = &request.session_id {
        migrate_session_user(&state, session_id_str, &user_id).await;
    }

    Ok((
        StatusCode::OK,
        Json(FinishRegisterResponse {
            user_id,
            success: true,
        }),
    )
        .into_response())
}

async fn migrate_session_user(state: &OAuthState, session_id_str: &str, new_user_id: &UserId) {
    use systemprompt_identifiers::SessionId;

    let session_id = SessionId::new(session_id_str.to_owned());
    let analytics_provider = state.analytics_provider();

    match analytics_provider
        .find_active_session_by_id(&session_id)
        .await
    {
        Ok(Some(session)) => {
            let Some(old_user_id) = session.user_id else {
                return;
            };
            match analytics_provider
                .migrate_user_sessions(&old_user_id, new_user_id)
                .await
            {
                Ok(count) => {
                    tracing::info!(
                        session_id = %session_id,
                        old_user_id = %old_user_id,
                        new_user_id = %new_user_id,
                        records_migrated = count,
                        "Successfully migrated user data"
                    );
                },
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        session_id = %session_id,
                        old_user_id = %old_user_id,
                        new_user_id = %new_user_id,
                        "Failed to migrate session"
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
