//! Per-execution Docker network isolation and process cleanup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{PathBuf, SchedulerError, SchedulerResult, docker_command};
pub(super) use process::{docker_status, private_log, safe_label, safe_name, wait_bounded};

#[path = "network_process.rs"]
mod process;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct NetworkOwnership<'a> {
    owner: &'a str,
    execution: &'a str,
    worker: &'a str,
    fence: i64,
}

#[derive(Debug)]
pub struct ExecutionNetwork {
    docker: PathBuf,
    name: String,
    relay: Option<String>,
    removed: bool,
    owner_label: String,
    execution_label: String,
    worker_label: String,
    fence_label: i64,
}

impl ExecutionNetwork {
    pub fn create(
        docker: PathBuf,
        name: String,
        owner: &str,
        execution: &str,
    ) -> SchedulerResult<Self> {
        Self::create_owned(
            docker,
            name,
            NetworkOwnership {
                owner,
                execution,
                worker: "",
                fence: 0,
            },
        )
    }

    pub fn create_fenced(
        docker: PathBuf,
        name: String,
        owner: &str,
        lease: &systemprompt_evaluation::repository::experiments::ExecutionLease,
    ) -> SchedulerResult<Self> {
        Self::create_owned(
            docker,
            name,
            NetworkOwnership {
                owner,
                execution: lease.execution_id.as_str(),
                worker: lease.worker_id.as_str(),
                fence: lease.fencing_token,
            },
        )
    }

    fn create_owned(
        docker: PathBuf,
        name: String,
        ownership: NetworkOwnership<'_>,
    ) -> SchedulerResult<Self> {
        let NetworkOwnership {
            owner,
            execution,
            worker,
            fence,
        } = ownership;
        if !docker.is_absolute()
            || !safe_name(&name)
            || !safe_label(owner)
            || !safe_label(execution)
            || matches!(name.as_str(), "host" | "bridge" | "default" | "none")
        {
            return Err(SchedulerError::config_error(
                "Invalid evaluator network configuration",
            ));
        }
        docker_status(
            &docker,
            &[
                "network",
                "create",
                "--internal",
                "--label",
                "systemprompt.evaluator=true",
                "--label",
                &format!("systemprompt.evaluator.owner={owner}"),
                "--label",
                &format!("systemprompt.evaluator.execution={execution}"),
                "--label",
                &format!("systemprompt.evaluator.worker={worker}"),
                "--label",
                &format!("systemprompt.evaluator.fence={fence}"),
                &name,
            ],
        )?;
        let network = Self {
            docker,
            name,
            relay: None,
            removed: false,
            owner_label: owner.to_owned(),
            execution_label: execution.to_owned(),
            worker_label: worker.to_owned(),
            fence_label: fence,
        };
        network.verify(&[])?;
        Ok(network)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start_relay(
        &mut self,
        image: &str,
        control_network: &str,
        upstream: &str,
        name: String,
    ) -> SchedulerResult<()> {
        if !safe_name(control_network)
            || matches!(control_network, "host" | "bridge" | "default" | "none")
            || !safe_name(&name)
            || systemprompt_evaluation::capabilities::proofs::ImmutableImage::parse(image).is_err()
        {
            return Err(SchedulerError::config_error(
                "Relay requires pinned image and dedicated control network",
            ));
        }
        docker_status(
            &self.docker,
            &[
                "run",
                "-d",
                "--name",
                &name,
                "--label",
                "systemprompt.evaluator=true",
                "--label",
                &format!("systemprompt.evaluator.owner={}", self.owner_label),
                "--label",
                &format!("systemprompt.evaluator.execution={}", self.execution_label),
                "--label",
                &format!("systemprompt.evaluator.worker={}", self.worker_label),
                "--label",
                &format!("systemprompt.evaluator.fence={}", self.fence_label),
                "--network",
                &self.name,
                "--read-only",
                "--cap-drop=ALL",
                "--security-opt=no-new-privileges",
                "--pids-limit=64",
                "--memory=256m",
                "--cpus=.25",
                "--user=1002:1002",
                "-e",
                &format!("SYSTEMPROMPT_RELAY_UPSTREAM={upstream}"),
                image,
            ],
        )?;
        docker_status(
            &self.docker,
            &["network", "connect", control_network, &name],
        )?;
        self.relay = Some(name.clone());
        self.verify(&[name])
    }

    pub fn verify(&self, expected: &[String]) -> SchedulerResult<()> {
        let (mut command, _docker_configuration) = docker_command(&self.docker)?;
        let output = command.args(["network", "inspect", &self.name]).output()?;
        if !output.status.success() {
            return Err(SchedulerError::config_error(
                "Execution network inspection failed",
            ));
        }
        let inspected: Vec<NetworkInspect> = serde_json::from_slice(&output.stdout)
            .map_err(|error| SchedulerError::Internal(error.to_string()))?;
        let network = inspected.into_iter().next().ok_or_else(|| {
            SchedulerError::config_error("Execution network inspection returned no network")
        })?;
        if !network.internal {
            return Err(SchedulerError::config_error(
                "Execution network is not internal",
            ));
        }
        let mut actual = network
            .containers
            .into_values()
            .map(|container| container.name)
            .collect::<Vec<_>>();
        let mut expected = expected.to_vec();
        actual.sort();
        expected.sort();
        if actual != expected {
            return Err(SchedulerError::config_error(
                "Execution network has unexpected members",
            ));
        }
        Ok(())
    }

    pub fn cleanup(&mut self) -> SchedulerResult<()> {
        if let Some(relay) = self.relay.take() {
            docker_status(&self.docker, &["rm", "--force", &relay])?;
        }
        docker_status(&self.docker, &["network", "rm", &self.name])?;
        self.removed = true;
        Ok(())
    }
}

// JSON: shape of `docker network inspect`; a missing key is a shape drift, not
// an empty network, so nothing here defaults.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NetworkInspect {
    internal: bool,
    containers: BTreeMap<String, NetworkMember>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NetworkMember {
    name: String,
}

impl Drop for ExecutionNetwork {
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        if let Some(relay) = self.relay.take()
            && let Err(error) = docker_status(&self.docker, &["rm", "--force", &relay])
        {
            tracing::error!(relay = %relay, %error, "evaluator relay container leaked; reconciliation must remove it");
        }
        if let Err(error) = docker_status(&self.docker, &["network", "rm", &self.name]) {
            tracing::error!(network = %self.name, %error, "evaluator network leaked; reconciliation must remove it");
        }
    }
}
