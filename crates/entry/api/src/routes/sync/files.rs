use axum::body::Body;
use axum::extract::Query;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_models::api::ApiError;
use tar::{Archive, Builder};

use super::types::{to_api_error, ApiResult, FileEntry, FileManifest, FilesQuery, UploadResult};

const ALLOWED_DIRS: &[&str] = &[
    "agents", "skills", "content", "mcp", "ai", "config", "profiles",
];

fn get_services_path() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("SYSTEMPROMPT_SERVICES_PATH") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
        return Err(format!(
            "SYSTEMPROMPT_SERVICES_PATH does not exist: {}",
            path
        ));
    }

    if let Ok(paths) = systemprompt_models::AppPaths::get() {
        let services = paths.system().services();
        if services.exists() {
            return Ok(services.to_path_buf());
        }
    }

    Err("Services path not configured".into())
}

fn collect_files(services_path: &Path, directories: &[&str]) -> Result<FileManifest, String> {
    let mut files = Vec::new();

    for dir in directories {
        if !ALLOWED_DIRS.contains(dir) {
            continue;
        }

        let dir_path = services_path.join(dir);
        if dir_path.exists() {
            collect_dir(&dir_path, services_path, &mut files)?;
        }
    }

    let mut hasher = Sha256::new();
    let mut total_size = 0u64;
    for file in &files {
        hasher.update(&file.checksum);
        total_size += file.size;
    }
    let checksum = format!("{:x}", hasher.finalize());

    Ok(FileManifest {
        files,
        timestamp: chrono::Utc::now(),
        checksum,
        total_size,
    })
}

fn collect_dir(dir: &Path, base: &Path, files: &mut Vec<FileEntry>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            collect_dir(&path, base, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(base)
                .map_err(|e| format!("Failed to get relative path: {}", e))?;

            let content = fs::read(&path).map_err(|e| format!("Failed to read file: {}", e))?;
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

fn create_tarball(base: &Path, manifest: &FileManifest) -> Result<Vec<u8>, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut tar = Builder::new(&mut encoder);
        for file in &manifest.files {
            let full_path = base.join(&file.path);
            tar.append_path_with_name(&full_path, &file.path)
                .map_err(|e| format!("Failed to add file to tarball: {}", e))?;
        }
        tar.finish()
            .map_err(|e| format!("Failed to finish tarball: {}", e))?;
    }
    encoder
        .finish()
        .map_err(|e| format!("Failed to finish gzip: {}", e))
}

fn extract_tarball(data: &[u8], target: &Path) -> Result<usize, String> {
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    let mut count = 0;

    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read tarball entries: {}", e))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;

        let entry_path = entry
            .path()
            .map_err(|e| format!("Failed to get entry path: {}", e))?;

        let entry_path_str = entry_path.to_string_lossy();
        if entry_path_str.contains("..") {
            return Err(format!("Invalid path in tarball: {}", entry_path_str));
        }

        let first_component = entry_path
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str());

        if !first_component.is_some_and(|c| ALLOWED_DIRS.contains(&c)) {
            return Err(format!("Path not in allowed directory: {}", entry_path_str));
        }

        let dest_path = target.join(&*entry_path);

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        entry
            .unpack(&dest_path)
            .map_err(|e| format!("Failed to unpack file: {}", e))?;
        count += 1;
    }

    Ok(count)
}

fn peek_manifest(data: &[u8]) -> Result<FileManifest, String> {
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    let mut files = Vec::new();
    let mut total_size = 0u64;

    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read tarball: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let size = entry.size();
        total_size += size;

        files.push(FileEntry {
            path: entry
                .path()
                .map_err(|e| format!("Invalid path: {}", e))?
                .to_string_lossy()
                .to_string(),
            checksum: String::new(),
            size,
        });
    }

    Ok(FileManifest {
        files,
        timestamp: chrono::Utc::now(),
        checksum: String::new(),
        total_size,
    })
}

pub async fn manifest(Query(query): Query<FilesQuery>) -> ApiResult<Json<FileManifest>> {
    let services_path = get_services_path().map_err(to_api_error)?;
    let directories = query.directories();

    let manifest = collect_files(&services_path, &directories).map_err(to_api_error)?;

    Ok(Json(manifest))
}

pub async fn download(Query(query): Query<FilesQuery>) -> Result<Response, ApiError> {
    let services_path = get_services_path().map_err(to_api_error)?;
    let directories = query.directories();

    let manifest = collect_files(&services_path, &directories).map_err(to_api_error)?;

    if query.dry_run {
        return Ok(Json(manifest).into_response());
    }

    let tarball = create_tarball(&services_path, &manifest).map_err(to_api_error)?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/gzip")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"files.tar.gz\"",
        )
        .header(header::CONTENT_LENGTH, tarball.len())
        .body(Body::from(tarball))
        .map_err(|e| ApiError::internal_error(e.to_string()))
}

pub async fn upload(
    Query(query): Query<FilesQuery>,
    body: axum::body::Bytes,
) -> ApiResult<Json<UploadResult>> {
    let services_path = get_services_path().map_err(to_api_error)?;

    if query.dry_run {
        let manifest = peek_manifest(&body).map_err(to_api_error)?;
        return Ok(Json(UploadResult {
            files_uploaded: manifest.files.len(),
            uploaded_at: chrono::Utc::now(),
            manifest: Some(manifest),
        }));
    }

    let count = extract_tarball(&body, &services_path).map_err(to_api_error)?;

    Ok(Json(UploadResult {
        files_uploaded: count,
        uploaded_at: chrono::Utc::now(),
        manifest: None,
    }))
}
