use axum::routing::get;
use axum::Router;

pub mod routes;

pub fn registry_router(ctx: &systemprompt_runtime::AppContext) -> Router {
    routes::registry::router(ctx)
}

pub fn contexts_router() -> Router<systemprompt_runtime::AppContext> {
    routes::contexts::router()
}

pub fn webhook_router() -> Router<systemprompt_runtime::AppContext> {
    routes::contexts::webhook_router()
}

pub fn tasks_router() -> Router<systemprompt_runtime::AppContext> {
    Router::new()
        .route("/", get(routes::tasks::list_tasks_by_user))
        .route(
            "/{task_id}",
            get(routes::tasks::get_task).delete(routes::tasks::delete_task),
        )
        .route(
            "/{task_id}/messages",
            get(routes::tasks::get_messages_by_task),
        )
        .route(
            "/{task_id}/artifacts",
            get(routes::artifacts::list_artifacts_by_task),
        )
}

pub fn artifacts_router() -> Router<systemprompt_runtime::AppContext> {
    Router::new()
        .route("/", get(routes::artifacts::list_artifacts_by_user))
        .route("/{artifact_id}", get(routes::artifacts::get_artifact))
}
