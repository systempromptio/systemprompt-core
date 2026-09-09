//! Evaluator worker transport with server-owned identity and fenced mutations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod handlers;

pub use crate::repository::evaluation::{EvaluationWorkerState, EvaluationWorkerStateBuilder};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::post;

pub fn router(state: EvaluationWorkerState) -> Router {
    Router::new()
        .route("/claim", post(handlers::claim))
        .route("/access", post(handlers::access))
        .route("/heartbeat", post(handlers::heartbeat))
        .route("/evidence", post(handlers::evidence))
        .route("/complete", post(handlers::complete))
        .layer(DefaultBodyLimit::max(17 * 1024 * 1024))
        .with_state(state)
}

pub(crate) fn router_from_context(
    ctx: &systemprompt_runtime::AppContext,
) -> anyhow::Result<Router> {
    let state = EvaluationWorkerState::builder((*ctx.db_pool().write_pool_arc()?).clone())
        .environment(ctx.config().api_external_url.clone())
        .build()?;
    Ok(router(state))
}
