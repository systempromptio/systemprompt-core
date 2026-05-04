//! # systemprompt-oauth
//!
//! OAuth 2.0 / OIDC, `WebAuthn`, and JWT authentication primitives for the
//! systemprompt.io AI governance platform. The crate provides:
//!
//! - **OAuth 2.0 / OIDC** — PKCE authorization code flow, dynamic client
//!   registration, refresh-token rotation, and audience/issuer validation.
//! - **`WebAuthn`** — passkey registration and authentication backed by
//!   `webauthn-rs`.
//! - **JWT** — admin and anonymous-session token generation, signing and
//!   validation utilities.
//! - **CIMD** — Client-Initiated Metadata Discovery validation for federated
//!   OAuth clients.
//! - **Repositories** — `sqlx`-backed Postgres persistence for clients,
//!   authorisation codes, refresh tokens, setup tokens and `WebAuthn`
//!   credentials.
//!
//! ## Feature flags
//!
//! | Feature | Default | Effect |
//! |---------|---------|--------|
//! | _none_  | n/a     | The crate currently exposes a single feature surface; all OAuth, `WebAuthn`, JWT and CIMD modules are always compiled. |
//!
//! No optional feature flags are defined at present. The
//! `[package.metadata.docs.rs] all-features = true` setting is retained so
//! future feature additions automatically appear in published docs.
//!
//! ## Layering
//!
//! `systemprompt-oauth` is a **domain** crate. It depends only on
//! `shared` and `infra` crates and is consumed by `app` and `entry`
//! layers (HTTP handlers, CLI commands).
//!
//! ## Errors
//!
//! Public APIs return [`OauthResult`] / [`OauthError`]. Variants enumerate
//! the security-meaningful failure modes (invalid grant, expired code,
//! PKCE mismatch, client not found, etc.) so HTTP handlers can map them
//! to RFC 6749 / RFC 8628 / `WebAuthn` error codes without string parsing.

pub mod constants;
pub mod error;
pub(crate) mod extension;
pub mod models;
pub(crate) mod queries;
pub mod repository;
pub mod services;
pub(crate) mod state;

pub use error::{OauthError, OauthResult};
pub use extension::OauthExtension;

pub use models::*;
pub use repository::OAuthRepository;
pub use services::providers::JwtValidationProviderImpl;
pub use services::validation::jwt::validate_jwt_token;
pub use services::{
    AnonymousSessionInfo, CreateAnonymousSessionInput, SessionCreationError,
    SessionCreationService, TemplateEngine, TokenValidator, extract_bearer_token,
    extract_cookie_token, is_browser_request,
};

pub use state::OAuthState;
pub use systemprompt_models::auth::{AuthError, AuthenticatedUser, BEARER_PREFIX};
