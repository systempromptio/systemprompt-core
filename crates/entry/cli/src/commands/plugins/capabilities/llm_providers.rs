//! `plugins capabilities llm-providers` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::Args;

use crate::CliConfig;
use crate::commands::plugins::discover_registry;
use crate::commands::plugins::types::{LlmProviderWithExtension, LlmProvidersListOutput};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct LlmProvidersArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub fn execute(args: &LlmProvidersArgs, _config: &CliConfig) -> CommandOutput {
    let registry = discover_registry();

    let providers: Vec<LlmProviderWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| args.extension.as_ref().is_none_or(|f| ext.id().contains(f)))
        .flat_map(|ext| {
            ext.llm_providers()
                .iter()
                .enumerate()
                .map(|(i, _provider)| LlmProviderWithExtension {
                    extension_id: systemprompt_identifiers::PluginId::new(ext.id()),
                    extension_name: ext.name().to_owned(),
                    provider_name: format!("llm_provider_{}", i),
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let total = providers.len();

    let output = LlmProvidersListOutput { providers, total };

    CommandOutput::table_of(vec!["extension_id", "provider_name"], &output.providers)
        .with_title("LLM Providers Across Extensions")
}
