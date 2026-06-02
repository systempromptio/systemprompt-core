use anyhow::Result;
use clap::Args;
use systemprompt_files::FileRepository;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;

use crate::CliConfig;
use crate::commands::core::files::types::AiFilesCountOutput;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct CountArgs {
    #[arg(
        long,
        help = "Filter by user ID (optional, counts all if not specified)"
    )]
    pub user: Option<String>,
}

pub(super) async fn execute(args: CountArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    let service = FileRepository::new(ctx.db_pool())?;

    let user_id = args.user.as_ref().map(|u| UserId::new(u.clone()));

    let count = match &user_id {
        Some(uid) => service.count_ai_images_by_user(uid).await?,
        None => service.count_ai_images().await?,
    };

    let output = AiFilesCountOutput { count, user_id };

    Ok(CommandOutput::card_value("AI Images Count", &output))
}
