//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::Args;

use crate::CliConfig;
use crate::commands::plugins::discover_registry;
use crate::commands::plugins::types::{RoleWithExtension, RolesListOutput};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct RolesArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub fn execute(args: &RolesArgs, _config: &CliConfig) -> CommandOutput {
    let registry = discover_registry();

    let roles: Vec<RoleWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| args.extension.as_ref().is_none_or(|f| ext.id().contains(f)))
        .flat_map(|ext| {
            ext.roles()
                .iter()
                .map(|role| RoleWithExtension {
                    extension_id: systemprompt_identifiers::PluginId::new(ext.id()),
                    extension_name: ext.name().to_owned(),
                    role_name: role.name.clone(),
                    display_name: role.display_name.clone(),
                    description: role.description.clone(),
                    permissions: role.permissions.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let total = roles.len();

    let output = RolesListOutput { roles, total };

    CommandOutput::table_of(
        vec!["extension_id", "role_name", "display_name", "description"],
        &output.roles,
    )
    .with_title("Roles Across Extensions")
}
