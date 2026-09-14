//! Fenced supervisor terminal persistence, independent of native admission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::adapters::NativeCompletion;
use crate::{SchedulerError, SchedulerResult};
use systemprompt_evaluation::experiments::execution::{EvidenceArchive, ExecutionEvidence};
use systemprompt_evaluation::repository::experiments::{
    CleanupReport, EvaluationRepositories, ExecutionCompletion, ExecutionLease, TerminalOutcome,
};
use systemprompt_identifiers::{EvalExecutionId, EvalWorkerId, UserId};

#[derive(Debug, Clone, Copy)]
pub struct CleanupResources<'a> {
    pub container_id: Option<&'a str>,
    pub network_id: Option<&'a str>,
}

#[derive(Debug)]
pub struct CleanupOutcome {
    owner: UserId,
    execution: EvalExecutionId,
    worker: EvalWorkerId,
    fence: i64,
    succeeded: bool,
}
impl CleanupOutcome {
    pub const fn verified(&self) -> bool {
        self.succeeded
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionTerminal {
    repositories: EvaluationRepositories,
}

impl ExecutionTerminal {
    pub fn new(repositories: &EvaluationRepositories) -> Self {
        Self {
            repositories: repositories.clone(),
        }
    }

    pub async fn cleanup(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        resources: CleanupResources<'_>,
        operation: impl FnOnce() -> SchedulerResult<()>,
    ) -> SchedulerResult<CleanupOutcome> {
        let outcome = operation();
        let succeeded = outcome.is_ok();
        let diagnostic = outcome.err().map(|error| error.to_string());
        self.repositories
            .lifecycle
            .record_cleanup(
                owner,
                lease,
                &CleanupReport {
                    container_id: resources.container_id,
                    network_id: resources.network_id,
                    succeeded,
                    error: diagnostic.as_deref(),
                },
            )
            .await
            .map_err(super::internal)?;
        Ok(CleanupOutcome {
            owner: owner.clone(),
            execution: lease.execution_id.clone(),
            worker: lease.worker_id.clone(),
            fence: lease.fencing_token,
            succeeded,
        })
    }

    pub async fn persist(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        evidence: &ExecutionEvidence,
        archive: &EvidenceArchive,
        completion: NativeCompletion,
        cleanup: &CleanupOutcome,
    ) -> SchedulerResult<TerminalOutcome> {
        if cleanup.owner != *owner
            || cleanup.execution != lease.execution_id
            || cleanup.worker != lease.worker_id
            || cleanup.fence != lease.fencing_token
            || cleanup.succeeded != evidence.cleanup_confirmed
        {
            return Err(SchedulerError::config_error(
                "Terminal evidence differs from acknowledged owned cleanup",
            ));
        }
        self.repositories
            .evidence
            .submit(owner, lease, evidence, archive)
            .await
            .map_err(super::internal)?;
        let outcome = if evidence.exit_code == Some(0)
            && evidence.cleanup_confirmed
            && completion == NativeCompletion::Completed
        {
            TerminalOutcome::Completed
        } else {
            TerminalOutcome::Error
        };
        self.repositories
            .experiments
            .complete(
                owner,
                lease,
                &ExecutionCompletion {
                    outcome,
                    summary: if outcome == TerminalOutcome::Completed {
                        "Execution, evidence export and cleanup verified"
                    } else {
                        "Execution failed or cleanup remains pending"
                    }
                    .to_owned(),
                },
            )
            .await
            .map_err(super::internal)?;
        Ok(outcome)
    }
}
