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

pub use triage::{TriageArgs, execute_as as triage_as};

use crate::kernel::KernelHost;
use anyhow::Result;
use clap::Subcommand;

use systemprompt_extension::cli::{CommandOutput, DataImpact};

#[derive(Debug, Subcommand)]
pub enum IssuesCommands {
    #[command(about = "List issue reports, most recently seen first")]
    List(list::ListArgs),

    #[command(about = "Show an issue report with its context and attachments")]
    Show(show::ShowArgs),

    #[command(about = "Move an issue report to a triage status")]
    Triage(TriageArgs),
}

pub async fn execute(cmd: IssuesCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match cmd {
        IssuesCommands::List(args) => list::execute(args, host).await,
        IssuesCommands::Show(args) => show::execute(args, host).await,
        IssuesCommands::Triage(args) => Box::pin(triage::execute(args, host)).await,
    }
}

impl IssuesCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::List(_) | Self::Show(_) | Self::Triage(_) => DataImpact::Preserving,
        }
    }
}
