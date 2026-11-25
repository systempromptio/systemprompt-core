//! Bridge plugin-file endpoint (`GET /v1/bridge/plugins/{id}/{*path}`).
//!
//! Bytes are assembled live from the same `plugin_bundles` pipeline the gateway
//! hashes into the signed manifest, so every file the bridge fetches is
//! byte-identical to its manifest hash. Serving a pre-generated static plugin
//! tree here would drift from that hash and fail bridge-side verification.
//! `archive/<sha256>` under a plugin is the whole plugin as one tar.gz
//! ([`super::plugin_archive`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path};

use axum::body::Body;
use axum::extract::Path as AxumPath;
use axum::http::{HeaderMap, header};
use axum::response::Response;
use systemprompt_identifiers::{PluginId, UserId};
use systemprompt_extension::bridge_catalog::{CatalogFreshness, DynResolvedBridgeCatalog};

use super::error::{BridgeError, authenticate_bridge};
use super::plugin_archive::ArchiveMemo;
use super::{BridgeRouteState, plugin_archive, resolved};
use systemprompt_extension::router::ApiHttpError;

pub async fn handle(
    memo: ArchiveMemo,
    state: BridgeRouteState,
    headers: HeaderMap,
    AxumPath((plugin_id, relative_path)): AxumPath<(String, String)>,
) -> Result<Response, ApiHttpError> {
    let (_claims, user) = authenticate_bridge(&state.authenticator, &headers).await?;

    if !relative_path_is_safe(&relative_path) {
        tracing::warn!(
            plugin_id = %plugin_id,
            path = %relative_path,
            "bridge: rejected non-canonical plugin file path"
        );
        return Err(BridgeError::InvalidPath.into());
    }

    let id = PluginId::try_new(&plugin_id).map_err(|e| {
        tracing::debug!(error = %e, plugin_id = %plugin_id, "bridge: malformed plugin id");
        BridgeError::PluginNotFound
    })?;

    if let Some(sha256) = plugin_archive::archive_sha(&relative_path) {
        return plugin_archive::serve(&memo, &state, &user.id, &id, sha256).await;
    }

    let resolved = resolve_entitled(&state, &user.id, &id).await?;

    let file = resolved
        .plugin_file(&id, relative_path.as_str())
        .ok_or(BridgeError::PluginNotFound)?
        .ok_or(BridgeError::FileNotFound)?;

    let mut response = Response::new(Body::from(file.to_vec()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(content_type(&relative_path)),
    );
    Ok(response)
}

// Why: the downloads of one sync follow its manifest, so the user's latest
// resolution already holds the bytes that manifest hashed; only a cold or
// expired memo pays for a fingerprinted resolution.
pub(super) async fn resolve_entitled(
    state: &BridgeRouteState,
    user_id: &UserId,
    id: &PluginId,
) -> Result<DynResolvedBridgeCatalog, BridgeError> {
    let grants = |resolved: &DynResolvedBridgeCatalog| resolved.plugins().iter().any(|p| &p.id == id);
    if let Some(latest) = state.catalog.latest(user_id)
        && grants(&latest)
    {
        return Ok(latest);
    }
    let services = state.host.services.view().config();
    let resolved = resolved::resolve_for_user(state, &services, user_id, CatalogFreshness::Memo)
        .await
        .map_err(|e| BridgeError::internal("plugin bundle: catalogue resolution failed", e))?;
    if !grants(&resolved) {
        tracing::warn!(
            plugin_id = %id,
            user_id = %user_id,
            "bridge: refused a plugin bundle the caller was not granted"
        );
        return Err(BridgeError::PluginNotFound);
    }
    Ok(resolved)
}

pub fn relative_path_is_safe(relative: &str) -> bool {
    !relative.is_empty()
        && Path::new(relative)
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

pub fn content_type(relative_path: &str) -> &'static str {
    systemprompt_models::mime::http_content_type(Path::new(relative_path))
}
