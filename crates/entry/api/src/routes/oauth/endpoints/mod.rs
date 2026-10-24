//! OAuth 2.0 / OIDC protocol endpoints.
//!
//! Collects every handler that backs the OAuth surface: [`authorize`],
//! [`token`], [`consent`], dynamic registration ([`register`],
//! [`client_config`]), introspection and revocation, [`userinfo`], [`logout`],
//! the [`anonymous`] grant, and the [`webauthn_complete`] step.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod anonymous;
pub mod authorize;
pub mod client_config;
pub mod consent;
pub mod introspect;
pub mod logout;
pub mod register;
pub mod revoke;
pub mod token;
pub mod userinfo;
pub mod webauthn_complete;

pub use anonymous::{AnonymousTokenRequest, AnonymousTokenResponse, generate_anonymous_token};
pub use authorize::{
    AuthorizeQuery, AuthorizeRequest, handle_authorize_get, handle_authorize_post, response_builder,
};
pub use client_config::{
    delete_client_configuration, get_client_configuration, update_client_configuration,
};
pub use consent::{
    ConsentQuery, ConsentRequest, ConsentResponse, handle_consent_get, handle_consent_post,
};
pub use introspect::{IntrospectRequest, IntrospectResponse, handle_introspect};
pub use logout::handle_logout;
pub use register::register_client;
pub use revoke::{RevokeRequest, handle_revoke};
pub use token::handle_token;
pub use userinfo::{UserinfoResponse, handle_userinfo};
pub use webauthn_complete::{
    WebAuthnCompleteQuery, WebAuthnCompleteResponse, handle_webauthn_complete,
};
