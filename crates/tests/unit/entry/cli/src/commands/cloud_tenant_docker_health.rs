//! Tests for the shared-container health wait.
//!
//! `wait_for_postgres_healthy` polls `docker compose ps` until the health
//! column reports healthy or the deadline passes; a scripted runner drives
//! both outcomes without a daemon.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::VecDeque;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};
use std::sync::Mutex;

use systemprompt_cli::cloud::tenant::docker::container::wait_for_postgres_healthy;
use systemprompt_cloud::{CommandRunner, CommandSpec, DockerCli};

struct ScriptedHealth {
    outputs: Mutex<VecDeque<Option<String>>>,
}

impl ScriptedHealth {
    fn docker(outputs: Vec<Option<&str>>) -> DockerCli {
        DockerCli::with_runner(Box::new(Self {
            outputs: Mutex::new(outputs.into_iter().map(|o| o.map(str::to_owned)).collect()),
        }))
    }

    fn next(&self) -> io::Result<Option<String>> {
        Ok(self
            .outputs
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Some("starting".to_owned())))
    }
}

impl CommandRunner for ScriptedHealth {
    fn output(&self, _spec: &CommandSpec) -> io::Result<Output> {
        match self.next()? {
            Some(health) => Ok(Output {
                status: ExitStatus::from_raw(0),
                stdout: health.into_bytes(),
                stderr: Vec::new(),
            }),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "no docker binary")),
        }
    }

    fn status(&self, _spec: &CommandSpec) -> io::Result<ExitStatus> {
        Ok(ExitStatus::from_raw(0))
    }

    fn status_with_stdin(&self, _spec: &CommandSpec, _stdin: &[u8]) -> io::Result<ExitStatus> {
        Ok(ExitStatus::from_raw(0))
    }
}

#[tokio::test]
async fn a_healthy_container_returns_immediately() {
    let docker = ScriptedHealth::docker(vec![Some("healthy")]);

    wait_for_postgres_healthy(&docker, std::path::Path::new("/tmp/shared.yaml"), 30)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_container_that_never_reports_healthy_times_out() {
    let docker = ScriptedHealth::docker(vec![Some("starting"), Some("starting")]);

    let err = wait_for_postgres_healthy(&docker, std::path::Path::new("/tmp/shared.yaml"), 0)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("Timeout waiting for PostgreSQL"));
    assert!(err.to_string().contains("/tmp/shared.yaml"));
}

#[tokio::test]
async fn a_docker_spawn_failure_is_surfaced() {
    let docker = ScriptedHealth::docker(vec![None]);

    let err = wait_for_postgres_healthy(&docker, std::path::Path::new("/tmp/shared.yaml"), 30)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("Failed to check container health"));
}
