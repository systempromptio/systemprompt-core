//! The `web` command group: configuration management for the static-site layer.
//!
//! Routes [`WebCommands`] to the content-type, template, asset, sitemap, and
//! validation subcommands, each operating on the on-disk web content config.

pub mod assets;
pub mod content_types;
pub mod paths;
pub mod sitemap;
pub mod templates;
pub mod types;
pub mod validate;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Subcommand)]
pub enum WebCommands {
    #[command(subcommand, about = "Manage content types")]
    ContentTypes(content_types::ContentTypesCommands),

    #[command(subcommand, about = "Manage templates")]
    Templates(templates::TemplatesCommands),

    #[command(subcommand, about = "List and inspect assets")]
    Assets(assets::AssetsCommands),

    #[command(subcommand, about = "Sitemap operations")]
    Sitemap(sitemap::SitemapCommands),

    #[command(about = "Validate web configuration")]
    Validate(validate::ValidateArgs),
}

pub fn execute(command: WebCommands, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    match command {
        WebCommands::ContentTypes(cmd) => content_types::execute(cmd, ctx.prompter(), config),
        WebCommands::Templates(cmd) => templates::execute(cmd, ctx.prompter(), config),
        WebCommands::Assets(cmd) => assets::execute(cmd, config),
        WebCommands::Sitemap(cmd) => sitemap::execute(cmd, config),
        WebCommands::Validate(args) => {
            let output = validate::execute(&args, config)?;
            let valid = output.valid;
            let error_count = output.errors.len();
            render_result(
                &CommandOutput::card_value("Web Configuration Validation", &output),
                config,
            );
            if !valid {
                return Err(anyhow::anyhow!(
                    "web configuration is invalid: {error_count} error(s), see report above",
                ));
            }
            Ok(())
        },
    }
}
