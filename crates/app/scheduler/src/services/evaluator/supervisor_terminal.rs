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

#[derive(Debug)]
pub struct NativeStart<'a> {
    pub launch: &'a super::ContainerLaunch,
    pub client: &'a super::NativeClient,
    pub purpose: super::ClientPurpose,
    pub prompt: &'a str,
    pub readiness: Option<(
        &'a systemprompt_evaluation::repository::experiments::WorkerRecord,
        &'a systemprompt_evaluation::capabilities::VerifiedNativeTarget,
    )>,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalEvidence<'a> {
    pub evidence: &'a ExecutionEvidence,
    pub archive: &'a EvidenceArchive,
    pub cleanup: &'a CleanupOutcome,
}

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

    pub async fn start_client(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        start: NativeStart<'_>,
        cleanup: impl FnOnce() -> SchedulerResult<()>,
    ) -> SchedulerResult<Option<super::ContainerExecution>> {
        match start
            .launch
            .start_for(start.client, start.purpose, start.prompt)
        {
            Ok(execution) => Ok(Some(execution)),
            Err(error) => {
                if let Some((worker, target)) = start.readiness
                    && let Err(observation_error) = self
                        .repositories
                        .events
                        .observe_readiness(
                            worker,
                            lease,
                            systemprompt_evaluation::capabilities::NativeReadiness {
                                target: target.clone(),
                                state: systemprompt_evaluation::capabilities::NativeReadinessState::Unavailable,
                                observed_at: None,
                                diagnostic: Some(super::failures::diagnostic("client", &error)),
                            },
                        )
                        .await
                {
                    tracing::warn!(execution_id = %lease.execution_id, %observation_error, "Startup readiness observation failed; blocked completion still required");
                }
                let witness = self
                    .cleanup(
                        owner,
                        lease,
                        CleanupResources {
                            container_id: None,
                            network_id: None,
                        },
                        cleanup,
                    )
                    .await?;
                self.block(
                    owner,
                    lease,
                    &witness,
                    &super::failures::diagnostic("client", &error),
                )
                .await?;
                Ok(None)
            },
        }
    }

    pub async fn cleanup(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        resources: CleanupResources<'_>,
        operation: impl FnOnce() -> SchedulerResult<()>,
    ) -> SchedulerResult<CleanupOutcome> {
        let outcome = self
            .repositories
            .lifecycle
            .with_cleanup_fence(owner, lease, operation)
            .await
            .map_err(super::internal)?;
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

    pub async fn block(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        cleanup: &CleanupOutcome,
        reason: &str,
    ) -> SchedulerResult<TerminalOutcome> {
        validate_cleanup(owner, lease, cleanup)?;
        self.repositories
            .experiments
            .complete(
                owner,
                lease,
                &ExecutionCompletion {
                    outcome: TerminalOutcome::Blocked,
                    summary: reason.chars().take(2048).collect(),
                },
            )
            .await
            .map_err(super::internal)?;
        Ok(TerminalOutcome::Blocked)
    }

    pub async fn persist_blocked(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        terminal: TerminalEvidence<'_>,
        reason: &str,
    ) -> SchedulerResult<TerminalOutcome> {
        let TerminalEvidence {
            evidence,
            archive,
            cleanup,
        } = terminal;
        validate_cleanup(owner, lease, cleanup)?;
        if cleanup.verified() != evidence.cleanup_confirmed {
            return Err(SchedulerError::config_error(
                "Blocked evidence differs from acknowledged cleanup",
            ));
        }
        self.repositories
            .evidence
            .submit(owner, lease, evidence, archive)
            .await
            .map_err(super::internal)?;
        self.block(owner, lease, cleanup, reason).await
    }

    pub async fn persist(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        terminal: TerminalEvidence<'_>,
        completion: NativeCompletion,
    ) -> SchedulerResult<TerminalOutcome> {
        let TerminalEvidence {
            evidence,
            archive,
            cleanup,
        } = terminal;
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

fn validate_cleanup(
    owner: &UserId,
    lease: &ExecutionLease,
    cleanup: &CleanupOutcome,
) -> SchedulerResult<()> {
    if cleanup.owner != *owner
        || cleanup.execution != lease.execution_id
        || cleanup.worker != lease.worker_id
        || cleanup.fence != lease.fencing_token
    {
        return Err(SchedulerError::config_error(
            "Blocked completion requires the acknowledged owned cleanup fence",
        ));
    }
    Ok(())
}
