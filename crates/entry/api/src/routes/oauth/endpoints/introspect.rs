//! RFC 7662 token introspection endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Form, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::ClientId;
use systemprompt_manifest::Config;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::validate_jwt_token;
use systemprompt_oauth::services::validation::validate_client_credentials;

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

#[derive(Debug, Deserialize)]
pub struct IntrospectRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct IntrospectResponse {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iat: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aud: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
}

pub async fn handle_introspect(
    State(state): State<OAuthState>,
    Extension(config): Extension<Arc<Config>>,
    OAuthRepo(repo): OAuthRepo,
    Form(request): Form<IntrospectRequest>,
) -> Result<Response, OAuthHttpError> {
    let client_id_str = request
        .client_id
        .clone()
        .ok_or_else(|| OAuthHttpError::invalid_client("Client authentication required"))?;
    let client_id = ClientId::new(client_id_str);

    validate_client_credentials(&repo, &client_id, request.client_secret.as_deref()).await?;

    let response = match introspect_token(&state, &config, &request.token) {
        Some(full) => {
            let authoritative_client_id = full.client_id.as_deref();
            if authoritative_client_id == Some(client_id.as_str()) {
                full
            } else {
                IntrospectResponse {
                    active: true,
                    ..IntrospectResponse::default()
                }
            }
        },
        None => IntrospectResponse {
            active: false,
            ..IntrospectResponse::default()
        },
    };
    Ok((StatusCode::OK, Json(response)).into_response())
}

fn introspect_token(
    state: &OAuthState,
    config: &Config,
    token: &str,
) -> Option<IntrospectResponse> {
    match validate_jwt_token(
        state.signing_keys(),
        token,
        &config.jwt_issuer,
        &config.jwt_audiences,
    ) {
        Ok(claims) => Some(IntrospectResponse {
            active: true,
            scope: Some(systemprompt_models::auth::permissions_to_string(
                &claims.custom.scope,
            )),
            client_id: claims
                .custom
                .client_id
                .as_ref()
                .map(|c| c.as_str().to_owned()),
            username: Some(claims.custom.username),
            token_type: Some("Bearer".to_owned()),
            exp: Some(claims.registered.exp),
            iat: Some(claims.registered.iat),
            sub: Some(claims.registered.sub),
            aud: claims
                .registered
                .aud
                .iter()
                .map(ToString::to_string)
                .collect(),
            iss: Some(claims.registered.iss),
            jti: Some(claims.registered.jti),
        }),
        Err(_) => None,
    }
}
