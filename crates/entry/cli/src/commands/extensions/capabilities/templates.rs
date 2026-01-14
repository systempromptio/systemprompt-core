use anyhow::Result;
use clap::Args;
use systemprompt_extension::ExtensionRegistry;

use crate::commands::extensions::types::{TemplateWithExtension, TemplatesListOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct TemplatesArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub fn execute(args: TemplatesArgs, _config: &CliConfig) -> Result<CommandResult<TemplatesListOutput>> {
    let registry = ExtensionRegistry::discover();

    let templates: Vec<TemplateWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| {
            args.extension
                .as_ref()
                .map_or(true, |f| ext.id().contains(f))
        })
        .flat_map(|ext| {
            let ext_id = ext.id().to_string();
            let ext_name = ext.name().to_string();

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
                    extension_id: ext_id.clone(),
                    extension_name: ext_name.clone(),
                    template_name: name,
                    description: desc,
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let total = templates.len();

    let output = TemplatesListOutput { templates, total };

    Ok(CommandResult::table(output)
        .with_title("Templates Across Extensions")
        .with_columns(vec![
            "extension_id".to_string(),
            "template_name".to_string(),
            "description".to_string(),
        ]))
}
