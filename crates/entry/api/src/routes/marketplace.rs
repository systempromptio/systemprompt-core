use axum::Router;
use axum::extract::{Path as AxumPath, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use std::path::PathBuf;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::api::ApiError;
use systemprompt_models::services::{MarketplaceConfig, ServicesConfig};
use systemprompt_runtime::AppContext;

const DEFAULT_MARKETPLACE_FALLBACK: &str = "default";

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
    reason = "error variants carry detailed validation context; boxing would just hide allocation without runtime benefit"
)]
fn load_services_config() -> Result<ServicesConfig, ApiError> {
    ConfigLoader::load()
        .map_err(|e| ApiError::internal_error(format!("Failed to load services config: {e}")))
}

fn resolve_default_id(services: &ServicesConfig) -> Option<String> {
    services
        .settings
        .default_marketplace_id
        .clone()
        .or_else(|| {
            if services
                .marketplaces
                .keys()
                .any(|k| k.as_str() == DEFAULT_MARKETPLACE_FALLBACK)
            {
                Some(DEFAULT_MARKETPLACE_FALLBACK.to_owned())
            } else {
                None
            }
        })
}

fn render_marketplace_json(id: &str, marketplace: &MarketplaceConfig) -> serde_json::Value {
    let plugin_entries: Vec<serde_json::Value> = marketplace
        .plugins
        .include
        .iter()
        .map(|plugin_id| {
            serde_json::json!({
                "name": plugin_id,
                "source": format!("./storage/files/plugins/{plugin_id}"),
            })
        })
        .collect();

    serde_json::json!({
        "name": id,
        "owner": { "name": marketplace.author.name.clone() },
        "metadata": {
            "description": marketplace.description.clone(),
            "version": marketplace.version.clone(),
        },
        "plugins": plugin_entries,
    })
}

async fn serve_default_marketplace_json(
    State(_ctx): State<AppContext>,
) -> Result<impl IntoResponse, ApiError> {
    let services = load_services_config()?;

    let id = resolve_default_id(&services).ok_or_else(|| {
        ApiError::not_found(
            "No default marketplace configured. Set settings.default_marketplace_id or define a \
             marketplace with id 'default'.",
        )
    })?;

    let marketplace = services
        .marketplaces
        .iter()
        .find(|(k, _)| k.as_str() == id)
        .map(|(_, v)| v)
        .ok_or_else(|| ApiError::not_found(format!("Default marketplace '{id}' is not defined")))?;

    let body = serde_json::to_vec_pretty(&render_marketplace_json(&id, marketplace))
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

    let entries: Vec<serde_json::Value> = services
        .marketplaces
        .iter()
        .map(|(id, m)| {
            serde_json::json!({
                "id": id.as_str(),
                "name": m.name,
                "description": m.description,
                "version": m.version,
                "visibility": m.visibility,
                "enabled": m.enabled,
            })
        })
        .collect();

    let body = serde_json::to_vec_pretty(&serde_json::json!({ "marketplaces": entries }))
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

    let marketplace = services
        .marketplaces
        .iter()
        .find(|(k, _)| k.as_str() == id)
        .map(|(_, v)| v)
        .ok_or_else(|| ApiError::not_found(format!("Marketplace '{id}' not found")))?;

    let body = serde_json::to_vec_pretty(&render_marketplace_json(&id, marketplace))
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
