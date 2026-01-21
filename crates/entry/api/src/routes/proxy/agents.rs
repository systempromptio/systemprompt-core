use crate::services::proxy::ProxyEngine;
use axum::extract::Path;
use axum::routing::any;
use axum::Router;
use systemprompt_runtime::{AppContext, ServiceCategory};

pub fn router(ctx: &AppContext) -> Router {
    let engine = ProxyEngine::new();
    let engine_with_path = engine.clone();

    Router::new()
        .route(
            "/{service_name}",
            any(move |Path(service_name): Path<String>, state, request| {
                let engine = engine.clone();
                async move {
                    engine
                        .handle_agent_request(Path((service_name,)), state, request)
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
                        engine
                            .handle_agent_request_with_path(
                                Path((service_name, path)),
                                state,
                                request,
                            )
                            .await
                    }
                },
            ),
        )
        .with_state(ctx.clone())
}

systemprompt_runtime::register_module_api!(
    "agents",
    ServiceCategory::Agent,
    router,
    false,
    systemprompt_runtime::ModuleType::Proxy
);
