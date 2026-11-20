//! `core files ai list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use systemprompt_files::FileRepository;
use systemprompt_identifiers::UserId;

use crate::cli::types::AiFilesListOutput;
use systemprompt_extension::cli::{CommandHost, CommandOutput, FileSummary};

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,

    #[arg(long, help = "Filter by user ID")]
    pub user: Option<String>,
}

pub(super) async fn execute(args: ListArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    execute_with_repository(args, &*crate::cli::file_repository(host).await?).await
}

pub async fn execute_with_repository(
    args: ListArgs,
    service: &FileRepository,
) -> Result<CommandOutput> {
    let files = match &args.user {
        Some(user_id) => {
            let user_id = UserId::new(user_id.clone());
            service
                .list_ai_images_by_user(&user_id, args.limit, args.offset)
                .await?
        },
        None => service.list_ai_images(args.limit, args.offset).await?,
    };

    let files: Vec<FileSummary> = files
        .into_iter()
        .map(|f| FileSummary {
            id: f.id,
            path: f.path,
            public_url: f.public_url,
            mime_type: f.mime_type,
            size_bytes: f.size_bytes,
            ai_content: f.ai_content,
            created_at: f.created_at,
        })
        .collect();

    let total = files.len() as i64;

    let output = AiFilesListOutput {
        files,
        total,
        limit: args.limit,
        offset: args.offset,
    };

    Ok(CommandOutput::table_of(
        vec!["id", "path", "mime_type", "size_bytes", "created_at"],
        &output.files,
    )
    .with_title("AI-Generated Images"))
}
