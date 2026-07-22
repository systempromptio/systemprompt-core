//! Unit tests for the docker-backed tenant helpers of systemprompt-cli.
//!
//! Drives `cloud::tenant::docker::{container, database}` through a scripted
//! `CommandRunner` so the psql/compose invocations, exit-code handling, and
//! stdout parsing are exercised without spawning real `docker` processes.

use std::collections::VecDeque;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};
use std::sync::Mutex;

use systemprompt_cli::cloud::tenant::docker::{container, database};
use systemprompt_cloud::{CommandRunner, CommandSpec, DockerCli};

#[derive(Debug, Clone)]
enum Resp {
    Ok(i32, Vec<u8>),
    Io,
}

impl Resp {
    fn ok(code: i32) -> Self {
        Self::Ok(code, Vec::new())
    }

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
fn sanitize_database_name_replaces_unsafe_chars() {
    assert_eq!(
        database::sanitize_database_name("my-tenant.01"),
        "my_tenant_01"
    );
    assert_eq!(database::sanitize_database_name("keep_09"), "keep_09");
}

#[test]
fn create_database_creates_when_absent() {
    let docker = StubRunner::docker(vec![Resp::stdout(0, ""), Resp::ok(0)]);
    database::create_database_for_tenant(&docker, "pw", 5432, "tenant_1").unwrap();
}

#[test]
fn create_database_short_circuits_when_present() {
    let docker = StubRunner::docker(vec![Resp::stdout(0, "1")]);
    database::create_database_for_tenant(&docker, "pw", 5432, "tenant_1").unwrap();
}

#[test]
fn create_database_fails_on_nonzero_create() {
    let docker = StubRunner::docker(vec![Resp::stdout(0, ""), Resp::ok(1)]);
    let err = database::create_database_for_tenant(&docker, "pw", 5432, "tenant_1").unwrap_err();
    assert!(err.to_string().contains("Failed to create database"));
}

#[test]
fn create_database_surfaces_spawn_failure() {
    let docker = StubRunner::docker(vec![Resp::Io]);
    let err = database::create_database_for_tenant(&docker, "pw", 5432, "tenant_1").unwrap_err();
    assert!(err.to_string().contains("failed to run"));
}

#[test]
fn drop_database_succeeds() {
    let docker = StubRunner::docker(vec![Resp::ok(0), Resp::ok(0)]);
    database::drop_database_for_tenant(&docker, "pw", 5432, "tenant_1").unwrap();
}

#[test]
fn drop_database_ignores_terminate_error_then_fails_on_drop() {
    let docker = StubRunner::docker(vec![Resp::Io, Resp::ok(1)]);
    let err = database::drop_database_for_tenant(&docker, "pw", 5432, "tenant_1").unwrap_err();
    assert!(err.to_string().contains("Failed to drop database"));
}

#[test]
fn ensure_admin_role_alters_existing_role() {
    let docker = StubRunner::docker(vec![Resp::stdout(0, "1"), Resp::ok(0)]);
    database::ensure_admin_role(&docker, "pw").unwrap();
}

#[test]
fn ensure_admin_role_creates_missing_role() {
    let docker = StubRunner::docker(vec![Resp::stdout(0, ""), Resp::ok(0)]);
    database::ensure_admin_role(&docker, "pw").unwrap();
}

#[test]
fn ensure_admin_role_fails_when_alter_rejected() {
    let docker = StubRunner::docker(vec![Resp::stdout(0, "1"), Resp::ok(1)]);
    let err = database::ensure_admin_role(&docker, "pw").unwrap_err();
    assert!(err.to_string().contains("Failed to update password"));
}

#[test]
fn is_shared_container_running_reads_stdout() {
    let running = StubRunner::docker(vec![Resp::stdout(0, "abc123")]);
    assert!(container::is_shared_container_running(&running));

    let stopped = StubRunner::docker(vec![Resp::stdout(0, "  \n")]);
    assert!(!container::is_shared_container_running(&stopped));

    let broken = StubRunner::docker(vec![Resp::Io]);
    assert!(!container::is_shared_container_running(&broken));
}

#[test]
fn get_container_password_parses_env_line() {
    let docker = StubRunner::docker(vec![Resp::stdout(
        0,
        "PATH=/usr/bin\nPOSTGRES_PASSWORD=s3cret\nLANG=C\n",
    )]);
    assert_eq!(
        container::get_container_password(&docker),
        Some("s3cret".to_owned())
    );
}

#[test]
fn get_container_password_none_when_absent_or_failed() {
    let missing = StubRunner::docker(vec![Resp::stdout(0, "PATH=/usr/bin\n")]);
    assert_eq!(container::get_container_password(&missing), None);

    let non_success = StubRunner::docker(vec![Resp::stdout(1, "POSTGRES_PASSWORD=x\n")]);
    assert_eq!(container::get_container_password(&non_success), None);

    let broken = StubRunner::docker(vec![Resp::Io]);
    assert_eq!(container::get_container_password(&broken), None);
}

#[test]
fn check_volume_exists_reads_stdout() {
    let present = StubRunner::docker(vec![Resp::stdout(0, "systemprompt-postgres-shared-data")]);
    assert!(container::check_volume_exists(&present));

    let absent = StubRunner::docker(vec![Resp::stdout(0, "")]);
    assert!(!container::check_volume_exists(&absent));
}

#[test]
fn remove_shared_volume_maps_exit_code() {
    let ok = StubRunner::docker(vec![Resp::ok(0)]);
    container::remove_shared_volume(&ok).unwrap();

    let busy = StubRunner::docker(vec![Resp::ok(1)]);
    let err = container::remove_shared_volume(&busy).unwrap_err();
    assert!(err.to_string().contains("Failed to remove volume"));
}

#[tokio::test]
async fn wait_for_postgres_healthy_returns_on_healthy() {
    let docker = StubRunner::docker(vec![Resp::stdout(0, "healthy")]);
    container::wait_for_postgres_healthy(&docker, std::path::Path::new("/tmp/shared.yaml"), 5)
        .await
        .unwrap();
}


mod container_state {
    use super::{Resp, StubRunner};
    use systemprompt_cli::ScriptedPrompter;
    use systemprompt_cli::cloud::tenant::docker::SharedContainerConfig;
    use systemprompt_cli::cloud::tenant::{handle_orphaned_volume, resolve_container_state};

