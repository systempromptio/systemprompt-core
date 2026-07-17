//! `core files list` command with MIME pattern filtering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_files::FileRepository;
use systemprompt_identifiers::{FileId, UserId};

use super::types::{FileListOutput, FileSummary};
use crate::CliConfig;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,

    #[arg(long, help = "Filter by user ID")]
    pub user: Option<String>,

    #[arg(long, visible_alias = "type", help = "Filter by MIME type pattern")]
    pub mime: Option<String>,
}

pub(super) async fn execute(args: ListArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(args, &ctx.db_pool().await?, &ctx.cli).await
}

pub(super) async fn execute_with_pool(
    args: ListArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let service = FileRepository::new(pool)?;

    let files = match &args.user {
        Some(user_id) => {
            let user_id = UserId::new(user_id.clone());
            service
                .list_by_user(&user_id, args.limit, args.offset)
                .await?
        },
        None => service.list_all(args.limit, args.offset).await?,
    };

    let files: Vec<FileSummary> = files
        .into_iter()
        .filter(|f| {
            args.mime
                .as_ref()
                .is_none_or(|pattern| matches_mime_pattern(&f.mime_type, pattern))
        })
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

    let output = FileListOutput {
        files,
        total,
        limit: args.limit,
        offset: args.offset,
    };

    Ok(CommandOutput::table_of(
        vec![
            "id",
            "path",
            "mime_type",
            "size_bytes",
            "ai_content",
            "created_at",
        ],
        &output.files,
    )
    .with_title("Files"))
}

fn matches_mime_pattern(mime_type: &str, pattern: &str) -> bool {
    if pattern.ends_with("/*") {
        let prefix = pattern.trim_end_matches("/*");
        mime_type.starts_with(prefix)
    } else {
        mime_type == pattern
    }
}
