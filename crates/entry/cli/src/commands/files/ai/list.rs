use anyhow::Result;
use clap::Args;
use systemprompt_core_database::DbPool;
use systemprompt_core_files::AiService;
use systemprompt_identifiers::{FileId, UserId};

use crate::commands::files::types::{AiFilesListOutput, FileSummary};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,

    #[arg(long, help = "Filter by user ID")]
    pub user: Option<String>,
}

pub async fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<AiFilesListOutput>> {
    let db = DbPool::from_env().await?;
    let service = AiService::new(&db)?;

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

    Ok(CommandResult::table(output)
        .with_title("AI-Generated Images")
        .with_columns(vec![
            "id".to_string(),
            "path".to_string(),
            "mime_type".to_string(),
            "size_bytes".to_string(),
            "created_at".to_string(),
        ]))
}
