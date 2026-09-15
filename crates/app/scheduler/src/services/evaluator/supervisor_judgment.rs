//! Bounded semantic judging of completed client executions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::adapters::{NativeCompletion, normalize_evidence};
use super::{
    ArtifactFile, ClientPurpose, ContainerExecution, ContainerLaunch, EvaluationTrafficClass,
    EvaluatorSupervisor, EvidenceJudgment, ExecutionOutcome, ExecutionStage, PreparedExecution,
    SchedulerError, SchedulerResult, StageEvent, capture_outputs, internal, judgment_prompt,
    parse_judgment, safe_suffix, write_private,
};

impl EvaluatorSupervisor {
    pub(super) async fn run_judgment(
        &self,
        run: &mut PreparedExecution,
        outcome: &mut ExecutionOutcome,
    ) -> SchedulerResult<Option<EvidenceJudgment>> {
        if !outcome.status.success() || outcome.native_completion != NativeCompletion::Completed {
            return Ok(None);
        }
        let stdout = outcome
            .artifacts
            .get("client-events.jsonl")
            .ok_or_else(|| SchedulerError::Internal("client evidence missing".to_owned()))?
            .bytes
            .clone();
        let evidence_dir = run.home.join("work/evidence");
        std::fs::create_dir_all(&evidence_dir)?;
        write_private(&evidence_dir.join("client-events.jsonl"), &stdout)?;
        self.repositories
            .gateway
            .set_traffic_class(
                &run.worker.owner_id,
                &run.lease,
                EvaluationTrafficClass::Judge,
            )
            .await
            .map_err(internal)?;
        let mut judge = self.start_judge(run, outcome)?;
        let status = self
            .await_exit(
                run,
                &mut judge,
                &mut outcome.last_heartbeat,
                "Judge cancellation cleanup was not fully acknowledged",
            )
            .await?;
        let bytes = capture_outputs(&judge, "judge", &mut outcome.artifacts)?;
        self.append_event(
            &run.worker,
            &run.lease,
            StageEvent {
                sequence: 3,
                stage: ExecutionStage::Verification,
                summary: "Completed separately metered bounded semantic judgment",
            },
        )
        .await?;
        if !status.success() {
            return Ok(None);
        }
        let normalized = normalize_evidence(run.client.adapter().map_err(internal)?, &bytes);
        outcome.artifacts.insert(
            "judge-normalized.json".to_owned(),
            ArtifactFile {
                bytes: serde_json::to_vec(&normalized).map_err(internal)?,
                executable: false,
            },
        );
        if normalized.output.completion != NativeCompletion::Completed {
            return Ok(None);
        }
        match parse_judgment(normalized.output.text.as_bytes()) {
            Ok(judgment) => Ok(Some(judgment)),
            Err(error) => {
                tracing::warn!(
                    execution_id = %run.lease.execution_id,
                    %error,
                    "semantic judge output rejected; execution stays unscored"
                );
                Ok(None)
            },
        }
    }

    fn start_judge(
        &self,
        run: &PreparedExecution,
        outcome: &ExecutionOutcome,
    ) -> SchedulerResult<ContainerExecution> {
        let judge_name = format!("eval-judge-{}", safe_suffix(&run.record.id));
        let launch = ContainerLaunch::builder(self.config.docker.clone(), run.directory.clone())
            .image(self.config.client_image.clone())
            .network(run.network.name().to_owned())
            .name(judge_name.clone())
            .output_stem("judge")
            .ownership(run.worker.owner_id.as_str(), run.record.id.as_str())
            .lease(&run.lease)
            .build()?;
        let evidence = outcome.artifacts.keys().collect::<Vec<_>>();
        let prompt = judgment_prompt(&run.case, &run.rubric, &evidence)?;
        let judge = launch.start_for(&run.client, ClientPurpose::Judge, &prompt)?;
        run.network.verify(&[judge_name, run.relay_name.clone()])?;
        Ok(judge)
    }
}
