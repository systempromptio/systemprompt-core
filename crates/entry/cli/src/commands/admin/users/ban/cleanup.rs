use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_users::BannedIpRepository;

use crate::commands::admin::users::types::BanCleanupOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct CleanupArgs {
    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub(super) async fn execute(args: CleanupArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let ban_repository = BannedIpRepository::new(&pool)?;

    if !args.yes {
        return Err(anyhow!(
            "This will delete all expired bans. Use --yes to confirm."
        ));
    }

    let cleaned = ban_repository.cleanup_expired().await?;

    let output = BanCleanupOutput {
        cleaned,
        message: format!("Cleaned up {} expired ban(s)", cleaned),
    };

    Ok(CommandOutput::card_value("Ban Cleanup", &output))
}
