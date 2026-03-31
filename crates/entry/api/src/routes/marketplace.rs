use axum::Router;
use axum::extract::Path as AxumPath;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use std::path::PathBuf;
use systemprompt_models::AppPaths;
use systemprompt_models::api::ApiError;
use systemprompt_runtime::AppContext;

pub fn router() -> Router<AppContext> {
    Router::new()
        .route("/marketplace.json", get(serve_marketplace))
        .route("/plugins/{plugin_id}/{*path}", get(serve_plugin_file))
}

fn resolve_plugins_path() -> Result<PathBuf, ApiError> {
    let paths = AppPaths::get().map_err(|e| ApiError::internal_error(e.to_string()))?;
    Ok(paths.system().services().join("plugins"))
}

fn resolve_system_path() -> Result<PathBuf, ApiError> {
    let paths = AppPaths::get().map_err(|e| ApiError::internal_error(e.to_string()))?;
    Ok(paths.system().root().to_path_buf())
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

async fn serve_marketplace() -> Result<impl IntoResponse, ApiError> {
    let system_path = resolve_system_path()?;
    let marketplace_path = system_path.join(".claude-plugin").join("marketplace.json");

    let content = tokio::fs::read(&marketplace_path).await.map_err(|_| {
        ApiError::not_found(
            "Marketplace manifest not found. Run 'systemprompt core plugins generate' first.",
        )
    })?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        content,
    ))
}

const BLOCKED_FILENAMES: &[&str] = &["config.yaml", "config.yml"];

async fn serve_plugin_file(
    AxumPath((plugin_id, file_path)): AxumPath<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let plugins_path = resolve_plugins_path()?;
    let plugin_dir = plugins_path.join(&plugin_id);

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
        .map_err(|_| ApiError::not_found(format!("File not found: {}", file_path)))?;

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
