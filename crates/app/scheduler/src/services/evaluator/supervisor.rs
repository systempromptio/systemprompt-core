//! Durable evaluator supervisor composed from fenced domain repositories.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::time::{Duration, Instant};

use systemprompt_evaluation::experiments::VariantSpec;
use systemprompt_evaluation::experiments::execution::{
    ArtifactEvidence, ArtifactFile, ClientCapabilities, EvidenceArchive, ExecutionEvidence,
    ExecutionLimits,
};
use systemprompt_evaluation::experiments::records::ExecutionRecord;
use systemprompt_evaluation::experiments::resources::{
    CaseContent, ResourceContent, RubricContent,
};
use systemprompt_evaluation::experiments::scoring::{self, EvidenceJudgment};
use systemprompt_evaluation::experiments::verification::{self, VerificationInput};
use systemprompt_evaluation::repository::experiments::{
    DeterministicMeasurement, EvaluationRepositories, EvaluationTrafficClass, ExecutionAssignment,
    ExecutionEvent, ExecutionLease, ExecutionStage, GeneratedSuggestion, TerminalOutcome,
    WorkerRecord,
};
use systemprompt_identifiers::{EvalExecutionId, EvalWorkerId, UserId};
use systemprompt_models::managed::RevisionBundle;

use super::client::{ClientPurpose, NativeClient};
use super::container::{ContainerExecution, ContainerLaunch, ExecutionNetwork};
use crate::{SchedulerError, SchedulerResult};

#[path = "supervisor_execution.rs"]
mod execution;
#[path = "supervisor_failures.rs"]
mod failures;
#[path = "supervisor_finalize.rs"]
mod finalize;
#[path = "supervisor_judgment.rs"]
mod judgment;
#[path = "supervisor_prepare.rs"]
mod prepare;
#[path = "supervisor_prompts.rs"]
mod prompts;
#[path = "supervisor_provision.rs"]
mod provision;
#[path = "supervisor_suggestion.rs"]
mod suggestion;
#[path = "supervisor_terminal.rs"]
pub mod terminal;
#[path = "supervisor_workspace.rs"]
pub mod workspace;

use execution::{ExecutionOutcome, capture_outputs};
use prepare::PreparedExecution;
use prompts::{
    execution_prompt, judgment_prompt, parse_judgment, parse_suggestion, suggestion_prompt,
};
use workspace::{
    WorkspaceDirectory, changed_workspace, evidence_references, image_digest,
    install_case_fixtures, internal, materialize_root, materialize_skills, safe_suffix,
    validate_config, workspace_state, write_private,
};

#[derive(Debug, Clone)]
pub struct EvaluatorSupervisorConfig {
    pub docker: PathBuf,
    pub workspace_root: PathBuf,
    pub environment: String,
    pub client_image: String,
    pub relay_image: String,
    pub relay_control_network: String,
    pub relay_upstream: String,
}

#[derive(Debug)]
pub struct EvaluatorSupervisor {
    pub(crate) config: EvaluatorSupervisorConfig,
    repositories: EvaluationRepositories,
}

#[derive(Debug, Clone, Copy)]
struct StageEvent<'a> {
    sequence: i64,
    stage: ExecutionStage,
    summary: &'a str,
}

impl EvaluatorSupervisor {
    pub fn new(
        repositories: &EvaluationRepositories,
        config: EvaluatorSupervisorConfig,
    ) -> SchedulerResult<Self> {
        validate_config(&config)?;
        Ok(Self {
            config,
            repositories: repositories.clone(),
        })
    }

    pub async fn run_once(
        &self,
        owner: &UserId,
        worker_id: &EvalWorkerId,
    ) -> SchedulerResult<bool> {
        let Some(mut run) = self.prepare_execution(owner, worker_id).await? else {
            return Ok(false);
        };
        let mut outcome = match self.run_client(&mut run).await {
            Ok(Some(outcome)) => outcome,
            Ok(None) => return Ok(true),
            Err(error) => {
                self.block_execution(
                    &run.worker.owner_id,
                    &run.lease,
                    run.record.variant_index,
                    &failures::diagnostic("client", &error),
                )
                .await?;
                return Ok(true);
            },
        };
        match self.run_judgment(&mut run, &mut outcome).await {
            Ok(judgment) => outcome.judgment = judgment,
            Err(error) => outcome.blocked = Some(failures::diagnostic("judge", &error)),
        }
        if outcome.blocked.is_none()
            && let Err(error) = self
                .generate_suggestion_if_needed(&mut run, &mut outcome)
                .await
        {
            outcome.blocked = Some(failures::diagnostic("suggestion", &error));
        }
        self.finalize_execution(run, outcome).await?;
        Ok(true)
    }

    async fn append_event(
        &self,
        worker: &WorkerRecord,
        lease: &ExecutionLease,
        event: StageEvent<'_>,
    ) -> SchedulerResult<()> {
        let event = ExecutionEvent::builder(event.sequence, event.stage)
            .summary(event.summary.to_owned())
            .build()
            .map_err(internal)?;
        self.repositories
            .events
            .append(worker, lease, &event)
            .await
            .map_err(internal)
    }

    async fn reconcile_owned_docker(&self, owner: &UserId) -> SchedulerResult<()> {
        for kind in ["container", "network"] {
            let owner_filter = format!("label=systemprompt.evaluator.owner={}", owner.as_str());
            let list_args = if kind == "container" {
                vec!["ps", "-aq", "--filter", owner_filter.as_str()]
            } else {
                vec!["network", "ls", "-q", "--filter", owner_filter.as_str()]
            };
            let (mut command, _docker_configuration) = super::docker::command(&self.config.docker)?;
            let output = command.args(&list_args).output()?;
            if !output.status.success() {
                return Err(SchedulerError::config_error(
                    "Unable to enumerate owned evaluator Docker objects",
                ));
            }
            for id in String::from_utf8(output.stdout)
                .map_err(internal)?
                .lines()
                .filter(|id| !id.is_empty())
            {
                let format = if kind == "container" {
                    "{{index .Config.Labels \"systemprompt.evaluator.execution\"}}"
                } else {
                    "{{index .Labels \"systemprompt.evaluator.execution\"}}"
                };
                let inspect_args = if kind == "container" {
                    vec!["inspect", "--format", format, id]
                } else {
                    vec!["network", "inspect", "--format", format, id]
                };
                let (mut command, _docker_configuration) =
                    super::docker::command(&self.config.docker)?;
                let inspected = command.args(inspect_args).output()?;
                if !inspected.status.success() {
                    return Err(SchedulerError::config_error(
                        "Unable to inspect owned evaluator Docker object",
                    ));
                }
                let execution = String::from_utf8(inspected.stdout)
                    .map_err(internal)?
                    .trim()
                    .to_owned();
                if execution.is_empty()
                    || !self
                        .repositories
                        .lifecycle
                        .execution_is_live(owner, &EvalExecutionId::new(execution))
                        .await
                        .map_err(internal)?
                {
                    let remove_args = if kind == "container" {
                        vec!["rm", "--force", id]
                    } else {
                        vec!["network", "rm", id]
                    };
                    let (mut command, _docker_configuration) =
                        super::docker::command(&self.config.docker)?;
                    let removed = command.args(remove_args).output()?;
                    if !removed.status.success() {
                        return Err(SchedulerError::config_error(
                            "Owned evaluator Docker cleanup was not acknowledged",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
