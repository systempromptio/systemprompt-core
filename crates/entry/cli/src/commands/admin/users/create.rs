use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use super::types::UserCreatedOutput;

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long)]
    pub name: String,

    #[arg(long)]
    pub email: String,

    #[arg(long)]
    pub full_name: Option<String>,

    #[arg(long)]
    pub display_name: Option<String>,
}

pub async fn execute(args: CreateArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    if args.name.trim().is_empty() {
        return Err(anyhow!("Name cannot be empty"));
    }

    if args.email.trim().is_empty() {
        return Err(anyhow!("Email cannot be empty"));
    }

    let user = user_service
        .create(
            &args.name,
            &args.email,
            args.full_name.as_deref(),
            args.display_name.as_deref(),
        )
        .await?;

    let output = UserCreatedOutput {
        id: user.id.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
        message: format!("User '{}' created successfully", user.name),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
        CliService::key_value("ID", output.id.as_str());
        CliService::key_value("Name", &output.name);
        CliService::key_value("Email", &output.email);
    }

    Ok(())
}
