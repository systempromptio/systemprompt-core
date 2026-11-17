//! `web sitemap`: sitemap inspection and generation for the web content
//! config.
//!
//! [`SitemapCommands`] shows the configured routes or generates a
//! `sitemap.xml`. `content-http` contributes it under the kernel's `web`
//! group. It needs the profile, secrets and paths, and leaves
//! the data behind the profile unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod generate;
pub mod show;
pub mod types;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_extension::cli::{
    CliCommandGroup, CliConfig, CommandDescriptor, CommandError, CommandHost, CommandOutput,
    DataImpact, DescribeCommand,
};
use systemprompt_manifest::Profile;

#[derive(Debug, Subcommand)]
pub enum SitemapCommands {
    #[command(about = "Show sitemap configuration", alias = "list")]
    Show(show::ShowArgs),

    #[command(about = "Generate sitemap.xml")]
    Generate(generate::GenerateArgs),
}

pub fn execute(
    command: SitemapCommands,
    config: &CliConfig,
    profile: &Profile,
) -> Result<CommandOutput> {
    match command {
        SitemapCommands::Show(args) => {
            show::execute(args, config, profile).context("Failed to show sitemap")
        },
        SitemapCommands::Generate(args) => {
            generate::execute(&args, config, profile).context("Failed to generate sitemap")
        },
    }
}

impl DescribeCommand for SitemapCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::PROFILE_SECRETS_AND_PATHS.with_data_impact(DataImpact::Preserving)
    }
}

impl CliCommandGroup for SitemapCommands {
    const NAME: &'static str = "sitemap";
    const ABOUT: &'static str = "Sitemap operations";

    fn run(
        self,
        host: &dyn CommandHost,
    ) -> impl Future<Output = Result<CommandOutput, CommandError>> + Send {
        std::future::ready(
            host.profile().and_then(|profile| {
                execute(self, host.output(), profile).map_err(CommandError::new)
            }),
        )
    }
}
