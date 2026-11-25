//! `core content files` command group: associate stored files with content.
//!
//! Dispatches the [`ContentFilesCommands`] subcommands (link, unlink, list,
//! featured) that manage the content-to-file relationships and the featured
//! image for a content item, over the `DynContentFileLinks` the `files`
//! member provides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod featured;
pub mod link;
pub mod list;
pub mod unlink;

use anyhow::{Context, Result};
use clap::Subcommand;

use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_extension::content_files::DynContentFileLinks;

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

pub async fn execute(cmd: ContentFilesCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match cmd {
        ContentFilesCommands::Link(args) => link::execute(args, host)
            .await
            .context("Failed to link file to content"),
        ContentFilesCommands::Unlink(args) => unlink::execute(args, host.prompter(), host)
            .await
            .context("Failed to unlink file from content"),
        ContentFilesCommands::List(args) => list::execute(args, host)
            .await
            .context("Failed to list content files"),
        ContentFilesCommands::Featured(args) => featured::execute(args, host)
            .await
            .context("Failed to get/set featured image"),
    }
}

async fn content_file_links(host: &dyn CommandHost) -> Result<DynContentFileLinks> {
    Ok(host.resolved().await?.get::<DynContentFileLinks>()?)
}
