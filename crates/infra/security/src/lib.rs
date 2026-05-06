//! Security infrastructure for systemprompt.io.
//!
//! Houses the request-level authentication primitives shared by the HTTP
//! API and the runtime layer:
//!
//! - JWT minting ([`jwt`]) for admin tokens and ([`session`]) for
//!   session-scoped tokens.
//! - Token extraction ([`extraction`]) from `Authorization` headers, MCP proxy
//!   headers, and cookies.
//! - Request validation ([`auth`]) that turns those tokens into a
//!   [`systemprompt_models::execution::context::RequestContext`].
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
//! use systemprompt_security::{AuthMode, AuthValidationService};
//!
//! # fn demo(headers: &axum::http::HeaderMap) -> systemprompt_security::AuthResult<()> {
//! let svc = AuthValidationService::new(
//!     "secret".to_string(),
//!     "systemprompt.io".to_string(),
//!     vec![JwtAudience::standard()],
//! );
//! let _ctx = svc.validate_request(headers, AuthMode::Required)?;
//! # Ok(())
//! # }
//! ```

pub mod auth;
pub mod authz;
pub mod error;
pub mod extraction;
pub mod jwt;
pub mod manifest_signing;
pub mod services;
pub mod session;

pub use auth::{AuthMode, AuthValidationService};
pub use error::{
    AuthError, AuthResult, JwtError, JwtResult, ManifestSigningError, ManifestSigningResult,
};
pub use extraction::{
    CookieExtractionError, CookieExtractor, ExtractionMethod, HeaderExtractor,
    HeaderInjectionError, HeaderInjector, TokenExtractionError, TokenExtractor,
};
pub use jwt::{AdminTokenParams, JwtService};
pub use services::ScannerDetector;
pub use session::{SessionGenerator, SessionParams, ValidatedSessionClaims};
