use anyhow::Result;
use clap::Args;
use systemprompt_core_files::ContentService;
use systemprompt_identifiers::{ContentId, FileId};
use systemprompt_runtime::AppContext;

use crate::commands::files::types::{ContentFileRow, ContentFilesOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(help = "Content ID")]
    pub content_id: String,
}

pub async fn execute(
    args: ListArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ContentFilesOutput>> {
    let ctx = AppContext::new().await?;
    let service = ContentService::new(ctx.db_pool())?;

    let content_id = ContentId::new(args.content_id.clone());

    let files = service.list_files_by_content(&content_id).await?;

    let files: Vec<ContentFileRow> = files
        .into_iter()
        .map(|(file, content_file)| ContentFileRow {
            file_id: FileId::new(file.id.to_string()),
            path: file.path,
            mime_type: file.mime_type,
            role: content_file.role,
            display_order: content_file.display_order,
        })
        .collect();

    let output = ContentFilesOutput { content_id, files };

    Ok(CommandResult::table(output)
        .with_title("Content Files")
        .with_columns(vec![
            "file_id".to_string(),
            "path".to_string(),
            "mime_type".to_string(),
            "role".to_string(),
            "display_order".to_string(),
        ]))
}
