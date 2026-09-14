//! Development-only suggestion generation from retained execution failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
        let references = outcome
            .artifacts
            .keys()
            .cloned()
            .chain(retained.iter().map(|request| request.as_str().to_owned()))
            .collect::<BTreeSet<_>>();
        let deterministic = verification::evaluate(VerificationInput {
            case: &run.case,
            evidence: &EvidenceArchive {
                files: outcome.artifacts.clone(),
            },
        });
        let failed = !deterministic.hard_failures.is_empty()
            || deterministic.checks.values().any(|passed| !passed)
            || !semantic_passed(run, outcome, &references);
        let suggestion_limit = run
            .assignment
            .spec
            .frozen
            .as_ref()
            .map_or(0, |frozen| frozen.cost_envelope.suggestion_calls);
        let allowed = outcome.status.success()
            && failed
            && suggestion_limit > 0
            && self
                .repositories
                .lifecycle
                .should_generate_suggestion(&run.worker.owner_id, &run.record.id, suggestion_limit)
                .await
                .map_err(internal)?;
        if !allowed {
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
        if status.success() {
            self.persist_generated_suggestion(run, &retained, &bytes)
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
        let normalized = run
            .client
            .adapter()
            .map_err(internal)?
            .normalize(bytes)
            .map_err(internal)?;
        normalized.validate().map_err(internal)?;
        let Ok(generated) = parse_suggestion(normalized.text.as_bytes()) else {
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
    references: &BTreeSet<String>,
) -> bool {
    let Some(judgment) = outcome.judgment.as_ref() else {
        return false;
    };
    match scoring::score(&run.rubric, judgment, references) {
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
