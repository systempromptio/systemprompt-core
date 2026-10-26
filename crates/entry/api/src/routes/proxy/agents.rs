//! Authenticated proxy routes forwarding to agent backends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::services::proxy::ProxyService;
use axum::Router;
use axum::extract::Path;
use axum::routing::any;
use systemprompt_runtime::AppContext;

pub fn router(ctx: &AppContext) -> Router {
    let identities = crate::repository::proxy_identities(ctx.db_pool());
    let engine = ProxyService::new(
        identities,
        Arc::clone(ctx.connector_tokens()),
        &ctx.services_state().view(),
    );
    let engine_with_path = engine.clone();

    Router::new()
        .route(
            "/{service_name}",
            any(move |Path(service_name): Path<String>, state, request| {
                let engine = engine.clone();
                async move {
                    Box::pin(engine.handle_agent_request(Path((service_name,)), state, request))
                        .await
                }
            }),
        )
        .route(
            "/{service_name}/{*path}",
            any(
                move |Path((service_name, path)): Path<(String, String)>, state, request| {
                    let engine = engine_with_path.clone();
                    async move {
                        Box::pin(engine.handle_agent_request_with_path(
                            Path((service_name, path)),
                            state,
                            request,
                        ))
                        .await
                    }
                },
            ),
        )
        .with_state(ctx.clone())
}
