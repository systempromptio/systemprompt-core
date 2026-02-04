use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_runtime::AppContext;
use systemprompt_users::BannedIpRepository;

use crate::commands::admin::users::types::BanCleanupOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Args)]
pub struct CleanupArgs {
    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(
    args: CleanupArgs,
    _config: &CliConfig,
) -> Result<CommandResult<BanCleanupOutput>> {
    let ctx = AppContext::new().await?;
    let ban_repository = BannedIpRepository::new(ctx.db_pool())?;

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

    Ok(CommandResult::text(output).with_title("Ban Cleanup"))
}
