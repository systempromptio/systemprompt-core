//! Request validation: turns an [`axum::http::HeaderMap`] into a
//! [`systemprompt_models::execution::context::RequestContext`] using a
//! configured JWT secret, issuer, and audience set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod hook_token;
mod validation;

pub use hook_token::{HookTokenValidator, ValidatedHookClaims};
pub use validation::AuthValidationService;
