use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_files::FileService;
use systemprompt_identifiers::FileId;
use systemprompt_runtime::AppContext;

use super::types::{FileSearchOutput, FileSummary};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct SearchArgs {
    #[arg(help = "Search query (matches file paths)")]
    pub query: String,

    #[arg(long, default_value = "20", help = "Maximum number of results")]
    pub limit: i64,
}

pub async fn execute(
    args: SearchArgs,
    config: &CliConfig,
) -> Result<CommandResult<FileSearchOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: SearchArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<FileSearchOutput>> {
    if args.query.trim().is_empty() {
        return Err(anyhow!("Search query cannot be empty"));
    }

    let service = FileService::new(pool)?;

    let found_files = service.search_by_path(&args.query, args.limit).await?;

    let total = found_files.len() as i64;
    let files: Vec<FileSummary> = found_files
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

    let output = FileSearchOutput {
        files,
        query: args.query,
        total,
    };

    Ok(CommandResult::table(output)
        .with_title("File Search Results")
        .with_columns(vec![
            "id".to_string(),
            "path".to_string(),
            "mime_type".to_string(),
            "size_bytes".to_string(),
            "created_at".to_string(),
        ]))
}
