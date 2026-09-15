//! `analytics conversations list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_analytics::ConversationAnalyticsRepository;
use systemprompt_analytics::models::reporting::{
    ConversationListRow as AgentRow, GatewaySessionListRow,
};
use systemprompt_logging::CliService;
use systemprompt_runtime::DatabaseContext;

use super::{ConversationListOutput, ConversationListRow};
use crate::CliConfig;
use crate::commands::analytics::shared::{export_to_csv, parse_time_range, resolve_export_path};
use crate::shared::CommandOutput;

/// Where a conversation was held. Agent contexts carry A2A tasks; gateway
/// sessions are `/v1/messages` clients (Claude Code, Cowork, SDKs) and are
/// what an instance without A2A agents has exclusively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ConversationSource {
    Agent,
    Gateway,
    All,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, alias = "from", default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time for range")]
    pub until: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Maximum conversations"
    )]
    pub limit: i64,

    #[arg(
        long,
        value_enum,
        default_value = "all",
        help = "Conversation source: agent contexts, gateway sessions, or both"
    )]
    pub source: ConversationSource,

    #[arg(long, help = "Filter by user id (exact match)")]
    pub user: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

const LIST_COLUMNS: [&str; 7] = [
    "context_id",
    "source",
    "user_id",
    "name",
    "task_count",
    "message_count",
    "updated_at",
];

fn agent_row(row: AgentRow) -> ConversationListRow {
    ConversationListRow {
        context: row.context_id.to_string(),
        source: "agent".to_owned(),
        user_id: row.user_id,
        name: row.name,
        task_count: row.task_count,
        message_count: row.message_count,
        created_at: row.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        updated_at: row.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

fn gateway_row(row: GatewaySessionListRow) -> ConversationListRow {
    ConversationListRow {
        context: row.session_id.to_string(),
        source: "gateway".to_owned(),
        user_id: row.user_id,
        name: None,
        task_count: 0,
        message_count: row.message_count,
        created_at: row.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        updated_at: row.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

pub(super) async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let repo = ConversationAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: ListArgs,
    repo: &ConversationAnalyticsRepository,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let user = args.user.as_deref();

    let mut rows: Vec<(chrono::DateTime<chrono::Utc>, ConversationListRow)> = Vec::new();
    if args.source != ConversationSource::Gateway {
        rows.extend(
            repo.list_agent_contexts(start, end, args.limit, user)
                .await?
                .into_iter()
                .map(|row| (row.updated_at, agent_row(row))),
        );
    }
    if args.source != ConversationSource::Agent {
        rows.extend(
            repo.list_gateway_sessions(start, end, args.limit, user)
                .await?
                .into_iter()
                .map(|row| (row.updated_at, gateway_row(row))),
        );
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    let conversations: Vec<ConversationListRow> = rows
        .into_iter()
        .take(args.limit.max(0) as usize)
        .map(|(_, row)| row)
        .collect();

    let output = ConversationListOutput {
        total: conversations.len() as i64,
        conversations,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.conversations, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandOutput::table_of(
            LIST_COLUMNS.to_vec(),
            &output.conversations,
        )
        .with_skip_render());
    }

    if output.conversations.is_empty() {
        CliService::warning("No conversations found");
        return Ok(CommandOutput::table_of(
            LIST_COLUMNS.to_vec(),
            &output.conversations,
        )
        .with_skip_render());
    }

    Ok(CommandOutput::table_of(
        LIST_COLUMNS.to_vec(),
        &output.conversations,
    )
    .with_title("Conversations"))
}
