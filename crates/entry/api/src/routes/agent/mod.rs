//! Agent (A2A) HTTP surface.
//!
//! Assembles the routers for the agent registry, contexts, tasks, artifacts,
//! responses, and webhook broadcasts that make up the agent-facing API.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod artifacts;
pub mod contexts;
pub mod registry;
pub mod responses;
pub mod tasks;

use axum::Router;
use axum::routing::get;
use systemprompt_identifiers::ContextId;
use systemprompt_models::api::ApiError;
use systemprompt_runtime::AppContext;

use crate::error::ApiHttpError;

#[expect(
    clippy::result_large_err,
    reason = "ApiHttpError is the handler error type; boxing it here would propagate to every \
              caller for negligible gain"
)]
pub(crate) fn parse_context_id(raw: &str) -> Result<ContextId, ApiHttpError> {
    ContextId::try_new(raw).map_err(|e| ApiHttpError::from(ApiError::from(e)))
}

pub fn registry_router(ctx: &AppContext) -> Router {
    registry::router(ctx)
}

pub fn contexts_router() -> Router<AppContext> {
    contexts::router()
}

pub fn webhook_router() -> Router<AppContext> {
    contexts::webhook_router()
}

pub fn tasks_router() -> Router<AppContext> {
    Router::new()
        .route("/", get(tasks::list_tasks_by_user))
        .route(
            "/{task_id}",
            get(tasks::get_task).delete(tasks::delete_task),
        )
        .route("/{task_id}/messages", get(tasks::get_messages_by_task))
        .route(
            "/{task_id}/artifacts",
            get(artifacts::list_artifacts_by_task),
        )
}

pub fn artifacts_router() -> Router<AppContext> {
    Router::new()
        .route("/", get(artifacts::list_artifacts_by_user))
        .route("/{artifact_id}", get(artifacts::get_artifact))
        .route("/{artifact_id}/ui", get(artifacts::get_artifact_ui))
}
