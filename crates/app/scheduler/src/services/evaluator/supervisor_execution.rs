//! Supervised client execution, heartbeats, and semantic judging.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ArtifactFile, BTreeMap, ClientPurpose, ContainerExecution, ContainerLaunch, Duration,
    EvaluationTrafficClass, EvaluatorSupervisor, EvidenceJudgment, ExecutionStage, ExitStatus,
    Instant, PreparedExecution, SchedulerError, SchedulerResult, StageEvent, changed_workspace,
    execution_prompt, internal, judgment_prompt, parse_judgment, safe_suffix, workspace_state,
    write_private,
};
use systemprompt_evaluation::repository::experiments::CleanupReport;

pub(super) struct ExecutionOutcome {
    pub status: ExitStatus,
    pub started: Instant,
    pub last_heartbeat: Instant,
    pub artifacts: BTreeMap<String, ArtifactFile>,
    pub judgment: Option<EvidenceJudgment>,
}

impl EvaluatorSupervisor {
    pub(super) async fn run_client(
        &self,
        run: &mut PreparedExecution,
    ) -> SchedulerResult<ExecutionOutcome> {
        let prompt = execution_prompt(&run.case)?;
        let mut execution = run.launch.start(&run.client, &prompt)?;
        run.network
            .verify(&[run.client_name.clone(), run.relay_name.clone()])?;
        self.append_event(
            &run.worker,
            &run.lease,
            StageEvent {
                sequence: 1,
                stage: ExecutionStage::Context,
                summary: "Started isolated Claude Code execution and authenticated relay",
            },
        )
        .await?;
        let started = Instant::now();
        let mut last_heartbeat = Instant::now();
        let status = self
            .await_exit(
                run,
                &mut execution,
                &mut last_heartbeat,
                "Cancellation cleanup was not fully acknowledged",
            )
            .await?;
        self.append_event(
            &run.worker,
            &run.lease,
            StageEvent {
                sequence: 2,
                stage: ExecutionStage::Verification,
                summary: "Client exited; collecting deterministic evidence",
            },
        )
        .await?;
        let mut artifacts = BTreeMap::new();
        capture_outputs(&execution, "client", &mut artifacts)?;
        artifacts.extend(changed_workspace(&run.home.join("work"), &run.baseline)?);
        let observed = workspace_state(&run.home.join(".claude/skills"))?;
        artifacts.insert(
            "installation-integrity.json".to_owned(),
            ArtifactFile {
                bytes: serde_jcs::to_vec(&serde_json::json!({
                    "expected": &run.installed_skill_state,
                    "observed": &observed,
                    "matches": run.installed_skill_state == observed,
                }))
                .map_err(internal)?,
                executable: false,
            },
        );
        Ok(ExecutionOutcome {
            status,
            started,
            last_heartbeat,
            artifacts,
            judgment: None,
        })
    }

    pub(super) async fn run_judgment(
        &self,
        run: &mut PreparedExecution,
        outcome: &mut ExecutionOutcome,
    ) -> SchedulerResult<Option<EvidenceJudgment>> {
        if !outcome.status.success() {
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
        let judge_name = format!("eval-judge-{}", safe_suffix(&run.record.id));
        let launch = ContainerLaunch::builder(self.config.docker.clone(), run.directory.clone())
            .image(self.config.client_image.clone())
            .network(run.network.name().to_owned())
            .name(judge_name.clone())
            .output_stem("judge")
            .ownership(run.worker.owner_id.as_str(), run.record.id.as_str())
            .build()?;
        let evidence = outcome.artifacts.keys().collect::<Vec<_>>();
        let prompt = judgment_prompt(&run.case, &run.rubric, &evidence)?;
        let mut judge = launch.start_for(&run.client, ClientPurpose::Judge, &prompt)?;
        run.network.verify(&[judge_name, run.relay_name.clone()])?;
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
        match parse_judgment(&bytes) {
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

    pub(super) async fn await_exit(
        &self,
        run: &mut PreparedExecution,
        execution: &mut ContainerExecution,
        last_heartbeat: &mut Instant,
        abort_message: &str,
    ) -> SchedulerResult<ExitStatus> {
        loop {
            if let Some(status) = execution.poll()? {
                return Ok(status);
            }
            if last_heartbeat.elapsed() >= Duration::from_secs(20) {
                if let Err(error) = self
                    .repositories
                    .experiments
                    .heartbeat(&run.worker.owner_id, &run.lease)
                    .await
                {
                    self.abort_lost_lease(run, execution, abort_message).await?;
                    return Err(internal(error));
                }
                *last_heartbeat = Instant::now();
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    pub(super) async fn abort_lost_lease(
        &self,
        run: &mut PreparedExecution,
        execution: &mut ContainerExecution,
        failure: &str,
    ) -> SchedulerResult<()> {
        let cancel = execution.cancel();
        let isolated = run.network.cleanup();
        let cleaned = std::fs::remove_dir_all(&run.directory);
        let confirmed = cancel.is_ok() && isolated.is_ok() && cleaned.is_ok();
        self.repositories
            .lifecycle
            .record_cleanup(
                &run.worker.owner_id,
                &run.lease,
                &CleanupReport {
                    container_id: Some(&run.client_name),
                    network_id: Some(&run.network_name),
                    succeeded: confirmed,
                    error: (!confirmed).then_some(failure),
                },
            )
            .await
            .map_err(internal)
    }
}

pub(super) fn capture_outputs(
    execution: &ContainerExecution,
    stem: &str,
    artifacts: &mut BTreeMap<String, ArtifactFile>,
) -> SchedulerResult<Vec<u8>> {
    let (stdout_path, stderr_path) = execution.output_paths();
    let stdout = std::fs::read(stdout_path)?;
    artifacts.insert(
        format!("{stem}-events.jsonl"),
        ArtifactFile {
            bytes: stdout.clone(),
            executable: false,
        },
    );
    artifacts.insert(
        format!("{stem}-stderr.log"),
        ArtifactFile {
            bytes: std::fs::read(stderr_path)?,
            executable: false,
        },
    );
    Ok(stdout)
}
