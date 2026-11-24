//! `/.well-known` OAuth metadata endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use systemprompt_models::modules::ApiPaths;

use super::discovery::DiscoveryState;

pub fn wellknown_routes(state: DiscoveryState) -> Router {
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
        .with_state(state)
}
