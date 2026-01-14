use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::UserService;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;
use tabled::{Table, Tabled};

use crate::commands::users::types::{SessionListOutput, SessionSummary};

#[derive(Debug, Args)]
pub struct ListArgs {
    pub user_id: String,

    #[arg(long)]
    pub active: bool,

    #[arg(long, default_value = "20")]
    pub limit: i64,
}

#[derive(Tabled)]
struct SessionRow {
    #[tabled(rename = "Session ID")]
    id: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "IP Address")]
    ip: String,
    #[tabled(rename = "Device")]
    device: String,
    #[tabled(rename = "Started")]
    started: String,
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;

    let user_id = UserId::new(&args.user_id);

    let existing = user_service.find_by_id(&user_id).await?;
    if existing.is_none() {
        CliService::error(&format!("User not found: {}", args.user_id));
        return Err(anyhow!("User not found"));
    }

    let sessions = if args.active {
        user_service.list_active_sessions(&user_id).await?
    } else {
        user_service
            .list_recent_sessions(&user_id, args.limit)
            .await?
    };

    let summaries: Vec<SessionSummary> = sessions
        .into_iter()
        .map(|s| SessionSummary {
            session_id: s.session_id.to_string(),
            ip_address: s.ip_address,
            user_agent: s.user_agent,
            device_type: s.device_type,
            started_at: s.started_at,
            last_activity_at: s.last_activity_at,
            is_active: s.ended_at.is_none(),
        })
        .collect();

    let output = SessionListOutput {
        total: summaries.len(),
        sessions: summaries,
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::section(&format!("Sessions for user '{}'", args.user_id));

        if output.sessions.is_empty() {
            CliService::info("No sessions found");
        } else {
            let rows: Vec<SessionRow> = output
                .sessions
                .iter()
                .map(|s| SessionRow {
                    id: s.session_id.clone(),
                    status: if s.is_active { "active" } else { "ended" }.to_string(),
                    ip: s
                        .ip_address
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    device: s
                        .device_type
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    started: s
                        .started_at.map_or_else(|| "unknown".to_string(), |t| t.format("%Y-%m-%d %H:%M").to_string()),
                })
                .collect();

            let table = Table::new(rows).to_string();
            CliService::output(&table);

            CliService::info(&format!("Total: {} session(s)", output.total));
        }
    }

    Ok(())
}
