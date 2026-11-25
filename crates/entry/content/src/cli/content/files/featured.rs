//! `core content files featured` command setting the featured image.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_extension::cli::{CliConfig, CommandHost, CommandOutput, FileSummary};
use systemprompt_extension::content_files::ContentFileLinks;
use systemprompt_identifiers::{ContentId, FileId};

use crate::cli::content::types::FeaturedImageOutput;

#[derive(Debug, Clone, Args)]
pub struct FeaturedArgs {
    #[arg(value_name = "CONTENT_ID", help = "Content ID")]
    pub content: String,

    #[arg(long, help = "Set featured image")]
    pub set: Option<String>,
}

pub(super) async fn execute(args: FeaturedArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let links = super::content_file_links(host).await?;
    execute_with_links(args, links.as_ref(), host.output()).await
}

pub async fn execute_with_links(
    args: FeaturedArgs,
    links: &dyn ContentFileLinks,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let content_id = ContentId::new(args.content.clone());

    if let Some(file_id_str) = args.set {
        let file_id = FileId::try_new(&file_id_str)
            .map_err(|e| anyhow!("Invalid file ID '{}': {}", file_id_str, e))?;
        links.set_featured(&content_id, &file_id).await?;

        let output = FeaturedImageOutput {
            content_id,
            file: None,
            message: "Featured image set successfully".to_owned(),
        };

        return Ok(CommandOutput::card_value("Featured Image Set", &output));
    }

    let file = links.featured(&content_id).await?;

    let file_summary = file.map(|f| FileSummary {
        id: f.file_id,
        path: f.path,
        public_url: f.public_url,
        mime_type: f.mime_type,
        size_bytes: f.size_bytes,
        ai_content: f.ai_content,
        created_at: f.created_at,
    });

    let message = file_summary.as_ref().map_or_else(
        || "No featured image set".to_owned(),
        |f| format!("Featured image: {}", f.path),
    );

    let output = FeaturedImageOutput {
        content_id,
        file: file_summary,
        message,
    };

    Ok(CommandOutput::card_value("Featured Image", &output))
}
