//! `/files/*`: streamed from the first layer that holds the file.
//!
//! The layers are `paths.storage/files` (user data and operator overrides),
//! the optional read-only `paths.shipped_files`, then the configured
//! [`FileStorage`] backend, so files written to an object store by any
//! replica are served by every replica. Bodies are streamed, a single
//! `Range` is answered with `206` (or `416`) unless a stale `If-Range` asks
//! for the whole file, and `HEAD` reads the metadata alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use systemprompt_files::FilesConfig;
use systemprompt_manifest::paths::constants::storage;
use systemprompt_models::api::ApiError;
use systemprompt_runtime::AppContext;
use systemprompt_storage::LocalFileStorage;
use systemprompt_traits::{
    ByteRange, FileStorage, FileStorageError, FileStorageResult, StoredFileId, StoredFileRead,
};

use super::cache::{asset_cache_policy, etag_matches, if_range_holds, not_modified_response};
use crate::error::ApiHttpError;

pub(super) struct FilesRequest<'a> {
    pub relative_path: &'a str,
    pub method: &'a Method,
    pub headers: &'a HeaderMap,
}

pub(super) async fn serve_files_path(
    ctx: &AppContext,
    files_config: &FilesConfig,
    req: FilesRequest<'_>,
) -> Response {
    if req.relative_path.chars().any(char::is_control)
        || req
            .relative_path
            .split(['/', '\\'])
            .any(|segment| segment == "..")
    {
        return ApiHttpError::not_found("Asset not found").into_response();
    }
    let range = (req.method == Method::GET)
        .then(|| req.headers.get(header::RANGE))
        .flatten()
        .and_then(|v| v.to_str().ok())
        .and_then(ByteRange::parse);

    let writable = LocalFileStorage::new(files_config.files());
    let shipped = ctx
        .app_paths()
        .storage()
        .shipped_files()
        .map(|root| LocalFileStorage::new(root.to_path_buf()));
    let local_id = StoredFileId::new(req.relative_path);
    let backend_id = StoredFileId::new(format!("{}/{}", storage::FILES, req.relative_path));
    let mut layers: Vec<(&dyn FileStorage, &StoredFileId)> = vec![(&writable, &local_id)];
    if let Some(shipped) = shipped.as_ref() {
        layers.push((shipped, &local_id));
    }
    layers.push((ctx.file_storage().as_ref(), &backend_id));

    for (layer, id) in layers {
        let opened = open_layer(layer, id, range, &req).await;
        match opened {
            Ok(read) => return respond(read, files_config, &req),
            Err(FileStorageError::NotFound(_) | FileStorageError::Validation(_)) => {},
            Err(FileStorageError::RangeNotSatisfiable { size }) => {
                return range_not_satisfiable(size);
            },
            Err(e) => {
                return ApiHttpError::from(ApiError::internal("Error reading stored file", e))
                    .into_response();
            },
        }
    }
    ApiHttpError::not_found("Asset not found").into_response()
}

async fn open_layer(
    layer: &dyn FileStorage,
    id: &StoredFileId,
    range: Option<ByteRange>,
    req: &FilesRequest<'_>,
) -> FileStorageResult<StoredFileRead> {
    if req.method == Method::HEAD {
        let metadata = layer.metadata(id).await?;
        return Ok(StoredFileRead {
            metadata,
            range: None,
            body: Box::pin(futures_util::stream::empty()),
        });
    }
    match layer.open(id, range).await {
        Ok(read)
            if read.range.is_some()
                && !if_range_holds(req.headers, read.metadata.etag.as_deref()) =>
        {
            layer.open(id, None).await
        },
        Err(FileStorageError::RangeNotSatisfiable { .. })
            if req.headers.contains_key(header::IF_RANGE) =>
        {
            layer.open(id, None).await
        },
        other => other,
    }
}

fn range_not_satisfiable(size: u64) -> Response {
    (
        StatusCode::RANGE_NOT_SATISFIABLE,
        [
            (header::CONTENT_RANGE, format!("bytes */{size}")),
            (header::ACCEPT_RANGES, "bytes".to_owned()),
        ],
    )
        .into_response()
}

fn respond(read: StoredFileRead, files_config: &FilesConfig, req: &FilesRequest<'_>) -> Response {
    let path = Path::new(req.relative_path);
    let cache_control = files_config
        .cache_control()
        .unwrap_or_else(|| asset_cache_policy(path));
    if let Some(etag) = read.metadata.etag.as_deref()
        && etag_matches(req.headers, etag)
    {
        return not_modified_response(etag, cache_control);
    }
    let status = if read.range.is_some() {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let length = read.range.map_or_else(
        || {
            read.metadata
                .size_bytes
                .and_then(|size| u64::try_from(size).ok())
        },
        |range| Some(range.length()),
    );
    let body = if req.method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(read.body)
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    let mut set = |name, value: String| {
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.insert(name, value);
        }
    };
    set(
        header::CONTENT_TYPE,
        systemprompt_models::mime::http_content_type(path).to_owned(),
    );
    set(header::CACHE_CONTROL, cache_control.to_owned());
    set(header::ACCEPT_RANGES, "bytes".to_owned());
    set(
        header::LAST_MODIFIED,
        read.metadata
            .updated_at
            .format("%a, %d %b %Y %H:%M:%S GMT")
            .to_string(),
    );
    if let Some(length) = length {
        set(header::CONTENT_LENGTH, length.to_string());
    }
    if let Some(etag) = read.metadata.etag {
        set(header::ETAG, etag);
    }
    if let Some(range) = read.range {
        set(header::CONTENT_RANGE, range.content_range());
    }
    response
}
