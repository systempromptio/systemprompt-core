//! JWT plane.
//!
//! Two stateless surfaces, each a free function or unit-struct method that
//! never holds JWT state of its own:
//!
//! - [`mint`] — issues administrator-scoped RS256 tokens via
//!   [`JwtService::generate_admin_token`]. Session-scoped tokens are minted by
//!   [`crate::session::SessionGenerator`] instead.
//! - [`decode`] — turns a raw `Bearer …` string into a typed
//!   [`JwtUserContext`], enforcing kid + RS256, re-deriving `user_type` from
//!   `scope` (defence-in-depth against a forged claim), and surfacing every
//!   failure as an [`crate::AuthError`] variant.
//!
//! Issuer/audience/`nbf` validation for full session decode lives in
//! [`crate::AuthValidationService`]; the bare [`decode::extract_user_context`]
//! is used by request-context middleware that does its own session and user
//! lookups against the database after decode.

pub mod decode;
pub mod mint;
pub mod validate;

pub use decode::{JwtUserContext, extract_user_context};
pub use mint::{AdminTokenParams, JwtService};
pub use validate::{JWT_LEEWAY_SECONDS, ValidationPolicy, decode_rs256_claims};
