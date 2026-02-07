use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tar::{Archive, Builder};

use crate::api_client::SyncApiClient;
use crate::error::SyncResult;
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

    async fn push(&self) -> SyncResult<SyncOperationResult> {
        let services_path = PathBuf::from(&self.config.services_path);
        let bundle = Self::collect_files(&services_path)?;
        let file_count = bundle.manifest.files.len();

        if self.config.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "files_push",
                file_count,
                serde_json::to_value(&bundle.manifest)?,
            ));
        }

        let data = Self::create_tarball(&services_path, &bundle.manifest)?;

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
            let manifest = Self::peek_manifest(&data)?;
            return Ok(SyncOperationResult::dry_run(
                "files_pull",
                manifest.files.len(),
                serde_json::to_value(&manifest)?,
            ));
        }

        let count = Self::extract_tarball(&data, &services_path)?;
        Ok(SyncOperationResult::success("files_pull", count))
    }

    fn collect_files(services_path: &Path) -> SyncResult<FileBundle> {
        let mut files = vec![];
        let include_dirs = ["agents", "skills", "content", "web", "config", "profiles"];

        for dir in include_dirs {
            let dir_path = services_path.join(dir);
            if dir_path.exists() {
                Self::collect_dir(&dir_path, services_path, &mut files)?;
            }
        }

        let mut hasher = Sha256::new();
        for file_entry in &files {
            hasher.update(&file_entry.checksum);
        }
        let checksum = format!("{:x}", hasher.finalize());

        Ok(FileBundle {
            manifest: FileManifest {
                files,
                timestamp: chrono::Utc::now(),
                checksum,
            },
            data: vec![],
        })
    }

    fn collect_dir(dir: &Path, base: &Path, files: &mut Vec<FileEntry>) -> SyncResult<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                Self::collect_dir(&path, base, files)?;
            } else if path.is_file() {
                let relative = path.strip_prefix(base)?;
                let content = fs::read(&path)?;
                let checksum = format!("{:x}", Sha256::digest(&content));

                files.push(FileEntry {
                    path: relative.to_string_lossy().to_string(),
                    checksum,
                    size: content.len() as u64,
                });
            }
        }
        Ok(())
    }

    fn create_tarball(base: &Path, manifest: &FileManifest) -> SyncResult<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut tar = Builder::new(&mut encoder);
            for file in &manifest.files {
                let full_path = base.join(&file.path);
                tar.append_path_with_name(&full_path, &file.path)?;
            }
            tar.finish()?;
        }
        Ok(encoder.finish()?)
    }

    fn extract_tarball(data: &[u8], target: &Path) -> SyncResult<usize> {
        let decoder = GzDecoder::new(data);
        let mut archive = Archive::new(decoder);
        let mut count = 0;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = target.join(entry.path()?);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            entry.unpack(&path)?;
            count += 1;
        }

        Ok(count)
    }

    fn peek_manifest(data: &[u8]) -> SyncResult<FileManifest> {
        let decoder = GzDecoder::new(data);
        let mut archive = Archive::new(decoder);
        let mut files = vec![];

        for entry in archive.entries()? {
            let entry = entry?;
            files.push(FileEntry {
                path: entry.path()?.to_string_lossy().to_string(),
                checksum: String::new(),
                size: entry.size(),
            });
        }

        Ok(FileManifest {
            files,
            timestamp: chrono::Utc::now(),
            checksum: String::new(),
        })
    }
}
