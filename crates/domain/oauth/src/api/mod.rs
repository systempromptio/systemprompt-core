use axum::Router;

pub mod routes;
pub mod wellknown;

pub fn router(ctx: &systemprompt_runtime::AppContext) -> Router {
    Router::new()
        .merge(routes::router())
        .with_state(ctx.clone())
}

pub fn public_router(ctx: &systemprompt_runtime::AppContext) -> Router {
    Router::new()
        .merge(routes::public_router())
        .with_state(ctx.clone())
}

pub fn authenticated_router(ctx: &systemprompt_runtime::AppContext) -> Router {
    Router::new()
        .merge(routes::authenticated_router())
        .with_state(ctx.clone())
}

systemprompt_runtime::register_module_api!(
    "oauth",
    systemprompt_runtime::ServiceCategory::Core,
    router,
    false
);
