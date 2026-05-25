use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::UserId;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::webauthn::WebAuthnRegistry;
use tracing::instrument;

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

#[derive(Debug, Deserialize)]
pub struct StartLinkQuery {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub(super) struct StartLinkUserInfo {
    pub id: UserId,
    pub email: String,
    pub name: String,
}

#[expect(unused_qualifications)]
#[instrument(skip(state, oauth_repo, params), fields(token_prefix = %params.token.chars().take(12).collect::<String>()))]
pub async fn start_link(
    Query(params): Query<StartLinkQuery>,
    State(state): State<OAuthState>,
    OAuthRepo(oauth_repo): OAuthRepo,
) -> Result<Response, OAuthHttpError> {
    if params.token.is_empty() {
        return Err(OAuthHttpError::invalid_request("Token is required"));
    }

    let user_provider = Arc::clone(state.user_provider());
    let webauthn_service = WebAuthnRegistry::get_or_create_service(oauth_repo, user_provider)
        .await
        .map_err(|e| OAuthHttpError::server_error(format!("Failed to initialize WebAuthn: {e}")))?;

    let (challenge, challenge_id, user_info) = webauthn_service
        .start_registration_with_token(&params.token, state.link_states())
        .await
        .map_err(|e| OAuthHttpError::link_failed(e.to_string()))?;

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

    let response = serde_json::json!({
        "challenge": challenge_json,
        "user": StartLinkUserInfo {
            id: user_info.id,
            email: user_info.email,
            name: user_info.name,
        }
    });

    Ok((StatusCode::OK, headers, Json(response)).into_response())
}
