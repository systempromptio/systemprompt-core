use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_files::FileRepository;
use systemprompt_identifiers::{FileId, UserId};
use systemprompt_runtime::AppContext;

use crate::CliConfig;
use crate::commands::core::files::types::{AiFilesListOutput, FileSummary};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,

    #[arg(long, help = "Filter by user ID")]
    pub user: Option<String>,
}

pub(super) async fn execute(args: ListArgs, config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let service = FileRepository::new(pool)?;

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
            id: FileId::new(f.id.to_string()),
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
