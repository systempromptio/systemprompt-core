//! `admin users ban remove` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_users::BannedIpRepository;

use crate::commands::admin::users::types::BanRemoveOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct RemoveArgs {
    pub ip: String,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub(super) async fn execute(args: RemoveArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    if !args.yes {
        return Err(anyhow!(
            "This will remove the IP ban. Use --yes to confirm."
        ));
    }

    let pool = ctx.db_pool().await?;
    let ban_repository = BannedIpRepository::new(&pool)?;

    let removed = ban_repository.unban_ip(&args.ip).await?;

    let output = BanRemoveOutput {
        ip_address: args.ip.clone(),
        removed,
        message: if removed {
            format!("IP address '{}' has been unbanned", args.ip)
        } else {
            format!("IP address '{}' was not banned", args.ip)
        },
    };

    Ok(CommandOutput::card_value("Ban Removed", &output))
}
