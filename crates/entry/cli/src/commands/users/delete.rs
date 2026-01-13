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

    #[arg(long)]
    pub hard: bool,
}

pub async fn execute(args: DeleteArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    if !args.yes {
        CliService::warning("This will delete the user. Use --yes to confirm.");
        return Err(anyhow!("Operation cancelled - confirmation required"));
    }

    let user_id = UserId::new(&args.user_id);

    let existing = user_service.find_by_id(&user_id).await?;
    if existing.is_none() {
        CliService::error(&format!("User not found: {}", args.user_id));
        return Err(anyhow!("User not found"));
    }

    if args.hard {
        user_service.delete_anonymous(&user_id).await?;
    } else {
        user_service.delete(&user_id).await?;
    }

    let delete_type = if args.hard { "hard" } else { "soft" };
    let output = UserDeletedOutput {
        id: user_id.clone(),
        message: format!(
            "User '{}' {} deleted successfully",
            args.user_id, delete_type
        ),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}
