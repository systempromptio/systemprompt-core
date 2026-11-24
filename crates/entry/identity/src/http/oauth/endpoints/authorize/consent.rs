//! The one-click consent page and the Allow decision posted from it.
//!
//! The page is rendered only for a person signed in to this browser and names
//! the client, the scopes, the account and where the browser returns to. Its
//! form restates the request and carries a consent token: the at-rest HMAC of
//! the person, their session and the request, PKCE method and response type
//! included (see
//! [`systemprompt_oauth::repository::ConsentBinding`]), valid for at most
//! [`CONSENT_TOKEN_TTL_SECS`] and never past the request's `max_age`. The page
//! is never framed and never cached.
//!
//! An Allow is honoured only from this server's own origin, with the same
//! browser session still live, for a request that validates again and whose
//! token matches. Anything else is answered in place, with no redirect, no
//! code and no stored consent. Consent is stored for operator-declared clients
//! only; the repository refuses the write for any other client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use systemprompt_extension::FrameOptionsOverride;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::{CONSENT_TOKEN_TTL_SECS, ConsentBinding, OAuthRepository};
use systemprompt_oauth::services::TemplateService;
use systemprompt_provider_contracts::FrameOptions;

use super::AuthorizeQuery;
use super::issue::{issue_code_redirect, session_grant};
use super::response_builder::redirect_display_host;
use super::session::{SignedInRequest, browser_sign_in};
use super::validation::{SelfOrigins, ValidatedAuthorizeRequest};
use crate::http::oauth::OAuthHttpError;

pub fn require_same_origin(
    headers: &HeaderMap,
    self_origins: &SelfOrigins,
) -> Result<(), OAuthHttpError> {
    let cross_site = headers
        .get("sec-fetch-site")
        .is_some_and(|site| site.as_bytes() != b"same-origin");
    let mut origins = headers.get_all(header::ORIGIN).iter();
    let own_origin = match (origins.next(), origins.next()) {
        (Some(origin), None) => origin
            .to_str()
            .ok()
            .and_then(|origin| url::Url::parse(origin).ok())
            .is_some_and(|origin| self_origins.matches(&origin.origin())),
        _ => false,
    };
    if cross_site || !own_origin {
        return Err(OAuthHttpError::access_denied(
            "The authorization form must be submitted from this server's own page",
        )
        .with_status(StatusCode::FORBIDDEN));
    }
    Ok(())
}

fn binding<'a>(request: &SignedInRequest<'a>) -> ConsentBinding<'a> {
    let SignedInRequest {
        params,
        validated,
        sign_in,
        redirect_uri,
    } = *request;
    ConsentBinding {
        user_id: &sign_in.user_id,
        session_id: &sign_in.session_id,
        client_id: &params.client_id,
        redirect_uri,
        scope: &validated.scope,
        state: params.state.as_deref().unwrap_or_default(),
        code_challenge: params.code_challenge.as_deref().unwrap_or_default(),
        code_challenge_method: params.code_challenge_method.as_deref().unwrap_or_default(),
        response_type: &params.response_type,
        resource: params.resource.as_deref(),
    }
}

fn token_expiry(max_age: Option<i64>, auth_time: i64, now: i64) -> i64 {
    let ttl_end = now.saturating_add(CONSENT_TOKEN_TTL_SECS);
    max_age
        .filter(|max_age| *max_age > 0)
        .map_or(ttl_end, |max_age| {
            ttl_end.min(auth_time.saturating_add(max_age))
        })
}

pub fn consent_page(
    repo: &OAuthRepository,
    request: &SignedInRequest<'_>,
    now: i64,
) -> Result<Response, OAuthHttpError> {
    let SignedInRequest {
        params,
        validated,
        sign_in,
        redirect_uri,
    } = *request;
    let token = repo.consent_token(
        &binding(request),
        token_expiry(params.max_age, sign_in.auth_time, now),
    )?;
    let redirect_host = redirect_display_host(redirect_uri);
    let context = HashMap::from([
        ("authorize_path", ApiPaths::OAUTH_AUTHORIZE),
        ("client_name", validated.client.client_name.as_str()),
        ("client_id", params.client_id.as_str()),
        ("account", sign_in.account.as_str()),
        ("scope", validated.scope.as_str()),
        ("redirect_host", redirect_host.as_str()),
        ("response_type", params.response_type.as_str()),
        ("redirect_uri", redirect_uri),
        ("state", params.state.as_deref().unwrap_or_default()),
        (
            "code_challenge",
            params.code_challenge.as_deref().unwrap_or_default(),
        ),
        (
            "code_challenge_method",
            params.code_challenge_method.as_deref().unwrap_or_default(),
        ),
        ("resource", params.resource.as_deref().unwrap_or_default()),
        ("consent_token", token.as_str()),
    ]);
    let page = TemplateService::render(TemplateService::load_consent_template(), context);
    let mut response = Html(page).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .extensions_mut()
        .insert(FrameOptionsOverride(FrameOptions::Deny));
    Ok(response)
}

/// An Allow posted from the consent page, with the request it restates
/// already validated again.
#[derive(Debug, Clone, Copy)]
pub struct Allow<'a> {
    pub params: &'a AuthorizeQuery,
    pub validated: &'a ValidatedAuthorizeRequest,
    pub redirect_uri: &'a str,
    pub consent_token: Option<&'a str>,
    pub issuer: &'a str,
}

pub async fn allow(
    state: &OAuthState,
    headers: &HeaderMap,
    allow: Allow<'_>,
) -> Result<Response, OAuthHttpError> {
    let Allow {
        params,
        validated,
        redirect_uri,
        consent_token,
        issuer,
    } = allow;
    let repo = state.oauth_repository();
    let sign_in = browser_sign_in(state, headers).await.ok_or_else(|| {
        OAuthHttpError::access_denied("No signed-in browser session")
            .with_status(StatusCode::FORBIDDEN)
    })?;
    let request = SignedInRequest {
        params,
        validated,
        sign_in: &sign_in,
        redirect_uri,
    };
    let now = chrono::Utc::now().timestamp();
    let genuine =
        repo.consent_token_matches(&binding(&request), consent_token.unwrap_or_default(), now)?;
    if !genuine {
        return Err(OAuthHttpError::invalid_request(
            "The consent request is invalid or has expired",
        ));
    }
    let grant = session_grant(&request)?;
    let scopes = OAuthRepository::parse_scopes(&validated.scope);
    let remembered = repo
        .record_consent(&sign_in.user_id, &params.client_id, &scopes)
        .await?;
    tracing::info!(
        client_id = %params.client_id,
        user_id = %sign_in.user_id,
        scopes = %validated.scope,
        remembered,
        "Authorization consent granted"
    );
    issue_code_redirect(repo, &grant, issuer).await
}
