//! Supervised client execution, heartbeats, and semantic judging.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::adapters::{NativeCompletion, normalize_evidence};
use super::terminal::{CleanupResources, ExecutionTerminal, NativeStart};
use super::{
    ArtifactFile, BTreeMap, ClientPurpose, ContainerExecution, Duration, EvaluatorSupervisor,
    EvidenceJudgment, ExecutionStage, ExitStatus, Instant, PreparedExecution, SchedulerError,
    SchedulerResult, StageEvent, changed_workspace, execution_prompt, internal, workspace_state,
};

pub(super) struct ExecutionOutcome {
    pub status: ExitStatus,
    pub native_completion: NativeCompletion,
    pub started: Instant,
    pub last_heartbeat: Instant,
    pub artifacts: BTreeMap<String, ArtifactFile>,
    pub judgment: Option<EvidenceJudgment>,
    pub blocked: Option<String>,
}

impl EvaluatorSupervisor {
    pub(super) async fn run_client(
        &self,
        run: &mut PreparedExecution,
    ) -> SchedulerResult<Option<ExecutionOutcome>> {
        let prompt = execution_prompt(&run.case)?;
        let target =
            systemprompt_evaluation::capabilities::admit_variant(&run.variant).map_err(internal)?;
        let Some(mut execution) = ExecutionTerminal::new(&self.repositories)
            .start_client(
                &run.worker.owner_id,
                &run.lease,
                NativeStart {
                    launch: &run.launch,
                    client: &run.client,
                    purpose: ClientPurpose::Execution,
                    prompt: &prompt,
                    readiness: Some((&run.worker, target)),
                },
                || self.cleanup_failed_execution(&run.worker.owner_id, &run.lease),
            )
            .await?
        else {
            return Ok(None);
        };
        run.network
            .verify(&[run.client_name.clone(), run.relay_name.clone()])?;
        self.append_event(
            &run.worker,
            &run.lease,
            StageEvent {
                sequence: 1,
                stage: ExecutionStage::Context,
                summary: "Started isolated native client execution and authenticated relay",
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
        let (artifacts, native_completion) = collect_client_artifacts(run, &execution)?;
        Ok(Some(ExecutionOutcome {
            status,
            native_completion,
            started,
            last_heartbeat,
            artifacts,
            judgment: None,
            blocked: None,
        }))
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
        ExecutionTerminal::new(&self.repositories)
            .cleanup(
                &run.worker.owner_id,
                &run.lease,
                CleanupResources {
                    container_id: Some(&run.client_name),
                    network_id: Some(&run.network_name),
                },
                || {
                    let cancel = execution.cancel();
                    let isolated = run.network.cleanup();
                    let cleaned = std::fs::remove_dir_all(&run.directory);
                    let causes: Vec<String> = [
                        ("container", cancel.err().map(|error| error.to_string())),
                        ("network", isolated.err().map(|error| error.to_string())),
                        ("workspace", cleaned.err().map(|error| error.to_string())),
                    ]
                    .into_iter()
                    .filter_map(|(step, error)| error.map(|error| format!("{step}: {error}")))
                    .collect();
                    if causes.is_empty() {
                        Ok(())
                    } else {
                        Err(SchedulerError::config_error(format!(
                            "{failure}: {}",
                            causes.join("; ")
                        )))
                    }
                },
            )
            .await
            .map(|_| ())
    }
}

fn collect_client_artifacts(
    run: &PreparedExecution,
    execution: &ContainerExecution,
) -> SchedulerResult<(BTreeMap<String, ArtifactFile>, NativeCompletion)> {
    let mut artifacts = BTreeMap::new();
    artifacts.insert(
        "native-environment.json".to_owned(),
        ArtifactFile {
            bytes: std::fs::read(run.directory.join("native-environment.json"))?,
            executable: false,
        },
    );
    let stdout = capture_outputs(execution, "client", &mut artifacts)?;
    let normalized = normalize_evidence(run.client.adapter().map_err(internal)?, &stdout);
    let native_completion = normalized.output.completion;
    artifacts.insert(
        "client-normalized.json".to_owned(),
        ArtifactFile {
            bytes: serde_json::to_vec(&normalized).map_err(internal)?,
            executable: false,
        },
    );
    artifacts.extend(changed_workspace(&run.home.join("work"), &run.baseline)?);
    let observed = workspace_state(&run.skill_directory)?;
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
    Ok((artifacts, native_completion))
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
    let verification_path = stdout_path.with_file_name(format!("{stem}-native-verification.json"));
    artifacts.insert(
        format!("{stem}-native-verification.json"),
        ArtifactFile {
            bytes: std::fs::read(verification_path)?,
            executable: false,
        },
    );
    Ok(stdout)
}
