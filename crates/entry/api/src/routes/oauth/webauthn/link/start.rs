//! `WebAuthn` account-link start endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;
use systemprompt_oauth::OAuthState;
use tracing::instrument;

use crate::routes::oauth::{OAuthHttpError, internal};

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

#[instrument(skip(state, params), fields(token_prefix = %params.token.chars().take(12).collect::<String>()))]
pub async fn start_link(
    Query(params): Query<StartLinkQuery>,
    State(state): State<OAuthState>,
) -> Result<Response, OAuthHttpError> {
    if params.token.is_empty() {
        return Err(OAuthHttpError::invalid_request("Token is required"));
    }

    let webauthn_service = state.webauthn()?;

    let (challenge, challenge_id, user_info) = webauthn_service
        .start_registration_with_token(&params.token)
        .await
        .map_err(|e| internal::reclassify(e, OAuthHttpError::link_failed))?;

    let mut challenge_json = serde_json::to_value(&challenge)
        .map_err(|e| internal::server_error("Failed to serialize challenge", e))?;

    if let Some(public_key) = challenge_json.get_mut("publicKey")
        && let Some(authenticator_selection) = public_key.get_mut("authenticatorSelection")
        && let Some(obj) = authenticator_selection.as_object_mut()
    {
        obj.remove("authenticatorAttachment");
    }

    let header_value = HeaderValue::from_str(&challenge_id)
        .map_err(|e| internal::server_error("Invalid challenge ID format", e))?;

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
