//! `core content files link` command attaching a file to content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_extension::cli::{CliConfig, CommandHost, CommandOutput};
use systemprompt_extension::content_files::{ContentFileLinks, ContentFileRole};
use systemprompt_identifiers::{ContentId, FileId};

use crate::cli::content::types::ContentLinkOutput;

#[derive(Debug, Clone, Args)]
pub struct LinkArgs {
    #[arg(value_name = "FILE_ID", help = "File ID")]
    pub file: String,

    #[arg(long, help = "Content ID")]
    pub content: String,

    #[arg(long, value_enum, help = "File role")]
    pub role: ContentFileRole,

    #[arg(long, default_value = "0", help = "Display order")]
    pub order: i32,
}

pub(super) async fn execute(args: LinkArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let links = super::content_file_links(host).await?;
    execute_with_links(args, links.as_ref(), host.output()).await
}

pub async fn execute_with_links(
    args: LinkArgs,
    links: &dyn ContentFileLinks,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let file_id = FileId::try_new(&args.file)
        .map_err(|e| anyhow!("Invalid file ID '{}': {}", args.file, e))?;
    let content_id = ContentId::new(args.content.clone());
    let role = args.role;

    links.link(&content_id, &file_id, role, args.order).await?;

    let output = ContentLinkOutput {
        file_id,
        content_id,
        role: role.as_str().to_owned(),
        message: "File linked to content successfully".to_owned(),
    };

    Ok(CommandOutput::card_value("File Linked", &output))
}
