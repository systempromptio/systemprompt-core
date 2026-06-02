use clap::Args;

use crate::CliConfig;
use crate::commands::plugins::discover_registry;
use crate::commands::plugins::types::{ToolWithExtension, ToolsListOutput};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct ToolsArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub(super) fn execute(args: &ToolsArgs, _config: &CliConfig) -> CommandOutput {
    let registry = discover_registry();

    let tools: Vec<ToolWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| args.extension.as_ref().is_none_or(|f| ext.id().contains(f)))
        .flat_map(|ext| {
            ext.tool_providers()
                .iter()
                .enumerate()
                .map(|(i, _provider)| ToolWithExtension {
                    extension_id: systemprompt_identifiers::PluginId::new(ext.id()),
                    extension_name: ext.name().to_owned(),
                    tool_name: format!("tool_provider_{}", i),
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let total = tools.len();

    let output = ToolsListOutput { tools, total };

    CommandOutput::table_of(vec!["extension_id", "tool_name"], &output.tools)
        .with_title("Tools Across Extensions")
}
