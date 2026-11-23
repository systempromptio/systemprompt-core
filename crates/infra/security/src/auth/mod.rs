//! Hook-token validation: the governance hook credential a client presents,
//! verified against the deployment's keys.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod hook_token;

pub use hook_token::{HookTokenValidator, ValidatedHookClaims};
