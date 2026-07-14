//! End-to-end `CrateDeployService::deploy` flow driven through the
//! `CommandRunner` seam plus wiremock for the cloud API. Each test creates its
//! own temp project root and changes the process cwd, which is safe under
//! nextest's process-per-test model.

use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};
use std::sync::{Arc, Mutex};
use std::{fs, io};

use serde_json::json;
use systemprompt_cloud::{CommandRunner, CommandSpec};
use systemprompt_identifiers::TenantId;
use systemprompt_sync::crate_deploy::CrateDeployService;
use systemprompt_sync::{SyncApiClient, SyncConfig, SyncError};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Default)]
struct StubRunner {
    calls: Arc<Mutex<Vec<String>>>,
    stdin_payloads: Arc<Mutex<Vec<Vec<u8>>>>,
    git_stdout: Vec<u8>,
    fail_prefix: Option<String>,
    spawn_fail_prefix: Option<String>,
    login_fails: bool,
}

impl StubRunner {
    fn exit(code: i32) -> ExitStatus {
        ExitStatus::from_raw(code << 8)
    }

    fn check_spawn(&self, rendered: &str) -> io::Result<()> {
        if let Some(prefix) = &self.spawn_fail_prefix
            && rendered.starts_with(prefix.as_str())
        {
            return Err(io::Error::new(io::ErrorKind::NotFound, "missing binary"));
        }
        Ok(())
    }
}

impl CommandRunner for StubRunner {
    fn output(&self, spec: &CommandSpec) -> io::Result<Output> {
        let rendered = spec.rendered();
        self.check_spawn(&rendered)?;
        self.calls.lock().unwrap().push(rendered);
        Ok(Output {
            status: Self::exit(0),
            stdout: self.git_stdout.clone(),
            stderr: Vec::new(),
        })
    }

    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus> {
        let rendered = spec.rendered();
        self.check_spawn(&rendered)?;
        self.calls.lock().unwrap().push(rendered.clone());
        let code = match &self.fail_prefix {
            Some(prefix) if rendered.starts_with(prefix.as_str()) => 1,
            _ => 0,
        };
        Ok(Self::exit(code))
    }

    fn status_with_stdin(&self, spec: &CommandSpec, stdin: &[u8]) -> io::Result<ExitStatus> {
        self.calls.lock().unwrap().push(spec.rendered());
        self.stdin_payloads.lock().unwrap().push(stdin.to_vec());
        Ok(Self::exit(i32::from(self.login_fails)))
    }
}

fn project_root_cwd() -> TempDir {
    let temp = TempDir::new().expect("tempdir");
    fs::create_dir_all(temp.path().join("infrastructure")).expect("infrastructure dir");
    std::env::set_current_dir(temp.path()).expect("chdir");
    temp
}

fn config(api_url: &str) -> SyncConfig {
    SyncConfig::builder(TenantId::new("t-crate"), api_url, "tok", "/services").build()
}

async fn mount_cloud_mocks(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t-crate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "fly_app_name": "appx" })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t-crate/registry-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "registry": "registry.fly.io",
            "username": "flyuser",
            "token": "registry-secret"
        })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/t-crate/deploy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "queued",
            "app_url": "https://appx.fly.dev"
        })))
        .mount(server)
        .await;
}

fn service_with(server_uri: &str, runner: StubRunner) -> CrateDeployService {
    let client = SyncApiClient::new(server_uri, "tok").expect("client");
    CrateDeployService::new(config(server_uri), client).with_runner(Box::new(runner))
}

#[tokio::test]
async fn deploy_with_custom_tag_skips_build_and_pushes() {
    let _cwd = project_root_cwd();
    let server = MockServer::start().await;
    mount_cloud_mocks(&server).await;

    let runner = StubRunner::default();
    let calls = Arc::clone(&runner.calls);
    let stdin = Arc::clone(&runner.stdin_payloads);
    let service = service_with(&server.uri(), runner);

    let result = service
        .deploy(true, Some("v9".to_owned()))
        .await
        .expect("deploy");

    assert!(result.success);
    assert_eq!(result.operation, "crate_deploy");
    assert_eq!(result.items_synced, 1);
    let details = result.details.expect("details");
    assert_eq!(details["image"], "registry.fly.io/appx:v9");
    assert_eq!(details["status"], "queued");
    assert_eq!(details["app_url"], "https://appx.fly.dev");

    let calls = calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 3);
    assert!(
        calls[0].starts_with(
            "docker build -f infrastructure/docker/app.Dockerfile -t registry.fly.io/appx:v9"
        ),
        "unexpected build call: {}",
        calls[0]
    );
    assert_eq!(
        calls[1],
        "docker login registry.fly.io -u flyuser --password-stdin"
    );
    assert_eq!(calls[2], "docker push registry.fly.io/appx:v9");
    assert_eq!(*stdin.lock().unwrap(), vec![b"registry-secret".to_vec()]);
}

