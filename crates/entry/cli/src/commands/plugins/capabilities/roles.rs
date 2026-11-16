//! `plugins capabilities roles` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::Args;
use systemprompt_extension::BuiltState;
use systemprompt_extension::cli::CommandOutput;

use crate::commands::plugins::composition;
use crate::commands::plugins::types::{RoleWithExtension, RolesListOutput};

#[derive(Debug, Clone, Args)]
pub struct RolesArgs {
    #[arg(long, help = "Filter by extension ID")]
    pub extension: Option<String>,
}

pub fn execute(args: &RolesArgs, built: &BuiltState) -> CommandOutput {
    let roles: Vec<RoleWithExtension> = composition::matching(built, args.extension.as_deref())
        .flat_map(|member| {
            member.sink.roles().iter().map(|role| RoleWithExtension {
                extension_id: member.owner.clone(),
                extension_name: member.metadata.name.to_owned(),
                role_name: role.name.clone(),
                display_name: role.display_name.clone(),
                description: role.description.clone(),
                permissions: role.permissions.clone(),
            })
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
