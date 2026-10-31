use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};

use systemprompt_cloud::docker::{image_repository, select_repo_digest};
use systemprompt_cloud::{CloudError, CommandRunner, CommandSpec, DockerCli};

struct Scripted {
    docker: Result<(i32, &'static str, &'static str), &'static str>,
    which: Result<(i32, &'static str), &'static str>,
}

fn output(code: i32, stdout: &str, stderr: &str) -> Output {
    Output {
        status: ExitStatus::from_raw(code << 8),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

impl CommandRunner for Scripted {
    fn output(&self, spec: &CommandSpec) -> io::Result<Output> {
        if spec.program == "docker" {
            self.docker
                .map(|(code, out, err)| output(code, out, err))
                .map_err(io::Error::other)
        } else {
            self.which
                .map(|(code, out)| output(code, out, ""))
                .map_err(io::Error::other)
        }
    }

    fn status(&self, _spec: &CommandSpec) -> io::Result<ExitStatus> {
        Ok(ExitStatus::from_raw(0))
    }

    fn status_with_stdin(&self, _spec: &CommandSpec, _stdin: &[u8]) -> io::Result<ExitStatus> {
        Ok(ExitStatus::from_raw(0))
    }
}

fn cli(
    docker: Result<(i32, &'static str, &'static str), &'static str>,
    which: Result<(i32, &'static str), &'static str>,
) -> DockerCli {
    DockerCli::with_runner(Box::new(Scripted { docker, which }))
}

fn message(error: CloudError) -> String {
    match error {
        CloudError::Docker { message, .. } => message,
        other => panic!("expected a docker error, got {other:?}"),
    }
}

#[test]
fn image_repository_strips_tag_and_digest_but_keeps_registry_port() {
    assert_eq!(image_repository("ghcr.io/acme/app:1.2"), "ghcr.io/acme/app");
    assert_eq!(
        image_repository("ghcr.io/acme/app@sha256:ab"),
        "ghcr.io/acme/app"
    );
    assert_eq!(image_repository("localhost:5000/app"), "localhost:5000/app");
    assert_eq!(image_repository("app"), "app");
}

#[test]
fn select_repo_digest_picks_the_entry_of_the_images_repository() {
    let digests = vec![
        "other.io/app@sha256:111".to_owned(),
        "malformed".to_owned(),
        "ghcr.io/acme/app@sha256:222".to_owned(),
    ];
    assert_eq!(
        select_repo_digest("ghcr.io/acme/app:latest", &digests).as_deref(),
        Some("sha256:222")
    );
    assert_eq!(select_repo_digest("ghcr.io/acme/none:1", &digests), None);
}

#[test]
fn repo_digest_returns_the_pushed_digest() {
    let docker = cli(
        Ok((0, r#"["ghcr.io/acme/app@sha256:abc"]"#, "")),
        Ok((0, "")),
    );
    assert_eq!(
        docker.repo_digest("ghcr.io/acme/app:v1").unwrap(),
        "sha256:abc"
    );
}

#[test]
fn repo_digest_refuses_an_unpushed_image() {
    let docker = cli(Ok((0, "[]", "")), Ok((0, "")));
    let text = message(docker.repo_digest("ghcr.io/acme/app:v1").unwrap_err());
    assert!(text.contains("has no registry digest"), "{text}");
}

#[test]
fn repo_digest_reports_a_failed_inspect_with_stderr() {
    let docker = cli(Ok((1, "", "No such image")), Ok((0, "")));
    let text = message(docker.repo_digest("app:v1").unwrap_err());
    assert!(text.contains("No such image"), "{text}");
    assert!(text.contains("docker image inspect"), "{text}");
}

#[test]
fn repo_digest_reports_a_spawn_failure_and_bad_json() {
    let spawn = cli(Err("no docker"), Ok((0, "")));
    assert!(matches!(
        spawn.repo_digest("app").unwrap_err(),
        CloudError::Docker {
            source: Some(_),
            ..
        }
    ));
    let garbage = cli(Ok((0, "not json", "")), Ok((0, "")));
    assert!(matches!(
        garbage.repo_digest("app").unwrap_err(),
        CloudError::Json(_)
    ));
}

#[test]
fn preflight_returns_the_server_version() {
    let docker = cli(Ok((0, " 27.1.0\n", "")), Ok((0, "")));
    assert_eq!(docker.preflight_daemon().unwrap(), "27.1.0");
}

#[test]
fn preflight_names_the_resolved_binary_when_the_daemon_is_down() {
    let docker = cli(Ok((1, "", "")), Ok((0, "/usr/local/bin/docker\n")));
    let text = message(docker.preflight_daemon().unwrap_err());
    assert!(text.contains("no output"), "{text}");
    assert!(
        text.contains("docker binary: /usr/local/bin/docker"),
        "{text}"
    );
}

#[test]
fn preflight_reports_a_docker_binary_missing_from_path() {
    let empty = cli(Ok((1, "", "refused")), Ok((0, "  ")));
    assert!(message(empty.preflight_daemon().unwrap_err()).contains("docker binary: not on PATH"));
    let failed = cli(Ok((1, "", "refused")), Ok((1, "")));
    assert!(message(failed.preflight_daemon().unwrap_err()).contains("docker binary: not on PATH"));
    let spawn = cli(Err("down"), Err("no sh"));
    let text = message(spawn.preflight_daemon().unwrap_err());
    assert!(text.contains("unresolved (no sh)"), "{text}");
    assert!(text.contains("down"), "{text}");
}
