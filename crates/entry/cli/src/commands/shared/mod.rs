//! Helpers shared across command groups.
//!
//! Currently exposes the [`ValidationIssue`] / [`ValidationOutput`] result
//! shapes used by the various `validate` subcommands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod validation;

pub use validation::{ValidationIssue, ValidationOutput};
