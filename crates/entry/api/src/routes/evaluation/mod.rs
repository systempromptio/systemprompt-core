//! Evaluator worker transport with server-owned identity and fenced mutations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod campaign_completion;
pub mod campaigns;
mod error;
mod handlers;
pub mod lifecycle;

pub use crate::repository::evaluation::{EvaluationWorkerState, EvaluationWorkerStateBuilder};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::post;

pub fn router(state: EvaluationWorkerState) -> Router {
    Router::new()
        .route("/claim", post(handlers::claim))
        .route("/assignment", post(handlers::assignment))
        .route("/events", post(handlers::event))
        .route("/access", post(handlers::access))
        .route("/heartbeat", post(handlers::heartbeat))
        .route("/evidence", post(handlers::evidence))
        .route("/complete", post(handlers::complete))
        .route("/approval", post(handlers::request_approval))
        .route("/measurement", post(handlers::measurement))
        .route("/cleanup", post(handlers::cleanup))
        .route("/reconcile", post(handlers::reconcile))
        .layer(DefaultBodyLimit::max(17 * 1024 * 1024))
        .with_state(state)
}

pub(crate) fn router_from_context(
    ctx: &systemprompt_runtime::AppContext,
) -> anyhow::Result<Router> {
    let state = EvaluationWorkerState::builder(ctx.evaluation_repositories().as_ref().clone())
        .environment(ctx.config().api_external_url.clone())
        .build()?;
    Ok(router(state))
}
mod inventory;
mod optimization_error;
pub mod optimization_origin;
mod optimization_resources;

pub mod consumer;

mod snapshots;

mod snapshot_stream;

mod snapshot_generation;

pub mod contract;

pub mod collections;

pub mod operations;

mod operation_handlers;

mod publications;

mod execution_pages;
