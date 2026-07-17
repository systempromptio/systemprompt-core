//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::{Args, ValueEnum};
use systemprompt_database::DbPool;
use systemprompt_files::{FileRepository, FileRole};
use systemprompt_identifiers::{ContentId, FileId};
use systemprompt_runtime::AppContext;

use crate::CliConfig;
use crate::commands::core::files::types::ContentLinkOutput;
use crate::shared::CommandOutput;

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
            FileRoleArg::Featured => Self::Featured,
            FileRoleArg::Attachment => Self::Attachment,
            FileRoleArg::Inline => Self::Inline,
            FileRoleArg::OgImage => Self::OgImage,
            FileRoleArg::Thumbnail => Self::Thumbnail,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct LinkArgs {
    #[arg(value_name = "FILE_ID", help = "File ID")]
    pub file: String,

    #[arg(long, help = "Content ID")]
    pub content: String,

    #[arg(long, value_enum, help = "File role")]
    pub role: FileRoleArg,

    #[arg(long, default_value = "0", help = "Display order")]
    pub order: i32,
}

pub(super) async fn execute(args: LinkArgs, config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: LinkArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let service = FileRepository::new(pool)?;

    let file_id = FileId::new(args.file.clone());
    let content_id = ContentId::new(args.content.clone());
    let role: FileRole = args.role.into();

    service
        .link_to_content(&content_id, &file_id, role, args.order)
        .await?;

    let output = ContentLinkOutput {
        file_id,
        content_id,
        role: role.as_str().to_owned(),
        message: "File linked to content successfully".to_owned(),
    };

    Ok(CommandOutput::card_value("File Linked", &output))
}
