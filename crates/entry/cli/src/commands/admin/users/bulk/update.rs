use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use crate::commands::admin::users::types::BulkUpdateOutput;

#[derive(Debug, Args)]
pub struct UpdateArgs {
    #[arg(long, help = "New status to set (active, inactive, suspended)")]
    pub set_status: String,

    #[arg(long, help = "Filter by current role (e.g., 'anonymous')")]
    pub role: Option<String>,

    #[arg(long, help = "Filter by current status")]
    pub status: Option<String>,

    #[arg(long, help = "Filter by age: users older than N days")]
    pub older_than: Option<i64>,

    #[arg(
        long,
        default_value = "100",
        help = "Maximum number of users to update"
    )]
    pub limit: i64,

    #[arg(
        long,
        help = "Dry run - show what would be updated without actually updating"
    )]
    pub dry_run: bool,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(args: UpdateArgs, config: &CliConfig) -> Result<()> {
    if !args.yes && !args.dry_run {
        CliService::warning(
            "This will update multiple users. Use --yes to confirm or --dry-run to preview.",
        );
        return Err(anyhow!("Operation cancelled - confirmation required"));
    }

    if args.role.is_none() && args.status.is_none() && args.older_than.is_none() {
        return Err(anyhow!(
            "At least one filter is required: --role, --status, or --older-than"
        ));
    }

    let valid_statuses = ["active", "inactive", "suspended"];
    if !valid_statuses.contains(&args.set_status.as_str()) {
        return Err(anyhow!(
            "Invalid status '{}'. Must be one of: {}",
            args.set_status,
            valid_statuses.join(", ")
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
            CliService::json(&BulkUpdateOutput {
                updated: 0,
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
                "would_update": users.len(),
                "new_status": args.set_status,
                "users": users.iter().map(|u| {
                    serde_json::json!({
                        "id": u.id.to_string(),
                        "name": u.name,
                        "current_status": u.status,
                    })
                }).collect::<Vec<_>>()
            }));
        } else {
            CliService::section(&format!(
                "Dry Run: Would update {} users to status '{}'",
                users.len(),
                args.set_status
            ));
            for user in &users {
                CliService::info(&format!(
                    "  {} ({:?} -> {})",
                    user.name, user.status, args.set_status
                ));
            }
        }
        return Ok(());
    }

    let user_ids: Vec<_> = users.iter().map(|u| u.id.clone()).collect();
    let updated = user_service
        .bulk_update_status(&user_ids, &args.set_status)
        .await?;

    let output = BulkUpdateOutput {
        updated,
        message: format!(
            "Updated {} user(s) to status '{}'",
            updated, args.set_status
        ),
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
    }

    Ok(())
}
