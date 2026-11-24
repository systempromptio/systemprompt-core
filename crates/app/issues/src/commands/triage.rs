//! `admin issues triage` command.
//!
//! The CLI session user is the triager the transition and its notification
//! to the reporter record; [`execute_as`] takes the triager explicitly.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use systemprompt_extension::cli::CommandOutput;
use systemprompt_files::issues::IssueService;
use systemprompt_identifiers::{IssueReportId, UserId};
use systemprompt_models::issues::IssueStatus;

#[derive(Debug, Args)]
pub struct TriageArgs {
    #[arg(help = "Issue report ID")]
    pub id: String,

    #[arg(
        long,
        help = "New status (open, triaged, needs_info, resolved, closed)"
    )]
    pub status: IssueStatus,

    #[arg(long, help = "Triage note shown to the reporter")]
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
struct TriageOutput {
    id: IssueReportId,
    from: IssueStatus,
    to: IssueStatus,
    note: Option<String>,
}

pub async fn execute_as(
    args: TriageArgs,
    triager: &UserId,
    issues: &IssueService,
) -> Result<CommandOutput> {
    let id = IssueReportId::try_new(args.id.as_str())
        .with_context(|| format!("invalid issue report id: {}", args.id))?;
    let from = issues
        .set_status(&id, args.status, args.note.clone(), triager)
        .await?;
    let output = TriageOutput {
        id,
        from,
        to: args.status,
        note: args.note,
    };
    Ok(CommandOutput::card_value(
        format!("Issue {}: {} -> {}", output.id.as_str(), from, args.status),
        &output,
    ))
}
