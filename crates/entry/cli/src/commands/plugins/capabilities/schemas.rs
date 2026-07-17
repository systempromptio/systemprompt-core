//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::Args;

use crate::CliConfig;
use crate::commands::plugins::discover_registry;
use crate::commands::plugins::types::{SchemaWithExtension, SchemasListOutput};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct SchemasArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub fn execute(args: &SchemasArgs, _config: &CliConfig) -> CommandOutput {
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
                    extension_name: ext.name().to_owned(),
                    table: schema.table.clone(),
                    source: "inline".to_owned(),
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let total = schemas.len();

    let output = SchemasListOutput { schemas, total };

    CommandOutput::table_of(vec!["extension_id", "table", "source"], &output.schemas)
        .with_title("Schemas Across Extensions")
}
