//! JWT request-context extraction.
//!
//! Provides [`JwtContextExtractor`], which validates bearer tokens and derives
//! a request context, together with the [`JtiRevocationChecker`] it consults to
//! reject revoked token identifiers.

mod context;
mod params;
mod revocation;
mod validation;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::params::{BuildContextParams, build_context, extract_common_headers};
    pub use super::validation::{UserCache, ValidatedUser, user_is_admin, validate_user_exists};
}

pub use context::JwtContextExtractor;
pub use revocation::JtiRevocationChecker;
pub use systemprompt_security::JwtUserContext;
