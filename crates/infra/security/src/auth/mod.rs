//! Request validation: turns an [`axum::http::HeaderMap`] into a
//! [`systemprompt_models::execution::context::RequestContext`] using a
//! configured JWT secret, issuer, and audience set.

mod hook_token;
mod validation;

pub use hook_token::{HookTokenValidator, ValidatedHookClaims};
pub use validation::{AuthMode, AuthValidationService};
