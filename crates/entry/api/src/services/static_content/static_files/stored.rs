//! `/files/*` reads that no local layer holds, answered from the configured
//! [`FileStorage`] backend so files written to an object store by any
//! replica are served by every replica.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use systemprompt_files::FilesConfig;
use systemprompt_manifest::paths::constants::storage;
use systemprompt_models::api::ApiError;
use systemprompt_traits::{FileStorage, FileStorageError, StoredFileId};

use super::cache::{asset_cache_policy, compute_etag, etag_matches, not_modified_response};
use crate::error::ApiHttpError;

pub(super) async fn serve_stored_file(
    backend: &dyn FileStorage,
    relative_path: &str,
    headers: &HeaderMap,
    files_config: &FilesConfig,
) -> axum::response::Response {
    let id = StoredFileId::new(format!("{}/{relative_path}", storage::FILES));
    let content = match backend.retrieve(&id).await {
        Ok(content) => content,
        Err(FileStorageError::NotFound(_) | FileStorageError::Validation(_)) => {
            return ApiHttpError::not_found("Asset not found").into_response();
        },
        Err(e) => {
            return ApiHttpError::from(ApiError::internal("Error reading stored file", e))
                .into_response();
        },
    };
    let path = Path::new(relative_path);
    let cache_control = files_config
        .cache_control()
        .unwrap_or_else(|| asset_cache_policy(path));
    let etag = compute_etag(&content);
    if etag_matches(headers, &etag) {
        return not_modified_response(&etag, cache_control);
    }
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                systemprompt_models::mime::http_content_type(path).to_owned(),
            ),
            (header::CACHE_CONTROL, cache_control.to_owned()),
            (header::ETAG, etag),
        ],
        content,
    )
        .into_response()
}
