//! Development-only suggestion generation from retained execution failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::*;

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
        let archive = EvidenceArchive {
            files: outcome.artifacts.clone(),
        };
        let deterministic = verification::evaluate(VerificationInput {
            case: &run.case,
            evidence: &archive,
        });
        let semantic_passed = outcome
            .judgment
            .as_ref()
            .and_then(|value| scoring::score(&run.rubric, value, &references).ok())
            .is_some_and(|score| score.passed);
        let suggestion_limit = run
            .assignment
            .spec
            .frozen
            .as_ref()
            .map_or(0, |frozen| frozen.cost_envelope.suggestion_calls);
        let failed = !deterministic.hard_failures.is_empty()
            || deterministic.checks.values().any(|passed| !passed)
            || !semantic_passed;
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
        let name = format!("eval-suggestion-{}", safe_suffix(&run.record.id));
        let launch = ContainerLaunch::builder(self.config.docker.clone(), run.directory.clone())
            .image(self.config.client_image.clone())
            .network(run.network.name().to_owned())
            .name(name.clone())
            .output_stem("suggestion")
            .ownership(run.worker.owner_id.as_str(), run.record.id.as_str())
            .build()?;
        let evidence = outcome.artifacts.keys().collect::<Vec<_>>();
        let prompt = suggestion_prompt(&run.case, &deterministic.hard_failures, &evidence)?;
        let mut execution = launch.start_for(&run.client, ClientPurpose::Suggestion, &prompt)?;
        run.network.verify(&[name, run.relay_name.clone()])?;
        let status = loop {
            if let Some(status) = execution.poll()? {
                break status;
            }
            if outcome.last_heartbeat.elapsed() >= Duration::from_secs(20) {
                if let Err(error) = self
                    .repositories
                    .experiments
                    .heartbeat(&run.worker.owner_id, &run.lease)
                    .await
                {
                    self.abort_lost_lease(
                        run,
                        &mut execution,
                        "Suggestion cancellation cleanup was not fully acknowledged",
                    )
                    .await?;
                    return Err(internal(error));
                }
                outcome.last_heartbeat = Instant::now();
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        };
        let (stdout_path, stderr_path) = execution.output_paths();
        let bytes = std::fs::read(stdout_path)?;
        outcome.artifacts.insert(
            "suggestion-events.jsonl".to_owned(),
            ArtifactFile {
                bytes: bytes.clone(),
                executable: false,
            },
        );
        outcome.artifacts.insert(
            "suggestion-stderr.log".to_owned(),
            ArtifactFile {
                bytes: std::fs::read(stderr_path)?,
                executable: false,
            },
        );
        if status.success() {
            self.persist_generated_suggestion(run, &retained, &bytes)
                .await?;
        }
        self.append_event(
            &run.worker,
            &run.lease,
            4,
            ExecutionStage::Verification,
            "Generated a separately metered development-only suggestion",
        )
        .await
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
