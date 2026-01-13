use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_database::DbPool;
use systemprompt_core_files::AiService;
use systemprompt_identifiers::UserId;

use crate::commands::files::types::AiFilesCountOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct CountArgs {
    #[arg(long, help = "Filter by user ID")]
    pub user: Option<String>,
}

pub async fn execute(
    args: CountArgs,
    _config: &CliConfig,
) -> Result<CommandResult<AiFilesCountOutput>> {
    let db = DbPool::from_env().await?;
    let service = AiService::new(&db)?;

    let user_id = args.user.as_ref().map(|u| UserId::new(u.clone()));

    let count = match &user_id {
        Some(uid) => service.count_ai_images_by_user(uid).await?,
        None => {
            return Err(anyhow!(
                "User ID is required for counting AI images. Use --user flag."
            ));
        },
    };

    let output = AiFilesCountOutput { count, user_id };

    Ok(CommandResult::card(output).with_title("AI Images Count"))
}
