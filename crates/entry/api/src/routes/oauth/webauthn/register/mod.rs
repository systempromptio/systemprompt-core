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
use systemprompt_manifest::Config;

use crate::routes::oauth::OAuthHttpError;

fn ensure_registration_enabled() -> Result<(), OAuthHttpError> {
    if Config::get()?.allow_registration {
        Ok(())
    } else {
        Err(OAuthHttpError::access_denied("registration_disabled")
            .with_status(StatusCode::FORBIDDEN))
    }
}
