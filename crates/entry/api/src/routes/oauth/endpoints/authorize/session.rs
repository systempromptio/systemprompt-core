//! The browser session `/authorize` accepts as proof of who is signed in.
//!
//! The authorization endpoint sits outside the session middleware, so it
//! verifies the `access_token` cookie itself. Only the token the first-party
//! web login mints counts: a browser session with no `client_id`, no
//! delegation, plugin scope or purpose, held interactively by a person. The
//! session row must be live and the user's, the token's `jti` unrevoked, and
//! the user active and not a service account. A bearer header is never read.
//! Every failure is the same answer, no session; the reason is logged and
//! never shown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_identifiers::{AccessTokenId, SessionId, UserId};
use systemprompt_models::auth::{JwtClaims, TokenInteraction, UserType};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::load_interactive_user;
use systemprompt_security::CookieExtractor;

use super::AuthorizeQuery;
use super::validation::ValidatedAuthorizeRequest;
use crate::services::middleware::session::attest_session;

const ROUTE: &str = "oauth/authorize";

/// A person signed in to this browser, and when they presented a credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSignIn {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub auth_time: i64,
    pub account: String,
}

/// A validated authorization request made by a person signed in to this
/// browser, with the registered `redirect_uri` the code returns to.
#[derive(Debug, Clone, Copy)]
pub struct SignedInRequest<'a> {
    pub params: &'a AuthorizeQuery,
    pub validated: &'a ValidatedAuthorizeRequest,
    pub sign_in: &'a BrowserSignIn,
    pub redirect_uri: &'a str,
}

#[must_use]
pub fn session_subject(claims: &JwtClaims) -> Option<(UserId, SessionId)> {
    let custom = &claims.custom;
    let web_login = custom.browser_session && !custom.ui_session && custom.client_id.is_none();
    let undelegated =
        custom.act.is_none() && custom.purpose.is_none() && custom.plugin_id.is_none();
    let interactive = custom.interaction != Some(TokenInteraction::NonInteractive);
    let person = matches!(custom.user_type, UserType::User | UserType::Admin);
    if !(web_login && undelegated && interactive && person) {
        return None;
    }
    let session_id = custom.session_id.clone()?;
    if session_id.as_str().is_empty() {
        return None;
    }
    let user_id = UserId::try_new(claims.registered.sub.as_str()).ok()?;
    Some((user_id, session_id))
}

pub async fn browser_sign_in(state: &OAuthState, headers: &HeaderMap) -> Option<BrowserSignIn> {
    let token = CookieExtractor::extract_access_token(headers).ok()?;
    let claims = state
        .token_authority()
        .verify_session_cookie(&token)
        .inspect_err(|error| tracing::debug!(%error, "authorize: session cookie refused"))
        .ok()?;
    let Some((user_id, session_id)) = session_subject(&claims) else {
        tracing::debug!("authorize: the cookie holds a token the web login did not mint");
        return None;
    };
    attest_session(state.session_provider(), &session_id, &user_id, ROUTE)
        .await
        .inspect_err(|error| tracing::debug!(%error, "authorize: session not attested"))
        .ok()?;
    let jti = AccessTokenId::try_new(claims.registered.jti.as_str()).ok()?;
    let revoked = state
        .oauth_repository()
        .is_jti_revoked(&jti)
        .await
        .inspect_err(|error| tracing::warn!(%error, "authorize: revocation lookup failed"))
        .ok()?;
    if revoked {
        return None;
    }
    let active = state
        .user_provider()
        .find_by_id(&user_id)
        .await
        .inspect_err(|error| tracing::warn!(%error, "authorize: user lookup failed"))
        .ok()?
        .is_some_and(|user| user.is_active);
    if !active {
        return None;
    }
    let person = load_interactive_user(state.user_provider().as_ref(), &user_id)
        .await
        .inspect_err(|error| tracing::debug!(%error, "authorize: not an interactive person"))
        .ok()?;
    Some(BrowserSignIn {
        user_id,
        session_id,
        auth_time: claims.custom.auth_time,
        account: person.email,
    })
}
