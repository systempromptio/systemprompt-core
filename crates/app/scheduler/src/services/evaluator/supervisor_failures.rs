//! Retained preparation failures and bounded cleanup of exact execution
//! resources.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::terminal::{CleanupResources, ExecutionTerminal};
use super::{
    EvaluatorSupervisor, ExecutionLease, SchedulerError, SchedulerResult, UserId, safe_suffix,
};
use std::io::{Read, Seek, SeekFrom};
use std::process::Stdio;
use systemprompt_loader::subprocess::{place_in_own_process_group, spawn_owned_supervised};

pub(super) struct ReadinessScope<'a> {
    pub worker: &'a super::WorkerRecord,
    pub lease: &'a ExecutionLease,
    pub assignment: &'a super::ExecutionAssignment,
    pub variant_index: i32,
}

impl EvaluatorSupervisor {
    pub(super) async fn block_execution(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        variant_index: i32,
        reason: &str,
    ) -> SchedulerResult<()> {
        match self
            .repositories
            .workers
            .get_owned(owner, &lease.worker_id, &self.config.environment)
            .await
        {
            Ok(worker) => match self.repositories.assignments.get(&worker, lease).await {
                Ok(assignment) => {
                    if let Err(observation_error) = self
                        .observe_targets(
                            ReadinessScope {
                                worker: &worker,
                                lease,
                                assignment: &assignment,
                                variant_index,
                            },
                            Some(reason.to_owned()),
                        )
                        .await
                    {
                        tracing::warn!(execution_id = %lease.execution_id, %observation_error, "Readiness observation failed; retaining blocked execution and cleanup");
                    }
                },
                Err(assignment_error) => {
                    tracing::warn!(execution_id = %lease.execution_id, %assignment_error, "Readiness target unavailable; retaining preparation failure");
                },
            },
            Err(worker_error) => {
                tracing::warn!(execution_id = %lease.execution_id, %worker_error, "Worker unavailable for readiness observation; cleanup remains fenced");
            },
        }
        let terminal = ExecutionTerminal::new(&self.repositories);
        let cleanup = terminal
            .cleanup(
                owner,
                lease,
                CleanupResources {
                    container_id: None,
                    network_id: None,
                },
                || self.cleanup_failed_execution(owner, lease),
            )
            .await?;
        terminal.block(owner, lease, &cleanup, reason).await?;
        Ok(())
    }

    pub(super) async fn observe_targets(
        &self,
        scope: ReadinessScope<'_>,
        failure: Option<String>,
    ) -> SchedulerResult<()> {
        use systemprompt_evaluation::capabilities::{
            NativeReadiness, NativeReadinessState, verified_native_targets,
        };
        let target = verified_native_targets().iter().find(|target| {
            target.supports_platform(std::env::consts::OS, std::env::consts::ARCH)
                && usize::try_from(scope.variant_index)
                    .ok()
                    .and_then(|index| scope.assignment.spec.variants.get(index))
                    .is_some_and(|variant| {
                        target.matches(variant, std::env::consts::OS, std::env::consts::ARCH)
                    })
        });
        if let Some(target) = target {
            self.repositories
                .events
                .observe_readiness(
                    scope.worker,
                    scope.lease,
                    NativeReadiness {
                        target: target.clone(),
                        state: if failure.is_some() {
                            NativeReadinessState::Unavailable
                        } else {
                            NativeReadinessState::LastVerified
                        },
                        observed_at: None,
                        diagnostic: failure,
                    },
                )
                .await
                .map_err(super::internal)?;
        }
        Ok(())
    }

