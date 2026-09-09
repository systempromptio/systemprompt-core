//! Evaluator worker transport with server-owned identity and fenced mutations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod handlers;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::post;
use sqlx::PgPool;
use systemprompt_evaluation::repository::experiments::{
    EvidenceRepository, ExperimentRepository, WorkerRepository,
};

#[derive(Clone, Debug)]
pub struct EvaluationWorkerState {
    experiments: ExperimentRepository,
    evidence: EvidenceRepository,
    workers: WorkerRepository,
    environment: String,
}

impl EvaluationWorkerState {
    pub fn builder(pool: PgPool) -> EvaluationWorkerStateBuilder {
        EvaluationWorkerStateBuilder {
            pool,
            environment: None,
        }
    }
}

#[derive(Debug)]
pub struct EvaluationWorkerStateBuilder {
    pool: PgPool,
    environment: Option<String>,
}

impl EvaluationWorkerStateBuilder {
    pub fn environment(mut self, environment: String) -> Self {
        self.environment = Some(environment);
        self
    }

    pub fn build(self) -> anyhow::Result<EvaluationWorkerState> {
        let environment = self
            .environment
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("Evaluator environment is required"))?;
        Ok(EvaluationWorkerState {
            experiments: ExperimentRepository::new(self.pool.clone()),
            evidence: EvidenceRepository::new(self.pool.clone()),
            workers: WorkerRepository::new(self.pool),
            environment,
        })
    }
}

pub fn router(state: EvaluationWorkerState) -> Router {
    Router::new()
        .route("/claim", post(handlers::claim))
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
