//! Ordered evidence export, completion, accounting, and cleanup finalization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ArtifactEvidence, ClientCapabilities, DeterministicMeasurement, EvaluatorSupervisor,
    EvidenceArchive, EvidenceJudgment, ExecutionCompletion, ExecutionEvidence, ExecutionOutcome,
    Instant, PreparedExecution, SchedulerResult, TerminalOutcome, VerificationInput,
    evidence_references, image_digest, internal, scoring, verification,
};
use sha2::{Digest, Sha256};
use systemprompt_evaluation::repository::experiments::CleanupReport;
impl EvaluatorSupervisor {
    pub(super) async fn finalize_execution(
        &self,
        mut run: PreparedExecution,
        outcome: ExecutionOutcome,
    ) -> SchedulerResult<()> {
        let artifact_evidence = outcome
            .artifacts
            .iter()
            .map(|(relative_path, file)| ArtifactEvidence {
                relative_path: relative_path.clone(),
                sha256: hex::encode(Sha256::digest(&file.bytes)),
                bytes: file.bytes.len() as u64,
            })
            .collect();
        let cleanup_confirmed = self.tear_down(&mut run).await?;
        let requests = self
            .repositories
            .evidence
            .list_request_ids(&run.worker.owner_id, &run.record.id)
            .await
            .map_err(internal)?;
        let evidence = ExecutionEvidence::builder()
            .execution_id(run.record.id.clone())
            .fencing_token(run.record.fencing_token)
            .capabilities(ClientCapabilities {
                client: run.variant.client,
                client_version: run.variant.client_version.clone(),
                adapter_version: "rust-evaluator-supervisor-v1".to_owned(),
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
            .map_err(internal)?;
        self.repositories
            .evidence
            .submit(
                &run.worker.owner_id,
                &run.lease,
                &evidence,
                &EvidenceArchive {
                    files: outcome.artifacts,
                },
            )
            .await
            .map_err(internal)?;
        let terminal = if outcome.status.success() && cleanup_confirmed {
            TerminalOutcome::Completed
        } else {
            TerminalOutcome::Error
        };
        self.repositories
            .experiments
            .complete(
                &run.worker.owner_id,
                &run.lease,
                &ExecutionCompletion {
                    outcome: terminal,
                    summary: terminal_summary(terminal).to_owned(),
                },
            )
            .await
            .map_err(internal)?;
        if terminal == TerminalOutcome::Completed {
            self.record_measurement(&run, &evidence, outcome.judgment, outcome.started)
                .await?;
        }
        Ok(())
    }

    async fn tear_down(&self, run: &mut PreparedExecution) -> SchedulerResult<bool> {
        let network_cleanup = run.network.cleanup();
        let workspace_cleanup = std::fs::remove_dir_all(&run.directory);
        let cleanup_confirmed = workspace_cleanup.is_ok() && network_cleanup.is_ok();
        let cleanup_error = workspace_cleanup
            .err()
            .map(|error| error.to_string())
            .or_else(|| network_cleanup.err().map(|error| error.to_string()));
        self.repositories
            .lifecycle
            .record_cleanup(
                &run.worker.owner_id,
                &run.lease,
                &CleanupReport {
                    container_id: Some(&run.client_name),
                    network_id: Some(&run.network_name),
                    succeeded: cleanup_confirmed,
                    error: cleanup_error.as_deref(),
                },
            )
            .await
            .map_err(internal)?;
        Ok(cleanup_confirmed)
    }

    async fn record_measurement(
        &self,
        run: &PreparedExecution,
        evidence: &ExecutionEvidence,
        judgment: Option<EvidenceJudgment>,
        started: Instant,
    ) -> SchedulerResult<()> {
        let archive = self
            .repositories
            .evidence
            .get_artifacts(&run.worker.owner_id, &run.record.id)
            .await
            .map_err(internal)?;
        let deterministic = verification::evaluate(VerificationInput {
            case: &run.case,
            evidence: &archive,
        });
        let references = evidence_references(evidence);
        let scored = judgment.as_ref().and_then(|value| {
            scoring::score(&run.rubric, value, &references)
                .ok()
                .map(|score| (value.clone(), score))
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
            latency_ms: started.elapsed().as_millis() as u64,
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

fn terminal_summary(outcome: TerminalOutcome) -> &'static str {
    if outcome == TerminalOutcome::Completed {
        "Execution, evidence export and cleanup verified"
    } else {
        "Execution failed or cleanup remains pending"
    }
}
