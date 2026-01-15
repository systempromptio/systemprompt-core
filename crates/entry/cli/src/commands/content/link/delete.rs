use crate::cli_settings::CliConfig;
use crate::commands::content::types::LinkDeleteOutput;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_content::services::LinkGenerationService;
use systemprompt_core_logging::CliService;
use systemprompt_identifiers::LinkId;
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

    if !args.yes && config.is_interactive() {
        CliService::warning(&format!(
            "This will permanently delete link: {}",
            args.link_id
        ));
        if !CliService::confirm("Are you sure you want to continue?")? {
            return Err(anyhow!("Operation cancelled"));
        }
    } else if !args.yes {
        return Err(anyhow!(
            "Use --yes to confirm deletion in non-interactive mode"
        ));
    }

    let ctx = AppContext::new().await?;
    let service = LinkGenerationService::new(ctx.db_pool())?;

    service
        .get_link_by_id(&link_id)
        .await?
        .ok_or_else(|| anyhow!("Link not found: {}", args.link_id))?;

    let deleted = service.delete_link(&link_id).await?;

    let output = LinkDeleteOutput {
        deleted,
        link_id: args.link_id,
    };

    Ok(CommandResult::card(output).with_title("Link Deleted"))
}
