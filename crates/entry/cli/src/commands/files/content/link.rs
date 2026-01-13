use anyhow::{anyhow, Result};
use clap::{Args, ValueEnum};
use systemprompt_core_database::DbPool;
use systemprompt_core_files::{ContentService, FileRole};
use systemprompt_identifiers::{ContentId, FileId};

use crate::commands::files::types::ContentLinkOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FileRoleArg {
    Featured,
    Attachment,
    Inline,
    OgImage,
    Thumbnail,
}

impl From<FileRoleArg> for FileRole {
    fn from(role: FileRoleArg) -> Self {
        match role {
            FileRoleArg::Featured => FileRole::Featured,
            FileRoleArg::Attachment => FileRole::Attachment,
            FileRoleArg::Inline => FileRole::Inline,
            FileRoleArg::OgImage => FileRole::OgImage,
            FileRoleArg::Thumbnail => FileRole::Thumbnail,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct LinkArgs {
    #[arg(help = "File ID")]
    pub file_id: String,

    #[arg(long, help = "Content ID")]
    pub content: String,

    #[arg(long, value_enum, help = "File role")]
    pub role: FileRoleArg,

    #[arg(long, default_value = "0", help = "Display order")]
    pub order: i32,
}

pub async fn execute(
    args: LinkArgs,
    _config: &CliConfig,
) -> Result<CommandResult<ContentLinkOutput>> {
    let db = DbPool::from_env().await?;
    let service = ContentService::new(&db)?;

    let file_id = FileId::new(args.file_id.clone());
    let content_id = ContentId::new(args.content.clone());
    let role: FileRole = args.role.into();

    service
        .link_to_content(&content_id, &file_id, role, args.order)
        .await?;

    let output = ContentLinkOutput {
        file_id,
        content_id,
        role: role.as_str().to_string(),
        message: "File linked to content successfully".to_string(),
    };

    Ok(CommandResult::card(output).with_title("File Linked"))
}
