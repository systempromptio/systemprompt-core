//! `/oauth/token` endpoint.
//!
//! Binds the form-encoded grant, folds HTTP Basic client credentials into it,
//! and hands it to [`TokenIssuanceOrchestrator`]; an [`IssuanceError`] answers
//! with its RFC 6749 error body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Form, Json};
use base64::Engine;
use systemprompt_models::RequestContext;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth_issuance::{
    IssuanceError, IssuanceResult, RequestOrigin, TokenIssuanceOrchestrator, TokenRequest,
};
use tracing::instrument;

use crate::routes::oauth::OAuthHttpError;
use crate::services::middleware::client_addr::ClientIp;

#[instrument(skip(state, _req_ctx, caller_ip, headers, request), fields(grant_type = %request.grant_type))]
pub async fn handle_token(
    Extension(_req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    ClientIp(caller_ip): ClientIp,
    headers: HeaderMap,
    Form(mut request): Form<TokenRequest>,
) -> Result<Response, OAuthHttpError> {
    tracing::info!(grant_type = %request.grant_type, "Token request received");

    apply_basic_client_auth(&headers, &mut request)?;

    let origin = RequestOrigin {
        headers: &headers,
        caller_ip,
    };
    let response = TokenIssuanceOrchestrator::new(&state)
        .issue(request, origin)
        .await?;
    Ok((StatusCode::OK, Json(response)).into_response())
}

// Why: RFC 6749 §2.3.1 — `client_secret_basic` carries
// `client_id:client_secret` percent-encoded inside HTTP Basic; a client must
// not also send them in the body, so a conflicting pair is rejected rather than
// silently preferred.
fn apply_basic_client_auth(headers: &HeaderMap, request: &mut TokenRequest) -> IssuanceResult<()> {
    let Some(encoded) = headers
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
    else {
        return Ok(());
    };

    let invalid = || IssuanceError::InvalidRequest {
        field: "authorization".to_owned(),
        message: "malformed HTTP Basic client credentials".to_owned(),
    };
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|_e| invalid())?;
    let decoded = String::from_utf8(decoded).map_err(|_e| invalid())?;
    let (client_id, client_secret) = decoded.split_once(':').ok_or_else(invalid)?;
    let client_id = urlencoding::decode(client_id).map_err(|_e| invalid())?;
    let client_secret = urlencoding::decode(client_secret).map_err(|_e| invalid())?;

    if request
        .client_id
        .as_deref()
        .is_some_and(|body_id| body_id != client_id)
        || request.client_secret.is_some()
    {
        return Err(IssuanceError::InvalidRequest {
            field: "client_id".to_owned(),
            message: "client credentials supplied both in the Authorization header and the body"
                .to_owned(),
        });
    }

    request.client_id = Some(client_id.into_owned());
    request.client_secret = Some(client_secret.into_owned());
    Ok(())
}
