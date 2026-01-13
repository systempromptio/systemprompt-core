use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::UserService;
use systemprompt_runtime::AppContext;

use crate::commands::users::types::SessionCleanupOutput;

#[derive(Debug, Args)]
pub struct CleanupArgs {
    #[arg(long, default_value = "30")]
    pub days: i32,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(args: CleanupArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    if !args.yes {
        CliService::warning(&format!(
            "This will delete anonymous users older than {} days. Use --yes to confirm.",
            args.days
        ));
        return Err(anyhow!("Operation cancelled - confirmation required"));
    }

    let cleaned = user_service.cleanup_old_anonymous(args.days).await?;

    let output = SessionCleanupOutput {
        cleaned,
        message: format!(
            "Cleaned up {} anonymous user(s) older than {} days",
            cleaned, args.days
        ),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}
