//! `admin issues show` command.
//!
//! The card carries the full report and, for each platform record the report
//! names, the CLI command that inspects it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use serde::Serialize;
use systemprompt_identifiers::IssueReportId;
use systemprompt_models::issues::IssueReport;

use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Issue report ID")]
    pub id: String,
}

#[derive(Debug, Serialize)]
struct IssueDetailOutput {
    #[serde(flatten)]
    report: IssueReport,
    follow_up: Vec<String>,
}

pub(super) async fn execute(args: ShowArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let app = ctx.app_context().await?;
    let id = IssueReportId::new(&args.id);
    let report = app
        .documents()
        .issues()
        .find(&id)
        .await?
        .ok_or_else(|| anyhow!("Issue report not found: {}", args.id))?;
    let title = format!("Issue: {}", report.title);
    let output = IssueDetailOutput {
        follow_up: follow_up(&report),
        report,
    };
    Ok(CommandOutput::card_value(title, &output))
}

fn follow_up(report: &IssueReport) -> Vec<String> {
    let context = &report.context;
    let mut commands = Vec::new();
    if let Some(trace) = &context.trace {
        commands.push(format!(
            "systemprompt infra logs trace show {} --all",
            trace.as_str()
        ));
    }
    if let Some(task) = &context.task {
        commands.push(format!(
            "systemprompt infra logs trace show {} --all",
            task.as_str()
        ));
    }
    if context.session.is_some() {
        commands.push(format!(
            "systemprompt admin users session list {}",
            report.reporter_user_id.as_str()
        ));
    }
    commands
}
