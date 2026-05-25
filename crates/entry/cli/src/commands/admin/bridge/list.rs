use std::time::Duration;

use anyhow::Result;
use clap::Args;
use serde::Serialize;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_oauth::repository::{BridgeSessionRepository, BridgeSessionRow};
use systemprompt_runtime::AppContext;

use crate::CliConfig;
use crate::shared::CommandResult;

const DEFAULT_WITHIN_SECS: u64 = 120;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, help = "Filter to a single user")]
    pub user_id: Option<UserId>,
    #[arg(
        long,
        default_value_t = DEFAULT_WITHIN_SECS,
        help = "Treat sessions with a heartbeat newer than this many seconds as active",
    )]
    pub within_secs: u64,
}

#[derive(Debug, Serialize)]
pub(super) struct BridgeListOutput {
    pub within_secs: u64,
    pub sessions: Vec<BridgeSessionSummary>,
}

#[derive(Debug, Serialize)]
pub(super) struct BridgeSessionSummary {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub hostname: String,
    pub bridge_version: String,
    pub os: String,
    pub last_heartbeat_at: String,
    pub last_activity_at: Option<String>,
    pub forwarded_total: i64,
}

pub(super) async fn execute(
    args: ListArgs,
    _config: &CliConfig,
) -> Result<CommandResult<BridgeListOutput>> {
    let ctx = AppContext::new().await?;
    let repo = BridgeSessionRepository::new(ctx.db_pool())?;
    let within = Duration::from_secs(args.within_secs);

    let rows = match args.user_id.as_ref().filter(|u| !u.as_str().is_empty()) {
        Some(user) => repo.list_active_for_user(user, within).await?,
        None => repo.list_active(within).await?,
    };

    let summaries = rows.into_iter().map(summary).collect::<Vec<_>>();
    let title = format!(
        "Active bridge sessions ({} total, last {}s)",
        summaries.len(),
        args.within_secs
    );

    let output = BridgeListOutput {
        within_secs: args.within_secs,
        sessions: summaries,
    };

    Ok(CommandResult::text(output).with_title(title))
}

fn summary(row: BridgeSessionRow) -> BridgeSessionSummary {
    BridgeSessionSummary {
        session_id: row.session_id,
        user_id: row.user_id,
        hostname: row.hostname,
        bridge_version: row.bridge_version,
        os: row.os,
        last_heartbeat_at: row.last_heartbeat_at.to_rfc3339(),
        last_activity_at: row.last_activity_at.map(|t| t.to_rfc3339()),
        forwarded_total: row.forwarded_total,
    }
}
