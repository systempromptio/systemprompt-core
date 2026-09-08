//! JWT request-context extraction.
//!
//! Provides [`JwtContextExtractor`], which validates bearer tokens and derives
//! a request context, together with the [`JtiRevocationChecker`] it consults to
//! reject revoked token identifiers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod context;
pub mod params;
mod revocation;
pub mod validation;

pub use context::JwtContextExtractor;
pub use revocation::JtiRevocationChecker;
pub use systemprompt_security::JwtUserContext;
