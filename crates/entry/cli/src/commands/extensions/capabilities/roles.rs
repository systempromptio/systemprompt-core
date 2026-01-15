use clap::Args;
use systemprompt_extension::ExtensionRegistry;

use crate::commands::extensions::types::{RoleWithExtension, RolesListOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct RolesArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub fn execute(args: &RolesArgs, _config: &CliConfig) -> CommandResult<RolesListOutput> {
    let registry = ExtensionRegistry::discover();

    let roles: Vec<RoleWithExtension> = registry
        .extensions()
        .iter()
        .filter(|ext| args.extension.as_ref().is_none_or(|f| ext.id().contains(f)))
        .flat_map(|ext| {
            ext.roles()
                .iter()
                .map(|role| RoleWithExtension {
                    extension_id: ext.id().to_string(),
                    extension_name: ext.name().to_string(),
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

    CommandResult::table(output)
        .with_title("Roles Across Extensions")
        .with_columns(vec![
            "extension_id".to_string(),
            "role_name".to_string(),
            "display_name".to_string(),
            "description".to_string(),
        ])
}
