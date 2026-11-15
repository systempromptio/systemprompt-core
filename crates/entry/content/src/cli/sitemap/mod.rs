//! `web sitemap`: sitemap inspection and generation for the web content
//! config.
//!
//! [`SitemapCommands`] shows the configured routes or generates a
//! `sitemap.xml`. It registers under the kernel's `web` group through
//! `register_cli_command!`, needs the profile, secrets and paths, and leaves
//! the data behind the profile unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod generate;
pub mod show;
pub mod types;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_cli::kernel::{
    CliCommandGroup, CliConfig, CommandContext, CommandDescriptor, DataImpact, DescribeCommand,
    render_result,
};
use systemprompt_manifest::Profile;

#[derive(Debug, Subcommand)]
pub enum SitemapCommands {
    #[command(about = "Show sitemap configuration", alias = "list")]
    Show(show::ShowArgs),

    #[command(about = "Generate sitemap.xml")]
    Generate(generate::GenerateArgs),
}

pub fn execute(command: SitemapCommands, config: &CliConfig, profile: &Profile) -> Result<()> {
    match command {
        SitemapCommands::Show(args) => {
            let result = show::execute(args, config, profile).context("Failed to show sitemap")?;
            render_result(&result, config);
            Ok(())
        },
        SitemapCommands::Generate(args) => {
            let result =
                generate::execute(&args, config, profile).context("Failed to generate sitemap")?;
            render_result(&result, config);
            Ok(())
        },
    }
}

impl DescribeCommand for SitemapCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::PROFILE_SECRETS_AND_PATHS.with_data_impact(DataImpact::Preserving)
    }
}

impl CliCommandGroup for SitemapCommands {
    const PARENT: &'static [&'static str] = &["web"];
    const NAME: &'static str = "sitemap";
    const ABOUT: &'static str = "Sitemap operations";

    fn run(self, ctx: &CommandContext) -> impl Future<Output = Result<()>> + Send {
        let outcome = ctx
            .profile()
            .and_then(|profile| execute(self, &ctx.cli, profile));
        async move { outcome }
    }
}

systemprompt_cli::register_cli_command!(SitemapCommands);
