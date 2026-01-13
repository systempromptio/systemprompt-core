use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_files::{File, FileService, TypeSpecificMetadata};
use systemprompt_identifiers::FileId;
use systemprompt_runtime::AppContext;

use super::types::{
    AudioMetadataOutput, ChecksumsOutput, DocumentMetadataOutput, FileDetailOutput,
    FileMetadataOutput, ImageMetadataOutput, VideoMetadataOutput,
};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "File ID or path")]
    pub identifier: String,
}

pub async fn execute(
    args: ShowArgs,
    _config: &CliConfig,
) -> Result<CommandResult<FileDetailOutput>> {
    let ctx = AppContext::new().await?;
    let service = FileService::new(ctx.db_pool())?;

    let file = find_file(&service, &args.identifier).await?;

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

    Ok(CommandResult::card(output).with_title(format!("File: {}", args.identifier)))
}

async fn find_file(service: &FileService, identifier: &str) -> Result<File> {
    if identifier.starts_with('/') || identifier.contains('/') {
        service
            .find_by_path(identifier)
            .await?
            .ok_or_else(|| anyhow!("File not found with path: {}", identifier))
    } else {
        let file_id = FileId::new(identifier.to_string());
        service
            .find_by_id(&file_id)
            .await?
            .ok_or_else(|| anyhow!("File not found with ID: {}", identifier))
    }
}

fn convert_metadata(file: &File) -> FileMetadataOutput {
    let Ok(metadata) = file.metadata() else {
        return FileMetadataOutput::default();
    };

    let checksums = metadata.checksums.map(|c| ChecksumsOutput {
        md5: c.md5,
        sha256: c.sha256,
    });

    let (image, document, audio, video) = match metadata.type_specific {
        Some(TypeSpecificMetadata::Image(img)) => (
            Some(ImageMetadataOutput {
                width: img.width,
                height: img.height,
                alt_text: img.alt_text,
                description: img.description,
            }),
            None,
            None,
            None,
        ),
        Some(TypeSpecificMetadata::Document(doc)) => (
            None,
            Some(DocumentMetadataOutput {
                title: doc.title,
                author: doc.author,
                page_count: doc.page_count,
            }),
            None,
            None,
        ),
        Some(TypeSpecificMetadata::Audio(aud)) => (
            None,
            None,
            Some(AudioMetadataOutput {
                duration_seconds: aud.duration_seconds,
                sample_rate: aud.sample_rate,
                channels: aud.channels,
            }),
            None,
        ),
        Some(TypeSpecificMetadata::Video(vid)) => (
            None,
            None,
            None,
            Some(VideoMetadataOutput {
                width: vid.width,
                height: vid.height,
                duration_seconds: vid.duration_seconds,
                frame_rate: vid.frame_rate,
            }),
        ),
        None => (None, None, None, None),
    };

    FileMetadataOutput {
        checksums,
        image,
        document,
        audio,
        video,
    }
}
