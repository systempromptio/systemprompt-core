use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tar::{Archive, Builder};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::api_client::SyncApiClient;
use crate::error::SyncResult;
use crate::{SyncConfig, SyncDirection, SyncOperationResult};

const INCLUDE_DIRS: [&str; 8] = [
    "agents", "skills", "content", "web", "config", "profiles", "plugins", "hooks",
];

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

        let diff = Self::compare_tarball_with_local(&data, &services_path)?;

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
                Self::add_dir_to_zip(&mut zip, &dir_path, services_path, options)?;
            }
        }

        zip.finish()?;
        Ok(zip_path)
    }

    pub fn apply(data: &[u8], services_path: &Path, paths: Option<&[String]>) -> SyncResult<usize> {
        paths.map_or_else(
            || Self::extract_tarball(data, services_path),
            |paths| Self::extract_tarball_selective(data, services_path, paths),
        )
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

        for dir in INCLUDE_DIRS {
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

    fn extract_tarball_selective(
        data: &[u8],
        target: &Path,
        paths_to_sync: &[String],
    ) -> SyncResult<usize> {
        let allowed: std::collections::HashSet<&str> =
            paths_to_sync.iter().map(String::as_str).collect();

        let decoder = GzDecoder::new(data);
        let mut archive = Archive::new(decoder);
        let mut count = 0;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let entry_path = entry.path()?.to_string_lossy().to_string();

            if !allowed.contains(entry_path.as_str()) {
                continue;
            }

            let path = target.join(&entry_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            entry.unpack(&path)?;
            count += 1;
        }

        Ok(count)
    }

    fn compare_tarball_with_local(data: &[u8], services_path: &Path) -> SyncResult<SyncDiffResult> {
        let temp_dir = tempfile::tempdir()?;
        Self::extract_tarball(data, temp_dir.path())?;

        let mut remote_files: HashMap<String, (String, u64)> = HashMap::new();
        for dir in INCLUDE_DIRS {
            let dir_path = temp_dir.path().join(dir);
            if dir_path.exists() {
                let mut entries = vec![];
                Self::collect_dir(&dir_path, temp_dir.path(), &mut entries)?;
                for entry in entries {
                    remote_files.insert(entry.path, (entry.checksum, entry.size));
                }
            }
        }

        let mut local_files: HashMap<String, String> = HashMap::new();
        for dir in INCLUDE_DIRS {
            let dir_path = services_path.join(dir);
            if dir_path.exists() {
                let mut entries = vec![];
                Self::collect_dir(&dir_path, services_path, &mut entries)?;
                for entry in entries {
                    local_files.insert(entry.path, entry.checksum);
                }
            }
        }

        let mut entries = Vec::new();
        let mut added = 0;
        let mut modified = 0;
        let mut unchanged = 0;

        for (path, (remote_checksum, size)) in &remote_files {
            match local_files.get(path) {
                Some(local_checksum) if local_checksum == remote_checksum => {
                    unchanged += 1;
                    entries.push(SyncDiffEntry {
                        path: path.clone(),
                        status: FileDiffStatus::Unchanged,
                        size: *size,
                    });
                },
                Some(_) => {
                    modified += 1;
                    entries.push(SyncDiffEntry {
                        path: path.clone(),
                        status: FileDiffStatus::Modified,
                        size: *size,
                    });
                },
                None => {
                    added += 1;
                    entries.push(SyncDiffEntry {
                        path: path.clone(),
                        status: FileDiffStatus::Added,
                        size: *size,
                    });
                },
            }
        }

        let mut deleted = 0;
        for path in local_files.keys() {
            if !remote_files.contains_key(path) {
                deleted += 1;
                entries.push(SyncDiffEntry {
                    path: path.clone(),
                    status: FileDiffStatus::Deleted,
                    size: 0,
                });
            }
        }

        entries.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(SyncDiffResult {
            entries,
            added,
            modified,
            deleted,
            unchanged,
        })
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

    fn add_dir_to_zip<W: Write + std::io::Seek>(
        zip: &mut ZipWriter<W>,
        dir: &Path,
        base: &Path,
        options: SimpleFileOptions,
    ) -> SyncResult<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                Self::add_dir_to_zip(zip, &path, base, options)?;
            } else if path.is_file() {
                let relative = path.strip_prefix(base)?;
                let name = relative.to_string_lossy().to_string();
                zip.start_file(&name, options)?;
                let mut file = fs::File::open(&path)?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                zip.write_all(&buf)?;
            }
        }
        Ok(())
    }
}
