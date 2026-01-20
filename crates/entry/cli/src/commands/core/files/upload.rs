use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use clap::Args;
use sha2::{Digest, Sha256};
use systemprompt_core_files::{FileUploadRequest, FileUploadService, FilesConfig};
use systemprompt_identifiers::{ContextId, SessionId, UserId};
use systemprompt_runtime::AppContext;
use tokio::fs;

use super::types::FileUploadOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct UploadArgs {
    #[arg(help = "Path to file to upload")]
    pub file_path: PathBuf,

    #[arg(long, help = "Context ID (required)")]
    pub context: String,

    #[arg(long, help = "User ID")]
    pub user: Option<String>,

    #[arg(long, help = "Session ID")]
    pub session: Option<String>,

    #[arg(long, help = "Mark as AI-generated content")]
    pub ai: bool,
}

pub async fn execute(
    args: UploadArgs,
    _config: &CliConfig,
) -> Result<CommandResult<FileUploadOutput>> {
    let ctx = AppContext::new().await?;
    let files_config = FilesConfig::get()?;
    let service = FileUploadService::new(ctx.db_pool(), files_config.clone())?;

    if !service.is_enabled() {
        return Err(anyhow!("File uploads are disabled in configuration"));
    }

    let file_path = args
        .file_path
        .canonicalize()
        .map_err(|e| anyhow!("File not found: {} - {}", args.file_path.display(), e))?;

    let bytes = fs::read(&file_path).await?;
    let bytes_base64 = STANDARD.encode(&bytes);
    let digest = Sha256::digest(&bytes);
    #[allow(clippy::expect_used)]
    let checksum_sha256 = digest.iter().fold(String::with_capacity(64), |mut acc, b| {
        use std::fmt::Write;
        write!(acc, "{b:02x}").expect("write to String is infallible");
        acc
    });
    let size_bytes = bytes.len() as i64;

    let mime_type = detect_mime_type(&file_path);
    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(String::from);

    let context_id = ContextId::new(args.context);

    let request = FileUploadRequest {
        name: filename,
        mime_type: mime_type.clone(),
        bytes_base64,
        context_id,
        user_id: args.user.map(UserId::new),
        session_id: args.session.map(SessionId::new),
        trace_id: None,
    };

    let result = service.upload_file(request).await?;

    let output = FileUploadOutput {
        file_id: result.file_id,
        path: result.path,
        public_url: result.public_url,
        size_bytes,
        mime_type,
        checksum_sha256,
    };

    Ok(CommandResult::card(output).with_title("File Uploaded"))
}

pub fn detect_mime_type(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);

    match extension.as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg".to_string(),
        Some("png") => "image/png".to_string(),
        Some("gif") => "image/gif".to_string(),
        Some("webp") => "image/webp".to_string(),
        Some("svg") => "image/svg+xml".to_string(),
        Some("bmp") => "image/bmp".to_string(),
        Some("tiff" | "tif") => "image/tiff".to_string(),
        Some("ico") => "image/x-icon".to_string(),
        Some("pdf") => "application/pdf".to_string(),
        Some("doc") => "application/msword".to_string(),
        Some("docx") => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string()
        },
        Some("xls") => "application/vnd.ms-excel".to_string(),
        Some("xlsx") => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()
        },
        Some("ppt") => "application/vnd.ms-powerpoint".to_string(),
        Some("pptx") => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string()
        },
        Some("txt") => "text/plain".to_string(),
        Some("csv") => "text/csv".to_string(),
        Some("md") => "text/markdown".to_string(),
        Some("html" | "htm") => "text/html".to_string(),
        Some("json") => "application/json".to_string(),
        Some("xml") => "application/xml".to_string(),
        Some("rtf") => "application/rtf".to_string(),
        Some("mp3") => "audio/mpeg".to_string(),
        Some("wav") => "audio/wav".to_string(),
        Some("ogg") => "audio/ogg".to_string(),
        Some("aac") => "audio/aac".to_string(),
        Some("flac") => "audio/flac".to_string(),
        Some("m4a") => "audio/mp4".to_string(),
        Some("mp4") => "video/mp4".to_string(),
        Some("webm") => "video/webm".to_string(),
        Some("mov") => "video/quicktime".to_string(),
        Some("avi") => "video/x-msvideo".to_string(),
        Some("mkv") => "video/x-matroska".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}
