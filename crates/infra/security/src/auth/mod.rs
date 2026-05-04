//! Request validation: turns an [`axum::http::HeaderMap`] into a
//! [`systemprompt_models::execution::context::RequestContext`] using a
//! configured JWT secret, issuer, and audience set.

mod validation;

pub use validation::{AuthMode, AuthValidationService};
