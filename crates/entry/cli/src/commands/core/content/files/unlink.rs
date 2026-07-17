//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_files::FileRepository;
use systemprompt_identifiers::{ContentId, FileId};
use systemprompt_runtime::AppContext;

use crate::CliConfig;
use crate::commands::core::files::types::ContentUnlinkOutput;
use crate::interactive::Prompter;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct UnlinkArgs {
    #[arg(value_name = "FILE_ID", help = "File ID (UUID format)")]
    pub file: String,

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

pub(super) async fn execute(
    args: UnlinkArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, prompter, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: UnlinkArgs,
    prompter: &dyn Prompter,
    pool: &DbPool,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let file_id = parse_file_id(&args.file)?;
    let content_id = ContentId::new(args.content.clone());

    if !args.yes {
        if config.is_interactive() {
            let confirmed = prompter.confirm(
                &format!(
                    "Unlink file '{}' from content '{}'?",
                    args.file, args.content
                ),
                false,
            )?;

            if !confirmed {
                return Err(anyhow!("Unlink cancelled by user"));
            }
        } else {
            return Err(anyhow!(
                "--yes is required to unlink files in non-interactive mode"
            ));
        }
    }

    if args.dry_run {
        let output = ContentUnlinkOutput {
            file_id,
            content_id,
            message: format!(
                "[DRY-RUN] Would unlink file '{}' from content '{}'",
                args.file, args.content
            ),
        };
        return Ok(CommandOutput::card_value("File Unlink (Dry Run)", &output));
    }

    let service = FileRepository::new(pool)?;

    service.unlink_from_content(&content_id, &file_id).await?;

    let output = ContentUnlinkOutput {
        file_id,
        content_id,
        message: "File unlinked from content successfully".to_owned(),
    };

    Ok(CommandOutput::card_value("File Unlinked", &output))
}

fn parse_file_id(id: &str) -> Result<FileId> {
    uuid::Uuid::parse_str(id).map_err(|_e| {
        anyhow!(
            "Invalid file ID format. Expected UUID like 'b75940ac-c50f-4d46-9fdd-ebb4970b2a7d', \
             got '{}'",
            id
        )
    })?;
    Ok(FileId::new(id.to_owned()))
}
