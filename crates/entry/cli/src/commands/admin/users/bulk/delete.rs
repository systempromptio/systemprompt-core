use anyhow::{anyhow, Result};
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use crate::commands::admin::users::types::BulkDeleteOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(long, help = "Filter by role (e.g., 'anonymous')")]
    pub role: Option<String>,

    #[arg(long, help = "Filter by status (e.g., 'inactive')")]
    pub status: Option<String>,

    #[arg(long, help = "Filter by age: users older than N days")]
    pub older_than: Option<i64>,

    #[arg(
        long,
        default_value = "100",
        help = "Maximum number of users to delete"
    )]
    pub limit: i64,

    #[arg(
        long,
        help = "Dry run - show what would be deleted without actually deleting"
    )]
    pub dry_run: bool,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DryRunOutput {
    pub dry_run: bool,
    pub would_delete: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum DeleteResult {
    DryRun(DryRunOutput),
    Executed(BulkDeleteOutput),
}

pub async fn execute(args: DeleteArgs, _config: &CliConfig) -> Result<CommandResult<DeleteResult>> {
    if !args.yes && !args.dry_run {
        return Err(anyhow!(
            "This will permanently delete users. Use --yes to confirm or --dry-run to preview."
        ));
    }

    if args.role.is_none() && args.status.is_none() && args.older_than.is_none() {
        return Err(anyhow!(
            "At least one filter is required: --role, --status, or --older-than"
        ));
    }

    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    let users = user_service
        .list_by_filter(
            args.status.as_deref(),
            args.role.as_deref(),
            args.older_than,
            args.limit,
        )
        .await?;

    if users.is_empty() {
        let output = BulkDeleteOutput {
            deleted: 0,
            message: "No users match the specified filters".to_string(),
        };
        return Ok(CommandResult::text(DeleteResult::Executed(output)).with_title("Bulk Delete"));
    }

    if args.dry_run {
        let output = DryRunOutput {
            dry_run: true,
            would_delete: users.len(),
            message: format!("Would delete {} user(s)", users.len()),
        };
        return Ok(
            CommandResult::text(DeleteResult::DryRun(output)).with_title("Bulk Delete (Dry Run)")
        );
    }

    let user_ids: Vec<_> = users.iter().map(|u| u.id.clone()).collect();
    let deleted = user_service.bulk_delete(&user_ids).await?;

    let output = BulkDeleteOutput {
        deleted,
        message: format!("Deleted {} user(s)", deleted),
    };

    Ok(CommandResult::text(DeleteResult::Executed(output)).with_title("Bulk Delete"))
}
