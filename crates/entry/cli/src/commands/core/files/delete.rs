use anyhow::{anyhow, Result};
use clap::Args;
use dialoguer::Confirm;
use systemprompt_files::FileService;
use systemprompt_identifiers::FileId;
use systemprompt_runtime::AppContext;

use super::types::FileDeleteOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct DeleteArgs {
    #[arg(help = "File ID (UUID format)")]
    pub file_id: String,

    #[arg(
        short = 'y',
        long,
        help = "Skip confirmation (required in non-interactive mode)"
    )]
    pub yes: bool,

    #[arg(long, help = "Preview deletion without executing")]
    pub dry_run: bool,
}

pub async fn execute(
    args: DeleteArgs,
    config: &CliConfig,
) -> Result<CommandResult<FileDeleteOutput>> {
    let file_id = parse_file_id(&args.file_id)?;

    let ctx = AppContext::new().await?;
    let service = FileService::new(ctx.db_pool())?;

    let file = service
        .find_by_id(&file_id)
        .await?
        .ok_or_else(|| anyhow!("File not found: {}", args.file_id))?;

    if args.dry_run {
        let output = FileDeleteOutput {
            file_id,
            message: format!(
                "[DRY-RUN] Would delete file '{}' ({})",
                file.path, args.file_id
            ),
        };
        return Ok(CommandResult::card(output).with_title("File Delete (Dry Run)"));
    }

    if !args.yes {
        if config.is_interactive() {
            let confirmed = Confirm::new()
                .with_prompt(format!(
                    "Delete file '{}' ({})? This action cannot be undone.",
                    file.path, args.file_id
                ))
                .default(false)
                .interact()?;

            if !confirmed {
                return Err(anyhow!("Deletion cancelled by user"));
            }
        } else {
            return Err(anyhow!(
                "--yes is required to delete files in non-interactive mode"
            ));
        }
    }

    service.delete(&file_id).await?;

    let output = FileDeleteOutput {
        file_id,
        message: format!("File '{}' deleted successfully", file.path),
    };

    Ok(CommandResult::card(output).with_title("File Deleted"))
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
