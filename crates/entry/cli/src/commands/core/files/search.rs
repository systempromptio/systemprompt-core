//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_files::FileRepository;
use systemprompt_identifiers::FileId;

use super::types::{FileSearchOutput, FileSummary};
use crate::CliConfig;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct SearchArgs {
    #[arg(help = "Search query (matches file paths)")]
    pub query: String,

    #[arg(long, default_value = "20", help = "Maximum number of results")]
    pub limit: i64,
}

pub(super) async fn execute(args: SearchArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(args, &ctx.db_pool().await?, &ctx.cli).await
}

pub(super) async fn execute_with_pool(
    args: SearchArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    if args.query.trim().is_empty() {
        return Err(anyhow!("Search query cannot be empty"));
    }

    let service = FileRepository::new(pool)?;

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

    Ok(CommandOutput::table_of(
        vec!["id", "path", "mime_type", "size_bytes", "created_at"],
        &output.files,
    )
    .with_title("File Search Results"))
}
