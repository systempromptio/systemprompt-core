use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use tar::{Archive, Builder};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::error::{SyncError, SyncResult};
use crate::files::{
    FileBundle, FileDiffStatus, FileEntry, FileManifest, SyncDiffEntry, SyncDiffResult,
};

pub const INCLUDE_DIRS: [&str; 8] = [
    "agents", "skills", "content", "web", "config", "profiles", "plugins", "hooks",
];

pub fn collect_files(services_path: &Path) -> SyncResult<FileBundle> {
    let mut files = vec![];

    for dir in INCLUDE_DIRS {
        let dir_path = services_path.join(dir);
        if dir_path.exists() {
            collect_dir(&dir_path, services_path, &mut files)?;
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

pub fn collect_dir(dir: &Path, base: &Path, files: &mut Vec<FileEntry>) -> SyncResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_dir(&path, base, files)?;
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

pub fn create_tarball(base: &Path, manifest: &FileManifest) -> SyncResult<Vec<u8>> {
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

pub fn extract_tarball(data: &[u8], target: &Path) -> SyncResult<usize> {
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

pub fn extract_tarball_selective(
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

pub fn compare_tarball_with_local(data: &[u8], services_path: &Path) -> SyncResult<SyncDiffResult> {
    let temp_dir = tempfile::tempdir()?;
    extract_tarball(data, temp_dir.path())?;

    let mut remote_files: HashMap<String, (String, u64)> = HashMap::new();
    for dir in INCLUDE_DIRS {
        let dir_path = temp_dir.path().join(dir);
        if dir_path.exists() {
            let mut entries = vec![];
            collect_dir(&dir_path, temp_dir.path(), &mut entries)?;
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
            collect_dir(&dir_path, services_path, &mut entries)?;
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

pub fn peek_manifest(data: &[u8]) -> SyncResult<FileManifest> {
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

pub fn add_dir_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    dir: &Path,
    base: &Path,
    options: SimpleFileOptions,
) -> SyncResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            add_dir_to_zip(zip, &path, base, options)?;
        } else if path.is_file() {
            let relative = path.strip_prefix(base)?;
            let name = relative.to_string_lossy().to_string();
            zip.start_file(&name, options)?;
            let mut file = fs::File::open(&path).map_err(|source| SyncError::FileOpenFailed {
                path: path.display().to_string(),
                source,
            })?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            zip.write_all(&buf)?;
        }
    }
    Ok(())
}
