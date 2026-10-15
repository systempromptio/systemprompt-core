//! Bridge plugin-archive endpoint
//! (`GET /v1/bridge/plugins/{id}/archive/{sha256}`).
//!
//! One gzipped tar per plugin replaces one request per file. The archive is
//! packed from the same in-memory bundle the manifest hashed, and its entries
//! are exactly that plugin's manifest file paths. A plugin's aggregate digest
//! names its content, so an archive is immutable for its sha: it is packed
//! once, kept in a small in-process memo, and served with an immutable
//! `Cache-Control`. A sha that is no longer the plugin's current digest
//! answers 409 so the bridge refetches the manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use axum::body::{Body, Bytes};
use axum::http::{HeaderValue, header};
use axum::response::Response;
use systemprompt_identifiers::{PluginId, UserId};
use systemprompt_marketplace::PluginBundle;
use systemprompt_runtime::AppContext;

use super::bridge_error::BridgeError;
use super::bridge_plugin_file::resolve_entitled;
use crate::error::ApiHttpError;

const ARCHIVE_PREFIX: &str = "archive/";

const MEMO_CAPACITY: usize = 64;

const MEMO_BYTES: usize = 256 * 1024 * 1024;

static PACKED: Mutex<VecDeque<(String, Bytes)>> = Mutex::new(VecDeque::new());

/// The sha of an `archive/<sha256>` plugin path, or `None` for a file path.
#[must_use]
pub fn archive_sha(relative_path: &str) -> Option<&str> {
    relative_path
        .strip_prefix(ARCHIVE_PREFIX)
        .filter(|sha| sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub(super) async fn serve(
    ctx: &AppContext,
    user_id: &UserId,
    id: &PluginId,
    sha256: &str,
) -> Result<Response, ApiHttpError> {
    let resolved = resolve_entitled(ctx, user_id, id).await?;
    let current = resolved
        .candidate
        .plugins
        .iter()
        .find(|entry| &entry.id == id)
        .ok_or(BridgeError::PluginNotFound)?;
    if !current.sha256.as_str().eq_ignore_ascii_case(sha256) {
        tracing::debug!(
            plugin_id = %id,
            requested = sha256,
            current = %current.sha256.as_str(),
            "bridge: plugin archive requested for a superseded digest"
        );
        return Err(BridgeError::StaleArchive.into());
    }
    let key = current.sha256.as_str().to_owned();

    let bytes = match memo_get(&key) {
        Some(bytes) => bytes,
        None => {
            let bundles = std::sync::Arc::clone(&resolved.bundles);
            let plugin = id.clone();
            let packed = tokio::task::spawn_blocking(move || {
                bundles.get(&plugin).map(pack).transpose()
            })
            .await
            .map_err(|e| BridgeError::internal("plugin archive: packing task failed", e))?
            .map_err(|e| BridgeError::internal("plugin archive: packing failed", e))?
            .ok_or(BridgeError::PluginNotFound)?;
            let bytes = Bytes::from(packed);
            memo_put(key.clone(), bytes.clone());
            bytes
        },
    };

    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/gzip"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    if let Ok(etag) = HeaderValue::from_str(&format!("\"{key}\"")) {
        headers.insert(header::ETAG, etag);
    }
    Ok(response)
}

/// Packs a plugin bundle into the archive the route serves: one entry per
/// bundle path, mode `0755` for executables.
pub fn pack(bundle: &PluginBundle) -> systemprompt_loader::BundleResult<Vec<u8>> {
    systemprompt_loader::bundle::pack::create_tarball_from_memory(
        bundle
            .iter()
            .map(|(path, file)| (path.as_str(), file.bytes.as_slice(), file.executable)),
    )
}

fn memo_get(sha256: &str) -> Option<Bytes> {
    PACKED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|(key, _)| key == sha256)
        .map(|(_, bytes)| bytes.clone())
}

fn memo_put(sha256: String, bytes: Bytes) {
    let mut guard = PACKED.lock().unwrap_or_else(PoisonError::into_inner);
    guard.retain(|(key, _)| key != &sha256);
    guard.push_back((sha256, bytes));
    while guard.len() > MEMO_CAPACITY
        || guard.iter().map(|(_, bytes)| bytes.len()).sum::<usize>() > MEMO_BYTES
    {
        if guard.pop_front().is_none() {
            break;
        }
    }
}
