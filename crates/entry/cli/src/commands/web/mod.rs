pub mod assets;
pub mod content_types;
pub mod sitemap;
pub mod templates;
pub mod types;
mod validate;

use anyhow::Result;
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::shared::render_result;
use crate::CliConfig;

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

pub async fn execute(command: WebCommands) -> Result<()> {
    let config = get_global_config();
    execute_with_config(command, &config).await
}

pub async fn execute_with_config(command: WebCommands, config: &CliConfig) -> Result<()> {
    match command {
        WebCommands::ContentTypes(cmd) => content_types::execute(cmd, config),
        WebCommands::Templates(cmd) => templates::execute(cmd, config),
        WebCommands::Assets(cmd) => assets::execute(cmd, config),
        WebCommands::Sitemap(cmd) => sitemap::execute(cmd, config).await,
        WebCommands::Validate(args) => {
            let result = validate::execute(&args, config)?;
            render_result(&result);
            Ok(())
        },
    }
}
