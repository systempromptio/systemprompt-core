use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use systemprompt_models::modules::ApiPaths;

pub fn wellknown_routes() -> Router {
    Router::new()
        .route(
            ApiPaths::WELLKNOWN_OAUTH_SERVER,
            get(super::discovery::handle_well_known).options(|| async { StatusCode::OK }),
        )
        .route(
            &format!("{}/", ApiPaths::WELLKNOWN_OAUTH_SERVER),
            get(super::discovery::handle_well_known).options(|| async { StatusCode::OK }),
        )
        .route(
            ApiPaths::WELLKNOWN_OPENID_CONFIG,
            get(super::discovery::handle_well_known).options(|| async { StatusCode::OK }),
        )
        .route(
            &format!("{}/", ApiPaths::WELLKNOWN_OPENID_CONFIG),
            get(super::discovery::handle_well_known).options(|| async { StatusCode::OK }),
        )
        .route(
            ApiPaths::WELLKNOWN_OAUTH_PROTECTED,
            get(super::discovery::handle_oauth_protected_resource)
                .options(|| async { StatusCode::OK }),
        )
        .route(
            &format!("{}/", ApiPaths::WELLKNOWN_OAUTH_PROTECTED),
            get(super::discovery::handle_oauth_protected_resource)
                .options(|| async { StatusCode::OK }),
        )
        .route(
            "/.well-known/oauth-protected-resource/{*path}",
            get(super::discovery::handle_oauth_protected_resource_with_path)
                .options(|| async { StatusCode::OK }),
        )
}
