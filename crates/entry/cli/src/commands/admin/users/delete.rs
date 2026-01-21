use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_logging::CliService;
use systemprompt_users::{UserAdminService, UserService};
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
    let admin_service = UserAdminService::new(user_service.clone());

    if !args.yes {
        CliService::warning("This will permanently delete the user. Use --yes to confirm.");
        return Err(anyhow!("Operation cancelled - confirmation required"));
    }

    let existing = admin_service.find_user(&args.user_id).await?;
    let Some(user) = existing else {
        CliService::error(&format!("User not found: {}", args.user_id));
        return Err(anyhow!("User not found"));
    };

    user_service.delete(&user.id).await?;

    let output = UserDeletedOutput {
        id: user.id.clone(),
        message: format!("User '{}' deleted successfully", user.name),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}
