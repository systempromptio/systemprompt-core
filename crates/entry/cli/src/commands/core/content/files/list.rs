use anyhow::{anyhow, Result};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_core_files::ContentService;
use systemprompt_identifiers::{ContentId, FileId};
use systemprompt_runtime::AppContext;

use crate::commands::core::files::types::{
    ContentFileRow, ContentFilesOutput, FileContentLinkRow, FileContentLinksOutput,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(long, help = "Filter by content ID (list files attached to content)")]
    pub content: Option<String>,

    #[arg(long, help = "Filter by file ID (list content linked to file)")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ListOutput {
    ContentFiles(ContentFilesOutput),
    FileContentLinks(FileContentLinksOutput),
}

pub async fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<ListOutput>> {
    let ctx = AppContext::new().await?;
    let service = ContentService::new(ctx.db_pool())?;

    match (&args.content, &args.file) {
        (Some(content_id_str), None) => {
            let content_id = ContentId::new(content_id_str.clone());
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

            Ok(CommandResult::table(ListOutput::ContentFiles(output))
                .with_title("Content Files")
                .with_columns(vec![
                    "file_id".to_string(),
                    "path".to_string(),
                    "mime_type".to_string(),
                    "role".to_string(),
                    "display_order".to_string(),
                ]))
        },
        (None, Some(file_id_str)) => {
            let file_id = parse_file_id(file_id_str)?;
            let links = service.list_content_by_file(&file_id).await?;

            let links: Vec<FileContentLinkRow> = links
                .into_iter()
                .map(|cf| FileContentLinkRow {
                    content_id: cf.content_id,
                    role: cf.role,
                    display_order: cf.display_order,
                    created_at: cf.created_at,
                })
                .collect();

            let output = FileContentLinksOutput { file_id, links };

            Ok(CommandResult::table(ListOutput::FileContentLinks(output))
                .with_title("File Content Links")
                .with_columns(vec![
                    "content_id".to_string(),
                    "role".to_string(),
                    "display_order".to_string(),
                    "created_at".to_string(),
                ]))
        },
        (Some(_), Some(_)) => Err(anyhow!(
            "Cannot specify both --content and --file. Use one or the other."
        )),
        (None, None) => Err(anyhow!(
            "Either --content or --file is required.\nUse --content <id> to list files attached \
             to content.\nUse --file <id> to list content linked to a file."
        )),
    }
}

fn parse_file_id(id: &str) -> Result<FileId> {
    uuid::Uuid::parse_str(id).map_err(|_| {
        anyhow!(
            "Invalid file ID format. Expected UUID like 'b75940ac-c50f-4d46-9fdd-ebb4970b2a7d', \
             got '{}'",
            id
        )
    })?;
    Ok(FileId::new(id.to_string()))
}
