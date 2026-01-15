mod generate;
mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum SitemapCommands {
    #[command(about = "Show sitemap configuration")]
    Show(show::ShowArgs),

    #[command(about = "Generate sitemap.xml")]
    Generate(generate::GenerateArgs),
}

pub fn execute(command: SitemapCommands, config: &CliConfig) -> Result<()> {
    match command {
        SitemapCommands::Show(args) => {
            let result = show::execute(args, config).context("Failed to show sitemap")?;
            render_result(&result);
            Ok(())
        },
        SitemapCommands::Generate(args) => {
            let result = generate::execute(&args, config).context("Failed to generate sitemap")?;
            render_result(&result);
            Ok(())
        },
    }
}
