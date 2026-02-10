use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Json};
use serde_json::json;
use systemprompt_models::modules::ApiPaths;

use super::static_files::StaticContentState;

pub async fn smart_fallback_handler(
    State(state): State<StaticContentState>,
    uri: Uri,
    method: Method,
    headers: HeaderMap,
    req_ctx: Option<axum::Extension<systemprompt_models::RequestContext>>,
) -> impl IntoResponse {
    let path = uri.path();

    if is_api_path(path) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Not Found",
                "message": format!("No route matches {method} {path}"),
                "path": path,
                "suggestions": get_api_suggestions(path)
            })),
        )
            .into_response();
    }

    super::serve_static_content(State(state), uri, headers, req_ctx)
        .await
        .into_response()
}

fn is_api_path(path: &str) -> bool {
    path.starts_with(ApiPaths::API_BASE)
        || path.starts_with(ApiPaths::WELLKNOWN_BASE)
        || path.starts_with("/server/")
        || path.starts_with("/mcp/")
        || path.starts_with("/health")
        || path.starts_with(ApiPaths::OPENAPI_BASE)
        || path.starts_with(ApiPaths::DOCS_BASE)
        || path.starts_with(ApiPaths::SWAGGER_BASE)
        || path.starts_with("/v1/")
        || path.starts_with("/auth/")
        || path.starts_with("/oauth/")
}

fn get_api_suggestions(path: &str) -> Vec<String> {
    if path.starts_with(ApiPaths::API_BASE) {
        vec![
            format!("{} - API discovery endpoint", ApiPaths::DISCOVERY),
            format!("{} - Health check", ApiPaths::HEALTH),
            format!(
                "{} - Core services (contexts, tasks, artifacts)",
                ApiPaths::CORE_BASE
            ),
            format!("{} - Agent registry", ApiPaths::AGENTS_REGISTRY),
            format!("{} - MCP server registry", ApiPaths::MCP_REGISTRY),
        ]
    } else if path.starts_with(ApiPaths::WELLKNOWN_BASE) {
        vec![
            format!("{} - OAuth metadata", ApiPaths::WELLKNOWN_OAUTH_SERVER),
            format!("{} - Agent card", ApiPaths::WELLKNOWN_AGENT_CARD),
        ]
    } else if path.contains("health") {
        vec![format!("{} - Health check endpoint", ApiPaths::HEALTH)]
    } else if path.contains("openapi") || path.contains("swagger") {
        vec![format!(
            "{} - API discovery (OpenAPI not yet available)",
            ApiPaths::DISCOVERY
        )]
    } else {
        vec![
            format!("{} - Start here for API discovery", ApiPaths::DISCOVERY),
            "/ - Frontend application".to_string(),
        ]
    }
}
