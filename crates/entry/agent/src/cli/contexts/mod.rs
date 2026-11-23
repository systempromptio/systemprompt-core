//! `core contexts`: the caller's conversation contexts.
//!
//! `agent-http` contributes the group under the kernel's `core` group with
//! the `caller-session` and `session-context` grants. Every command acts on
//! the caller's own contexts, named by the ids of the caller's session; `use`
//! and `new` switch the stored CLI session through the kernel, which first
//! confirms the caller owns the context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod create;
pub mod delete;
pub mod edit;
pub mod list;
pub mod new;
pub mod resolve;
pub mod show;
mod types;
pub mod use_context;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_database::{DbPool, pool_from_handle};
use systemprompt_extension::cli::{
    CallerSession, CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput,
    DataImpact, DescribeCommand,
};

pub use types::{
    ContextCreatedOutput, ContextDeletedOutput, ContextListOutput, ContextSummary,
    ContextSwitchedOutput, ContextUpdatedOutput,
};

#[derive(Debug, Subcommand)]
pub enum ContextsCommands {
    #[command(about = "List all contexts with stats")]
    List(list::ListArgs),

    #[command(about = "Show context details")]
    Show(show::ShowArgs),

    #[command(about = "Create a new context")]
    Create(create::CreateArgs),

    #[command(about = "Rename a context")]
    Edit(edit::EditArgs),

    #[command(about = "Delete a context")]
    Delete(delete::DeleteArgs),

    #[command(name = "use", about = "Set session's active context")]
    Use(use_context::UseArgs),

    #[command(about = "Create a new context and set it as active")]
    New(new::NewArgs),
}

async fn caller_and_pool(host: &dyn CommandHost) -> Result<(CallerSession, DbPool)> {
    let caller = host.caller_session().await?;
    let pool = pool_from_handle(host.db().await?)?;
    Ok((caller, pool))
}

pub async fn execute(cmd: ContextsCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match cmd {
        ContextsCommands::List(args) => Box::pin(list::execute(args, host)).await,
        ContextsCommands::Show(args) => Box::pin(show::execute(args, host)).await,
        ContextsCommands::Create(args) => Box::pin(create::execute(args, host)).await,
        ContextsCommands::Edit(args) => Box::pin(edit::execute(args, host)).await,
        ContextsCommands::Delete(args) => Box::pin(delete::execute(args, host)).await,
        ContextsCommands::Use(args) => Box::pin(use_context::execute(args, host)).await,
        ContextsCommands::New(args) => Box::pin(new::execute(args, host)).await,
    }
}

impl ContextsCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Delete(_) => DataImpact::Destructive,
            Self::List(_)
            | Self::Show(_)
            | Self::Create(_)
            | Self::Edit(_)
            | Self::Use(_)
            | Self::New(_) => DataImpact::Preserving,
        }
    }
}

impl DescribeCommand for ContextsCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(self.data_impact())
    }
}

impl CliCommandGroup for ContextsCommands {
    const NAME: &'static str = "contexts";
    const ABOUT: &'static str = "Context management";

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host).await.map_err(crate::cli::command_error)
    }
}
