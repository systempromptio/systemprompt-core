//! Public marketplace and plugin-file routes.
//!
//! [`router`] serves the default `marketplace.json`, lists and renders
//! individual marketplaces (JSON and raw `config.yaml`), and streams plugin
//! files. Plugin files are served from `plugin_bundles` — the same active,
//! content-gated map the signed manifest is hashed from — so the byte stream
//! and the manifest cannot drift: a plugin the manifest excludes (content-less
//! or outside the active marketplace) is absent from the map and yields a 404,
//! and an internal `config.yaml` is never part of the generated bundle to leak.

use axum::Router;
use axum::extract::{Path as AxumPath, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use std::path::{Path, PathBuf};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_loader::ConfigLoader;
use systemprompt_marketplace::{
    CatalogContent, MarketplaceService, plugin_bundles_cached, render_marketplace_json,
    render_marketplace_list,
};
use systemprompt_models::bridge::ids::PluginId;
use systemprompt_models::services::ServicesConfig;
use systemprompt_runtime::AppContext;

use crate::error::ApiHttpError;

pub fn router() -> Router<AppContext> {
    Router::new()
        .route("/marketplace.json", get(serve_default_marketplace_json))
        .route("/marketplaces", get(list_marketplaces))
        .route("/marketplaces/{id}", get(get_marketplace))
        .route(
            "/marketplaces/{id}/manifest.yaml",
            get(get_marketplace_yaml),
        )
        .route("/plugins/{plugin_id}/{*path}", get(serve_plugin_file))
}

fn marketplaces_path(ctx: &AppContext) -> PathBuf {
    ctx.app_paths().system().services().join("marketplaces")
}

fn resolve_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => "application/json",
        Some("md") => "text/markdown; charset=utf-8",
        Some("sh") => "application/x-sh",
        Some("yaml" | "yml") => "text/yaml; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[expect(
    clippy::result_large_err,
    reason = "ApiError carries response context that is intentionally large; boxing here would \
              propagate to every caller for negligible gain"
)]
fn load_services_config() -> Result<ServicesConfig, ApiHttpError> {
    ConfigLoader::load()
        .map_err(|e| ApiHttpError::internal_error(format!("Failed to load services config: {e}")))
}

async fn serve_default_marketplace_json(
    State(_ctx): State<AppContext>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let services = load_services_config()?;
    let service = MarketplaceService::new(&services);

    let (id, marketplace) = service.resolve_default()?;

    let body = serde_json::to_vec_pretty(&render_marketplace_json(id.as_str(), marketplace))
        .map_err(|e| ApiHttpError::internal_error(e.to_string()))?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        body,
    ))
}

async fn list_marketplaces(
    State(_ctx): State<AppContext>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let services = load_services_config()?;
    let service = MarketplaceService::new(&services);

    let body = serde_json::to_vec_pretty(&render_marketplace_list(service.list()))
        .map_err(|e| ApiHttpError::internal_error(e.to_string()))?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        body,
    ))
}

async fn get_marketplace(
    State(_ctx): State<AppContext>,
    AxumPath(id): AxumPath<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let services = load_services_config()?;
    let service = MarketplaceService::new(&services);

    let id = MarketplaceId::new(id);
    let marketplace = service.get(&id)?;

    let body = serde_json::to_vec_pretty(&render_marketplace_json(id.as_str(), marketplace))
        .map_err(|e| ApiHttpError::internal_error(e.to_string()))?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        body,
    ))
}

async fn get_marketplace_yaml(
    State(ctx): State<AppContext>,
    AxumPath(id): AxumPath<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let marketplaces_root = marketplaces_path(&ctx);
    let requested = marketplaces_root.join(&id).join("config.yaml");

    // A missing marketplaces root means no marketplace can resolve — that is a
    // not-found for the requested id, not a server fault.
    let canonical_root = marketplaces_root
        .canonicalize()
        .map_err(|_e| ApiHttpError::not_found(format!("Marketplace '{id}' not found")))?;
    let canonical_requested = requested
        .canonicalize()
        .map_err(|_e| ApiHttpError::not_found(format!("Marketplace '{id}' has no config.yaml")))?;

    if !canonical_requested.starts_with(&canonical_root) {
        return Err(ApiHttpError::forbidden("Invalid marketplace id"));
    }

    if !canonical_requested.is_file() {
        return Err(ApiHttpError::not_found(format!(
            "Marketplace '{id}' has no config.yaml"
        )));
    }

    let content = tokio::fs::read(&canonical_requested)
        .await
        .map_err(|e| ApiHttpError::internal_error(e.to_string()))?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/yaml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        content,
    ))
}

async fn serve_plugin_file(
    State(ctx): State<AppContext>,
    AxumPath((plugin_id, file_path)): AxumPath<(String, String)>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let services = load_services_config()?;
    let profile = ProfileBootstrap::get()
        .map_err(|e| ApiHttpError::internal_error(format!("profile not ready: {e}")))?;
    let api_external_url = &profile.server.api_external_url;
    let services_root = ctx.app_paths().system().services();

    let catalog = CatalogContent::load_cached(&services, services_root, api_external_url)
        .map_err(|e| ApiHttpError::internal_error(e.to_string()))?;
    let bundles = plugin_bundles_cached(&services, &catalog.as_content())
        .map_err(|e| ApiHttpError::internal_error(e.to_string()))?;

    let id = PluginId::try_new(&plugin_id)
        .map_err(|_e| ApiHttpError::not_found(format!("Plugin '{plugin_id}' not found")))?;
    let Some(bundle) = bundles.get(&id) else {
        return Err(ApiHttpError::not_found(format!(
            "Plugin '{plugin_id}' not found"
        )));
    };

    let Some(file) = bundle.get(&file_path) else {
        return Err(ApiHttpError::not_found(format!(
            "File not found: {file_path}"
        )));
    };

    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                resolve_mime_type(Path::new(&file_path)),
            ),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        file.bytes.clone(),
    ))
}
