use axum::Json;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;
use systemprompt_identifiers::{AuthorizationCode, ClientId, UserId};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::{AuthCodeParams, OAuthRepository};
use systemprompt_oauth::services::webauthn::WebAuthnRegistry;
use systemprompt_oauth::services::{generate_secure_token, is_browser_request};

#[derive(Debug, Deserialize)]
pub struct WebAuthnCompleteQuery {
    pub user_id: UserId,
    pub auth_token: Option<String>,
    pub response_type: Option<String>,
    pub client_id: Option<ClientId>,
    pub redirect_uri: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub response_mode: Option<String>,
    pub resource: Option<String>,
}

async fn verify_completion(
    params: &WebAuthnCompleteQuery,
    state: &OAuthState,
    repo: &OAuthRepository,
) -> Result<(UserId, String), OAuthHttpError> {
    let auth_token = params
        .auth_token
        .as_deref()
        .ok_or_else(|| OAuthHttpError::invalid_request("Missing auth_token parameter"))?;

    let webauthn_service =
        WebAuthnRegistry::get_or_create_service(repo.clone(), Arc::clone(state.user_provider()))
            .await
            .map_err(|e| {
                OAuthHttpError::server_error(format!("WebAuthn service initialization failed: {e}"))
            })?;

    let verified_user_id = webauthn_service
        .consume_verified_authentication(auth_token)
        .await
        .map_err(|_e| OAuthHttpError::access_denied("Invalid or expired authentication token"))?;

    if params.user_id != verified_user_id {
        return Err(OAuthHttpError::access_denied(
            "User identity verification failed",
        ));
    }

    if params.client_id.is_none() {
        return Err(OAuthHttpError::invalid_request(
            "Missing client_id parameter",
        ));
    }

    let redirect_uri = params
        .redirect_uri
        .clone()
        .ok_or_else(|| OAuthHttpError::invalid_request("Missing redirect_uri parameter"))?;

    Ok((verified_user_id, redirect_uri))
}

pub async fn handle_webauthn_complete(
    headers: HeaderMap,
    Query(params): Query<WebAuthnCompleteQuery>,
    State(state): State<OAuthState>,
    OAuthRepo(repo): OAuthRepo,
) -> Result<Response, OAuthHttpError> {
    let (verified_user_id, redirect_uri) = verify_completion(&params, &state, &repo).await?;

    let user = state.user_provider().find_by_id(&verified_user_id).await?;
    if user.is_none() {
        return Err(OAuthHttpError::access_denied("User not found"));
    }

    let authorization_code = generate_secure_token("auth_code");
    store_authorization_code(&repo, &authorization_code, &params).await?;

    Ok(create_successful_response(
        &headers,
        &redirect_uri,
        &authorization_code,
        &params,
    ))
}

async fn store_authorization_code(
    repo: &OAuthRepository,
    code_str: &str,
    query: &WebAuthnCompleteQuery,
) -> Result<(), OAuthHttpError> {
    let client_id = query
        .client_id
        .as_ref()
        .ok_or_else(|| OAuthHttpError::invalid_request("client_id is required"))?;
    let redirect_uri = query
        .redirect_uri
        .as_ref()
        .ok_or_else(|| OAuthHttpError::invalid_request("redirect_uri is required"))?;
    let scope = query.scope.as_ref().map_or_else(
        || {
            let default_roles = OAuthRepository::get_default_roles();
            if default_roles.is_empty() {
                "user".to_owned()
            } else {
                default_roles.join(" ")
            }
        },
        Clone::clone,
    );

    let code = AuthorizationCode::new(code_str);

    let mut builder =
        AuthCodeParams::builder(&code, client_id, &query.user_id, redirect_uri, &scope);

    if let (Some(challenge), Some(method)) = (
        query.code_challenge.as_deref(),
        query
            .code_challenge_method
            .as_deref()
            .filter(|s| !s.is_empty()),
    ) {
        builder = builder.with_pkce(challenge, method);
    }

    if let Some(resource) = query.resource.as_deref() {
        builder = builder.with_resource(resource);
    }

    repo.store_authorization_code(builder.build()).await?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct WebAuthnCompleteResponse {
    pub authorization_code: String,
    pub state: String,
    pub redirect_uri: String,
    pub client_id: ClientId,
}

fn create_successful_response(
    headers: &HeaderMap,
    redirect_uri: &str,
    authorization_code: &str,
    params: &WebAuthnCompleteQuery,
) -> Response {
    let state = params.state.as_deref().filter(|s| !s.is_empty());

    if is_browser_request(headers) {
        let mut target = format!("{redirect_uri}?code={authorization_code}");

        if let Some(client_id_val) = params.client_id.as_ref() {
            target.push_str(&format!(
                "&client_id={}",
                urlencoding::encode(client_id_val.as_str())
            ));
        }

        if let Some(state_val) = state {
            target.push_str(&format!("&state={}", urlencoding::encode(state_val)));
        }
        Redirect::to(&target).into_response()
    } else {
        let response_data = WebAuthnCompleteResponse {
            authorization_code: authorization_code.to_owned(),
            state: state.unwrap_or("").to_owned(),
            redirect_uri: redirect_uri.to_owned(),
            client_id: params
                .client_id
                .clone()
                .unwrap_or_else(|| ClientId::new("")),
        };

        Json(response_data).into_response()
    }
}
