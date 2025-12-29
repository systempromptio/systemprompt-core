use anyhow::Result;
use axum::Router;
use systemprompt_models::Config;
use tower_http::cors::CorsLayer;

pub fn create_base_router() -> Router {
    Router::new().route("/health", axum::routing::get(health_check))
}

pub fn apply_cors_layer(router: Router) -> Result<Router> {
    let config = Config::get()?;

    let mut cors_layer = CorsLayer::new()
        .allow_headers(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any);

    for origin in &config.cors_allowed_origins {
        if let Ok(header_value) = origin.parse::<http::HeaderValue>() {
            cors_layer = cors_layer.allow_origin(header_value);
        }
    }

    Ok(router.layer(cors_layer))
}

async fn health_check() -> impl axum::response::IntoResponse {
    "OK"
}

pub fn create_mcp_router(base_router: Router, mcp_router: Router) -> Router {
    base_router.nest("/mcp", mcp_router)
}

pub const fn add_middleware(router: Router) -> Router {
    router
}
