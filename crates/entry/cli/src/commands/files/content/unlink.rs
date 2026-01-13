use anyhow::Result;
use clap::Args;
use systemprompt_core_database::DbPool;
use systemprompt_core_files::ContentService;
use systemprompt_identifiers::{ContentId, FileId};

use crate::commands::files::types::ContentUnlinkOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct UnlinkArgs {
    #[arg(help = "File ID")]
    pub file_id: String,

    #[arg(long, help = "Content ID")]
    pub content: String,
}

pub async fn execute(args: UnlinkArgs, _config: &CliConfig) -> Result<CommandResult<ContentUnlinkOutput>> {
    let db = DbPool::from_env().await?;
    let service = ContentService::new(&db)?;

    let file_id = FileId::new(args.file_id.clone());
    let content_id = ContentId::new(args.content.clone());

    service.unlink_from_content(&content_id, &file_id).await?;

    let output = ContentUnlinkOutput {
        file_id,
        content_id,
        message: "File unlinked from content successfully".to_string(),
    };

    Ok(CommandResult::card(output).with_title("File Unlinked"))
}
