use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use crate::commands::admin::users::types::SessionCleanupOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Args)]
pub struct CleanupArgs {
    #[arg(long, default_value = "30")]
    pub days: i32,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(
    args: CleanupArgs,
    _config: &CliConfig,
) -> Result<CommandResult<SessionCleanupOutput>> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    if !args.yes {
        return Err(anyhow!(
            "This will delete anonymous users older than {} days. Use --yes to confirm.",
            args.days
        ));
    }

    let cleaned = user_service.cleanup_old_anonymous(args.days).await?;

    let output = SessionCleanupOutput {
        cleaned,
        message: format!(
            "Cleaned up {} anonymous user(s) older than {} days",
            cleaned, args.days
        ),
    };

    Ok(CommandResult::text(output).with_title("Session Cleanup"))
}
