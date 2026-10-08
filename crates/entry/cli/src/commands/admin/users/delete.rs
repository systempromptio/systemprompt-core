//! `admin users delete` command.
//!
//! Deleting a user archives it (NFR-5.2): the account stops signing in and
//! leaves every listing, its sessions, API keys and device certificates are
//! revoked, and its history stays, restorable with `admin users restore`
//! inside `retention.archived_users_days`. `--purge` is the physical delete,
//! refused unless the user is already archived and not under legal hold;
//! `database_cleanup` purges archives past the window on its own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use std::sync::Arc;
use systemprompt_users::{
    ArchiveParams, PurgeCount, UserAdminService, UserRepository, UserService,
};

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
        help = "Confirm: archive the user (or, with --purge, delete them and all their data)"
    )]
    pub yes: bool,

    #[arg(
        long,
        conflicts_with = "yes",
        help = "Report what a purge would remove, table by table, and change nothing"
    )]
    pub dry_run: bool,

    #[arg(long, help = "Why the user is archived; recorded on the account")]
    pub reason: Option<String>,

    #[arg(long, help = "Place the archive under legal hold: never purged until released")]
    pub legal_hold: bool,

    #[arg(
        long,
        conflicts_with = "legal_hold",
        help = "Physically delete an already-archived user and every row keyed on them"
    )]
    pub purge: bool,
}

pub(super) async fn execute(args: DeleteArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let user_service = UserService::new(Arc::new(UserRepository::new(&pool)));
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
                "Purging '{}' ({}) would remove",
                user.name, user.id
            )),
        );
    }

    if !args.yes {
        let action = if args.purge {
            "permanently delete"
        } else {
            "archive"
        };
        return Err(anyhow!(
            "This will {action} user '{}' ({}). Use --yes to confirm, or --dry-run to see what a \
             purge would remove.",
            user.name,
            user.id
        ));
    }

    if args.purge {
        let removed = user_service.purge(&user.id).await?;
        let rows_removed: i64 = removed.iter().map(|c| c.rows).sum();
        let output = UserDeletedOutput {
            id: user.id.clone(),
            message: format!(
                "User '{}' purged ({rows_removed} rows across {} tables)",
                user.name,
                removed.len()
            ),
        };
        return Ok(CommandOutput::card_value("User Purged", &output));
    }

    let outcome = user_service
        .archive(
            &user.id,
            ArchiveParams {
                archived_by: None,
                reason: args.reason.as_deref(),
                legal_hold: args.legal_hold,
            },
        )
        .await?;
    let output = UserDeletedOutput {
        id: user.id.clone(),
        message: format!(
            "User '{}' archived: {} sessions, {} API keys and {} device certificates revoked{}. \
             Restore with `admin users restore {}`.",
            user.name,
            outcome.sessions_revoked,
            outcome.api_keys_revoked,
            outcome.device_certs_revoked,
            if args.legal_hold { "; under legal hold" } else { "" },
            user.id
        ),
    };
    Ok(CommandOutput::card_value("User Archived", &output))
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
