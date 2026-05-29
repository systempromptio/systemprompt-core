//! JWT request-context extraction.
//!
//! Provides [`JwtContextExtractor`], which validates bearer tokens and derives
//! a request context, together with the [`JtiRevocationChecker`] it consults to
//! reject revoked token identifiers.

mod context;
mod params;
mod revocation;
mod validation;

pub use context::JwtContextExtractor;
pub use revocation::JtiRevocationChecker;
pub use systemprompt_security::JwtUserContext;
