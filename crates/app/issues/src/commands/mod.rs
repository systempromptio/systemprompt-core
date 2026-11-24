//! `admin issues` command tree: operator triage of the bug and feedback
//! reports users and agents file.
//!
//! [`IssuesCommands`](crate::commands::IssuesCommands) lists reports, shows one with the follow-up commands
//! for the platform records it names, and moves a report through the triage
//! states as the CLI session user. The `issues` member contributes the group
//! under the kernel's `admin` group; every command reads the issue service
//! from the `DocumentsHandlesSlot` the `documents` member provides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod list;
mod show;
mod triage;

pub use list::ListArgs;
pub use show::ShowArgs;
pub use triage::{TriageArgs, execute_as as triage_as};

use std::sync::Arc;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
};
use systemprompt_files::documents::{DocumentsHandlesSlot, bound};
use systemprompt_files::issues::IssueService;

async fn issue_service(host: &dyn CommandHost) -> Result<Arc<IssueService>> {
    let slot = host.resolved().await?.get::<DocumentsHandlesSlot>()?;
    Ok(Arc::clone(&bound(&slot)?.issues))
}

#[derive(Debug, Subcommand)]
pub enum IssuesCommands {
    #[command(about = "List issue reports, most recently seen first")]
    List(ListArgs),

    #[command(about = "Show an issue report with its context and attachments")]
    Show(ShowArgs),

    #[command(about = "Move an issue report to a triage status")]
    Triage(TriageArgs),
}

pub async fn execute(cmd: IssuesCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    let issues = issue_service(host).await?;
    match cmd {
        IssuesCommands::List(args) => list::execute(args, &issues).await,
        IssuesCommands::Show(args) => show::execute(args, &issues).await,
        IssuesCommands::Triage(args) => {
            let session = host.caller_session().await?;
            triage::execute_as(args, session.user_id(), &issues).await
        },
    }
}

impl IssuesCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::List(_) | Self::Show(_) | Self::Triage(_) => DataImpact::Preserving,
        }
    }
}

impl DescribeCommand for IssuesCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(self.data_impact())
    }
}

impl CliCommandGroup for IssuesCommands {
    const NAME: &'static str = "issues";
    const ABOUT: &'static str = "Issue report triage (bug and feedback reports)";

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host)
            .await
            .map_err(|error| match error.downcast::<CommandError>() {
                Ok(failed) => failed,
                Err(error) => CommandError::new(error),
            })
    }
}
