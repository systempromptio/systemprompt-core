use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::UserService;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;

use super::types::UserDeletedOutput;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub user_id: String,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(args: DeleteArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    if !args.yes {
        CliService::warning("This will permanently delete the user. Use --yes to confirm.");
        return Err(anyhow!("Operation cancelled - confirmation required"));
    }

    let user_id = UserId::new(&args.user_id);

    let existing = user_service.find_by_id(&user_id).await?;
    if existing.is_none() {
        CliService::error(&format!("User not found: {}", args.user_id));
        return Err(anyhow!("User not found"));
    }

    user_service.delete(&user_id).await?;

    let output = UserDeletedOutput {
        id: user_id.to_string(),
        message: format!("User '{}' deleted successfully", args.user_id),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}
