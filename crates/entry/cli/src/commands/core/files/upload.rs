//! `core files upload` command with extension-based MIME detection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::Args;
use sha2::{Digest, Sha256};
use systemprompt_files::{FileUploadRequest, FileUploadService, FilesConfig};
use systemprompt_identifiers::{ContextId, SessionId, UserId};
use systemprompt_runtime::AppContext;
use tokio::fs;

use super::types::FileUploadOutput;
use crate::CliConfig;
use crate::shared::CommandOutput;

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

pub async fn execute(args: UploadArgs, _config: &CliConfig) -> Result<CommandOutput> {
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
    let checksum_sha256 = digest.iter().fold(String::with_capacity(64), |mut acc, b| {
        acc.push_str(&format!("{b:02x}"));
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

    Ok(CommandOutput::card_value("File Uploaded", &output))
}

const EXTENSION_MIME_TABLE: &[(&[&str], &str)] = &[
    (&["jpg", "jpeg"], "image/jpeg"),
    (&["png"], "image/png"),
    (&["gif"], "image/gif"),
    (&["webp"], "image/webp"),
    (&["svg"], "image/svg+xml"),
    (&["bmp"], "image/bmp"),
    (&["tiff", "tif"], "image/tiff"),
    (&["ico"], "image/x-icon"),
    (&["pdf"], "application/pdf"),
    (&["doc"], "application/msword"),
    (
        &["docx"],
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    (&["xls"], "application/vnd.ms-excel"),
    (
        &["xlsx"],
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ),
    (&["ppt"], "application/vnd.ms-powerpoint"),
    (
        &["pptx"],
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ),
    (&["txt"], "text/plain"),
    (&["csv"], "text/csv"),
    (&["md"], "text/markdown"),
    (&["html", "htm"], "text/html"),
    (&["json"], "application/json"),
    (&["xml"], "application/xml"),
    (&["rtf"], "application/rtf"),
    (&["mp3"], "audio/mpeg"),
    (&["wav"], "audio/wav"),
    (&["ogg"], "audio/ogg"),
    (&["aac"], "audio/aac"),
    (&["flac"], "audio/flac"),
    (&["m4a"], "audio/mp4"),
    (&["mp4"], "video/mp4"),
    (&["webm"], "video/webm"),
    (&["mov"], "video/quicktime"),
    (&["avi"], "video/x-msvideo"),
    (&["mkv"], "video/x-matroska"),
];

pub fn detect_mime_type(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);
    let Some(ext) = extension.as_deref() else {
        return "application/octet-stream".to_owned();
    };
    EXTENSION_MIME_TABLE
        .iter()
        .find(|(exts, _)| exts.contains(&ext))
        .map_or("application/octet-stream", |(_, mime)| *mime)
        .to_owned()
}
