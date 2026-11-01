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
use systemprompt_identifiers::{IssueReportId, UserId};
use systemprompt_models::issues::IssueStatus;

use crate::context::CommandContext;
use crate::session::get_or_create_session;
use crate::shared::CommandOutput;

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

pub(super) async fn execute(args: TriageArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let session_ctx = get_or_create_session(ctx).await?;
    execute_as(args, &session_ctx.session.user_id, ctx).await
}

pub async fn execute_as(
    args: TriageArgs,
    triager: &UserId,
    ctx: &CommandContext,
) -> Result<CommandOutput> {
    let app = ctx.app_context().await?;
    let id = IssueReportId::try_new(args.id.as_str())
        .with_context(|| format!("invalid issue report id: {}", args.id))?;
    let from = app
        .documents()
        .issues()
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
