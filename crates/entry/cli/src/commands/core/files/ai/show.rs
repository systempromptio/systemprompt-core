use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_files::{FileService, TypeSpecificMetadata};
use systemprompt_identifiers::FileId;
use systemprompt_runtime::AppContext;

use crate::commands::core::files::types::{
    ChecksumsOutput, FileDetailOutput, FileMetadataOutput, ImageMetadataOutput,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "AI image file ID (UUID format)")]
    pub file_id: String,
}

pub async fn execute(
    args: ShowArgs,
    _config: &CliConfig,
) -> Result<CommandResult<FileDetailOutput>> {
    let file_id = parse_file_id(&args.file_id)?;

    let ctx = AppContext::new().await?;
    let service = FileService::new(ctx.db_pool())?;

    let file = service
        .find_by_id(&file_id)
        .await?
        .ok_or_else(|| anyhow!("File not found: {}", args.file_id))?;

    if !file.ai_content {
        return Err(anyhow!(
            "File '{}' is not an AI-generated image. Use 'files show' for regular files.",
            args.file_id
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

    Ok(CommandResult::card(output).with_title(format!("AI Image: {}", args.file_id)))
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

fn convert_metadata(file: &systemprompt_core_files::File) -> FileMetadataOutput {
    let Ok(metadata) = file.metadata() else {
        return FileMetadataOutput::default();
    };

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
