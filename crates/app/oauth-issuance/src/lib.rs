//! `systemprompt-oauth-issuance` — the OAuth 2.0 token-issuance workflow.
//!
//! The token endpoint's grants cross domains: they read clients, codes and
//! refresh tokens from `systemprompt-oauth`, resolve owners and delegates
//! through the user provider, bind an analytics session, and sign with the
//! `systemprompt-security` key authority under the profile's JWT settings.
//! This crate composes those into one workflow and leaves the HTTP surface to
//! the API crate:
//!
//! - [`TokenIssuanceOrchestrator`] dispatches a [`TokenRequest`] by
//!   `grant_type` — authorization-code, refresh-token rotation,
//!   client-credentials, RFC 8693 token-exchange and the RFC 7523 jwt-bearer
//!   ID-JAG redemption — and answers a [`TokenResponse`].
//! - [`client_credentials`] mints a token for a client acting as itself,
//!   intersecting scopes with the client grant and its owner's roles.
//! - [`token_exchange`] validates subject tokens and ID-JAGs, assembles the
//!   delegated `act` chain and issues ID-JAGs from upstream OIDC tokens.
//! - [`user_tokens`] mints the access/refresh pair for user-bound grants.
//!
//! Every failure is an [`IssuanceError`], partitioned by RFC 6749 error code.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod client_credentials;
pub mod error;
pub mod orchestrator;
pub mod request;
mod session;
pub mod token_exchange;
pub mod user_tokens;
pub mod validation;

pub use client_credentials::{ClientCredentialsError, ClientTokenOptions, generate_client_tokens};
pub use error::{IssuanceError, IssuanceResult};
pub use orchestrator::TokenIssuanceOrchestrator;
pub use request::{RequestOrigin, TokenRequest, TokenResponse};
pub use token_exchange::{
    TokenExchangeRequest, build_act_chain, handle_token_exchange, intersect_scopes, peek_issuer,
};
pub use user_tokens::{
    GeneratedTokens, UserTokenError, UserTokenParams, generate_tokens_by_user_id,
    resolve_user_permissions,
};
