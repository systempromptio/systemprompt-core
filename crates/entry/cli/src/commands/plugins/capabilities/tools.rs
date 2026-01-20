use clap::Args;
use systemprompt_extension::ExtensionRegistry;

use crate::commands::plugins::types::{ToolWithExtension, ToolsListOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ToolsArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub fn execute(args: &ToolsArgs, _config: &CliConfig) -> CommandResult<ToolsListOutput> {
    let registry = ExtensionRegistry::discover();

    let tools: Vec<ToolWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| args.extension.as_ref().is_none_or(|f| ext.id().contains(f)))
        .flat_map(|ext| {
            ext.tool_providers()
                .iter()
                .enumerate()
                .map(|(i, _provider)| ToolWithExtension {
                    extension_id: ext.id().to_string(),
                    extension_name: ext.name().to_string(),
                    tool_name: format!("tool_provider_{}", i),
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let total = tools.len();

    let output = ToolsListOutput { tools, total };

    CommandResult::table(output)
        .with_title("Tools Across Extensions")
        .with_columns(vec!["extension_id".to_string(), "tool_name".to_string()])
}
