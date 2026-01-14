use clap::Args;
use systemprompt_extension::{ExtensionRegistry, SchemaSource};

use crate::commands::extensions::types::{SchemaWithExtension, SchemasListOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct SchemasArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub fn execute(args: &SchemasArgs, _config: &CliConfig) -> CommandResult<SchemasListOutput> {
    let registry = ExtensionRegistry::discover();

    let mut schemas: Vec<SchemaWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| {
            args.extension
                .as_ref()
                .is_none_or( |f| ext.id().contains(f))
        })
        .flat_map(|ext| {
            ext.schemas()
                .iter()
                .map(|schema| {
                    let source = match &schema.sql {
                        SchemaSource::Inline(_) => "inline".to_string(),
                        SchemaSource::File(path) => path.display().to_string(),
                    };

                    SchemaWithExtension {
                        extension_id: ext.id().to_string(),
                        extension_name: ext.name().to_string(),
                        table: schema.table.clone(),
                        source,
                        migration_weight: ext.migration_weight(),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    schemas.sort_by_key(|s| s.migration_weight);

    let total = schemas.len();

    let output = SchemasListOutput { schemas, total };

    CommandResult::table(output)
        .with_title("Schemas Across Extensions")
        .with_columns(vec![
            "extension_id".to_string(),
            "table".to_string(),
            "migration_weight".to_string(),
            "source".to_string(),
        ])
}
