use anyhow::Result;
use clap::Args;
use systemprompt_files::AiService;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;

use crate::commands::core::files::types::AiFilesCountOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct CountArgs {
    #[arg(
        long,
        help = "Filter by user ID (optional, counts all if not specified)"
    )]
    pub user: Option<String>,
}

pub async fn execute(
    args: CountArgs,
    _config: &CliConfig,
) -> Result<CommandResult<AiFilesCountOutput>> {
    let ctx = AppContext::new().await?;
    let service = AiService::new(ctx.db_pool())?;

    let user_id = args.user.as_ref().map(|u| UserId::new(u.clone()));

    let count = match &user_id {
        Some(uid) => service.count_ai_images_by_user(uid).await?,
        None => service.count_ai_images().await?,
    };

    let output = AiFilesCountOutput { count, user_id };

    Ok(CommandResult::card(output).with_title("AI Images Count"))
}
