//! Ordered evidence export, completion, accounting, and cleanup finalization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::failures::ReadinessScope;
use super::terminal::{CleanupOutcome, CleanupResources, ExecutionTerminal, TerminalEvidence};
use super::{
    ArtifactEvidence, ClientCapabilities, DeterministicMeasurement, EvaluatorSupervisor,
    EvidenceArchive, ExecutionEvidence, ExecutionOutcome, PreparedExecution, SchedulerResult,
    TerminalOutcome, VerificationInput, evidence_references, image_digest, internal, scoring,
    verification,
};
use sha2::{Digest, Sha256};
impl EvaluatorSupervisor {
    pub(super) async fn finalize_execution(
        &self,
        mut run: PreparedExecution,
        mut outcome: ExecutionOutcome,
    ) -> SchedulerResult<()> {
        let cleanup = self.tear_down(&mut run).await?;
        let cleanup_confirmed = cleanup.verified();
        let readiness_failure = if cleanup_confirmed {
            outcome.blocked.clone()
        } else {
            Some(
                "Native owned cleanup could not be verified; reconciliation is required".to_owned(),
            )
        };
        if let Err(error) = self
            .observe_targets(
                ReadinessScope {
                    worker: &run.worker,
                    lease: &run.lease,
                    assignment: &run.assignment,
                    variant_index: run.record.variant_index,
                },
                readiness_failure,
            )
            .await
        {
            tracing::warn!(execution_id = %run.lease.execution_id, %error, "Readiness observation failed; terminal evidence still required");
        }
        let evidence = self
            .build_evidence(&run, &outcome, cleanup_confirmed)
            .await?;
        let terminal_service = ExecutionTerminal::new(&self.repositories);
        let archive = EvidenceArchive {
            files: std::mem::take(&mut outcome.artifacts),
        };
        let terminal_evidence = TerminalEvidence {
            evidence: &evidence,
            archive: &archive,
            cleanup: &cleanup,
        };
        let terminal = if let Some(reason) = &outcome.blocked {
            terminal_service
                .persist_blocked(&run.worker.owner_id, &run.lease, terminal_evidence, reason)
                .await?
        } else {
            terminal_service
                .persist(
                    &run.worker.owner_id,
                    &run.lease,
                    terminal_evidence,
                    outcome.native_completion,
                )
                .await?
        };
        if matches!(
            terminal,
            TerminalOutcome::Completed | TerminalOutcome::Blocked
        ) {
            self.record_measurement(&run, &evidence, outcome).await?;
        }
        Ok(())
    }

    async fn build_evidence(
        &self,
        run: &PreparedExecution,
        outcome: &ExecutionOutcome,
        cleanup_confirmed: bool,
    ) -> SchedulerResult<ExecutionEvidence> {
        let artifact_evidence = outcome
            .artifacts
            .iter()
            .map(|(relative_path, file)| ArtifactEvidence {
                relative_path: relative_path.clone(),
                sha256: hex::encode(Sha256::digest(&file.bytes)),
                bytes: file.bytes.len() as u64,
            })
            .collect();
        let requests = self
            .repositories
            .evidence
            .list_request_ids(&run.worker.owner_id, &run.record.id)
            .await
            .map_err(internal)?;
        ExecutionEvidence::builder()
            .execution_id(run.record.id.clone())
            .fencing_token(run.record.fencing_token)
            .capabilities(ClientCapabilities {
                client: run.variant.client,
                client_version: run.variant.client_version.clone(),
                adapter_version: run
                    .client
                    .adapter()
                    .map_err(internal)?
                    .adapter_version()
                    .to_owned(),
                image_digest: image_digest(&self.config.client_image)?,
                supports_session_resume: false,
            })
            .installed_bundle_digest(run.assignment.skill_bundle.digest.clone())
            .candidate_bundle_digest(run.variant.skill_bundle_digest.clone())
            .workspace_digest(run.assignment.configuration.digest.clone())
            .requests(requests)
            .artifacts(artifact_evidence)
            .exit_code(outcome.status.code())
            .elapsed_milliseconds(outcome.started.elapsed().as_millis() as u64)
            .cleanup_confirmed(cleanup_confirmed)
            .build()
            .map_err(internal)
    }

    async fn tear_down(&self, run: &mut PreparedExecution) -> SchedulerResult<CleanupOutcome> {
        ExecutionTerminal::new(&self.repositories)
            .cleanup(
                &run.worker.owner_id,
                &run.lease,
                CleanupResources {
                    container_id: Some(&run.client_name),
                    network_id: Some(&run.network_name),
                },
                || {
                    let network = run.network.cleanup();
                    let workspace = std::fs::remove_dir_all(&run.directory)
                        .map_err(super::SchedulerError::from);
                    workspace.and(network)
                },
            )
            .await
    }

    async fn record_measurement(
        &self,
        run: &PreparedExecution,
        evidence: &ExecutionEvidence,
        outcome: ExecutionOutcome,
    ) -> SchedulerResult<()> {
        let archive = self
            .repositories
            .evidence
            .get_artifacts(&run.worker.owner_id, &run.record.id)
            .await
            .map_err(internal)?;
        let mut deterministic = verification::evaluate(VerificationInput {
            case: &run.case,
            evidence: &archive,
        });
        if let Some(reason) = outcome.blocked {
            deterministic.hard_failures.push(reason);
        }
        let references = evidence_references(evidence);
        let scored = outcome.judgment.as_ref().and_then(|value| {
            match scoring::score(&run.rubric, value, &references) {
                Ok(score) => Some((value.clone(), score)),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        execution_id = %run.record.id,
                        "Judgment could not be scored; execution recorded unscored"
                    );
                    None
                },
            }
        });
        let accounting = self
            .repositories
            .lifecycle
            .execution_accounting(&run.worker.owner_id, &run.record.id)
            .await
            .map_err(internal)?;
        let deterministic_passed = deterministic.hard_failures.is_empty()
            && deterministic.checks.values().all(|passed| *passed);
        let measurement = DeterministicMeasurement {
            hard_failures: deterministic.hard_failures,
            checks: deterministic.checks,
            judgment: scored.as_ref().map(|(value, _)| value.clone()),
            quality_milli: scored.as_ref().map(|(_, score)| score.score_milli),
            latency_ms: outcome.started.elapsed().as_millis() as u64,
            input_tokens: accounting.input_tokens,
            output_tokens: accounting.output_tokens,
            tool_calls: accounting.tool_calls,
            attempted_cost_microdollars: accounting.attempted_cost_microdollars,
            accounting_status: accounting.status,
            verified_success: deterministic_passed
                && scored.as_ref().is_some_and(|(_, score)| score.passed),
        };
        self.repositories
            .lifecycle
            .record_measurement(&run.worker.owner_id, &run.lease, &measurement)
            .await
            .map_err(internal)
    }
}
