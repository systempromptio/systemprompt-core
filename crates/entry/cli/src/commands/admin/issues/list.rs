//! `admin issues list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use serde::Serialize;
use systemprompt_files::issues::IssueListQuery;
use systemprompt_identifiers::{IssueReportId, UserId};
use systemprompt_models::issues::{IssueKind, IssueReport, IssueSeverity, IssueStatus};

use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
        help = "Filter by status (open, triaged, needs_info, resolved, closed)"
    )]
    pub status: Option<IssueStatus>,

    #[arg(long, value_name = "USER_ID", help = "Filter by reporter user ID")]
    pub reporter: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "50",
        help = "Maximum reports to show"
    )]
    pub limit: u32,
}

#[derive(Debug, Serialize)]
struct IssueRow {
    id: IssueReportId,
    status: IssueStatus,
    severity: IssueSeverity,
    kind: IssueKind,
    occurrences: i32,
    via_agent: Option<String>,
    reporter: UserId,
    last_seen_at: String,
    title: String,
}

impl From<IssueReport> for IssueRow {
    fn from(report: IssueReport) -> Self {
        Self {
            id: report.id,
            status: report.status,
            severity: report.severity,
            kind: report.kind,
            occurrences: report.occurrences,
            via_agent: report.via_agent,
            reporter: report.reporter_user_id,
            last_seen_at: report.last_seen_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            title: report.title,
        }
    }
}

pub(super) async fn execute(args: ListArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let app = ctx.app_context().await?;
    let query = IssueListQuery {
        reporter: args.reporter.as_deref().map(UserId::new),
        status: args.status,
        before: None,
        limit: args.limit,
    };
    let rows: Vec<IssueRow> = app
        .documents()
        .issues()
        .list(&query)
        .await?
        .into_iter()
        .map(IssueRow::from)
        .collect();
    Ok(CommandOutput::table_of(
        vec![
            "id",
            "status",
            "severity",
            "kind",
            "occurrences",
            "via_agent",
            "reporter",
            "last_seen_at",
            "title",
        ],
        &rows,
    )
    .with_title(format!("Issue reports ({})", rows.len())))
}
