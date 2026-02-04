use anyhow::Result;
use chrono::Utc;
use clap::Args;
use std::fs::File;
use std::io::Write;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use super::types::{UserExportItem, UserExportOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ExportArgs {
    #[arg(
        short,
        long,
        help = "Output file path (prints to stdout if not specified)"
    )]
    pub output: Option<String>,

    #[arg(long, help = "Filter by role")]
    pub role: Option<String>,

    #[arg(long, help = "Filter by status")]
    pub status: Option<String>,

    #[arg(
        long,
        default_value = "1000",
        help = "Maximum number of users to export"
    )]
    pub limit: i64,
}

pub async fn execute(
    args: ExportArgs,
    config: &CliConfig,
) -> Result<CommandResult<UserExportOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ExportArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<UserExportOutput>> {
    let user_service = UserService::new(pool)?;

    let users = user_service
        .list_by_filter(
            args.status.as_deref(),
            args.role.as_deref(),
            None,
            args.limit,
        )
        .await?;

    let export_items: Vec<UserExportItem> = users
        .into_iter()
        .map(|u| UserExportItem {
            id: u.id.clone(),
            name: u.name,
            email: u.email,
            full_name: u.full_name,
            display_name: u.display_name,
            status: u.status,
            email_verified: u.email_verified,
            roles: u.roles,
            is_bot: u.is_bot,
            is_scanner: u.is_scanner,
            created_at: u.created_at,
            updated_at: u.updated_at,
        })
        .collect();

    let output = UserExportOutput {
        total: export_items.len(),
        users: export_items,
        exported_at: Utc::now(),
    };

    if let Some(path) = args.output {
        let json = serde_json::to_string_pretty(&output)?;
        let mut file = File::create(&path)?;
        file.write_all(json.as_bytes())?;

        let total = output.total;
        Ok(CommandResult::text(output).with_title(format!("Exported {total} users to {path}")))
    } else {
        Ok(CommandResult::copy_paste(output).with_title("User Export"))
    }
}
