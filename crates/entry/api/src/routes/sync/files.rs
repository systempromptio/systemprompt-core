//! Cloud-sync file-transfer handlers for the `services/` tree.
//!
//! Exposes manifest, download, and upload endpoints that move the allow-listed
//! service directories as a gzipped tarball. The blocking filesystem and
//! tar/gzip work lives in [`super::archive`] and runs under `spawn_blocking`
//! so a large transfer never parks a Tokio worker.

use axum::Json;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use systemprompt_models::api::ApiError;
use systemprompt_runtime::AppContext;

use super::archive::{
    collect_files, create_tarball, extract_tarball, get_services_path, peek_manifest,
};
use super::types::{ApiResult, FileManifest, FilesQuery, UploadResult, to_api_error};

async fn run_blocking<T, F>(job: F) -> Result<T, ApiError>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(job)
        .await
        .map_err(to_api_error)?
        .map_err(to_api_error)
}

pub(super) async fn manifest(
    State(ctx): State<AppContext>,
    Query(query): Query<FilesQuery>,
) -> ApiResult<Json<FileManifest>> {
    let services_path = get_services_path(&ctx).map_err(to_api_error)?;
    let directories = owned_directories(&query);

    let manifest = run_blocking(move || {
        let refs: Vec<&str> = directories.iter().map(String::as_str).collect();
        collect_files(&services_path, &refs)
    })
    .await?;

    Ok(Json(manifest))
}

pub(super) async fn download(
    State(ctx): State<AppContext>,
    Query(query): Query<FilesQuery>,
) -> Result<Response, ApiError> {
    let services_path = get_services_path(&ctx).map_err(to_api_error)?;
    let directories = owned_directories(&query);
    let dry_run = query.dry_run;

    let (manifest, tarball) = run_blocking(move || {
        let refs: Vec<&str> = directories.iter().map(String::as_str).collect();
        let manifest = collect_files(&services_path, &refs)?;
        if dry_run {
            return Ok((manifest, None));
        }
        let tarball = create_tarball(&services_path, &manifest)?;
        Ok((manifest, Some(tarball)))
    })
    .await?;

    let Some(tarball) = tarball else {
        return Ok(Json(manifest).into_response());
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/gzip")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"files.tar.gz\"",
        )
        .header(header::CONTENT_LENGTH, tarball.len())
        .body(Body::from(tarball))
        .map_err(to_api_error)
}

pub(super) async fn upload(
    State(ctx): State<AppContext>,
    Query(query): Query<FilesQuery>,
    body: axum::body::Bytes,
) -> ApiResult<Json<UploadResult>> {
    let services_path = get_services_path(&ctx).map_err(to_api_error)?;

    if query.dry_run {
        let manifest = run_blocking(move || peek_manifest(&body)).await?;
        return Ok(Json(UploadResult {
            files_uploaded: manifest.files.len(),
            uploaded_at: chrono::Utc::now(),
            manifest: Some(manifest),
        }));
    }

    let count = run_blocking(move || extract_tarball(&body, &services_path)).await?;

    Ok(Json(UploadResult {
        files_uploaded: count,
        uploaded_at: chrono::Utc::now(),
        manifest: None,
    }))
}

fn owned_directories(query: &FilesQuery) -> Vec<String> {
    query
        .directories()
        .iter()
        .map(|d| (*d).to_owned())
        .collect()
}
