//! Repository-backed state constructed once for evaluator worker routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_evaluation::repository::experiments::{
    EvidenceRepository, ExecutionCapabilityRepository, ExperimentRepository, WorkerRepository,
};

#[derive(Clone, Debug)]
pub struct EvaluationWorkerState {
    pub(crate) capabilities: ExecutionCapabilityRepository,
    pub(crate) experiments: ExperimentRepository,
    pub(crate) evidence: EvidenceRepository,
    pub(crate) workers: WorkerRepository,
    pub(crate) environment: String,
}

impl EvaluationWorkerState {
    pub const fn builder(pool: PgPool) -> EvaluationWorkerStateBuilder {
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
            capabilities: ExecutionCapabilityRepository::new(self.pool.clone()),
            experiments: ExperimentRepository::new(self.pool.clone()),
            evidence: EvidenceRepository::new(self.pool.clone()),
            workers: WorkerRepository::new(self.pool),
            environment,
        })
    }
}
