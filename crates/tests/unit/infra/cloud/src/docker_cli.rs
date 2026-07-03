//! Unit tests for DockerCli over a stubbed CommandRunner

use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{ExitStatus, Output};
use std::sync::{Arc, Mutex};

use systemprompt_cloud::{CommandRunner, CommandSpec, DockerCli, SystemCommandRunner};

#[derive(Debug, Clone)]
enum Call {
    Output(CommandSpec),
    Status(CommandSpec),
    StatusWithStdin(CommandSpec, Vec<u8>),
}

struct StubRunner {
    calls: Arc<Mutex<Vec<Call>>>,
    exit_code: i32,
    fail_io: bool,
}

impl StubRunner {
    fn new(exit_code: i32) -> (Self, Arc<Mutex<Vec<Call>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: Arc::clone(&calls),
                exit_code,
                fail_io: false,
            },
            calls,
        )
    }

    fn failing_io() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            exit_code: 0,
            fail_io: true,
        }
    }

    fn check_io(&self) -> io::Result<()> {
        if self.fail_io {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no docker binary"));
        }
        Ok(())
    }

    fn exit_status(&self) -> ExitStatus {
        ExitStatus::from_raw(self.exit_code << 8)
    }
}

impl CommandRunner for StubRunner {
    fn output(&self, spec: &CommandSpec) -> io::Result<Output> {
        self.check_io()?;
        self.calls.lock().unwrap().push(Call::Output(spec.clone()));
        Ok(Output {
            status: self.exit_status(),
            stdout: b"stub-stdout".to_vec(),
            stderr: Vec::new(),
        })
    }

    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus> {
        self.check_io()?;
        self.calls.lock().unwrap().push(Call::Status(spec.clone()));
        Ok(self.exit_status())
    }

    fn status_with_stdin(&self, spec: &CommandSpec, stdin: &[u8]) -> io::Result<ExitStatus> {
        self.check_io()?;
        self.calls
            .lock()
            .unwrap()
            .push(Call::StatusWithStdin(spec.clone(), stdin.to_vec()));
        Ok(self.exit_status())
    }
}

#[test]
fn test_build_image_invokes_docker_build_in_context_dir() {
    let (runner, calls) = StubRunner::new(0);
    let docker = DockerCli::with_runner(Box::new(runner));

    docker
        .build_image(Path::new("/proj"), Path::new("/proj/Dockerfile"), "img:1")
        .unwrap();

    let calls = calls.lock().unwrap();
    let Call::Status(spec) = &calls[0] else {
        panic!("expected a status call");
    };
    assert_eq!(spec.program, "docker");
    assert_eq!(
        spec.args,
        vec![
            "build",
            "--no-cache",
            "-f",
            "/proj/Dockerfile",
            "-t",
            "img:1",
            "."
        ]
    );
    assert_eq!(spec.current_dir.as_deref(), Some(Path::new("/proj")));
}

#[test]
fn test_build_image_failure_message() {
    let (runner, _calls) = StubRunner::new(1);
    let docker = DockerCli::with_runner(Box::new(runner));

    let err = docker
        .build_image(Path::new("/proj"), Path::new("/proj/Dockerfile"), "img:1")
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Command failed: docker build --no-cache -f /proj/Dockerfile -t img:1 ."
    );
}

#[test]
fn test_build_image_spawn_failure_message() {
    let docker = DockerCli::with_runner(Box::new(StubRunner::failing_io()));

    let err = docker
        .build_image(Path::new("/proj"), Path::new("/proj/Dockerfile"), "img:1")
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Failed to run: docker build --no-cache -f /proj/Dockerfile -t img:1 ."
    );
}

#[test]
fn test_login_pipes_token_to_stdin() {
    let (runner, calls) = StubRunner::new(0);
    let docker = DockerCli::with_runner(Box::new(runner));

    docker.login("registry.fly.io", "x", "tok-123").unwrap();

    let calls = calls.lock().unwrap();
    let Call::StatusWithStdin(spec, stdin) = &calls[0] else {
        panic!("expected a status-with-stdin call");
    };
    assert_eq!(
        spec.args,
        vec!["login", "registry.fly.io", "-u", "x", "--password-stdin"]
    );
    assert_eq!(stdin, b"tok-123");
}

