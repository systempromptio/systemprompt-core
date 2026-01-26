use crate::cli_settings::CliConfig;
use crate::commands::core::content::types::LinkDeleteOutput;
use crate::interactive::require_confirmation;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_content::services::LinkGenerationService;
use systemprompt_identifiers::LinkId;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Link ID")]
    pub link_id: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}

pub async fn execute(
    args: DeleteArgs,
    config: &CliConfig,
) -> Result<CommandResult<LinkDeleteOutput>> {
    let link_id = LinkId::new(args.link_id.clone());

    if config.is_interactive() && !args.yes {
        CliService::warning(&format!(
            "This will permanently delete link: {}",
            args.link_id
        ));
    }

    require_confirmation("Are you sure you want to continue?", args.yes, config)?;

    let ctx = AppContext::new().await?;
    let service = LinkGenerationService::new(ctx.db_pool())?;

    service
        .get_link_by_id(&link_id)
        .await?
        .ok_or_else(|| anyhow!("Link not found: {}", args.link_id))?;

    let deleted = service.delete_link(&link_id).await?;

    let output = LinkDeleteOutput { deleted, link_id };

    Ok(CommandResult::card(output).with_title("Link Deleted"))
}
