//! Helpers shared across command groups.
//!
//! Currently exposes the [`ValidationIssue`] / [`ValidationOutput`] result
//! shapes used by the various `validate` subcommands.

mod validation;

pub use validation::{ValidationIssue, ValidationOutput};
