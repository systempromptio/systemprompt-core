//! Bulk user operations applied by filter.
//!
//! [`BulkCommands`] exposes filter-driven delete and status-update commands,
//! dispatched by [`execute`]. Both are write operations requiring full profile
//! context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod delete;
mod update;

use crate::kernel::KernelHost;
use anyhow::Result;
use clap::Subcommand;
use crate::shared::CommandOutput;

#[derive(Debug, Subcommand)]
pub enum BulkCommands {
    #[command(about = "Bulk delete users by filter")]
    Delete(delete::DeleteArgs),

    #[command(about = "Bulk update user status by filter")]
    Update(update::UpdateArgs),
}

pub(super) async fn execute(cmd: BulkCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match cmd {
        BulkCommands::Delete(args) => {
            let result = delete::execute(args, host).await?;
            Ok(result)
        },
        BulkCommands::Update(args) => {
            let result = update::execute(args, host).await?;
            Ok(result)
        },
    }
}
