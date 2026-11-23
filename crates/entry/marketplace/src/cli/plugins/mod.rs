//! `core plugins` command group: list, show, validate, and generate plugins.
//!
//! [`PluginsCommands`] enumerates the subcommands; [`execute`] dispatches each
//! to its submodule and returns the result. Every command reads the services
//! tree through the services-tree grant and needs no database; `generate`
//! also reads the loaded services configuration from the open
//! `ServicesState` seed and the published GitHub link from the grant. A
//! failing `validate` attaches its report to the error. Output payload types
//! live in [`types`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

pub mod generate;
pub mod list;
pub mod show;
pub mod validate;

use std::path::Path;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
};
use systemprompt_identifiers::PluginId;
use systemprompt_manifest::{PluginConfig, PluginConfigFile, ServicesConfig};

#[derive(Debug, Subcommand)]
pub enum PluginsCommands {
    #[command(about = "List configured plugins")]
    List(list::ListArgs),

    #[command(about = "Show plugin details")]
    Show(show::ShowArgs),

    #[command(about = "Validate plugin configuration")]
    Validate(validate::ValidateArgs),

    #[command(about = "Generate Claude Code plugin output")]
    Generate(generate::GenerateArgs),
}

pub async fn execute(command: PluginsCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match command {
        PluginsCommands::List(args) => list::execute(args, host).context("Failed to list plugins"),
        PluginsCommands::Show(args) => show::execute(&args, host).context("Failed to show plugin"),
        PluginsCommands::Validate(args) => {
            let (result, valid) = validate::execute(args, host)
                .await
                .context("Failed to validate plugins")?;
            if !valid {
                return Err(anyhow::Error::new(
                    CommandError::new(anyhow::anyhow!("Plugin validation failed"))
                        .with_output(result),
                ));
            }
            Ok(result)
        },
        PluginsCommands::Generate(args) => generate::execute(&args, host)
            .await
            .context("Failed to generate plugins"),
    }
}

pub fn selected_plugins<'a>(
    services: &'a ServicesConfig,
    id: Option<&PluginId>,
) -> Result<Vec<&'a PluginConfig>> {
    let Some(id) = id else {
        let mut plugins: Vec<&PluginConfig> = services.plugins.values().collect();
        plugins.sort_by(|a, b| a.id.cmp(&b.id));
        return Ok(plugins);
    };
    services
        .plugins
        .get(id.as_str())
        .map(|plugin| vec![plugin])
        .ok_or_else(|| anyhow::anyhow!("Plugin '{id}' not found"))
}

pub fn read_plugin_config(config_path: &Path) -> Result<PluginConfigFile> {
    let content = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", config_path.display()))
}

impl PluginsCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::List(_) | Self::Show(_) | Self::Validate(_) | Self::Generate(_) => {
                DataImpact::Preserving
            },
        }
    }
}

impl DescribeCommand for PluginsCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::PROFILE_SECRETS_AND_PATHS.with_data_impact(self.data_impact())
    }
}

impl CliCommandGroup for PluginsCommands {
    const NAME: &'static str = "plugins";
    const ABOUT: &'static str = "Plugin management and marketplace generation";

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host).await.map_err(super::command_error)
    }
}
