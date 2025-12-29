use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;

pub fn wellknown_routes(ctx: &AppContext) -> Router {
    Router::new()
        .route(
            ApiPaths::WELLKNOWN_OAUTH_SERVER,
            get(super::routes::discovery::handle_well_known).options(|| async { StatusCode::OK }),
        )
        .route(
            &format!("{}/", ApiPaths::WELLKNOWN_OAUTH_SERVER),
            get(super::routes::discovery::handle_well_known).options(|| async { StatusCode::OK }),
        )
        .route(
            ApiPaths::WELLKNOWN_OPENID_CONFIG,
            get(super::routes::discovery::handle_well_known).options(|| async { StatusCode::OK }),
        )
        .route(
            &format!("{}/", ApiPaths::WELLKNOWN_OPENID_CONFIG),
            get(super::routes::discovery::handle_well_known).options(|| async { StatusCode::OK }),
        )
        .route(
            ApiPaths::WELLKNOWN_OAUTH_PROTECTED,
            get(super::routes::discovery::handle_oauth_protected_resource)
                .options(|| async { StatusCode::OK }),
        )
        .route(
            &format!("{}/", ApiPaths::WELLKNOWN_OAUTH_PROTECTED),
            get(super::routes::discovery::handle_oauth_protected_resource)
                .options(|| async { StatusCode::OK }),
        )
        .with_state(ctx.clone())
}
