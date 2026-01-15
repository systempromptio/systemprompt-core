use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::UserService;
use systemprompt_runtime::AppContext;

use crate::commands::users::types::BulkDeleteOutput;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    /// Filter by role (e.g., 'anonymous')
    #[arg(long)]
    pub role: Option<String>,

    /// Filter by status (e.g., 'inactive')
    #[arg(long)]
    pub status: Option<String>,

    /// Filter by age: users older than N days
    #[arg(long)]
    pub older_than: Option<i64>,

    /// Maximum number of users to delete
    #[arg(long, default_value = "100")]
    pub limit: i64,

    /// Dry run - show what would be deleted without actually deleting
    #[arg(long)]
    pub dry_run: bool,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(args: DeleteArgs, config: &CliConfig) -> Result<()> {
    if !args.yes && !args.dry_run {
        CliService::warning(
            "This will permanently delete users. Use --yes to confirm or --dry-run to preview.",
        );
        return Err(anyhow!("Operation cancelled - confirmation required"));
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
        if config.is_json_output() {
            CliService::json(&BulkDeleteOutput {
                deleted: 0,
                message: "No users match the specified filters".to_string(),
            });
        } else {
            CliService::info("No users match the specified filters");
        }
        return Ok(());
    }

    if args.dry_run {
        if config.is_json_output() {
            CliService::json(&serde_json::json!({
                "dry_run": true,
                "would_delete": users.len(),
                "users": users.iter().map(|u| {
                    serde_json::json!({
                        "id": u.id.to_string(),
                        "name": u.name,
                        "email": u.email,
                        "status": u.status,
                        "roles": u.roles,
                        "created_at": u.created_at,
                    })
                }).collect::<Vec<_>>()
            }));
        } else {
            CliService::section(&format!("Dry Run: Would delete {} users", users.len()));
            for user in &users {
                CliService::info(&format!(
                    "  {} ({}) - {:?}",
                    user.name, user.email, user.roles
                ));
            }
        }
        return Ok(());
    }

    let user_ids: Vec<_> = users.iter().map(|u| u.id.clone()).collect();
    let deleted = user_service.bulk_delete(&user_ids).await?;

    let output = BulkDeleteOutput {
        deleted,
        message: format!("Deleted {} user(s)", deleted),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}
