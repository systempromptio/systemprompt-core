use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContentId, ContextId, FileId, SessionId, TraceId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileListOutput {
    pub files: Vec<FileSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileSummary {
    pub id: FileId,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub ai_content: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileDetailOutput {
    pub id: FileId,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub ai_content: bool,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub context_id: Option<ContextId>,
    pub metadata: FileMetadataOutput,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct FileMetadataOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksums: Option<ChecksumsOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageMetadataOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<DocumentMetadataOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioMetadataOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoMetadataOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChecksumsOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageMetadataOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DocumentMetadataOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct AudioMetadataOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u8>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct VideoMetadataOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_rate: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileUploadOutput {
    pub file_id: FileId,
    pub path: String,
    pub public_url: String,
    pub size_bytes: i64,
    pub mime_type: String,
    pub checksum_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileDeleteOutput {
    pub file_id: FileId,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileValidationOutput {
    pub valid: bool,
    pub mime_type: String,
    pub category: String,
    pub size_bytes: u64,
    pub max_size_bytes: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileConfigOutput {
    pub uploads_enabled: bool,
    pub max_file_size_bytes: u64,
    pub persistence_mode: String,
    pub storage_root: String,
    pub url_prefix: String,
    pub allowed_types: AllowedTypesOutput,
    pub storage_paths: StoragePathsOutput,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct AllowedTypesOutput {
    pub images: bool,
    pub documents: bool,
    pub audio: bool,
    pub video: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StoragePathsOutput {
    pub uploads: String,
    pub images: String,
    pub documents: String,
    pub audio: String,
    pub video: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentFilesOutput {
    pub content_id: ContentId,
    pub files: Vec<ContentFileRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentFileRow {
    pub file_id: FileId,
    pub path: String,
    pub mime_type: String,
    pub role: String,
    pub display_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentLinkOutput {
    pub file_id: FileId,
    pub content_id: ContentId,
    pub role: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentUnlinkOutput {
    pub file_id: FileId,
    pub content_id: ContentId,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeaturedImageOutput {
    pub content_id: ContentId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<FileSummary>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiFilesListOutput {
    pub files: Vec<FileSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiFilesCountOutput {
    pub count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<UserId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileContentLinksOutput {
    pub file_id: FileId,
    pub links: Vec<FileContentLinkRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileContentLinkRow {
    pub content_id: ContentId,
    pub role: String,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileStatsOutput {
    pub total_files: i64,
    pub total_size_bytes: i64,
    pub ai_images_count: i64,
    pub by_category: FileCategoryStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileCategoryStats {
    pub images: CategoryStat,
    pub documents: CategoryStat,
    pub audio: CategoryStat,
    pub video: CategoryStat,
    pub other: CategoryStat,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CategoryStat {
    pub count: i64,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileSearchOutput {
    pub files: Vec<FileSummary>,
    pub query: String,
    pub total: i64,
}
