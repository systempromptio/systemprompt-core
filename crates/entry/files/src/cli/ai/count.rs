//! `core files ai count` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use systemprompt_files::FileRepository;
use systemprompt_identifiers::UserId;

use crate::cli::types::AiFilesCountOutput;
use systemprompt_extension::cli::{CommandHost, CommandOutput};

#[derive(Debug, Clone, Args)]
pub struct CountArgs {
    #[arg(
        long,
        help = "Filter by user ID (optional, counts all if not specified)"
    )]
    pub user: Option<String>,
}

pub async fn execute(args: CountArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    execute_with_repository(args, &*crate::cli::file_repository(host).await?).await
}

pub async fn execute_with_repository(
    args: CountArgs,
    service: &FileRepository,
) -> Result<CommandOutput> {

    let user_id = args.user.as_ref().map(|u| UserId::new(u.clone()));

    let count = match &user_id {
        Some(uid) => service.count_ai_images_by_user(uid).await?,
        None => service.count_ai_images().await?,
    };

    let output = AiFilesCountOutput { count, user_id };

    Ok(CommandOutput::card_value("AI Images Count", &output))
}
