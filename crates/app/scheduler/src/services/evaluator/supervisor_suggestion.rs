//! Development-only suggestion generation from retained execution failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::adapters::{NativeCompletion, normalize_evidence};
use super::{
    BTreeSet, ClientPurpose, ContainerExecution, ContainerLaunch, EvaluationTrafficClass,
    EvaluatorSupervisor, EvidenceArchive, ExecutionOutcome, ExecutionStage, PreparedExecution,
    SchedulerResult, StageEvent, VerificationInput, capture_outputs, internal, parse_suggestion,
    safe_suffix, scoring, suggestion_prompt, verification,
};

impl EvaluatorSupervisor {
    pub(super) async fn generate_suggestion_if_needed(
        &self,
        run: &mut PreparedExecution,
        outcome: &mut ExecutionOutcome,
    ) -> SchedulerResult<()> {
        let retained = self
            .repositories
            .evidence
            .list_request_ids(&run.worker.owner_id, &run.record.id)
            .await
            .map_err(internal)?;
        let deterministic = verification::evaluate(VerificationInput {
            case: &run.case,
            evidence: &EvidenceArchive {
                files: outcome.artifacts.clone(),
            },
        });
        let failed = !deterministic.hard_failures.is_empty()
            || deterministic.checks.values().any(|passed| !passed)
            || !semantic_passed(run, outcome, &retained);
        if !failed || !self.suggestion_allowed(run, outcome).await? {
            return Ok(());
        }
        self.repositories
            .gateway
            .set_traffic_class(
                &run.worker.owner_id,
                &run.lease,
                EvaluationTrafficClass::Suggestion,
            )
            .await
            .map_err(internal)?;
        let mut execution =
            self.start_suggestion_client(run, outcome, &deterministic.hard_failures)?;
        let status = self
            .await_exit(
                run,
                &mut execution,
                &mut outcome.last_heartbeat,
                "Suggestion cancellation cleanup was not fully acknowledged",
            )
            .await?;
        let bytes = capture_outputs(&execution, "suggestion", &mut outcome.artifacts)?;
        let normalized = normalize_evidence(run.client.adapter().map_err(internal)?, &bytes);
        outcome.artifacts.insert(
            "suggestion-normalized.json".to_owned(),
            super::ArtifactFile {
                bytes: serde_json::to_vec(&normalized).map_err(internal)?,
                executable: false,
            },
        );
        if status.success() && normalized.output.completion == NativeCompletion::Completed {
            self.persist_generated_suggestion(run, &retained, normalized.output.text.as_bytes())
                .await?;
        }
        self.append_event(
            &run.worker,
            &run.lease,
            StageEvent {
                sequence: 4,
                stage: ExecutionStage::Verification,
                summary: "Generated a separately metered development-only suggestion",
            },
        )
        .await
    }

    async fn suggestion_allowed(
        &self,
        run: &PreparedExecution,
        outcome: &ExecutionOutcome,
    ) -> SchedulerResult<bool> {
        let suggestion_limit = run
            .assignment
            .spec
            .frozen
            .as_ref()
            .map_or(0, |frozen| frozen.cost_envelope.suggestion_calls);
        if !outcome.status.success()
            || outcome.native_completion != NativeCompletion::Completed
            || suggestion_limit == 0
        {
            return Ok(false);
        }
        self.repositories
            .lifecycle
            .should_generate_suggestion(&run.worker.owner_id, &run.record.id, suggestion_limit)
            .await
            .map_err(internal)
    }

    fn start_suggestion_client(
        &self,
        run: &PreparedExecution,
        outcome: &ExecutionOutcome,
        hard_failures: &[String],
    ) -> SchedulerResult<ContainerExecution> {
        let name = format!("eval-suggestion-{}", safe_suffix(&run.record.id));
        let launch = ContainerLaunch::builder(self.config.docker.clone(), run.directory.clone())
            .image(self.config.client_image.clone())
            .network(run.network.name().to_owned())
            .name(name.clone())
            .output_stem("suggestion")
            .ownership(run.worker.owner_id.as_str(), run.record.id.as_str())
            .lease(&run.lease)
            .build()?;
        let evidence = outcome.artifacts.keys().collect::<Vec<_>>();
        let prompt = suggestion_prompt(&run.case, hard_failures, &evidence)?;
        let execution = launch.start_for(&run.client, ClientPurpose::Suggestion, &prompt)?;
        run.network.verify(&[name, run.relay_name.clone()])?;
        Ok(execution)
    }

    async fn persist_generated_suggestion(
        &self,
        run: &PreparedExecution,
        retained: &[systemprompt_identifiers::AiRequestId],
        bytes: &[u8],
    ) -> SchedulerResult<()> {
        let Ok(generated) = parse_suggestion(bytes) else {
            return Ok(());
        };
        let after = self
            .repositories
            .evidence
            .list_request_ids(&run.worker.owner_id, &run.record.id)
            .await
            .map_err(internal)?;
        if let Some(request) = after.iter().find(|request| !retained.contains(request)) {
            self.repositories
                .lifecycle
                .record_generated_suggestion(
                    &run.worker.owner_id,
                    &run.record.id,
                    request,
                    &generated,
                )
                .await
                .map_err(internal)?;
        }
        Ok(())
    }
}

fn semantic_passed(
    run: &PreparedExecution,
    outcome: &ExecutionOutcome,
    retained: &[systemprompt_identifiers::AiRequestId],
) -> bool {
    let Some(judgment) = outcome.judgment.as_ref() else {
        return false;
    };
    let references = outcome
        .artifacts
        .keys()
        .cloned()
        .chain(retained.iter().map(|request| request.as_str().to_owned()))
        .collect::<BTreeSet<_>>();
    match scoring::score(&run.rubric, judgment, &references) {
        Ok(score) => score.passed,
        Err(error) => {
            tracing::warn!(
                execution_id = %run.record.id,
                %error,
                "semantic judgment could not be scored; treated as failed"
            );
            false
        },
    }
}
