use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::{ChallengeId, UserId};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::webauthn::WebAuthnRegistry;
use tracing::instrument;
use webauthn_rs::prelude::*;

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

#[derive(Debug, Deserialize)]
pub struct StartAuthQuery {
    pub email: String,
    pub oauth_state: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StartAuthResponse {
    #[serde(rename = "publicKey")]
    pub public_key: serde_json::Value,
    pub challenge_id: ChallengeId,
}

#[instrument(skip(state, oauth_repo, params), fields(email = %params.email))]
pub async fn start_auth(
    Query(params): Query<StartAuthQuery>,
    State(state): State<OAuthState>,
    OAuthRepo(oauth_repo): OAuthRepo,
) -> Result<Response, OAuthHttpError> {
    let user_provider = Arc::clone(state.user_provider());

    let webauthn_service = WebAuthnRegistry::get_or_create_service(oauth_repo, user_provider)
        .await
        .map_err(|e| OAuthHttpError::server_error(format!("Failed to initialize WebAuthn: {e}")))?;

    let (challenge, challenge_id) = webauthn_service
        .start_authentication(&params.email, params.oauth_state)
        .await
        .map_err(|e| {
            let http: OAuthHttpError = e.into();
            if matches!(http.code(), crate::routes::oauth::OAuthErrorCode::NotFound) {
                http
            } else {
                OAuthHttpError::authentication_failed(http.description().to_owned())
            }
        })?;

    let challenge_json = serde_json::to_value(&challenge)
        .map_err(|e| OAuthHttpError::server_error(format!("Failed to serialize challenge: {e}")))?;

    let mut public_key = challenge_json
        .get("publicKey")
        .cloned()
        .ok_or_else(|| OAuthHttpError::server_error("Missing publicKey in challenge"))?;

    if let Some(obj) = public_key.as_object_mut() {
        obj.remove("authenticatorAttachment");
    }

    Ok((
        StatusCode::OK,
        Json(StartAuthResponse {
            public_key,
            challenge_id: ChallengeId::new(challenge_id),
        }),
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
pub struct FinishAuthRequest {
    pub challenge_id: ChallengeId,
    pub credential: PublicKeyCredential,
}

#[derive(Debug, Serialize)]
pub struct FinishAuthResponse {
    pub user_id: UserId,
    pub oauth_state: Option<String>,
    pub success: bool,
    pub auth_token: Option<String>,
}

#[instrument(skip(state, oauth_repo, request), fields(challenge_id = %request.challenge_id))]
pub async fn finish_auth(
    State(state): State<OAuthState>,
    OAuthRepo(oauth_repo): OAuthRepo,
    Json(request): Json<FinishAuthRequest>,
) -> Result<Response, OAuthHttpError> {
    let user_provider = Arc::clone(state.user_provider());

    let webauthn_service = WebAuthnRegistry::get_or_create_service(oauth_repo, user_provider)
        .await
        .map_err(|e| OAuthHttpError::server_error(format!("Failed to initialize WebAuthn: {e}")))?;

    let (user_id, oauth_state) = webauthn_service
        .finish_authentication(request.challenge_id.as_str(), &request.credential)
        .await
        .map_err(|e| OAuthHttpError::authentication_failed(e.to_string()))?;

    let auth_token = systemprompt_oauth::services::generate_secure_token("webauthn_verified");
    webauthn_service
        .store_verified_authentication(auth_token.clone(), user_id.clone())
        .await;

    Ok((
        StatusCode::OK,
        Json(FinishAuthResponse {
            user_id,
            oauth_state,
            success: true,
            auth_token: Some(auth_token),
        }),
    )
        .into_response())
}
