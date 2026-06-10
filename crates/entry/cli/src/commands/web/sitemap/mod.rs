//! Sitemap inspection and generation for the web content config.
//!
//! Dispatches the `web sitemap` subcommands ([`SitemapCommands`]) to show the
//! configured routes or generate a `sitemap.xml`.

mod generate;
mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::CliConfig;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum SitemapCommands {
    #[command(about = "Show sitemap configuration", alias = "list")]
    Show(show::ShowArgs),

    #[command(about = "Generate sitemap.xml")]
    Generate(generate::GenerateArgs),
}

pub fn execute(command: SitemapCommands, config: &CliConfig) -> Result<()> {
    match command {
        SitemapCommands::Show(args) => {
            let result = show::execute(args, config).context("Failed to show sitemap")?;
            render_result(&result, config);
            Ok(())
        },
        SitemapCommands::Generate(args) => {
            let result = generate::execute(&args, config).context("Failed to generate sitemap")?;
            render_result(&result, config);
            Ok(())
        },
    }
}