#[test]
fn test_login_failure_message() {
    let (runner, _calls) = StubRunner::new(1);
    let docker = DockerCli::with_runner(Box::new(runner));

    let err = docker.login("registry.fly.io", "x", "tok").unwrap_err();
    assert_eq!(err.to_string(), "Docker login failed");
}

#[test]
fn test_push_failure_message() {
    let (runner, _calls) = StubRunner::new(1);
    let docker = DockerCli::with_runner(Box::new(runner));

    let err = docker.push("img:1").unwrap_err();
    assert_eq!(err.to_string(), "Docker push failed for image: img:1");
}

#[test]
fn test_push_spawn_failure_message() {
    let docker = DockerCli::with_runner(Box::new(StubRunner::failing_io()));

    let err = docker.push("img:1").unwrap_err();
    assert_eq!(err.to_string(), "failed to spawn `docker push img:1`");
}

#[test]
fn test_build_image_success_returns_ok() {
    let (runner, calls) = StubRunner::new(0);
    let docker = DockerCli::with_runner(Box::new(runner));

    docker
        .build_image(Path::new("/proj"), Path::new("/proj/Dockerfile"), "img:ok")
        .expect("build succeeds on exit 0");

    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn test_login_success_returns_ok() {
    let (runner, _calls) = StubRunner::new(0);
    let docker = DockerCli::with_runner(Box::new(runner));

    docker
        .login("registry.fly.io", "user", "tok")
        .expect("login succeeds on exit 0");
}

#[test]
fn test_push_success_returns_ok() {
    let (runner, calls) = StubRunner::new(0);
    let docker = DockerCli::with_runner(Box::new(runner));

    docker.push("img:pushok").expect("push succeeds on exit 0");

    let calls = calls.lock().unwrap();
    let Call::Status(spec) = &calls[0] else {
        panic!("expected a status call");
    };
    assert_eq!(spec.args, vec!["push", "img:pushok"]);
}

#[test]
fn test_command_spec_rendered_joins_program_and_args() {
    let spec = CommandSpec::docker(["ps", "-a"]);
    assert_eq!(spec.rendered(), "docker ps -a");
}

fn true_spec() -> CommandSpec {
    CommandSpec {
        program: "true".to_owned(),
        args: Vec::new(),
        current_dir: None,
    }
}

#[test]
fn test_system_runner_status_runs_real_process() {
    let runner = SystemCommandRunner;
    let status = runner
        .status(&true_spec())
        .expect("`true` spawns and exits");
    assert!(status.success());
}

#[test]
fn test_system_runner_output_captures_stdout() {
    let runner = SystemCommandRunner;
    let spec = CommandSpec {
        program: "printf".to_owned(),
        args: vec!["cov-marker".to_owned()],
        current_dir: None,
    };
    let output = runner.output(&spec).expect("`printf` spawns");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"cov-marker");
}

#[test]
fn test_system_runner_status_honours_current_dir() {
    let temp = tempfile::TempDir::new().unwrap();
    let runner = SystemCommandRunner;
    let spec = CommandSpec {
        program: "true".to_owned(),
        args: Vec::new(),
        current_dir: Some(temp.path().to_path_buf()),
    };
    let status = runner.status(&spec).expect("`true` spawns in cwd");
    assert!(status.success());
}

#[test]
fn test_system_runner_status_with_stdin_pipes_bytes() {
    let runner = SystemCommandRunner;
    let spec = CommandSpec {
        program: "cat".to_owned(),
        args: Vec::new(),
        current_dir: None,
    };
    let status = runner
        .status_with_stdin(&spec, b"piped-bytes")
        .expect("`cat` consumes stdin");
    assert!(status.success());
}

#[test]
fn test_raw_output_and_status_prefix_docker() {
    let (runner, calls) = StubRunner::new(0);
    let docker = DockerCli::with_runner(Box::new(runner));

    let output = docker.output(&["ps", "-q"]).unwrap();
    assert_eq!(output.stdout, b"stub-stdout");
    let status = docker.status(&["volume", "rm", "v"]).unwrap();
    assert!(status.success());

    let calls = calls.lock().unwrap();
    let Call::Output(spec) = &calls[0] else {
        panic!("expected an output call");
    };
    assert_eq!(spec.program, "docker");
    assert_eq!(spec.args, vec!["ps", "-q"]);
    let Call::Status(spec) = &calls[1] else {
        panic!("expected a status call");
    };
    assert_eq!(spec.args, vec!["volume", "rm", "v"]);
}
