//! Session-scoped JWT minting and the validated-claims wrapper produced
//! by [`crate::auth::AuthValidationService`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod claims;
mod generator;

pub use claims::ValidatedSessionClaims;
pub use generator::{SessionGenerator, SessionParams};
