//! Helpers shared across command groups.
//!
//! Exposes the [`ValidationIssue`] / [`ValidationOutput`] result shapes
//! used by the various `validate` subcommands, and [`rendered`], the output
//! a command returns when it has already rendered through
//! `CommandHost::render`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod validation;

pub mod mcp_tools;

pub use validation::{ValidationIssue, ValidationOutput};

use systemprompt_extension::cli::CommandOutput;

#[must_use]
pub fn rendered() -> CommandOutput {
    CommandOutput::text(String::new()).with_skip_render()
}
