//! The `web` command group: configuration management for the static-site layer.
//!
//! Routes [`WebCommands`] to the content-type, template, asset and validation
//! subcommands, each operating on the on-disk web content config. Capabilities
//! register further subgroups under `web` (such as `web sitemap`) through
//! `register_cli_command!`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod assets;
pub mod content_types;
pub mod paths;
pub mod templates;
pub mod types;
pub mod validate;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::descriptor::{CommandDescriptor, DescribeCommand};
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Subcommand)]
pub enum WebCommands {
    #[command(subcommand, about = "Manage content types")]
    ContentTypes(content_types::ContentTypesCommands),

    #[command(subcommand, about = "Manage templates")]
    Templates(templates::TemplatesCommands),

    #[command(subcommand, about = "List and inspect assets")]
    Assets(assets::AssetsCommands),

    #[command(about = "Validate web configuration")]
    Validate(validate::ValidateArgs),
}

impl DescribeCommand for WebCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::PROFILE_SECRETS_AND_PATHS
    }
}

pub fn execute(command: WebCommands, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    match command {
        WebCommands::ContentTypes(cmd) => {
            content_types::execute(cmd, ctx.prompter(), config, ctx.profile()?)
        },
        WebCommands::Templates(cmd) => templates::execute(cmd, ctx.prompter(), config, ctx.boot()?),
        WebCommands::Assets(cmd) => assets::execute(cmd, config, ctx.boot()?),
        WebCommands::Validate(args) => {
            let output = validate::execute(&args, config, ctx.boot()?)?;
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
