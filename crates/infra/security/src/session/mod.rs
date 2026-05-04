//! Session-scoped JWT minting and the validated-claims wrapper produced
//! by [`crate::auth::AuthValidationService`].

mod claims;
mod generator;

pub use claims::ValidatedSessionClaims;
pub use generator::{SessionGenerator, SessionParams};
