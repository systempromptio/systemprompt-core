//! `admin users delete` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use std::sync::Arc;
use systemprompt_users::{PurgeCount, UserAdminService, UserRepository, UserService};

use super::types::UserDeletedOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(value_name = "USER_ID", help = "User id, email, or name")]
    pub user: String,

    #[arg(
        short = 'y',
        long,
        help = "Confirm permanent deletion of the user and all their data"
    )]
    pub yes: bool,

    #[arg(
        long,
        conflicts_with = "yes",
        help = "Report what the deletion would remove, table by table, and change nothing"
    )]
    pub dry_run: bool,
}

pub(super) async fn execute(args: DeleteArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let user_service = UserService::new(Arc::new(UserRepository::new(&pool)?));
    let admin_service = UserAdminService::new(user_service.clone());

    let existing = admin_service.find_user(&args.user).await?;
    let Some(user) = existing else {
        return Err(anyhow!("User not found: {}", args.user));
    };

    if args.dry_run {
        let counts = user_service.purge_preview(&user.id).await?;
        let rows: Vec<PurgeRow> = counts.iter().map(PurgeRow::from).collect();
        return Ok(
            CommandOutput::table_of(vec!["owner", "table", "rows"], &rows).with_title(format!(
                "Deleting '{}' ({}) would remove",
                user.name, user.id
            )),
        );
    }

    if !args.yes {
        return Err(anyhow!(
            "This will permanently delete user '{}' ({}). Use --yes to confirm, or --dry-run to \
             see what goes with them.",
            user.name,
            user.id
        ));
    }

    let removed = user_service.delete(&user.id).await?;
    let rows_removed: i64 = removed.iter().map(|c| c.rows).sum();

    let output = UserDeletedOutput {
        id: user.id.clone(),
        message: format!(
            "User '{}' deleted successfully ({rows_removed} rows across {} tables)",
            user.name,
            removed.len()
        ),
    };

    Ok(CommandOutput::card_value("User Deleted", &output))
}

#[derive(Debug, serde::Serialize)]
struct PurgeRow {
    owner: &'static str,
    table: &'static str,
    rows: i64,
}

impl From<&PurgeCount> for PurgeRow {
    fn from(count: &PurgeCount) -> Self {
        Self {
            owner: count.owner,
            table: count.table,
            rows: count.rows,
        }
    }
}
