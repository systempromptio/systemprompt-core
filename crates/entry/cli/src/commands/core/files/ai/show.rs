//! `core files ai show` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_files::{FileRepository, TypeSpecificMetadata};
use systemprompt_identifiers::FileId;
use systemprompt_runtime::AppContext;

use crate::CliConfig;
use crate::commands::core::files::types::{
    ChecksumsOutput, FileDetailOutput, FileMetadataOutput, ImageMetadataOutput,
};
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(value_name = "FILE_ID", help = "AI image file ID (UUID format)")]
    pub file: String,
}

pub(super) async fn execute(args: ShowArgs, config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let file_id = parse_file_id(&args.file)?;

    let service = FileRepository::new(pool)?;

    let file = service
        .find_by_id(&file_id)
        .await?
        .ok_or_else(|| anyhow!("File not found: {}", args.file))?;

    if !file.ai_content {
        return Err(anyhow!(
            "File '{}' is not an AI-generated image. Use 'files show' for regular files.",
            args.file
        ));
    }

    let metadata_output = convert_metadata(&file);

    let output = FileDetailOutput {
        id: file.id(),
        path: file.path,
        public_url: file.public_url,
        mime_type: file.mime_type,
        size_bytes: file.size_bytes,
        ai_content: file.ai_content,
        user_id: file.user_id,
        session_id: file.session_id,
        trace_id: file.trace_id,
        context_id: file.context_id,
        metadata: metadata_output,
        created_at: file.created_at,
        updated_at: file.updated_at,
    };

    Ok(CommandOutput::card_value(
        format!("AI Image: {}", args.file),
        &output,
    ))
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

fn convert_metadata(file: &systemprompt_files::File) -> FileMetadataOutput {
    let metadata = file.metadata.0.clone();

    let checksums = metadata.checksums.map(|c| ChecksumsOutput {
        md5: c.md5,
        sha256: c.sha256,
    });

    let image = match metadata.type_specific {
        Some(TypeSpecificMetadata::Image(img)) => Some(ImageMetadataOutput {
            width: img.width,
            height: img.height,
            alt_text: img.alt_text,
            description: img.description,
        }),
        _ => None,
    };

    FileMetadataOutput {
        checksums,
        image,
        document: None,
        audio: None,
        video: None,
    }
}
