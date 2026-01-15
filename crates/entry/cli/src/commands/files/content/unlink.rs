use anyhow::{anyhow, Result};
use clap::Args;
use dialoguer::Confirm;
use systemprompt_core_files::ContentService;
use systemprompt_identifiers::{ContentId, FileId};
use systemprompt_runtime::AppContext;

use crate::commands::files::types::ContentUnlinkOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct UnlinkArgs {
    #[arg(help = "File ID (UUID format)")]
    pub file_id: String,

    #[arg(long, help = "Content ID")]
    pub content: String,

    #[arg(
        short = 'y',
        long,
        help = "Skip confirmation (required in non-interactive mode)"
    )]
    pub yes: bool,

    #[arg(long, help = "Preview unlink without executing")]
    pub dry_run: bool,
}

pub async fn execute(
    args: UnlinkArgs,
    config: &CliConfig,
) -> Result<CommandResult<ContentUnlinkOutput>> {
    let file_id = parse_file_id(&args.file_id)?;
    let content_id = ContentId::new(args.content.clone());

    // Handle confirmation requirement
    if !args.yes {
        if config.is_interactive() {
            let confirmed = Confirm::new()
                .with_prompt(format!(
                    "Unlink file '{}' from content '{}'?",
                    args.file_id, args.content
                ))
                .default(false)
                .interact()?;

            if !confirmed {
                return Err(anyhow!("Unlink cancelled by user"));
            }
        } else {
            return Err(anyhow!(
                "--yes is required to unlink files in non-interactive mode"
            ));
        }
    }

    // Handle dry-run
    if args.dry_run {
        let output = ContentUnlinkOutput {
            file_id,
            content_id,
            message: format!(
                "[DRY-RUN] Would unlink file '{}' from content '{}'",
                args.file_id, args.content
            ),
        };
        return Ok(CommandResult::card(output).with_title("File Unlink (Dry Run)"));
    }

    let ctx = AppContext::new().await?;
    let service = ContentService::new(ctx.db_pool())?;

    service.unlink_from_content(&content_id, &file_id).await?;

    let output = ContentUnlinkOutput {
        file_id,
        content_id,
        message: "File unlinked from content successfully".to_string(),
    };

    Ok(CommandResult::card(output).with_title("File Unlinked"))
}

fn parse_file_id(id: &str) -> Result<FileId> {
    uuid::Uuid::parse_str(id).map_err(|_| {
        anyhow!(
            "Invalid file ID format. Expected UUID like 'b75940ac-c50f-4d46-9fdd-ebb4970b2a7d', \
             got '{}'",
            id
        )
    })?;
    Ok(FileId::new(id.to_string()))
}
