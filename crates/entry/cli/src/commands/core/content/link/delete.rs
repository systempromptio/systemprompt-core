//! `core content link delete` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::commands::core::content::types::LinkDeleteOutput;
use crate::context::CommandContext;
use crate::interactive::require_confirmation;
use crate::shared::CommandOutput;
use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_content::services::LinkGenerationService;
use systemprompt_identifiers::LinkId;
use systemprompt_logging::CliService;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Link ID")]
    pub link_id: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}

pub async fn execute(args: DeleteArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let link_id = LinkId::new(args.link_id.clone());

    if ctx.cli.is_interactive() && !args.yes {
        CliService::warning(&format!(
            "This will permanently delete link: {}",
            args.link_id
        ));
    }

    require_confirmation(
        ctx.prompter(),
        "Are you sure you want to continue?",
        args.yes,
        &ctx.cli,
    )?;

    let pool = ctx.db_pool().await?;
    let service = LinkGenerationService::new(&pool)?;

    service
        .get_link_by_id(&link_id)
        .await?
        .ok_or_else(|| anyhow!("Link not found: {}", args.link_id))?;

    let deleted = service.delete_link(&link_id).await?;

    let output = LinkDeleteOutput { deleted, link_id };

    Ok(CommandOutput::card_value("Link Deleted", &output))
}
