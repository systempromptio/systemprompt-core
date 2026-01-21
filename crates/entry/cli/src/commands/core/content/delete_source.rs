use super::types::DeleteSourceOutput;
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_content::ContentRepository;
use systemprompt_logging::CliService;
use systemprompt_identifiers::SourceId;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct DeleteSourceArgs {
    #[arg(help = "Source ID")]
    pub source_id: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}

pub async fn execute(
    args: DeleteSourceArgs,
    config: &CliConfig,
) -> Result<CommandResult<DeleteSourceOutput>> {
    if !args.yes && config.is_interactive() {
        CliService::warning(&format!(
            "This will permanently delete ALL content from source: {}",
            args.source_id
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
    let repo = ContentRepository::new(ctx.db_pool())?;

    let source = SourceId::new(args.source_id.clone());
    let deleted_count = repo.delete_by_source(&source).await?;

    let output = DeleteSourceOutput {
        deleted_count,
        source_id: source,
    };

    Ok(CommandResult::card(output).with_title("Source Content Deleted"))
}
