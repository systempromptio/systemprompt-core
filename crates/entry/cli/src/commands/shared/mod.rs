//! Helpers shared across command groups.
//!
//! Exposes the [`ValidationIssue`] / [`ValidationOutput`] result shapes
//! used by the various `validate` subcommands, [`rendered`] (the output a
//! command returns when it has already rendered through `CommandHost::render`)
//! [`app_context`], the runtime context the kernel host builds, and [`services_config`], the services configuration under the
//! host's services root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod validation;

pub mod mcp_tools;

pub use validation::{ValidationIssue, ValidationOutput};

use std::sync::Arc;
use crate::kernel::KernelHost;

use systemprompt_extension::cli::{CommandError, CommandOutput};
use systemprompt_loader::ConfigLoader;
use systemprompt_manifest::ServicesConfig;
use systemprompt_runtime::AppContext;

#[must_use]
pub fn rendered() -> CommandOutput {
    CommandOutput::text(String::new()).with_skip_render()
}

pub async fn app_context(host: &dyn KernelHost) -> Result<Arc<AppContext>, CommandError> {
    host.app_context().await
}

pub fn services_config(host: &dyn KernelHost) -> anyhow::Result<ServicesConfig> {
    let profile = host.profile()?;
    let root = host.services_root()?;
    Ok(ConfigLoader::for_profile(profile, &root.path).load()?)
}