    pub(super) fn cleanup_failed_execution(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
    ) -> SchedulerResult<()> {
        let cleanup_started = std::time::Instant::now();
        let mut count = 0usize;
        let filters = [
            format!("label=systemprompt.evaluator.owner={}", owner.as_str()),
            format!(
                "label=systemprompt.evaluator.execution={}",
                lease.execution_id.as_str()
            ),
            format!("label=systemprompt.evaluator.worker={}", lease.worker_id),
            format!("label=systemprompt.evaluator.fence={}", lease.fencing_token),
        ];
        for kind in ["container", "network"] {
            let list = list_arguments(kind, &filters);
            let objects = self.failure_cleanup_command(&list, cleanup_started)?;
            for id in objects.lines().filter(|id| !id.is_empty()) {
                count += 1;
                if count > 16 {
                    return Err(SchedulerError::config_error(
                        "Owned cleanup exceeded its object limit",
                    ));
                }
                if id.len() > 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(SchedulerError::config_error(
                        "Owned cleanup returned an invalid object identity",
                    ));
                }
                let remove = if kind == "container" {
                    vec!["rm", "--force", id]
                } else {
                    vec!["network", "rm", id]
                };
                self.failure_cleanup_command(&remove, cleanup_started)?;
            }
            if !self
                .failure_cleanup_command(&list, cleanup_started)?
                .trim()
                .is_empty()
            {
                return Err(SchedulerError::config_error(
                    "Owned execution resources remain after cleanup",
                ));
            }
        }
        let directory = self.config.workspace_root.join(format!(
            "{}-f{}",
            safe_suffix(&lease.execution_id),
            lease.fencing_token
        ));
        match std::fs::remove_dir_all(directory) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn failure_cleanup_command(
        &self,
        arguments: &[&str],
        cleanup_started: std::time::Instant,
    ) -> SchedulerResult<String> {
        let mut output = tempfile::tempfile()?;
        let (mut command, _configuration) = super::super::docker::command(&self.config.docker)?;
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(output.try_clone()?)
            .stderr(Stdio::null());
        place_in_own_process_group(&mut command);
        let mut child = spawn_owned_supervised(command)?;
        let started = std::time::Instant::now();
        loop {
            if output.metadata()?.len() > 16_384
                || started.elapsed() > std::time::Duration::from_secs(10)
                || cleanup_started.elapsed() > std::time::Duration::from_secs(30)
            {
                child.kill()?;
                child.wait()?;
                return Err(SchedulerError::config_error(
                    "Owned cleanup exceeded its time or output bound",
                ));
            }
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    return Err(SchedulerError::config_error(
                        "Owned execution cleanup was not acknowledged",
                    ));
                }
                output.seek(SeekFrom::Start(0))?;
                let mut bytes = Vec::new();
                output.take(16_385).read_to_end(&mut bytes)?;
                if bytes.len() > 16_384 {
                    return Err(SchedulerError::config_error(
                        "Owned cleanup exceeded its output bound",
                    ));
                }
                return String::from_utf8(bytes).map_err(super::internal);
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }
}

fn list_arguments<'a>(kind: &str, filters: &'a [String; 4]) -> Vec<&'a str> {
    let mut arguments = if kind == "container" {
        vec!["ps", "-aq"]
    } else {
        vec!["network", "ls", "-q"]
    };
    for filter in filters {
        arguments.extend(["--filter", filter.as_str()]);
    }
    arguments
}

pub(super) fn diagnostic(stage: &str, error: &SchedulerError) -> String {
    let detail = match error {
        SchedulerError::ConfigError { message } => {
            const REASONS: [&str; 9] = [
                "Pinned image configuration could not be established",
                "Image manifest resolved to a different retained config identity",
                "Pinned executable bytes do not match native admission",
                "Observed executable version does not match native admission",
                "Native pin verification exceeded time or output bounds",
                "Native pin verification command failed",
                "Adapter configuration readback failed",
                "Execution network has unexpected members",
                "Execution network is not internal",
            ];
            REASONS.iter().find(|reason| message.starts_with(**reason)).map_or_else(
                || "native configuration or preparation rejected; verify retained target pins, isolated workspace and network configuration".to_owned(),
                |reason| (*reason).to_owned(),
            )
        },
        SchedulerError::Io(error) => format!("local I/O {:?}; verify executable, image and workspace access", error.kind()),
        _ => "owned assignment or execution operation failed; inspect retained execution and worker configuration".to_owned(),
    };
    format!("Native {stage} blocked: {detail}")
}
