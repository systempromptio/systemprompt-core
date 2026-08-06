//! Unit tests for the docker-backed tenant helpers of systemprompt-cli.
//!
//! Drives `cloud::tenant::docker::container` through a scripted `CommandRunner`
//! so the compose invocations, exit-code handling, and stdout parsing are
//! exercised without spawning real `docker` processes.

use std::collections::VecDeque;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};
use std::sync::Mutex;

use systemprompt_cli::cloud::tenant::docker::{TenantContainer, container};
use systemprompt_cloud::{CommandRunner, CommandSpec, DockerCli};

#[derive(Debug, Clone)]
enum Resp {
    Ok(i32, Vec<u8>),
    Io,
}

impl Resp {
    fn stdout(code: i32, stdout: &str) -> Self {
        Self::Ok(code, stdout.as_bytes().to_vec())
    }
}

struct StubRunner {
    responses: Mutex<VecDeque<Resp>>,
    calls: Mutex<Vec<CommandSpec>>,
}

impl StubRunner {
    fn new(responses: Vec<Resp>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn docker(responses: Vec<Resp>) -> DockerCli {
        DockerCli::with_runner(Box::new(Self::new(responses)))
    }

    fn next(&self, spec: &CommandSpec) -> io::Result<Resp> {
        self.calls.lock().unwrap().push(spec.clone());
        match self.responses.lock().unwrap().pop_front() {
            Some(Resp::Io) => Err(io::Error::new(io::ErrorKind::NotFound, "no docker binary")),
            Some(resp) => Ok(resp),
            None => panic!("StubRunner ran out of scripted responses for {:?}", spec),
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

#[test]
fn database_url_names_the_projects_own_port_and_credentials() {
    let container = TenantContainer::new("acme_ff".to_owned(), "s3cret".to_owned(), 5433);

    assert_eq!(
        container.database_url(),
        "postgres://systemprompt:s3cret@localhost:5433/systemprompt"
    );
}

#[test]
fn compose_path_is_named_for_the_project() {
    let container = TenantContainer::new("acme_ff".to_owned(), "pw".to_owned(), 5432);

    assert_eq!(
        container.compose_path(),
        container::compose_path_for_project("acme_ff")
    );
}

#[test]
fn two_tenants_never_share_a_compose_file_or_port() {
    let a = TenantContainer::new("acme_01".to_owned(), "pw".to_owned(), 5432);
    let b = TenantContainer::new("acme_02".to_owned(), "pw".to_owned(), 5433);

    assert_ne!(a.compose_path(), b.compose_path());
    assert_ne!(a.database_url(), b.database_url());
}

#[test]
fn is_project_running_reads_stdout() {
    let running = StubRunner::docker(vec![Resp::stdout(0, "abc123")]);
    assert!(container::is_project_running(&running, "acme_ff"));

    let stopped = StubRunner::docker(vec![Resp::stdout(0, "  \n")]);
    assert!(!container::is_project_running(&stopped, "acme_ff"));

    let broken = StubRunner::docker(vec![Resp::Io]);
    assert!(!container::is_project_running(&broken, "acme_ff"));
}

#[test]
fn remove_project_is_a_noop_when_no_compose_file_exists() {
    let docker = StubRunner::docker(vec![]);

    container::remove_project(&docker, "project_that_was_never_created").unwrap();
}

#[tokio::test]
async fn wait_for_postgres_healthy_returns_on_healthy() {
    let docker = StubRunner::docker(vec![Resp::stdout(0, "healthy")]);
    container::wait_for_postgres_healthy(
        &docker,
        "acme_ff",
        std::path::Path::new("/tmp/acme_ff.yaml"),
        5,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn wait_for_postgres_healthy_times_out_while_unhealthy() {
    let docker = StubRunner::docker(vec![
        Resp::stdout(0, "starting"),
        Resp::stdout(0, "starting"),
    ]);
    let err = container::wait_for_postgres_healthy(
        &docker,
        "acme_ff",
        std::path::Path::new("/tmp/acme_ff.yaml"),
        0,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("Timeout waiting for PostgreSQL"));
}
