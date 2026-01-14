use anyhow::Result;
use clap::Args;
use systemprompt_extension::ExtensionRegistry;

use crate::commands::extensions::types::{LlmProviderWithExtension, LlmProvidersListOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct LlmProvidersArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub fn execute(args: LlmProvidersArgs, _config: &CliConfig) -> Result<CommandResult<LlmProvidersListOutput>> {
    let registry = ExtensionRegistry::discover();

    let providers: Vec<LlmProviderWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| {
            args.extension
                .as_ref()
                .map_or(true, |f| ext.id().contains(f))
        })
        .flat_map(|ext| {
            ext.llm_providers()
                .iter()
                .enumerate()
                .map(|(i, _provider)| LlmProviderWithExtension {
                    extension_id: ext.id().to_string(),
                    extension_name: ext.name().to_string(),
                    provider_name: format!("llm_provider_{}", i),
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let total = providers.len();

    let output = LlmProvidersListOutput { providers, total };

    Ok(CommandResult::table(output)
        .with_title("LLM Providers Across Extensions")
        .with_columns(vec![
            "extension_id".to_string(),
            "provider_name".to_string(),
        ]))
}
