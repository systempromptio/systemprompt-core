use crate::cli_settings::CliConfig;
use anyhow::Result;
use chrono::Utc;
use clap::Args;
use std::fs::File;
use std::io::Write;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::UserService;
use systemprompt_runtime::AppContext;

use super::types::{UserExportItem, UserExportOutput};

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Output file path (prints to stdout if not specified)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Filter by role
    #[arg(long)]
    pub role: Option<String>,

    /// Filter by status
    #[arg(long)]
    pub status: Option<String>,

    /// Maximum number of users to export
    #[arg(long, default_value = "1000")]
    pub limit: i64,
}

pub async fn execute(args: ExportArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

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

    let json = serde_json::to_string_pretty(&output)?;

    if let Some(path) = args.output {
        let mut file = File::create(&path)?;
        file.write_all(json.as_bytes())?;

        if !config.is_json_output() {
            CliService::success(&format!("Exported {} users to {}", output.total, path));
        }
    } else if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::output(&json);
    }

    Ok(())
}
