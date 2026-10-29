//! `admin bridge list` command summarising recent bridge sessions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use anyhow::Result;
use clap::Args;
use serde::Serialize;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_manifest::services::BridgeReleasesSpec;
use systemprompt_models::bridge::instance::{BridgeVersionWindow, WindowStatus};
use systemprompt_oauth::repository::{BridgeSessionRepository, BridgeSessionRow};

use crate::context::CommandContext;
use crate::shared::CommandOutput;

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
    pub release_version: Option<String>,
    pub state: Option<String>,
    pub window_status: WindowStatus,
    pub os: String,
    pub last_heartbeat_at: String,
    pub last_activity_at: Option<String>,
    pub forwarded_total: i64,
}

pub(super) async fn execute(args: ListArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let app = ctx.app_context().await?;
    let repo = BridgeSessionRepository::new(app.db_pool());
    let within = Duration::from_secs(args.within_secs);

    let rows = match args.user_id.as_ref().filter(|u| !u.as_str().is_empty()) {
        Some(user) => repo.list_active_for_user(user, within).await?,
        None => repo.list_active(within).await?,
    };

    let window = app
        .services_config()
        .gateway_config()
        .and_then(|g| g.bridge_releases.as_ref())
        .map(BridgeReleasesSpec::version_window)
        .unwrap_or_default();
    let summaries = rows
        .into_iter()
        .map(|row| summary(row, &window))
        .collect::<Vec<_>>();
    let title = format!(
        "Active bridge sessions ({} total, last {}s)",
        summaries.len(),
        args.within_secs
    );

    let output = BridgeListOutput {
        within_secs: args.within_secs,
        sessions: summaries,
    };

    Ok(CommandOutput::card_value(title, &output))
}

fn summary(row: BridgeSessionRow, window: &BridgeVersionWindow) -> BridgeSessionSummary {
    let window_status = row
        .release_version
        .as_deref()
        .map_or(WindowStatus::Unknown, |release| window.status(release));
    BridgeSessionSummary {
        session_id: row.session_id,
        user_id: row.user_id,
        hostname: row.hostname,
        bridge_version: row.bridge_version,
        release_version: row.release_version,
        state: row.state,
        window_status,
        os: row.os,
        last_heartbeat_at: row.last_heartbeat_at.to_rfc3339(),
        last_activity_at: row.last_activity_at.map(|t| t.to_rfc3339()),
        forwarded_total: row.forwarded_total,
    }
}
