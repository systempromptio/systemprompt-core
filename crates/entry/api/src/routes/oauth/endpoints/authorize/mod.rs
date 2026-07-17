//! OAuth 2.0 authorization endpoint.
//!
//! Hosts the GET/POST `/authorize` handlers and the request types
//! [`AuthorizeQuery`] and [`AuthorizeRequest`] they bind. The flow validates
//! the inbound request ([`validation`]), then renders the `WebAuthn` challenge
//! page ([`response_builder`]) rather than supporting password auth.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod handler;
pub mod response_builder;
pub mod validation;

pub use handler::{handle_authorize_get, handle_authorize_post};

use serde::Deserialize;
use systemprompt_identifiers::ClientId;

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
}

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
    pub username: Option<String>,
    pub password: Option<String>,
    pub resource: Option<String>,
}
