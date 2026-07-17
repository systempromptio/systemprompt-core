//! `core content files` command group: associate stored files with content.
//!
//! Dispatches the [`ContentFilesCommands`] subcommands (link, unlink, list,
//! featured) that manage the content-to-file relationships and the featured
//! image for a content item.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod featured;
pub mod link;
pub mod list;
pub mod unlink;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::CliConfig;
use crate::interactive::Prompter;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum ContentFilesCommands {
    #[command(about = "Link a file to content with a specific role")]
    Link(link::LinkArgs),

    #[command(about = "Unlink a file from content")]
    Unlink(unlink::UnlinkArgs),

    #[command(about = "List files attached to content")]
    List(list::ListArgs),

    #[command(about = "Get or set the featured image for content")]
    Featured(featured::FeaturedArgs),
}

pub async fn execute(
    cmd: ContentFilesCommands,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<()> {
    match cmd {
        ContentFilesCommands::Link(args) => {
            let result = link::execute(args, config)
                .await
                .context("Failed to link file to content")?;
            render_result(&result, config);
            Ok(())
        },
        ContentFilesCommands::Unlink(args) => {
            let result = unlink::execute(args, prompter, config)
                .await
                .context("Failed to unlink file from content")?;
            render_result(&result, config);
            Ok(())
        },
        ContentFilesCommands::List(args) => {
            let result = list::execute(args, config)
                .await
                .context("Failed to list content files")?;
            render_result(&result, config);
            Ok(())
        },
        ContentFilesCommands::Featured(args) => {
            let result = featured::execute(args, config)
                .await
                .context("Failed to get/set featured image")?;
            render_result(&result, config);
            Ok(())
        },
    }
}
