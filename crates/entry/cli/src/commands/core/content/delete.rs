use super::types::DeleteOutput;
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_content::ContentRepository;
use systemprompt_identifiers::{ContentId, SourceId};
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(help = "Content ID or slug")]
    pub identifier: String,

    #[arg(long, help = "Source ID (required when using slug)")]
    pub source: Option<String>,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,

    #[arg(long, help = "Preview deletion without executing")]
    pub dry_run: bool,
}

pub async fn execute(args: DeleteArgs, config: &CliConfig) -> Result<CommandResult<DeleteOutput>> {
    let ctx = AppContext::new().await?;
    let repo = ContentRepository::new(ctx.db_pool())?;

    let content = if args.identifier.starts_with("content_")
        || args.identifier.contains('-') && args.identifier.len() > 30
    {
        let id = ContentId::new(args.identifier.clone());
        repo.get_by_id(&id)
            .await?
            .ok_or_else(|| anyhow!("Content not found: {}", args.identifier))?
    } else {
        let source_id = args
            .source
            .as_ref()
            .ok_or_else(|| anyhow!("Source ID required when using slug (use --source)"))?;
        let source = SourceId::new(source_id.clone());
        repo.get_by_source_and_slug(&source, &args.identifier)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Content not found: {} in source {}",
                    args.identifier,
                    source_id
                )
            })?
    };

    if args.dry_run {
        let output = DeleteOutput {
            deleted: false,
            content_id: content.id.clone(),
            message: Some(format!(
                "[DRY-RUN] Would delete content '{}' ({})",
                content.title, content.id
            )),
        };
        return Ok(CommandResult::card(output).with_title("Content Delete (Dry Run)"));
    }

    if !args.yes && config.is_interactive() {
        CliService::warning(&format!(
            "This will permanently delete content: {}",
            args.identifier
        ));
        if !CliService::confirm("Are you sure you want to continue?")? {
            return Err(anyhow!("Operation cancelled"));
        }
    } else if !args.yes {
        return Err(anyhow!(
            "Use --yes to confirm deletion in non-interactive mode"
        ));
    }

    repo.delete(&content.id).await?;

    let output = DeleteOutput {
        deleted: true,
        content_id: content.id.clone(),
        message: None,
    };

    Ok(CommandResult::card(output).with_title("Content Deleted"))
}
