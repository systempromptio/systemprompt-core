use super::types::DeleteOutput;
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_content::ContentRepository;
use systemprompt_core_logging::CliService;
use systemprompt_identifiers::ContentId;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Content ID")]
    pub content_id: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}

pub async fn execute(args: DeleteArgs, config: &CliConfig) -> Result<CommandResult<DeleteOutput>> {
    if !args.yes && config.interactive {
        CliService::warning(&format!(
            "This will permanently delete content: {}",
            args.content_id
        ));
        if !CliService::confirm("Are you sure you want to continue?") {
            return Err(anyhow!("Operation cancelled"));
        }
    } else if !args.yes {
        return Err(anyhow!("Use --yes to confirm deletion in non-interactive mode"));
    }

    let ctx = AppContext::new().await?;
    let repo = ContentRepository::new(ctx.db_pool())?;

    let id = ContentId::new(args.content_id.clone());

    let existing = repo.get_by_id(&id).await?;
    if existing.is_none() {
        return Err(anyhow!("Content not found: {}", args.content_id));
    }

    repo.delete(&id).await?;

    let output = DeleteOutput {
        deleted: true,
        content_id: args.content_id,
    };

    Ok(CommandResult::card(output).with_title("Content Deleted"))
}
