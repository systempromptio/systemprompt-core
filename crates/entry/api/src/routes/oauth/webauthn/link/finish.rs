//! `WebAuthn` account-link finish endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ChallengeId, UserId};
use systemprompt_oauth::OAuthState;
use tracing::instrument;
use webauthn_rs::prelude::RegisterPublicKeyCredential;

use crate::routes::oauth::{OAuthHttpError, internal};

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

#[instrument(skip(state, body), fields(challenge_id = tracing::field::Empty))]
pub async fn finish_link(
    State(state): State<OAuthState>,
    body: Result<Json<FinishLinkRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, OAuthHttpError> {
    let request = crate::routes::oauth::webauthn::json_body(body)?;
    tracing::Span::current().record(
        "challenge_id",
        tracing::field::display(&request.challenge_id),
    );
    let webauthn_service = state.webauthn()?;

    let user_id = webauthn_service
        .finish_registration_with_token(&request.challenge_id, &request.token, &request.credential)
        .await
        .map_err(|e| internal::reclassify(e, OAuthHttpError::link_failed))?;

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
