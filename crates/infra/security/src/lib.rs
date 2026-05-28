//! Security infrastructure for systemprompt.io.
//!
//! Houses the request-level authentication primitives shared by the HTTP
//! API and the runtime layer:
//!
//! - Asymmetric signing key plane ([`keys`]) — the in-process `TokenAuthority`
//!   holds the active RSA keypair, exposes the public set for
//!   `/.well-known/jwks.json`, and caches federated JWKS documents under a
//!   bounded LRU with an HTTPS allowlist.
//! - JWT minting ([`jwt`]) for admin tokens and ([`session`]) for
//!   session-scoped tokens. Tokens are signed RS256 via `TokenAuthority` and
//!   carry a `kid` header; HS256 is rejected on validation.
//! - Token extraction ([`extraction`]) from `Authorization` headers, MCP proxy
//!   headers, and cookies.
//! - Request validation ([`auth`]) that turns those tokens into a
//!   [`systemprompt_models::execution::context::RequestContext`], resolving
//!   non-self-issued tokens against `profile.security.trusted_issuers` and
//!   propagating the RFC 8693 `act_chain` onto the per-request context.
//! - At-rest hashing ([`at_rest`]) — `hmac_sha256` / `hmac_sha256_hex` under
//!   the deployment `oauth_at_rest_pepper`, used to store refresh-token ids and
//!   authorisation codes as digests rather than plaintext.
//! - Bridge manifest signing ([`manifest_signing`]) with Ed25519 keys.
//! - Lightweight scanner / bot detection ([`services`]).
//! - Authorization decision plane ([`authz`]) — deny-overrides resolver,
//!   `access_control_rules` repository, and `AuthzDecisionHook` extension
//!   surface shared by the gateway and MCP enforcement sites.
//!
//! All public fallible APIs return typed errors from [`error`] — `anyhow`
//! is not used in any public signature.
//!
//! # Feature flags
//!
//! This crate has no Cargo features; everything compiles by default.
//!
//! # Example
//!
//! ```no_run
//! use systemprompt_models::auth::JwtAudience;
//! use systemprompt_security::AuthValidationService;
//!
//! # fn demo(headers: &axum::http::HeaderMap) -> systemprompt_security::AuthResult<()> {
//! let svc = AuthValidationService::new("systemprompt.io".to_string(), JwtAudience::standard());
//! let _ctx = svc.validate_request(headers)?;
//! # Ok(())
//! # }
//! ```

pub mod at_rest;
pub mod auth;
pub mod authz;
pub mod error;
pub mod extraction;
pub mod jwt;
pub mod keys;
pub mod manifest_signing;
pub mod policy;
pub mod services;
pub mod session;

pub use at_rest::{hmac_sha256, hmac_sha256_hex};

pub use auth::{AuthValidationService, HookTokenValidator, ValidatedHookClaims};
pub use authz::CompositeAuthzHook;
pub use error::{
    AuthError, AuthResult, JwtError, JwtResult, ManifestSigningError, ManifestSigningResult,
};
pub use extraction::{
    CookieExtractionError, CookieExtractor, ExtractionMethod, HeaderExtractor,
    HeaderInjectionError, HeaderInjector, TokenExtractionError, TokenExtractor,
};
pub use jwt::{AdminTokenParams, JwtService, JwtUserContext, extract_user_context};
pub use services::ScannerDetector;
pub use session::{SessionGenerator, SessionParams, ValidatedSessionClaims};
