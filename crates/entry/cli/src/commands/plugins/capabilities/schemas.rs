//! `plugins capabilities schemas` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::Args;
use systemprompt_extension::BuiltState;
use systemprompt_extension::cli::CommandOutput;

use crate::commands::plugins::composition;
use crate::commands::plugins::types::{SchemaWithExtension, SchemasListOutput};

#[derive(Debug, Clone, Args)]
pub struct SchemasArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub fn execute(args: &SchemasArgs, built: &BuiltState) -> CommandOutput {
    let schemas: Vec<SchemaWithExtension> = composition::matching(built, args.extension.as_deref())
        .flat_map(|member| {
            member.sink.schemas().iter().map(|schema| SchemaWithExtension {
                extension_id: member.owner.clone(),
                extension_name: member.metadata.name.to_owned(),
                table: schema.table.clone(),
            })
        })
        .collect();

    let total = schemas.len();
    let output = SchemasListOutput { schemas, total };

    CommandOutput::table_of(vec!["extension_id", "table"], &output.schemas)
        .with_title("Schemas Across Extensions")
}
