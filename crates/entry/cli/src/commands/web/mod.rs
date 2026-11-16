//! The `web` command group: configuration management for the static-site layer.
//!
//! Routes [`WebCommands`] to the content-type, template, asset and validation
//! subcommands, each operating on the on-disk web content config. Capabilities
//! contribute further subgroups under `web` (such as `web sitemap`) with
//! `CommandDecl::group_under`.
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
use crate::kernel::KernelHost;
use clap::Subcommand;

use crate::commands::shared::rendered;
use systemprompt_extension::cli::{CommandDescriptor, CommandOutput, DescribeCommand};

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

pub fn execute(command: WebCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    let config = host.output();
    match command {
        WebCommands::ContentTypes(cmd) => {
            content_types::execute(cmd, host.prompter(), config, host.profile()?)
        },
        WebCommands::Templates(cmd) => {
            templates::execute(cmd, host.prompter(), config, &host.services_root()?.path)
        },
        WebCommands::Assets(cmd) => {
            assets::execute(cmd, config, &host.services_root()?.path, host.profile()?)
        },
        WebCommands::Validate(args) => {
            let output =
                validate::execute(&args, config, &host.services_root()?.path, host.profile()?)?;
            let valid = output.valid;
            let error_count = output.errors.len();
            host.render(&CommandOutput::card_value(
                "Web Configuration Validation",
                &output,
            ));
            if !valid {
                return Err(anyhow::anyhow!(
                    "web configuration is invalid: {error_count} error(s), see report above",
                ));
            }
            Ok(rendered())
        },
    }
}
