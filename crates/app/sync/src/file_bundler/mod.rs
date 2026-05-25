//! Tarball / zip helpers used by [`crate::files`]: bundle a services
//! directory into a gzip tarball, peek manifests, extract back to disk, and
//! diff a remote tarball against the local working tree.
//!
//! Extraction and remote-vs-local diffing live in the [`extract`] submodule.

mod extract;

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;
use tar::{Archive, Builder};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::error::{SyncError, SyncResult};
use crate::files::{FileBundle, FileEntry, FileManifest};

pub(crate) use extract::{compare_tarball_with_local, extract_tarball, extract_tarball_selective};

pub(crate) const INCLUDE_DIRS: [&str; 8] = [
    "agents", "skills", "content", "web", "config", "profiles", "plugins", "hooks",
];

pub(crate) fn collect_files(services_path: &Path) -> SyncResult<FileBundle> {
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
    let checksum = hex::encode(hasher.finalize());

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
            collect_dir(&path, base, files)?;
        } else if path.is_file() {
            let relative = path.strip_prefix(base)?;
            let content = fs::read(&path)?;
            let checksum = hex::encode(Sha256::digest(&content));

            files.push(FileEntry {
                path: relative.to_string_lossy().to_string(),
                checksum,
                size: content.len() as u64,
            });
        }
    }
    Ok(())
}

pub(crate) fn create_tarball(base: &Path, manifest: &FileManifest) -> SyncResult<Vec<u8>> {
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

pub(crate) fn peek_manifest(data: &[u8]) -> SyncResult<FileManifest> {
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

pub(crate) fn add_dir_to_zip<W: Write + std::io::Seek>(
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
            let buf = fs::read(&path).map_err(|source| SyncError::FileOpenFailed {
                path: path.display().to_string(),
                source,
            })?;
            zip.write_all(&buf)?;
        }
    }
    Ok(())
}
