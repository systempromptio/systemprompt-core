//! Request/response types for the file-download endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_loader::BundleError;
use systemprompt_models::api::ApiError;

pub(super) type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, thiserror::Error)]
pub(super) enum SyncError {
    #[error("services path not configured")]
    ServicesPathMissing,
    #[error("failed to walk the services tree")]
    Walk(#[source] std::io::Error),
    #[error("failed to pack the services tarball")]
    Pack(#[source] BundleError),
    #[error("blocking file-sync task failed")]
    Join(#[source] tokio::task::JoinError),
    #[error("failed to build the download response")]
    Response(#[source] http::Error),
}

impl From<SyncError> for ApiError {
    fn from(error: SyncError) -> Self {
        Self::internal("File sync operation failed", error)
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct FilesQuery {
    pub filter: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

impl FilesQuery {
    pub(super) fn directories(&self) -> Vec<&str> {
        const ALL_DIRS: &[&str] = &[
            "agents", "skills", "rules", "content", "mcp", "ai", "config", "profiles",
        ];

        self.filter.as_ref().map_or_else(
            || ALL_DIRS.to_vec(),
            |filter| {
                filter
                    .split(',')
                    .map(str::trim)
                    .filter(|d| ALL_DIRS.contains(d))
                    .collect()
            },
        )
    }
}

pub(super) use systemprompt_manifest::services::FileEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct FileManifest {
    pub files: Vec<FileEntry>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub checksum: String,
    #[serde(default)]
    pub total_size: u64,
}
