//! Tests for the local-tenant shared-container state machine.
//!
//! Drives `resolve_container_state` and `handle_orphaned_volume` over every
//! (config, running) combination through a scripted docker runner and a
//! scripted prompter, so the reuse, restart, adopt, and orphaned-volume
//! branches are exercised without a real docker daemon.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::VecDeque;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};
use std::sync::Mutex;

use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::cloud::tenant::create::{handle_orphaned_volume, resolve_container_state};
use systemprompt_cli::cloud::tenant::docker::SharedContainerConfig;
use systemprompt_cloud::{CommandRunner, CommandSpec, DockerCli};

#[derive(Debug, Clone)]
enum Resp {
    Ok(i32, Vec<u8>),
    Io,
}

struct StubRunner {
    responses: Mutex<VecDeque<Resp>>,
}

impl StubRunner {
    fn docker(responses: Vec<Resp>) -> DockerCli {
        DockerCli::with_runner(Box::new(Self {
            responses: Mutex::new(responses.into_iter().collect()),
        }))
    }

    fn next(&self, spec: &CommandSpec) -> io::Result<Resp> {
        match self.responses.lock().unwrap().pop_front() {
            Some(Resp::Io) => Err(io::Error::new(io::ErrorKind::NotFound, "no docker binary")),
            Some(resp) => Ok(resp),
            None => panic!("StubRunner ran out of scripted responses for {spec:?}"),
        }
    }
}

fn exit(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

impl CommandRunner for StubRunner {
    fn output(&self, spec: &CommandSpec) -> io::Result<Output> {
        let Resp::Ok(code, stdout) = self.next(spec)? else {
            unreachable!("Io handled in next")
        };
        Ok(Output {
            status: exit(code),
            stdout,
            stderr: Vec::new(),
        })
    }

    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus> {
        let Resp::Ok(code, _) = self.next(spec)? else {
            unreachable!("Io handled in next")
        };
        Ok(exit(code))
    }

    fn status_with_stdin(&self, spec: &CommandSpec, _stdin: &[u8]) -> io::Result<ExitStatus> {
        let Resp::Ok(code, _) = self.next(spec)? else {
            unreachable!("Io handled in next")
        };
        Ok(exit(code))
    }
}

fn stored_config() -> SharedContainerConfig {
    SharedContainerConfig::new("stored-password".to_owned(), 6543)
}

#[test]
fn running_container_with_config_is_reused_without_restart() {
    let docker = StubRunner::docker(Vec::new());
    let prompter = ScriptedPrompter::new(Vec::<String>::new());

    let (config, needs_start) =
        resolve_container_state(&docker, Some(stored_config()), true, &prompter).unwrap();

    assert_eq!(config.admin_password, "stored-password");
    assert_eq!(config.port, 6543);
    assert!(!needs_start);
}

#[test]
fn stopped_container_with_config_is_restarted_with_stored_credentials() {
    let docker = StubRunner::docker(Vec::new());
    let prompter = ScriptedPrompter::new(Vec::<String>::new());

    let (config, needs_start) =
        resolve_container_state(&docker, Some(stored_config()), false, &prompter).unwrap();

    assert_eq!(config.admin_password, "stored-password");
    assert!(needs_start);
}

#[test]
fn unmanaged_running_container_is_adopted_with_its_own_password() {
    let docker = StubRunner::docker(vec![Resp::Ok(
        0,
        b"PGDATA=/data\nPOSTGRES_PASSWORD=adopted-pw\n".to_vec(),
    )]);
    let prompter = ScriptedPrompter::new(["yes"]);

    let (config, needs_start) = resolve_container_state(&docker, None, true, &prompter).unwrap();

    assert_eq!(config.admin_password, "adopted-pw");
    assert_eq!(config.port, 5432);
    assert!(!needs_start);
}

#[test]
fn unmanaged_running_container_without_password_is_an_error() {
    let docker = StubRunner::docker(vec![Resp::Ok(0, b"PGDATA=/data\n".to_vec())]);
    let prompter = ScriptedPrompter::new(["yes"]);

    let err = resolve_container_state(&docker, None, true, &prompter).unwrap_err();
    assert!(err.to_string().contains("Could not retrieve password"));
}

#[test]
fn declining_to_reuse_a_container_reports_the_removal_command() {
    let docker = StubRunner::docker(Vec::new());
    let prompter = ScriptedPrompter::new(["no"]);

    let err = resolve_container_state(&docker, None, true, &prompter).unwrap_err();
    assert!(err.to_string().contains("systemprompt-postgres-shared"));
}

#[test]
fn fresh_install_generates_a_password_and_requests_a_start() {
    let docker = StubRunner::docker(vec![Resp::Ok(0, Vec::new())]);
    let prompter = ScriptedPrompter::new(Vec::<String>::new());

    let (config, needs_start) = resolve_container_state(&docker, None, false, &prompter).unwrap();

    assert!(!config.admin_password.is_empty());
    assert_eq!(config.port, 5432);
    assert!(config.tenant_databases.is_empty());
    assert!(needs_start);
}

#[test]
fn orphaned_volume_is_removed_when_the_reset_is_confirmed() {
    let docker = StubRunner::docker(vec![
        Resp::Ok(0, b"systemprompt-postgres-shared-data\n".to_vec()),
        Resp::Ok(0, Vec::new()),
    ]);
    let prompter = ScriptedPrompter::new(["yes"]);

    handle_orphaned_volume(&docker, &prompter).unwrap();
}

#[test]
fn orphaned_volume_removal_failure_is_surfaced() {
    let docker = StubRunner::docker(vec![
        Resp::Ok(0, b"systemprompt-postgres-shared-data\n".to_vec()),
        Resp::Ok(1, Vec::new()),
    ]);
    let prompter = ScriptedPrompter::new(["yes"]);

    let err = handle_orphaned_volume(&docker, &prompter).unwrap_err();
    assert!(err.to_string().contains("Failed to remove volume"));
}

#[test]
fn keeping_an_orphaned_volume_blocks_container_creation() {
    let docker = StubRunner::docker(vec![Resp::Ok(
        0,
        b"systemprompt-postgres-shared-data\n".to_vec(),
    )]);
    let prompter = ScriptedPrompter::new(["no"]);

    let err = handle_orphaned_volume(&docker, &prompter).unwrap_err();
    assert!(err.to_string().contains("docker volume rm"));
}

#[test]
fn docker_spawn_failure_is_treated_as_no_volume() {
    let docker = StubRunner::docker(vec![Resp::Io]);
    let prompter = ScriptedPrompter::new(Vec::<String>::new());

    handle_orphaned_volume(&docker, &prompter).unwrap();
}