    fn config() -> SharedContainerConfig {
        SharedContainerConfig::new("pw".to_owned(), 5555)
    }

    #[test]
    fn existing_config_and_running_container_is_reused_without_start() {
        let docker = StubRunner::docker(vec![]);
        let prompter = ScriptedPrompter::new(Vec::<String>::new());

        let (resolved, needs_start) =
            resolve_container_state(&docker, Some(config()), true, &prompter).unwrap();

        assert!(!needs_start);
        assert_eq!(resolved.admin_password, "pw");
        assert_eq!(resolved.port, 5555);
    }

    #[test]
    fn existing_config_with_stopped_container_requests_start() {
        let docker = StubRunner::docker(vec![]);
        let prompter = ScriptedPrompter::new(Vec::<String>::new());

        let (_, needs_start) =
            resolve_container_state(&docker, Some(config()), false, &prompter).unwrap();

        assert!(needs_start);
    }

    #[test]
    fn unknown_running_container_declined_is_an_error() {
        let docker = StubRunner::docker(vec![]);
        let prompter = ScriptedPrompter::new(["n"]);

        let err = resolve_container_state(&docker, None, true, &prompter).unwrap_err();
        assert!(err.to_string().contains("stop the existing one"));
    }

    #[test]
    fn unknown_running_container_accepted_reads_password_from_inspect() {
        let docker = StubRunner::docker(vec![Resp::stdout(
            0,
            "POSTGRES_USER=admin\nPOSTGRES_PASSWORD=sekret\n",
        )]);
        let prompter = ScriptedPrompter::new(["y"]);

        let (resolved, needs_start) =
            resolve_container_state(&docker, None, true, &prompter).unwrap();

        assert!(!needs_start);
        assert_eq!(resolved.admin_password, "sekret");
    }

    #[test]
    fn unknown_running_container_without_password_is_an_error() {
        let docker = StubRunner::docker(vec![Resp::stdout(0, "POSTGRES_USER=admin\n")]);
        let prompter = ScriptedPrompter::new(["y"]);

        let err = resolve_container_state(&docker, None, true, &prompter).unwrap_err();
        assert!(err.to_string().contains("Could not retrieve password"));
    }

    #[test]
    fn fresh_environment_generates_password_and_starts() {
        let docker = StubRunner::docker(vec![Resp::stdout(0, "")]);
        let prompter = ScriptedPrompter::new(Vec::<String>::new());

        let (resolved, needs_start) =
            resolve_container_state(&docker, None, false, &prompter).unwrap();

        assert!(needs_start);
        assert!(!resolved.admin_password.is_empty());
    }

    #[test]
    fn orphaned_volume_absent_is_a_noop() {
        let docker = StubRunner::docker(vec![Resp::stdout(0, "")]);
        let prompter = ScriptedPrompter::new(Vec::<String>::new());

        handle_orphaned_volume(&docker, &prompter).unwrap();
    }

    #[test]
    fn orphaned_volume_reset_declined_is_an_error() {
        let docker = StubRunner::docker(vec![Resp::stdout(0, "vol-id\n")]);
        let prompter = ScriptedPrompter::new(["n"]);

        let err = handle_orphaned_volume(&docker, &prompter).unwrap_err();
        assert!(err.to_string().contains("docker volume rm"));
    }

    #[test]
    fn orphaned_volume_reset_accepted_removes_volume() {
        let docker = StubRunner::docker(vec![Resp::stdout(0, "vol-id\n"), Resp::ok(0)]);
        let prompter = ScriptedPrompter::new(["y"]);

        handle_orphaned_volume(&docker, &prompter).unwrap();
    }
}
