//! OAuth 2.0 authorization endpoint.
//!
//! Hosts the GET/POST `/authorize` handlers and the request types
//! [`AuthorizeQuery`] and [`AuthorizeRequest`] they bind. The flow validates
//! the inbound request ([`validation`]), recognises a person already signed in
//! to this browser ([`session`]), and decides ([`decision`]) between issuing
//! the code at once ([`issue`]), the one-click consent page ([`consent`]),
//! sending the person to sign in ([`login_redirect`]) and the `WebAuthn`
//! challenge page ([`response_builder`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod consent;
pub mod decision;
mod handler;
pub mod issue;
pub mod login_redirect;
pub mod response_builder;
pub mod session;
pub mod validation;

pub use handler::{handle_authorize_get, handle_authorize_post};
pub use login_redirect::login_page_redirect_target;

use serde::Deserialize;
use systemprompt_identifiers::ClientId;

/// The query of `GET /authorize`.
///
/// `reauth_since` is not a client parameter: the endpoint appends it, as Unix
/// seconds, to the return address it hands the login page, and on the way
/// back accepts only a sign-in at or after it. It is unsigned because a caller
/// who supplies it can only make the endpoint stricter.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthorizeQuery {
    pub response_type: String,
    pub client_id: ClientId,
    pub redirect_uri: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub response_mode: Option<String>,
    pub display: Option<String>,
    pub prompt: Option<String>,
    pub max_age: Option<i64>,
    pub ui_locales: Option<String>,
    pub resource: Option<String>,
    pub login_hint: Option<String>,
    pub reauth_since: Option<i64>,
}

/// The form of `POST /authorize`.
///
/// It carries the Allow or Deny decision (`user_consent`) on the authorization
/// request the hidden fields restate, and the `consent_token` binding an Allow
/// to the page it was rendered on.
#[derive(Debug, Deserialize)]
pub struct AuthorizeRequest {
    pub response_type: String,
    pub client_id: ClientId,
    pub redirect_uri: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub user_consent: Option<String>,
    pub resource: Option<String>,
    pub consent_token: Option<String>,
}
