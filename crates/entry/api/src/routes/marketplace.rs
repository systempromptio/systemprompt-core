use axum::Router;
use axum::extract::{Path as AxumPath, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use std::path::PathBuf;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_loader::ConfigLoader;
use systemprompt_marketplace::{MarketplaceError, MarketplaceService, render_marketplace_json, render_marketplace_list};
use systemprompt_models::api::ApiError;
use systemprompt_models::services::ServicesConfig;
use systemprompt_runtime::AppContext;

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

// Why: orphan rule forbids `impl From<MarketplaceError> for ApiError` here —
// both types are foreign to this crate — so the variant mapping is a free fn.
#[expect(
    clippy::result_large_err,
    reason = "ApiError carries response context that is intentionally large; boxing here would \
              propagate to every caller for negligible gain"
)]
fn map_marketplace_error(error: MarketplaceError) -> ApiError {
    match error {
        MarketplaceError::NotFound(_) | MarketplaceError::NoDefault => {
            ApiError::not_found(error.to_string())
        },
        MarketplaceError::Validation(_) => ApiError::bad_request(error.to_string()),
        MarketplaceError::Catalog(_)
        | MarketplaceError::Signing(_)
        | MarketplaceError::Filter(_) => ApiError::internal_error(error.to_string()),
    }
}

fn plugins_path(ctx: &AppContext) -> PathBuf {
    ctx.app_paths().system().services().join("plugins")
}

fn marketplaces_path(ctx: &AppContext) -> PathBuf {
    ctx.app_paths().system().services().join("marketplaces")
}

fn resolve_mime_type(path: &std::path::Path) -> &'static str {
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
fn load_services_config() -> Result<ServicesConfig, ApiError> {
    ConfigLoader::load()
        .map_err(|e| ApiError::internal_error(format!("Failed to load services config: {e}")))
}

async fn serve_default_marketplace_json(
    State(_ctx): State<AppContext>,
) -> Result<impl IntoResponse, ApiError> {
    let services = load_services_config()?;
    let service = MarketplaceService::new(&services);

    let (id, marketplace) = service.resolve_default().map_err(map_marketplace_error)?;

    let body = serde_json::to_vec_pretty(&render_marketplace_json(id.as_str(), marketplace))
        .map_err(|e| ApiError::internal_error(e.to_string()))?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        body,
    ))
}

async fn list_marketplaces(State(_ctx): State<AppContext>) -> Result<impl IntoResponse, ApiError> {
    let services = load_services_config()?;
    let service = MarketplaceService::new(&services);

    let body = serde_json::to_vec_pretty(&render_marketplace_list(service.list()))
        .map_err(|e| ApiError::internal_error(e.to_string()))?;

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
) -> Result<impl IntoResponse, ApiError> {
    let services = load_services_config()?;
    let service = MarketplaceService::new(&services);

    let id = MarketplaceId::new(id);
    let marketplace = service.get(&id).map_err(map_marketplace_error)?;

    let body = serde_json::to_vec_pretty(&render_marketplace_json(id.as_str(), marketplace))
        .map_err(|e| ApiError::internal_error(e.to_string()))?;

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
) -> Result<impl IntoResponse, ApiError> {
    if id.contains('/') || id.contains("..") {
        return Err(ApiError::forbidden("Invalid marketplace id"));
    }

    let path = marketplaces_path(&ctx).join(&id).join("config.yaml");
    if !path.is_file() {
        return Err(ApiError::not_found(format!(
            "Marketplace '{id}' has no config.yaml"
        )));
    }

    let content = tokio::fs::read(&path)
        .await
        .map_err(|e| ApiError::internal_error(e.to_string()))?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/yaml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        content,
    ))
}

const BLOCKED_FILENAMES: &[&str] = &["config.yaml", "config.yml"];

async fn serve_plugin_file(
    State(ctx): State<AppContext>,
    AxumPath((plugin_id, file_path)): AxumPath<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let plugin_dir = plugins_path(&ctx).join(&plugin_id);

    if !plugin_dir.exists() {
        return Err(ApiError::not_found(format!(
            "Plugin '{}' not found",
            plugin_id
        )));
    }

    let requested = plugin_dir.join(&file_path);

    let canonical_plugin_dir = plugin_dir
        .canonicalize()
        .map_err(|e| ApiError::internal_error(e.to_string()))?;

    let canonical_requested = requested
        .canonicalize()
        .map_err(|_e| ApiError::not_found(format!("File not found: {}", file_path)))?;

    if !canonical_requested.starts_with(&canonical_plugin_dir) {
        return Err(ApiError::forbidden("Path traversal not allowed"));
    }

    if let Some(filename) = canonical_requested.file_name().and_then(|f| f.to_str()) {
        if BLOCKED_FILENAMES.contains(&filename) {
            return Err(ApiError::forbidden(
                "Access to configuration files is not allowed",
            ));
        }
    }

    if !canonical_requested.is_file() {
        return Err(ApiError::not_found(format!(
            "File not found: {}",
            file_path
        )));
    }

    let content = tokio::fs::read(&canonical_requested)
        .await
        .map_err(|e| ApiError::internal_error(e.to_string()))?;

    let mime_type = resolve_mime_type(&canonical_requested);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime_type),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        content,
    ))
}
