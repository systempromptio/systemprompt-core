//! `core files delete` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_files::{FileDeleteService, FileRepository, StoredObjectOutcome};
use systemprompt_identifiers::FileId;
use systemprompt_traits::FileStorage;

use super::types::FileDeleteOutput;
use crate::CliConfig;
use crate::context::CommandContext;
use crate::interactive::Prompter;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct DeleteArgs {
    #[arg(value_name = "FILE_ID", help = "File ID (UUID format)")]
    pub file: String,

    #[arg(
        short = 'y',
        long,
        help = "Skip confirmation (required in non-interactive mode)"
    )]
    pub yes: bool,

    #[arg(long, help = "Preview deletion without executing")]
    pub dry_run: bool,
}

pub(super) async fn execute(
    args: DeleteArgs,
    prompter: &dyn Prompter,
    ctx: &CommandContext,
) -> Result<CommandOutput> {
    let app = ctx.app_context().await?;
    execute_with_pool(
        args,
        prompter,
        app.db_pool(),
        Arc::clone(app.file_storage()),
        &ctx.cli,
    )
    .await
}

pub async fn execute_with_pool(
    args: DeleteArgs,
    prompter: &dyn Prompter,
    pool: &DbPool,
    storage: Arc<dyn FileStorage>,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let file_id = parse_file_id(&args.file)?;

    let repository = FileRepository::new(pool);

    let file = repository
        .find_by_id(&file_id)
        .await?
        .ok_or_else(|| anyhow!("File not found: {}", args.file))?;

    if args.dry_run {
        let output = FileDeleteOutput {
            file_id,
            message: format!(
                "[DRY-RUN] Would delete file '{}' ({})",
                file.path, args.file
            ),
        };
        return Ok(CommandOutput::card_value("File Delete (Dry Run)", &output));
    }

    if !args.yes {
        if config.is_interactive() {
            let confirmed = prompter.confirm(
                &format!(
                    "Delete file '{}' ({})? This action cannot be undone.",
                    file.path, args.file
                ),
                false,
            )?;

            if !confirmed {
                return Err(anyhow!("Deletion cancelled by user"));
            }
        } else {
            return Err(anyhow!(
                "--yes is required to delete files in non-interactive mode"
            ));
        }
    }

    let outcome = FileDeleteService::new(repository, storage)
        .delete_file(&file)
        .await?;

    let message = match outcome {
        StoredObjectOutcome::Removed => {
            format!("File '{}' and its stored object deleted", file.path)
        },
        StoredObjectOutcome::AlreadyAbsent => format!(
            "File '{}' deleted; its stored object was already absent",
            file.path
        ),
        StoredObjectOutcome::NotInStorage => format!(
            "File record '{}' deleted; the file on disk is outside file storage and was left in \
             place",
            file.path
        ),
    };

    let output = FileDeleteOutput { file_id, message };

    Ok(CommandOutput::card_value("File Deleted", &output))
}

fn parse_file_id(id: &str) -> Result<FileId> {
    FileId::try_new(id).map_err(|_e| {
        anyhow!(
            "Invalid file ID format. Expected UUID like 'b75940ac-c50f-4d46-9fdd-ebb4970b2a7d', \
             got '{}'",
            id
        )
    })
}
