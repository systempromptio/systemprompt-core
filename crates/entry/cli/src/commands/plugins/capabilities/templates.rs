use clap::Args;

use crate::CliConfig;
use crate::commands::plugins::discover_registry;
use crate::commands::plugins::types::{TemplateWithExtension, TemplatesListOutput};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct TemplatesArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub(super) fn execute(args: &TemplatesArgs, _config: &CliConfig) -> CommandOutput {
    let registry = discover_registry();

    let templates: Vec<TemplateWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| args.extension.as_ref().is_none_or(|f| ext.id().contains(f)))
        .flat_map(|ext| {
            let ext_id = ext.id().to_owned();
            let ext_name = ext.name().to_owned();

            ext.template_providers()
                .iter()
                .flat_map(|provider| {
                    provider
                        .templates()
                        .iter()
                        .map(|t| (t.name.clone(), t.content_types.join(", ")))
                        .collect::<Vec<_>>()
                })
                .map(|(name, desc)| TemplateWithExtension {
                    extension_id: systemprompt_identifiers::PluginId::new(ext_id.clone()),
                    extension_name: ext_name.clone(),
                    template_name: name,
                    description: desc,
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let total = templates.len();

    let output = TemplatesListOutput { templates, total };

    CommandOutput::table_of(
        vec!["extension_id", "template_name", "description"],
        &output.templates,
    )
    .with_title("Templates Across Extensions")
}
