//! High-level push / pull / diff for the on-disk `services/` directory:
//! bundles eligible files into a tarball, talks to the cloud, and reports a
//! per-file diff.

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

/// In-memory file bundle: a [`FileManifest`] plus the encoded tarball bytes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileBundle {
    /// Manifest describing every file in `data`.
    pub manifest: FileManifest,
    /// Encoded tarball bytes (skipped during JSON serialisation).
    #[serde(skip)]
    pub data: Vec<u8>,
}

/// Manifest of files contained in a bundle, with a checksum and timestamp.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileManifest {
    /// Per-file entries.
    pub files: Vec<FileEntry>,
    /// Time the manifest was generated.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Cumulative SHA-256 of the manifest itself.
    pub checksum: String,
}

/// A single file inside a [`FileManifest`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative path inside the bundle.
    pub path: String,
    /// SHA-256 hex of the file contents.
    pub checksum: String,
    /// File size in bytes.
    pub size: u64,
}

/// Per-file classification produced by `compare_tarball_with_local`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileDiffStatus {
    /// File exists in the remote bundle but not locally.
    Added,
    /// File exists on both sides with differing checksums.
    Modified,
    /// File exists locally but not in the remote bundle.
    Deleted,
    /// File exists on both sides with identical checksums.
    Unchanged,
}

/// One row in a [`SyncDiffResult`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncDiffEntry {
    /// Relative path of the file.
    pub path: String,
    /// Diff classification.
    pub status: FileDiffStatus,
    /// File size in bytes.
    pub size: u64,
}

/// Aggregate diff between a remote bundle and a local services directory.
#[derive(Debug)]
pub struct SyncDiffResult {
    /// Per-file entries.
    pub entries: Vec<SyncDiffEntry>,
    /// Added file count.
    pub added: usize,
    /// Modified file count.
    pub modified: usize,
    /// Deleted file count.
    pub deleted: usize,
    /// Unchanged file count.
    pub unchanged: usize,
}

impl SyncDiffResult {
    /// Whether the diff contains any non-trivial changes.
    pub const fn has_changes(&self) -> bool {
        self.added > 0 || self.modified > 0 || self.deleted > 0
    }

    /// Collect the paths of every file whose status is not `Unchanged`.
    pub fn changed_paths(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.status != FileDiffStatus::Unchanged)
            .map(|e| e.path.clone())
            .collect()
    }
}

/// Output of [`FileSyncService::download_and_diff`]: the downloaded tarball
/// plus a precomputed per-file diff against the local services directory.
#[derive(Debug)]
pub struct PullDownload {
    /// Raw tarball bytes downloaded from the cloud.
    pub data: Vec<u8>,
    /// Per-file diff against the local services directory.
    pub diff: SyncDiffResult,
}

/// Drives push / pull / diff of an on-disk services directory.
#[derive(Debug)]
pub struct FileSyncService {
    config: SyncConfig,
    api_client: SyncApiClient,
}

impl FileSyncService {
    /// Construct a new file sync service.
    pub const fn new(config: SyncConfig, api_client: SyncApiClient) -> Self {
        Self { config, api_client }
    }

    /// Run the configured push or pull.
    pub async fn sync(&self) -> SyncResult<SyncOperationResult> {
        match self.config.direction {
            SyncDirection::Push => self.push().await,
            SyncDirection::Pull => self.pull().await,
        }
    }

    /// Download the remote tarball and return it together with a per-file
    /// diff against the local services directory.
    pub async fn download_and_diff(&self) -> SyncResult<PullDownload> {
        let services_path = PathBuf::from(&self.config.services_path);
        let data = self
            .api_client
            .download_files(&self.config.tenant_id)
            .await?;

        let diff = compare_tarball_with_local(&data, &services_path)?;

        Ok(PullDownload { data, diff })
    }

    /// Snapshot every directory in `INCLUDE_DIRS` into a timestamped
    /// `backup/<ts>.zip` next to `services_path` and return the path.
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

    /// Extract the supplied tarball into `services_path`, optionally
    /// restricted to the relative paths in `paths`.
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
