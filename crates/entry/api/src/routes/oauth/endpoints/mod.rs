//! OAuth 2.0 / OIDC protocol endpoints.
//!
//! Collects every handler that backs the OAuth surface: [`authorize`],
//! [`token`], [`callback`], [`consent`], dynamic registration ([`register`],
//! [`client_config`]), introspection and revocation, [`userinfo`], [`logout`],
//! the [`anonymous`] grant, and the [`webauthn_complete`] step.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod anonymous;
pub mod authorize;
pub mod callback;
pub mod client_config;
pub mod consent;
pub mod introspect;
pub mod logout;
pub mod register;
pub mod revoke;
pub mod token;
pub mod userinfo;
pub mod webauthn_complete;

pub use anonymous::*;
pub use authorize::{
    AuthorizeQuery, AuthorizeRequest, handle_authorize_get, handle_authorize_post, response_builder,
};
pub use callback::*;
pub use client_config::*;
pub use consent::*;
pub use introspect::*;
pub use logout::handle_logout;
pub use register::*;
pub use revoke::*;
pub use token::{TokenError, TokenResult, generation, handle_token};
pub use userinfo::*;
pub use webauthn_complete::*;
