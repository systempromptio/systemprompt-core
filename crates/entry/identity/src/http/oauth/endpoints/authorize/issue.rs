//! Minting an authorization code and returning it to the client, shared by
//! the passkey completion and the browser-session path of `/authorize`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AuthorizationCode, ClientId, UserId};
use systemprompt_models::oauth::OAuthServerConfig;
use systemprompt_oauth::repository::{MintAuthCodeParams, OAuthRepository};

use super::session::SignedInRequest;
use crate::http::oauth::OAuthHttpError;
use systemprompt_security::http::RequestBaseUrl;

/// The grant an authorization code is minted for. `authenticated_at` is when
/// the person presented a credential, never the time of the mint.
#[derive(Debug, Clone, Copy)]
pub struct CodeGrant<'a> {
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub redirect_uri: &'a str,
    pub scope: Option<&'a str>,
    pub code_challenge: &'a str,
    pub code_challenge_method: &'a str,
    pub resource: Option<&'a str>,
    pub state: Option<&'a str>,
    pub authenticated_at: DateTime<Utc>,
}

pub fn session_grant<'a>(request: &SignedInRequest<'a>) -> Result<CodeGrant<'a>, OAuthHttpError> {
    let SignedInRequest {
        params,
        validated,
        sign_in,
        redirect_uri,
    } = *request;
    let authenticated_at = DateTime::from_timestamp(sign_in.auth_time, 0)
        .ok_or_else(|| OAuthHttpError::login_required("The browser session states no sign-in"))?;
    Ok(CodeGrant {
        client_id: &params.client_id,
        user_id: &sign_in.user_id,
        redirect_uri,
        scope: Some(validated.scope.as_str()),
        code_challenge: params.code_challenge.as_deref().unwrap_or_default(),
        code_challenge_method: params.code_challenge_method.as_deref().unwrap_or_default(),
        resource: params.resource.as_deref(),
        state: params.state.as_deref(),
        authenticated_at,
    })
}

// Why: RFC 9207 requires `iss` to match the issuer advertised by discovery.
#[must_use]
pub fn response_issuer(base: &RequestBaseUrl) -> String {
    OAuthServerConfig::from_api_server_url(base.as_str()).issuer
}

pub async fn mint_code(
    repo: &OAuthRepository,
    grant: &CodeGrant<'_>,
) -> Result<AuthorizationCode, OAuthHttpError> {
    Ok(repo
        .mint_authorization_code(MintAuthCodeParams {
            client_id: grant.client_id,
            user_id: grant.user_id,
            redirect_uri: grant.redirect_uri,
            scope: grant.scope,
            code_challenge: grant.code_challenge,
            code_challenge_method: grant.code_challenge_method,
            resource: grant.resource,
            authenticated_at: grant.authenticated_at,
        })
        .await?)
}

#[must_use]
pub fn code_redirect_target(
    grant: &CodeGrant<'_>,
    code: &AuthorizationCode,
    issuer: &str,
) -> String {
    let separator = if grant.redirect_uri.contains('?') {
        '&'
    } else {
        '?'
    };
    let mut target = format!(
        "{}{separator}code={}&client_id={}",
        grant.redirect_uri,
        urlencoding::encode(code.as_str()),
        urlencoding::encode(grant.client_id.as_str())
    );
    if let Some(state) = grant.state.filter(|state| !state.is_empty()) {
        target.push_str(&format!("&state={}", urlencoding::encode(state)));
    }
    target.push_str(&format!("&iss={}", urlencoding::encode(issuer)));
    target
}

pub async fn issue_code_redirect(
    repo: &OAuthRepository,
    grant: &CodeGrant<'_>,
    issuer: &str,
) -> Result<Response, OAuthHttpError> {
    let code = mint_code(repo, grant).await?;
    let mut response = Redirect::to(&code_redirect_target(grant, &code, issuer)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
