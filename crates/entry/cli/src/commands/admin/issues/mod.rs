//! `admin issues` command tree: operator triage of the bug and feedback
//! reports users and agents file.
//!
//! [`IssuesCommands`] lists reports, shows one with the follow-up commands
//! for the platform records it names, and moves a report through the triage
//! states as the CLI session user.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod list;
mod show;
mod triage;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::descriptor::DataImpact;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum IssuesCommands {
    #[command(about = "List issue reports, most recently seen first")]
    List(list::ListArgs),

    #[command(about = "Show an issue report with its context and attachments")]
    Show(show::ShowArgs),

    #[command(about = "Move an issue report to a triage status")]
    Triage(triage::TriageArgs),
}

pub async fn execute(cmd: IssuesCommands, ctx: &CommandContext) -> Result<()> {
    let result = match cmd {
        IssuesCommands::List(args) => list::execute(args, ctx).await?,
        IssuesCommands::Show(args) => show::execute(args, ctx).await?,
        IssuesCommands::Triage(args) => Box::pin(triage::execute(args, ctx)).await?,
    };
    render_result(&result, &ctx.cli);
    Ok(())
}

impl IssuesCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::List(_) | Self::Show(_) | Self::Triage(_) => DataImpact::Preserving,
        }
    }
}
