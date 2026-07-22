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
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::JwtToken;
use systemprompt_marketplace::{CatalogContent, plugin_bundles_cached};
use systemprompt_models::bridge::ids::PluginId;
use systemprompt_runtime::AppContext;

use super::bridge_data;
use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

type HttpError = (StatusCode, String);

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    AxumPath((plugin_id, relative_path)): AxumPath<(String, String)>,
) -> Result<Response, HttpError> {
    authenticate(&jwt_extractor, &headers).await?;

    if !relative_path_is_safe(&relative_path) {
        tracing::warn!(
            plugin_id = %plugin_id,
            path = %relative_path,
            "bridge: rejected non-canonical plugin file path"
        );
        return Err((StatusCode::BAD_REQUEST, "Invalid path".to_owned()));
    }

    let id = PluginId::try_new(&plugin_id).map_err(|e| {
        tracing::debug!(error = %e, plugin_id = %plugin_id, "bridge: malformed plugin id");
        (StatusCode::NOT_FOUND, "Plugin not found".to_owned())
    })?;

    let bundles = build_bundles(&ctx)?;
    let bundle = bundles
        .get(&id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Plugin not found".to_owned()))?;
    let file = bundle
        .get(relative_path.as_str())
        .ok_or_else(|| (StatusCode::NOT_FOUND, "File not found".to_owned()))?;

    let mut response = Response::new(Body::from(file.bytes.clone()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(content_type(&relative_path)),
    );
    Ok(response)
}

async fn authenticate(
    jwt_extractor: &JwtContextExtractor,
    headers: &HeaderMap,
) -> Result<(), HttpError> {
    let credential = extract_credential(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_owned(),
        )
    })?;
    jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map(|_| ())
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))
}

fn build_bundles(
    ctx: &AppContext,
) -> Result<
    Arc<std::collections::BTreeMap<PluginId, systemprompt_marketplace::PluginBundle>>,
    HttpError,
> {
    let internal = |stage: &'static str, e: &dyn std::fmt::Display| -> HttpError {
        tracing::error!(error = %e, stage, "bridge: plugin bundle assembly failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Plugin bundle unavailable".to_owned(),
        )
    };

    let services = bridge_data::load_services_config().map_err(|e| internal("services", &e))?;
    let profile = ProfileBootstrap::get().map_err(|e| internal("profile", &e))?;
    let catalog = CatalogContent::load(
        &services,
        ctx.app_paths().system().services(),
        &profile.server.api_external_url,
    )
    .map_err(|e| internal("catalog", &e))?;

    plugin_bundles_cached(&services, &catalog.as_content()).map_err(|e| internal("bundle", &e))
}

#[cfg(feature = "test-api")]
pub mod test_api {
    #[must_use]
    pub fn relative_path_is_safe(relative: &str) -> bool {
        super::relative_path_is_safe(relative)
    }

    #[must_use]
    pub fn content_type(relative_path: &str) -> &'static str {
        super::content_type(relative_path)
    }
}

fn relative_path_is_safe(relative: &str) -> bool {
    !relative.is_empty()
        && Path::new(relative)
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn content_type(relative_path: &str) -> &'static str {
    let ext = Path::new(relative_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("md") => "text/markdown; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("json") => "application/json",
        Some("yaml" | "yml") => "application/yaml",
        Some("toml") => "application/toml",
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}
