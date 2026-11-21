//! The marketplace capability's commands: `core marketplace`,
//! `core plugins` and `core skills`.
//!
//! `marketplace-http` contributes the three groups under the kernel's
//! `core` group, each requesting `KernelGrant::ServicesTree`, the one grant
//! their reserved CLI rows list. Every command reads the services tree only
//! through [`CommandHost::services_tree`] and the loaded services
//! configuration only through the open `ServicesState` seed, never the
//! profile:
//!
//! | group | descriptor | database |
//! |---|---|---|
//! | `core plugins` | profile, secrets and paths | none (detached kernel) |
//! | `core marketplace explain` | full | none |
//! | `core marketplace import` | none | none |
//! | `core skills` | full | the owner-bound `Arc<OwnerSkillView>` the `managed_resources` member provides; disk only when it cannot be resolved |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod marketplace;
pub mod plugins;
pub mod skills;

use std::sync::Arc;

use anyhow::Result;
use systemprompt_extension::cli::{CommandError, CommandHost};
use systemprompt_loader::ServicesState;
use systemprompt_manifest::services::ServicesConfig;

pub use marketplace::MarketplaceCommands;
pub use plugins::PluginsCommands;
pub use skills::SkillsCommands;

async fn services_config(host: &dyn CommandHost) -> Result<Arc<ServicesConfig>> {
    Ok(host
        .resolved()
        .await?
        .get::<ServicesState>()?
        .view()
        .config())
}

fn command_error(error: anyhow::Error) -> CommandError {
    match error.downcast::<CommandError>() {
        Ok(failed) => failed,
        Err(error) => CommandError::new(error),
    }
}
