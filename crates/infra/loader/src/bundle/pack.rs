//! Packing a services tree into a bundle archive.
//!
//! The file list is walked in sorted order and paths are stored with `/`
//! separators, so packing the same tree twice on the same platform yields the
//! same [`ServicesBundleManifest::content_hash`] and the same archive layout.
//! Ownership is derived from the tree rather than declared, so a manifest
//! cannot claim ids the bundle does not carry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::{fs, io};

use chrono::Utc;
use flate2::Compression;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use systemprompt_models::services::bundle::{
    BUNDLE_ALLOWED_DIRS, BUNDLE_FORMAT_VERSION, BUNDLE_MANIFEST_FILE, BundleOwnership,
    BundleSourceInfo, FileEntry, ServicesBundleManifest, SignedBundleManifest,
};
use tar::Builder;

use super::error::{BundleError, BundleResult};
use super::extract::BUNDLE_TREE_PREFIX;

const OWNED_ID_DIRS: &[&str] = &[
    "marketplaces",
    "plugins",
    "skills",
    "rules",
    "hooks",
    "artifacts",
];

pub fn collect_files(root: &Path, directories: &[&str]) -> io::Result<Vec<FileEntry>> {
    let mut files = Vec::new();
    for dir in directories {
        let dir_path = root.join(dir);
        if dir_path.is_dir() {
            collect_dir(&dir_path, root, &mut files)?;
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn collect_dir(dir: &Path, base: &Path, files: &mut Vec<FileEntry>) -> io::Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .map(|e| e.path())
        .collect();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            collect_dir(&path, base, files)?;
        } else if path.is_file() {
            let relative = path.strip_prefix(base).map_err(io::Error::other)?;
            let content = fs::read(&path)?;
            files.push(FileEntry {
                path: relative_slug(relative),
                sha256: hex::encode(Sha256::digest(&content)),
                size: content.len() as u64,
            });
        }
    }
    Ok(())
}

fn relative_slug(path: &Path) -> String {
    path.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[must_use]
pub fn manifest_checksum(files: &[FileEntry]) -> (String, u64) {
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    for file in files {
        hasher.update(&file.sha256);
        total += file.size;
    }
    (hex::encode(hasher.finalize()), total)
}

pub fn derive_ownership(root: &Path) -> io::Result<BundleOwnership> {
    let mut owns = BundleOwnership::default();
    for dir in BUNDLE_ALLOWED_DIRS {
        if root.join(dir).is_dir() {
            owns.dirs.push((*dir).to_owned());
        }
    }
    for dir in OWNED_ID_DIRS {
        let ids = child_dir_names(&root.join(dir))?;
        match *dir {
            "marketplaces" => owns.marketplaces = ids,
            "plugins" => owns.plugins = ids,
            "skills" => owns.skills = ids,
            "rules" => owns.rules = ids,
            "hooks" => owns.hooks = ids,
            _ => owns.artifacts = ids,
        }
    }
    Ok(owns)
}

fn child_dir_names(dir: &Path) -> io::Result<Vec<String>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = fs::read_dir(dir)?
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Ok(names)
}

pub fn build_manifest(
    root: &Path,
    version: &str,
    requires_core: &str,
    source: BundleSourceInfo,
) -> BundleResult<ServicesBundleManifest> {
    let files = collect_files(root, BUNDLE_ALLOWED_DIRS)?;
    let (_checksum, total_size) = manifest_checksum(&files);
    let owns = derive_ownership(root)?;
    Ok(ServicesBundleManifest {
        format: BUNDLE_FORMAT_VERSION,
        version: version.to_owned(),
        created_at: Utc::now(),
        requires_core: requires_core.to_owned(),
        source,
        content_hash: ServicesBundleManifest::compute_content_hash(&files),
        files,
        total_size,
        owns,
    })
}

pub fn write_tarball(root: &Path, signed: &SignedBundleManifest, out: &Path) -> BundleResult<()> {
    let manifest_json = serde_json::to_vec_pretty(signed)
        .map_err(|e| BundleError::policy(format!("manifest is not serialisable: {e}")))?;

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(out)?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    {
        let mut tar = Builder::new(&mut encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest_json.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, BUNDLE_MANIFEST_FILE, manifest_json.as_slice())
            .map_err(|e| BundleError::extract(out, e))?;

        for entry in &signed.manifest.files {
            let full = root.join(&entry.path);
            let name = format!("{BUNDLE_TREE_PREFIX}/{}", entry.path);
            tar.append_path_with_name(&full, &name)
                .map_err(|e| BundleError::extract(&full, e))?;
        }
        tar.finish().map_err(|e| BundleError::extract(out, e))?;
    }
    encoder.finish()?;
    Ok(())
}

pub fn create_tarball_bytes(root: &Path, files: &[FileEntry]) -> BundleResult<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut tar = Builder::new(&mut encoder);
        for entry in files {
            let full = root.join(&entry.path);
            tar.append_path_with_name(&full, &entry.path)
                .map_err(|e| BundleError::extract(&full, e))?;
        }
        tar.finish().map_err(|e| BundleError::extract(root, e))?;
    }
    Ok(encoder.finish()?)
}
