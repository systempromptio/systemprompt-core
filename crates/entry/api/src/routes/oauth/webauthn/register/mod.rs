//! `WebAuthn` passkey registration flow.
//!
//! Exposes the paired [`start_register`]/[`finish_register`] ceremony that
//! enrols a new user's first credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod finish;
mod start;

pub use finish::finish_register;
pub use start::start_register;

use axum::http::StatusCode;
use systemprompt_models::Config;

use crate::routes::oauth::OAuthHttpError;

// Why: `security.allow_registration` must gate the endpoint itself, not just
// the authorize page's "register" link — this route creates users and is
// mounted publicly.
fn ensure_registration_enabled() -> Result<(), OAuthHttpError> {
    let allowed = Config::get().map_or(true, |c| c.allow_registration);
    if allowed {
        Ok(())
    } else {
        Err(OAuthHttpError::access_denied("registration_disabled")
            .with_status(StatusCode::FORBIDDEN))
    }
}
