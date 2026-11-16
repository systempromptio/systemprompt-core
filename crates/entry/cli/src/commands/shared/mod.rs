//! Helpers shared across command groups.
//!
//! Exposes the [`ValidationIssue`] / [`ValidationOutput`] result shapes
//! used by the various `validate` subcommands, [`rendered`] (the output a
//! command returns when it has already rendered through `CommandHost::render`)
//! and [`app_context`], the runtime context read from the kernel's resolved
//! state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod validation;

pub mod mcp_tools;

pub use validation::{ValidationIssue, ValidationOutput};

use std::sync::Arc;

use systemprompt_extension::cli::{CommandError, CommandHost, CommandOutput};
use systemprompt_runtime::AppContext;

#[must_use]
pub fn rendered() -> CommandOutput {
    CommandOutput::text(String::new()).with_skip_render()
}

pub async fn app_context(host: &dyn CommandHost) -> Result<Arc<AppContext>, CommandError> {
    host.resolved()
        .await?
        .get::<Arc<AppContext>>()
        .map_err(CommandError::new)
}
