//! `plugins config` command: the configuration sections the admitted
//! members declare, and their schemas.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use crate::kernel::KernelHost;
use clap::Args;
use systemprompt_extension::cli::{CommandOutput};
use systemprompt_identifiers::ExtensionId;

use super::composition;
use super::types::{ExtensionConfigListOutput, ExtensionConfigOutput, ExtensionConfigSummary};

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[arg(
        help = "Extension ID (optional - lists all if not specified)",
        value_parser = systemprompt_extension::cli::parsers::parse_extension_id
    )]
    pub id: Option<ExtensionId>,
}

pub fn execute(args: &ConfigArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let built = composition::built(host)?;

    if let Some(id) = &args.id {
        let member = built
            .members()
            .find(|member| &member.owner == id)
            .ok_or_else(|| anyhow!("Extension '{}' not found", id))?;

        let configs = member
            .sink
            .configs()
            .iter()
            .map(|config| {
                Ok(ExtensionConfigOutput {
                    extension_id: member.owner.clone(),
                    config_prefix: config.prefix().to_owned(),
                    config_schema: serde_json::to_value(config.schema())?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        return Ok(CommandOutput::card_value(
            format!("Extension Config: {}", id),
            &configs,
        ));
    }

    let mut extensions: Vec<ExtensionConfigSummary> = built
        .members()
        .flat_map(|member| {
            member
                .sink
                .configs()
                .iter()
                .map(|config| ExtensionConfigSummary {
                    extension_id: member.owner.clone(),
                    config_prefix: config.prefix().to_owned(),
                })
        })
        .collect();

    extensions.sort_by(|a, b| a.extension_id.cmp(&b.extension_id));
    let total = extensions.len();
    let output = ExtensionConfigListOutput { extensions, total };

    Ok(CommandOutput::table_of(
        vec!["extension_id", "config_prefix"],
        &output.extensions,
    )
    .with_title("Extension Configurations"))
}
