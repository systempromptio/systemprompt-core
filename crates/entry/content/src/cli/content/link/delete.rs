//! `core content link delete` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli::content::types::LinkDeleteOutput;
use anyhow::{Result, anyhow};
use clap::Args;
use std::sync::Arc;
use systemprompt_content::ContentRepositories;
use systemprompt_content::services::LinkGenerationService;
use systemprompt_extension::cli::{CommandHost, CommandOutput, require_confirmation};
use systemprompt_identifiers::LinkId;
use systemprompt_logging::CliService;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Link ID", value_parser = systemprompt_extension::cli::parsers::parse_link_id)]
    pub link_id: LinkId,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}

pub async fn execute(args: DeleteArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    if host.output().is_interactive() && !args.yes {
        CliService::warning(&format!(
            "This will permanently delete link: {}",
            args.link_id
        ));
    }

    require_confirmation(
        host.prompter(),
        "Are you sure you want to continue?",
        args.yes,
        host.output(),
    )?;

    let service = LinkGenerationService::new(
        host.resolved()
            .await?
            .get::<Arc<ContentRepositories>>()?
            .link
            .clone(),
    );

    service
        .find_link_by_id(&args.link_id)
        .await?
        .ok_or_else(|| anyhow!("Link not found: {}", args.link_id))?;

    let deleted = service.delete_link(&args.link_id).await?;

    let output = LinkDeleteOutput {
        deleted,
        link_id: args.link_id,
    };

    Ok(CommandOutput::card_value("Link Deleted", &output))
}
