//! Role-management subcommands for users.
//!
//! [`RoleCommands`] covers assigning arbitrary roles plus the built-in admin
//! promote/demote shortcuts. All operations require a full profile context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod assign;
mod demote;
mod promote;

use crate::context::CommandContext;
use crate::shared::render_result;
use anyhow::{Result, bail};
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum RoleCommands {
    #[command(about = "Assign roles to a user")]
    Assign(assign::AssignArgs),

    #[command(
        about = "Promote a user to the admin role",
        long_about = "Promote a user to the admin role.\n\nThis command operates only on the \
                      built-in 'admin' role. To assign other roles use:\n  systemprompt admin \
                      users role assign <USER_ID> --roles <ROLE>..."
    )]
    Promote(promote::PromoteArgs),

    #[command(
        about = "Demote a user from the admin role",
        long_about = "Demote a user from the admin role.\n\nThis command operates only on the \
                      built-in 'admin' role. To revoke other roles use:\n  systemprompt admin \
                      users role assign <USER_ID> --roles <ROLE>..."
    )]
    Demote(demote::DemoteArgs),
}

pub(super) async fn execute(cmd: RoleCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped() {
        bail!("Role management operations require full profile context");
    }

    match cmd {
        RoleCommands::Assign(args) => {
            let result = assign::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        RoleCommands::Promote(args) => {
            let result = promote::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        RoleCommands::Demote(args) => {
            let result = demote::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
