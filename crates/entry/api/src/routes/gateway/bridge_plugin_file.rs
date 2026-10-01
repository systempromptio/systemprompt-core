//! Bridge plugin-file endpoint (`GET /v1/bridge/plugins/{id}/{*path}`).
//!
//! Bytes are assembled live from the same `plugin_bundles` pipeline the gateway
//! hashes into the signed manifest, so every file the bridge fetches is
//! byte-identical to its manifest hash. Serving a pre-generated static plugin
//! tree here would drift from that hash and fail bridge-side verification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Component, Path};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::Path as AxumPath;
use axum::http::{HeaderMap, header};
use axum::response::Response;
use systemprompt_config::ProfileBootstrap;
use systemprompt_models::bridge::ids::PluginId;
use systemprompt_runtime::AppContext;

use super::bridge_error::{BridgeError, authenticate_bridge};
use super::{bridge_data, bridge_resolved};
use crate::error::ApiHttpError;
use crate::services::middleware::JwtContextExtractor;

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    AxumPath((plugin_id, relative_path)): AxumPath<(String, String)>,
) -> Result<Response, ApiHttpError> {
    let (_claims, user) = authenticate_bridge(&jwt_extractor, &headers).await?;

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

    let services = bridge_data::load_services_config()
        .map_err(|e| BridgeError::internal("plugin bundle: services config load failed", e))?;
    let profile = ProfileBootstrap::get()
        .map_err(|e| BridgeError::internal("plugin bundle: profile not ready", e))?;
    let resolved = bridge_resolved::resolve_for_user(
        &ctx,
        &services,
        profile,
        &user.id,
        bridge_resolved::Freshness::Memo,
    )
    .await
    .map_err(|e| BridgeError::internal("plugin bundle: catalogue resolution failed", e))?;

    if !resolved.candidate.plugins.iter().any(|p| p.id == id) {
        tracing::warn!(
            plugin_id = %plugin_id,
            user_id = %user.id,
            "bridge: refused a plugin bundle the caller was not granted"
        );
        return Err(BridgeError::PluginNotFound.into());
    }

    let bundle = resolved
        .bundles
        .get(&id)
        .ok_or(BridgeError::PluginNotFound)?;
    let file = bundle
        .get(relative_path.as_str())
        .ok_or(BridgeError::FileNotFound)?;

    let mut response = Response::new(Body::from(file.bytes.clone()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(content_type(&relative_path)),
    );
    Ok(response)
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
