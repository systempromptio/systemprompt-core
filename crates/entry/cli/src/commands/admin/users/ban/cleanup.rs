use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::BannedIpRepository;
use systemprompt_runtime::AppContext;

use crate::commands::admin::users::types::BanCleanupOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct CleanupArgs {
    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(args: CleanupArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let ban_repository = BannedIpRepository::new(ctx.db_pool())?;

    if !args.yes {
        CliService::warning("This will delete all expired bans. Use --yes to confirm.");
        return Err(anyhow!("Operation cancelled - confirmation required"));
    }

    let cleaned = ban_repository.cleanup_expired().await?;

    let output = BanCleanupOutput {
        cleaned,
        message: format!("Cleaned up {} expired ban(s)", cleaned),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}
