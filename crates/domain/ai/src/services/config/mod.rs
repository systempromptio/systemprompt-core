//! Configuration validation for [`systemprompt_manifest::services::AiConfig`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod error;
pub mod validator;

pub use error::AiConfigError;
pub use validator::ConfigValidator;
