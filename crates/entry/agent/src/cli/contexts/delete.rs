//! `core contexts delete` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, bail};
use clap::Args;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CallerSession, CliConfig, CommandHost, CommandOutput, Prompter};

use super::caller_and_pool;
use super::resolve::resolve_context;
use super::types::ContextDeletedOutput;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Context ID (full, partial prefix, or name)")]
    pub context: String,

    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,
}

pub(super) async fn execute(args: DeleteArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let (caller, pool) = caller_and_pool(host).await?;
    execute_with_pool(args, &caller, &pool, host.output(), host.prompter()).await
}

pub async fn execute_with_pool(
    args: DeleteArgs,
    caller: &CallerSession,
    pool: &DbPool,
    config: &CliConfig,
    prompter: &dyn Prompter,
) -> Result<CommandOutput> {
    let repo = ContextRepository::new(pool);

    let context_id = resolve_context(&args.context, caller.user_id(), &repo).await?;

    if caller.context_id() == Some(&context_id) {
        bail!(
            "Cannot delete the active context. Switch to a different context first with 'contexts \
             use <id>'."
        );
    }

    let context = repo
        .get_context(&context_id, caller.user_id())
        .await
        .context("Failed to fetch context details")?;

    if !args.yes && !config.is_interactive() {
        bail!("--yes is required in non-interactive mode");
    }

    if !args.yes {
        let question = format!(
            "Delete context '{}' ({})?",
            context.name,
            context_id.as_str()
        );
        if !prompter.confirm(&question, false)? {
            let cancelled = ContextDeletedOutput {
                id: context_id,
                message: "Deletion cancelled".to_owned(),
            };
            return Ok(CommandOutput::card_value(
                "Context Delete Cancelled",
                &cancelled,
            ));
        }
    }

    repo.delete_context(&context_id, caller.user_id())
        .await
        .context("Failed to delete context")?;

    let output = ContextDeletedOutput {
        id: context_id,
        message: format!("Context '{}' deleted successfully", context.name),
    };

    Ok(CommandOutput::card_value("Context Deleted", &output))
}
