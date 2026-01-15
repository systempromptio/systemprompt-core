use serde::{Deserialize, Serialize};
use systemprompt_models::api::ApiError;

pub type ApiResult<T> = Result<T, ApiError>;

pub fn to_api_error(e: impl std::fmt::Display) -> ApiError {
    ApiError::internal_error(e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct FilesQuery {
    pub filter: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

impl FilesQuery {
    pub fn directories(&self) -> Vec<&str> {
        const ALL_DIRS: &[&str] = &[
            "agents", "skills", "content", "mcp", "ai", "config", "profiles",
        ];

        match &self.filter {
            Some(filter) => filter
                .split(',')
                .map(str::trim)
                .filter(|d| ALL_DIRS.contains(d))
                .collect(),
            None => ALL_DIRS.to_vec(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub checksum: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifest {
    pub files: Vec<FileEntry>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub checksum: String,
    #[serde(default)]
    pub total_size: u64,
}

#[derive(Debug, Serialize)]
pub struct UploadResult {
    pub files_uploaded: usize,
    pub uploaded_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<FileManifest>,
}