#[tokio::test]
async fn deploy_without_custom_tag_builds_release_and_uses_git_sha() {
    let _cwd = project_root_cwd();
    let server = MockServer::start().await;
    mount_cloud_mocks(&server).await;

    let runner = StubRunner {
        git_stdout: b"abc123\n".to_vec(),
        ..StubRunner::default()
    };
    let calls = Arc::clone(&runner.calls);
    let service = service_with(&server.uri(), runner);

    let result = service.deploy(false, None).await.expect("deploy");

    let details = result.details.expect("details");
    let image = details["image"].as_str().expect("image string");
    assert!(image.starts_with("registry.fly.io/appx:deploy-"), "{image}");
    assert!(image.ends_with("-abc123"), "{image}");

    let calls = calls.lock().unwrap().clone();
    assert_eq!(calls[0], "git rev-parse --short HEAD");
    assert_eq!(
        calls[1],
        "cargo build --release --manifest-path=core/Cargo.toml --bin systemprompt"
    );
    assert!(calls[2].starts_with("docker build"));
}

#[tokio::test]
async fn deploy_outside_project_root_is_rejected() {
    let temp = TempDir::new().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("chdir");

    let service = service_with("http://127.0.0.1:1", StubRunner::default());
    let err = service.deploy(true, None).await.expect_err("must fail");
    assert!(matches!(err, SyncError::NotProjectRoot));
}

#[tokio::test]
async fn deploy_maps_non_utf8_git_output_to_sha_unavailable() {
    let _cwd = project_root_cwd();
    let server = MockServer::start().await;
    mount_cloud_mocks(&server).await;

    let runner = StubRunner {
        git_stdout: vec![0xff, 0xfe],
        ..StubRunner::default()
    };
    let service = service_with(&server.uri(), runner);

    let err = service.deploy(true, None).await.expect_err("must fail");
    assert!(matches!(err, SyncError::GitShaUnavailable));
}

#[tokio::test]
async fn deploy_surfaces_release_build_failure_with_command() {
    let _cwd = project_root_cwd();
    let server = MockServer::start().await;
    mount_cloud_mocks(&server).await;

    let runner = StubRunner {
        fail_prefix: Some("cargo build".to_owned()),
        ..StubRunner::default()
    };
    let service = service_with(&server.uri(), runner);

    let err = service
        .deploy(false, Some("v1".to_owned()))
        .await
        .expect_err("must fail");
    match err {
        SyncError::CommandFailed { command } => {
            assert!(command.starts_with("cargo build --release"), "{command}");
        },
        other => panic!("expected CommandFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn deploy_surfaces_docker_login_failure() {
    let _cwd = project_root_cwd();
    let server = MockServer::start().await;
    mount_cloud_mocks(&server).await;

    let runner = StubRunner {
        login_fails: true,
        ..StubRunner::default()
    };
    let service = service_with(&server.uri(), runner);

    let err = service
        .deploy(true, Some("v1".to_owned()))
        .await
        .expect_err("must fail");
    assert!(matches!(err, SyncError::DockerLoginFailed));
}

#[tokio::test]
async fn deploy_maps_spawn_failure_to_command_spawn_failed() {
    let _cwd = project_root_cwd();
    let server = MockServer::start().await;
    mount_cloud_mocks(&server).await;

    let runner = StubRunner {
        spawn_fail_prefix: Some("docker build".to_owned()),
        ..StubRunner::default()
    };
    let service = service_with(&server.uri(), runner);

    let err = service
        .deploy(true, Some("v1".to_owned()))
        .await
        .expect_err("must fail");
    match err {
        SyncError::CommandSpawnFailed { command, .. } => {
            assert!(command.starts_with("docker build"), "{command}");
        },
        other => panic!("expected CommandSpawnFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn deploy_without_app_reports_tenant_no_app() {
    let _cwd = project_root_cwd();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t-crate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "fly_app_name": null })))
        .mount(&server)
        .await;

    let service = service_with(&server.uri(), StubRunner::default());
    let err = service
        .deploy(true, Some("v1".to_owned()))
        .await
        .expect_err("must fail");
    assert!(matches!(err, SyncError::TenantNoApp));
}
