mod featured;
mod link;
mod list;
mod unlink;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum ContentCommands {
    #[command(about = "Link a file to content with a specific role")]
    Link(link::LinkArgs),

    #[command(about = "Unlink a file from content")]
    Unlink(unlink::UnlinkArgs),

    #[command(about = "List files attached to content")]
    List(list::ListArgs),

    #[command(about = "Get or set the featured image for content")]
    Featured(featured::FeaturedArgs),
}

pub async fn execute(cmd: ContentCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        ContentCommands::Link(args) => {
            let result = link::execute(args, config)
                .await
                .context("Failed to link file to content")?;
            render_result(&result);
            Ok(())
        },
        ContentCommands::Unlink(args) => {
            let result = unlink::execute(args, config)
                .await
                .context("Failed to unlink file from content")?;
            render_result(&result);
            Ok(())
        },
        ContentCommands::List(args) => {
            let result = list::execute(args, config)
                .await
                .context("Failed to list content files")?;
            render_result(&result);
            Ok(())
        },
        ContentCommands::Featured(args) => {
            let result = featured::execute(args, config)
                .await
                .context("Failed to get/set featured image")?;
            render_result(&result);
            Ok(())
        },
    }
}
