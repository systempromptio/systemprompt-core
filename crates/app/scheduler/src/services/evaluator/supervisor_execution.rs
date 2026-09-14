//! Supervised client execution, heartbeats, and semantic judging.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::*;

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
            1,
            ExecutionStage::Context,
            "Started isolated Claude Code execution and authenticated relay",
        )
        .await?;
        let started = Instant::now();
        let mut last_heartbeat = Instant::now();
        let status = loop {
            if let Some(status) = execution.poll()? {
                break status;
            }
            if last_heartbeat.elapsed() >= Duration::from_secs(20) {
                if let Err(error) = self
                    .repositories
                    .experiments
                    .heartbeat(&run.worker.owner_id, &run.lease)
                    .await
                {
                    self.abort_lost_lease(
                        run,
                        &mut execution,
                        "Cancellation cleanup was not fully acknowledged",
                    )
                    .await?;
                    return Err(internal(error));
                }
                last_heartbeat = Instant::now();
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        };
        self.append_event(
            &run.worker,
            &run.lease,
            2,
            ExecutionStage::Verification,
            "Client exited; collecting deterministic evidence",
        )
        .await?;
        let (stdout_path, stderr_path) = execution.output_paths();
        let stdout = std::fs::read(stdout_path)?;
        let stderr = std::fs::read(stderr_path)?;
        let mut artifacts = BTreeMap::from([
            (
                "client-events.jsonl".to_owned(),
                ArtifactFile {
                    bytes: stdout,
                    executable: false,
                },
            ),
            (
                "client-stderr.log".to_owned(),
                ArtifactFile {
                    bytes: stderr,
                    executable: false,
                },
            ),
        ]);
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
        let prompt = judgment_prompt(&run.case, &run.rubric, outcome.artifacts.keys())?;
        let mut judge = launch.start_for(&run.client, ClientPurpose::Judge, &prompt)?;
        run.network.verify(&[judge_name, run.relay_name.clone()])?;
        let status = loop {
            if let Some(status) = judge.poll()? {
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
                        &mut judge,
                        "Judge cancellation cleanup was not fully acknowledged",
                    )
                    .await?;
                    return Err(internal(error));
                }
                outcome.last_heartbeat = Instant::now();
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        };
        let (stdout_path, stderr_path) = judge.output_paths();
        let bytes = std::fs::read(stdout_path)?;
        outcome.artifacts.insert(
            "judge-events.jsonl".to_owned(),
            ArtifactFile {
                bytes: bytes.clone(),
                executable: false,
            },
        );
        outcome.artifacts.insert(
            "judge-stderr.log".to_owned(),
            ArtifactFile {
                bytes: std::fs::read(stderr_path)?,
                executable: false,
            },
        );
        self.append_event(
            &run.worker,
            &run.lease,
            3,
            ExecutionStage::Verification,
            "Completed separately metered bounded semantic judgment",
        )
        .await?;
        Ok(status
            .success()
            .then(|| parse_judgment(&bytes).ok())
            .flatten())
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
                Some(&run.client_name),
                Some(&run.network_name),
                confirmed,
                (!confirmed).then_some(failure),
            )
            .await
            .map_err(internal)
    }
}
