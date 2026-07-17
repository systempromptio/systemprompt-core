//! `WebAuthn` account-link finish endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::{ChallengeId, UserId};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::webauthn::WebAuthnRegistry;
use tracing::instrument;
use webauthn_rs::prelude::RegisterPublicKeyCredential;

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

#[derive(Debug, Deserialize)]
pub struct FinishLinkRequest {
    pub challenge_id: ChallengeId,
    pub token: String,
    pub credential: RegisterPublicKeyCredential,
}

#[derive(Debug, Serialize)]
pub(super) struct FinishLinkResponse {
    pub success: bool,
    pub user_id: UserId,
    pub message: String,
}

#[instrument(skip(state, oauth_repo, request), fields(challenge_id = %request.challenge_id))]
pub async fn finish_link(
    State(state): State<OAuthState>,
    OAuthRepo(oauth_repo): OAuthRepo,
    Json(request): Json<FinishLinkRequest>,
) -> Result<Response, OAuthHttpError> {
    let user_provider = Arc::clone(state.user_provider());
    let webauthn_service =
        WebAuthnRegistry::get_or_create_service(oauth_repo, user_provider).await?;

    let user_id = webauthn_service
        .finish_registration_with_token(
            request.challenge_id.as_str(),
            &request.token,
            &request.credential,
            state.link_states(),
        )
        .await
        .map_err(|e| OAuthHttpError::link_failed(e.to_string()))?;

    tracing::info!(user_id = %user_id, "Credential linked successfully");
    Ok((
        StatusCode::OK,
        Json(FinishLinkResponse {
            success: true,
            user_id,
            message: "Passkey registered successfully".to_owned(),
        }),
    )
        .into_response())
}
