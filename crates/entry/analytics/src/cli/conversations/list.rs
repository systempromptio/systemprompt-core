//! `analytics conversations list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::{Args, ValueEnum};
use std::collections::HashSet;
use std::path::PathBuf;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_logging::CliService;
use systemprompt_traits::usage_reports::ReportUnavailable;
use systemprompt_traits::usage_reports::agent::{AgentContextRow, DynAgentActivityReport};
use systemprompt_traits::usage_reports::ai::{DynAiUsageReport, GatewaySessionListRow};

use super::{ConversationListOutput, ConversationListRow};
use crate::cli::shared::{export_to_csv, parse_time_range, report_port};
use systemprompt_extension::cli::{CommandHost, CommandOutput};

const REPORT: &str = "analytics conversations list";

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
        allow_negative_numbers = true,
        value_parser = clap::value_parser!(i64).range(1..),
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

fn agent_row(row: AgentContextRow) -> ConversationListRow {
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

async fn gateway_rows(
    host: &dyn CommandHost,
    agents: Option<&DynAgentActivityReport>,
    (start, end): (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    limit: i64,
    user: Option<&UserId>,
) -> Result<Vec<(chrono::DateTime<chrono::Utc>, ConversationListRow)>> {
    let gateway = report_port::<DynAiUsageReport>(host, REPORT, "gateway").await?;
    let sessions: Vec<GatewaySessionListRow> = gateway
        .list_gateway_sessions(start, end, limit, user)
        .await?;
    let agent_contexts: HashSet<SessionId> = match agents {
        Some(agents) => {
            let ids: Vec<SessionId> = sessions.iter().map(|row| row.session_id.clone()).collect();
            agents
                .agent_context_sessions(&ids)
                .await?
                .into_iter()
                .collect()
        },
        None => HashSet::new(),
    };
    Ok(sessions
        .into_iter()
        .filter(|row| !agent_contexts.contains(&row.session_id))
        .map(|row| (row.updated_at, gateway_row(row)))
        .collect())
}

pub(super) async fn execute(args: ListArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let user = args.user.as_deref().map(UserId::new);
    let user = user.as_ref();
    let agents = host.resolved().await?.optional::<DynAgentActivityReport>();

    let mut rows: Vec<(chrono::DateTime<chrono::Utc>, ConversationListRow)> = Vec::new();
    if args.source != ConversationSource::Gateway {
        match &agents {
            Some(agents) => rows.extend(
                agents
                    .list_agent_contexts(start, end, args.limit, user)
                    .await?
                    .into_iter()
                    .map(|row| (row.updated_at, agent_row(row))),
            ),
            None if args.source == ConversationSource::Agent => {
                return Err(ReportUnavailable {
                    report: REPORT,
                    capability: "agents",
                }
                .into());
            },
            None => {},
        }
    }
    if args.source != ConversationSource::Agent {
        rows.extend(gateway_rows(host, agents.as_ref(), (start, end), args.limit, user).await?);
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.0));
    let conversations: Vec<ConversationListRow> = rows
        .into_iter()
        .take(usize::try_from(args.limit).unwrap_or(usize::MAX))
        .map(|(_, row)| row)
        .collect();

    let output = ConversationListOutput {
        total: conversations.len() as i64,
        conversations,
    };

    if let Some(ref path) = args.export {
        export_to_csv(&output.conversations, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(
            CommandOutput::table_of(LIST_COLUMNS.to_vec(), &output.conversations)
                .with_skip_render(),
        );
    }

    if output.conversations.is_empty() {
        CliService::warning("No conversations found");
        return Ok(
            CommandOutput::table_of(LIST_COLUMNS.to_vec(), &output.conversations)
                .with_skip_render(),
        );
    }

    Ok(
        CommandOutput::table_of(LIST_COLUMNS.to_vec(), &output.conversations)
            .with_title("Conversations"),
    )
}
