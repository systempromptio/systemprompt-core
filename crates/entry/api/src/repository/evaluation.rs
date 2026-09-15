//! Repository-backed state constructed once for evaluator worker routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_evaluation::repository::experiments::{
    AssignmentRepository, EvaluationLifecycleRepository, EvaluationRepositories,
    EvidenceRepository, ExecutionCapabilityRepository, ExecutionEventRepository,
    ExperimentRepository, WorkerRepository,
};

#[derive(Clone, Debug)]
pub struct EvaluationWorkerState {
    pub(crate) assignments: AssignmentRepository,
    pub(crate) events: ExecutionEventRepository,
    pub(crate) capabilities: ExecutionCapabilityRepository,
    pub(crate) experiments: ExperimentRepository,
    pub(crate) evidence: EvidenceRepository,
    pub(crate) workers: WorkerRepository,
    pub(crate) lifecycle: EvaluationLifecycleRepository,
    pub(crate) environment: String,
}

impl EvaluationWorkerState {
    pub const fn builder(repositories: EvaluationRepositories) -> EvaluationWorkerStateBuilder {
        EvaluationWorkerStateBuilder {
            repositories,
            environment: None,
        }
    }
}

#[derive(Debug)]
pub struct EvaluationWorkerStateBuilder {
    repositories: EvaluationRepositories,
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
        let EvaluationRepositories {
            assignments,
            events,
            capabilities,
            experiments,
            evidence,
            lifecycle,
            workers,
            ..
        } = self.repositories;
        Ok(EvaluationWorkerState {
            assignments,
            events,
            capabilities,
            experiments,
            evidence,
            workers,
            lifecycle,
            environment,
        })
    }
}
