use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::api_client::SyncApiClient;
use crate::error::SyncResult;
use crate::file_bundler::{
    INCLUDE_DIRS, add_dir_to_zip, collect_files, compare_tarball_with_local, create_tarball,
    extract_tarball, extract_tarball_selective, peek_manifest,
};
use crate::{SyncConfig, SyncDirection, SyncOperationResult};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileBundle {
    pub manifest: FileManifest,
    #[serde(skip)]
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileManifest {
    pub files: Vec<FileEntry>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub checksum: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub checksum: String,
    pub size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileDiffStatus {
    Added,
    Modified,
    Deleted,
    Unchanged,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncDiffEntry {
    pub path: String,
    pub status: FileDiffStatus,
    pub size: u64,
}

#[derive(Debug)]
pub struct SyncDiffResult {
    pub entries: Vec<SyncDiffEntry>,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub unchanged: usize,
}

impl SyncDiffResult {
    pub const fn has_changes(&self) -> bool {
        self.added > 0 || self.modified > 0 || self.deleted > 0
    }

    pub fn changed_paths(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.status != FileDiffStatus::Unchanged)
            .map(|e| e.path.clone())
            .collect()
    }
}

#[derive(Debug)]
pub struct PullDownload {
    pub data: Vec<u8>,
    pub diff: SyncDiffResult,
}

#[derive(Debug)]
pub struct FileSyncService {
    config: SyncConfig,
    api_client: SyncApiClient,
}

impl FileSyncService {
    pub const fn new(config: SyncConfig, api_client: SyncApiClient) -> Self {
        Self { config, api_client }
    }

    pub async fn sync(&self) -> SyncResult<SyncOperationResult> {
        match self.config.direction {
            SyncDirection::Push => self.push().await,
            SyncDirection::Pull => self.pull().await,
        }
    }

    pub async fn download_and_diff(&self) -> SyncResult<PullDownload> {
        let services_path = PathBuf::from(&self.config.services_path);
        let data = self
            .api_client
            .download_files(&self.config.tenant_id)
            .await?;

        let diff = compare_tarball_with_local(&data, &services_path)?;

        Ok(PullDownload { data, diff })
    }

    pub fn backup_services(services_path: &Path) -> SyncResult<PathBuf> {
        let project_root = services_path.parent().unwrap_or(services_path);
        let backup_dir = project_root.join("backup");
        fs::create_dir_all(&backup_dir)?;

        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let zip_path = backup_dir.join(format!("{timestamp}.zip"));

        let file = fs::File::create(&zip_path)?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        for dir in INCLUDE_DIRS {
            let dir_path = services_path.join(dir);
            if dir_path.exists() {
                add_dir_to_zip(&mut zip, &dir_path, services_path, options)?;
            }
        }

        zip.finish()?;
        Ok(zip_path)
    }

    pub fn apply(data: &[u8], services_path: &Path, paths: Option<&[String]>) -> SyncResult<usize> {
        paths.map_or_else(
            || extract_tarball(data, services_path),
            |paths| extract_tarball_selective(data, services_path, paths),
        )
    }

    async fn push(&self) -> SyncResult<SyncOperationResult> {
        let services_path = PathBuf::from(&self.config.services_path);
        let bundle = collect_files(&services_path)?;
        let file_count = bundle.manifest.files.len();

        if self.config.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "files_push",
                file_count,
                serde_json::to_value(&bundle.manifest)?,
            ));
        }

        let data = create_tarball(&services_path, &bundle.manifest)?;

        let upload = self
            .api_client
            .upload_files(&self.config.tenant_id, data)
            .await?;

        Ok(SyncOperationResult::success(
            "files_push",
            upload.files_uploaded,
        ))
    }

    async fn pull(&self) -> SyncResult<SyncOperationResult> {
        let services_path = PathBuf::from(&self.config.services_path);
        let data = self
            .api_client
            .download_files(&self.config.tenant_id)
            .await?;

        if self.config.dry_run {
            let manifest = peek_manifest(&data)?;
            return Ok(SyncOperationResult::dry_run(
                "files_pull",
                manifest.files.len(),
                serde_json::to_value(&manifest)?,
            ));
        }

        let count = extract_tarball(&data, &services_path)?;
        Ok(SyncOperationResult::success("files_pull", count))
    }

}
