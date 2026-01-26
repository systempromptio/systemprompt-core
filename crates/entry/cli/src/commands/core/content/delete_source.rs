use super::types::DeleteSourceOutput;
use crate::cli_settings::CliConfig;
use crate::interactive::require_confirmation;
use crate::shared::CommandResult;
use anyhow::Result;
use clap::Args;
use systemprompt_content::ContentRepository;
use systemprompt_identifiers::SourceId;
use systemprompt_logging::CliService;
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
    if config.is_interactive() && !args.yes {
        CliService::warning(&format!(
            "This will permanently delete ALL content from source: {}",
            args.source_id
        ));
    }

    require_confirmation("Are you sure you want to continue?", args.yes, config)?;

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
