use clap::Args;

use crate::CliConfig;
use crate::commands::plugins::discover_registry;
use crate::commands::plugins::types::{SchemaWithExtension, SchemasListOutput};
use crate::shared::CommandResult;

#[derive(Debug, Clone, Args)]
pub struct SchemasArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub(super) fn execute(args: &SchemasArgs, _config: &CliConfig) -> CommandResult<SchemasListOutput> {
    let registry = discover_registry();

    let schemas: Vec<SchemaWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| args.extension.as_ref().is_none_or(|f| ext.id().contains(f)))
        .flat_map(|ext| {
            ext.schemas()
                .iter()
                .map(|schema| SchemaWithExtension {
                    extension_id: systemprompt_identifiers::PluginId::new(ext.id()),
                    extension_name: ext.name().to_string(),
                    table: schema.table.clone(),
                    source: "inline".to_string(),
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let total = schemas.len();

    let output = SchemasListOutput { schemas, total };

    CommandResult::table(output)
        .with_title("Schemas Across Extensions")
        .with_columns(vec![
            "extension_id".to_string(),
            "table".to_string(),
            "source".to_string(),
        ])
}
