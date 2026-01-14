use anyhow::{anyhow, Result};
use clap::Args;
use dialoguer::Confirm;
use systemprompt_core_files::FileService;
use systemprompt_identifiers::FileId;
use systemprompt_runtime::AppContext;

use super::types::FileDeleteOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct DeleteArgs {
    #[arg(help = "File ID")]
    pub file_id: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}

pub async fn execute(
    args: DeleteArgs,
    config: &CliConfig,
) -> Result<CommandResult<FileDeleteOutput>> {
    let file_id = FileId::new(args.file_id.clone());

    let ctx = AppContext::new().await?;
    let service = FileService::new(ctx.db_pool())?;

    let file = service
        .find_by_id(&file_id)
        .await?
        .ok_or_else(|| anyhow!("File not found: {}", args.file_id))?;

    if !args.yes && config.is_interactive() {
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
    }

    service.delete(&file_id).await?;

    let output = FileDeleteOutput {
        file_id,
        message: format!("File '{}' deleted successfully", file.path),
    };

    Ok(CommandResult::card(output).with_title("File Deleted"))
}
