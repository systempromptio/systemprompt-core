//! Wiremock + stubbed-docker tests for `DeployOrchestrator`: step/event
//! ordering, skip-push, dry-run, pre-sync skip semantics, and the
//! hostname guard.

use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{ExitStatus, Output};
use std::sync::{Arc, Mutex};
use std::{fs, io};

use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::json;
use systemprompt_cloud::{CloudCredentials, CommandRunner, CommandSpec, DockerCli, ProjectContext};
use systemprompt_identifiers::{CloudAuthToken, Email, TenantId};
use systemprompt_sync::deploy::{
    DeployEvent, DeployOptions, DeployOrchestrator, DeployOutcome, DeployProgress, DeployPrompt,
    DeployRequest, PreSyncOptions,
};
use systemprompt_sync::{SyncApiClient, SyncError, SyncResult};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct StubRunner {
    calls: Arc<Mutex<Vec<String>>>,
}

impl StubRunner {
    fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: Arc::clone(&calls),
            },
            calls,
        )
    }

    fn record(&self, spec: &CommandSpec) {
        let op = spec.args.first().cloned().unwrap_or_default();
        self.calls.lock().unwrap().push(op);
    }
}

impl CommandRunner for StubRunner {
    fn output(&self, spec: &CommandSpec) -> io::Result<Output> {
        self.record(spec);
        Ok(Output {
            status: ExitStatus::from_raw(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus> {
        self.record(spec);
        Ok(ExitStatus::from_raw(0))
    }

    fn status_with_stdin(&self, spec: &CommandSpec, _stdin: &[u8]) -> io::Result<ExitStatus> {
        self.record(spec);
        Ok(ExitStatus::from_raw(0))
    }
}

struct RecordingProgress {
    events: Mutex<Vec<String>>,
    confirm_response: bool,
}

impl RecordingProgress {
    const fn new(confirm_response: bool) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            confirm_response,
        }
    }

    fn labels(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

fn label(event: &DeployEvent<'_>) -> String {
    let debug = format!("{event:?}");
    debug
        .split([' ', '(', '{'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

impl DeployProgress for RecordingProgress {
    fn event(&self, event: &DeployEvent<'_>) {
        self.events.lock().unwrap().push(label(event));
    }

    fn confirm(&self, _prompt: &DeployPrompt) -> SyncResult<bool> {
        self.events.lock().unwrap().push("Confirm".to_owned());
        Ok(self.confirm_response)
    }
}

fn scaffold_project(profile: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("target/release")).unwrap();
    fs::write(root.join("target/release/systemprompt"), b"binary").unwrap();
    fs::create_dir_all(root.join("storage/files")).unwrap();
    fs::create_dir_all(root.join("services/web/templates")).unwrap();
    fs::create_dir_all(root.join("services/config")).unwrap();
    fs::write(root.join("services/config/config.yaml"), "{}").unwrap();

    let dockerfile = ProjectContext::new(root.to_path_buf()).profile_dockerfile(profile);
    fs::create_dir_all(dockerfile.parent().unwrap()).unwrap();
    fs::write(
        &dockerfile,
        "FROM debian\nCOPY target/release/systemprompt /app/bin/\n",
    )
    .unwrap();
    temp
}

fn request(server_uri: &str, root: &Path, options: DeployOptions) -> DeployRequest {
    DeployRequest {
        tenant_id: TenantId::new("t-deploy"),
        tenant_name: "demo".to_owned(),
        profile_name: "prod".to_owned(),
        project_root: root.to_path_buf(),
        credentials: CloudCredentials::new(
            CloudAuthToken::new("tok".to_owned()),
            server_uri.to_owned(),
            Email::new("dev@example.com".to_owned()),
        ),
        hostname: Some("demo.fly.dev".to_owned()),
        secrets_path: root.join("no-secrets.json"),
        signing_key_path: root.join("no-signing-key.pem"),
        options,
    }
}

async fn mount_deploy_mocks(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "tenant_bearer",
            "expires_in": 600
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-deploy/registry-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "registry": "registry.fly.io",
                "username": "x",
                "token": "registry-secret",
                "repository": "repo/app",
                "tag": "v1"
            }
        })))
        .mount(server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/tenants/t-deploy/secrets"))
        .respond_with(ResponseTemplate::new(204))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-deploy/deploy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "status": "deploying",
                "app_url": "https://demo.fly.dev"
            }
        })))
        .mount(server)
        .await;
}

fn orchestrator_with_stub() -> (DeployOrchestrator, Arc<Mutex<Vec<String>>>) {
    let (stub, calls) = StubRunner::new();
    let orchestrator =
        DeployOrchestrator::new().with_docker(DockerCli::with_runner(Box::new(stub)));
    (orchestrator, calls)
}

fn tarball_with_file(name: &str, content: &[u8]) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, name, content).unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

#[tokio::test]
async fn full_deploy_emits_steps_in_order() {
    let server = MockServer::start().await;
    mount_deploy_mocks(&server).await;
    let project = scaffold_project("prod");
    let (orchestrator, docker_calls) = orchestrator_with_stub();
    let progress = RecordingProgress::new(true);

    let req = request(
        &server.uri(),
        project.path(),
        DeployOptions {
            skip_push: false,
            dry_run: false,
            pre_sync: None,
        },
    );
    let report = orchestrator.deploy(&req, &progress).await.expect("deploy");

    assert_eq!(
        progress.labels(),
        vec![
            "ArtifactsResolved",
            "RegistryAuthStarted",
            "RegistryAuthFinished",
            "ImageResolved",
            "BuildStarted",
            "BuildFinished",
            "PushStarted",
            "PushFinished",
            "SecretsPhaseStarted",
            "SecretsFileMissing",
            "CredentialsSyncStarted",
            "CredentialsSynced",
            "ProfilePathConfigured",
            "DeployStarted",
            "Deployed",
        ]
    );
    assert_eq!(
        *docker_calls.lock().unwrap(),
        vec!["build", "login", "push"]
    );

    match report.outcome {
        DeployOutcome::Deployed {
            image,
            status,
            app_url,
        } => {
            assert_eq!(image, "registry.fly.io/repo/app:v1");
            assert_eq!(status, "deploying");
            assert_eq!(app_url.as_deref(), Some("https://demo.fly.dev"));
        },
        DeployOutcome::DryRun => panic!("expected a deployed outcome"),
    }
}

#[tokio::test]
async fn skip_push_builds_but_never_pushes() {
    let server = MockServer::start().await;
    mount_deploy_mocks(&server).await;
    let project = scaffold_project("prod");
    let (orchestrator, docker_calls) = orchestrator_with_stub();
    let progress = RecordingProgress::new(true);

    let req = request(
        &server.uri(),
        project.path(),
        DeployOptions {
            skip_push: true,
            dry_run: false,
            pre_sync: None,
        },
    );
    orchestrator.deploy(&req, &progress).await.expect("deploy");

    let labels = progress.labels();
    assert!(labels.contains(&"PushSkipped".to_owned()));
    assert!(!labels.contains(&"PushStarted".to_owned()));
    assert!(!labels.contains(&"PushFinished".to_owned()));
    assert_eq!(*docker_calls.lock().unwrap(), vec!["build"]);
}

#[tokio::test]
async fn dry_run_pre_sync_stops_before_build() {
    let server = MockServer::start().await;
    let tarball = tarball_with_file("agents/demo.yaml", b"name: demo\n");
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t-deploy/files"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
        .mount(&server)
        .await;

    let project = scaffold_project("prod");
    let (orchestrator, docker_calls) = orchestrator_with_stub();
    let sync_client = SyncApiClient::new(&server.uri(), "tok").expect("client");
    let orchestrator = orchestrator.with_sync_client(sync_client);
    let progress = RecordingProgress::new(false);

    let req = request(
        &server.uri(),
        project.path(),
        DeployOptions {
            skip_push: false,
            dry_run: true,
            pre_sync: Some(PreSyncOptions {
                no_sync: false,
                assume_yes: false,
            }),
        },
    );
    let report = orchestrator.deploy(&req, &progress).await.expect("deploy");

    assert_eq!(
        progress.labels(),
        vec!["PreSyncStarted", "SyncDryRunStarted", "SyncDryRunFinished"]
    );
    assert!(matches!(report.outcome, DeployOutcome::DryRun));
    assert!(docker_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn no_sync_flag_skips_pre_sync_and_still_deploys() {
    let server = MockServer::start().await;
    mount_deploy_mocks(&server).await;
    let project = scaffold_project("prod");
    let (orchestrator, _docker_calls) = orchestrator_with_stub();
    let progress = RecordingProgress::new(true);

    let req = request(
        &server.uri(),
        project.path(),
        DeployOptions {
            skip_push: false,
            dry_run: true,
            pre_sync: Some(PreSyncOptions {
                no_sync: true,
                assume_yes: false,
            }),
        },
    );
    let report = orchestrator.deploy(&req, &progress).await.expect("deploy");

    let labels = progress.labels();
    assert_eq!(
        labels.first().map(String::as_str),
        Some("PreSyncSkippedByFlag")
    );
    assert!(!labels.contains(&"Confirm".to_owned()));
    assert!(matches!(report.outcome, DeployOutcome::Deployed { .. }));
}

#[tokio::test]
async fn declined_confirmation_skips_sync_and_still_deploys() {
    let server = MockServer::start().await;
    mount_deploy_mocks(&server).await;
    let project = scaffold_project("prod");
    let (orchestrator, _docker_calls) = orchestrator_with_stub();
    let progress = RecordingProgress::new(false);

    let req = request(
        &server.uri(),
        project.path(),
        DeployOptions {
            skip_push: false,
            dry_run: false,
            pre_sync: Some(PreSyncOptions {
                no_sync: false,
                assume_yes: false,
            }),
        },
    );
    let report = orchestrator.deploy(&req, &progress).await.expect("deploy");

    let labels = progress.labels();
    assert_eq!(
        &labels[..3],
        ["PreSyncStarted", "Confirm", "PreSyncDeclined"]
    );
    assert!(matches!(report.outcome, DeployOutcome::Deployed { .. }));
}

#[tokio::test]
async fn pre_sync_without_hostname_fails_closed() {
    let server = MockServer::start().await;
    let project = scaffold_project("prod");
    let (orchestrator, docker_calls) = orchestrator_with_stub();
    let progress = RecordingProgress::new(true);

    let mut req = request(
        &server.uri(),
        project.path(),
        DeployOptions {
            skip_push: false,
            dry_run: false,
            pre_sync: Some(PreSyncOptions {
                no_sync: false,
                assume_yes: true,
            }),
        },
    );
    req.hostname = None;

    let err = orchestrator
        .deploy(&req, &progress)
        .await
        .expect_err("must fail without hostname");
    assert!(matches!(err, SyncError::HostnameNotConfigured));
    assert!(docker_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn present_secrets_file_and_signing_key_are_provisioned() {
    let server = MockServer::start().await;
    mount_deploy_mocks(&server).await;
    let project = scaffold_project("prod");
    fs::write(
        project.path().join("secrets.json"),
        json!({"anthropic": "sk-ant", "my_custom": "v1"}).to_string(),
    )
    .unwrap();
    fs::write(project.path().join("signing-key.pem"), "PEM-BYTES").unwrap();

    let (orchestrator, _docker_calls) = orchestrator_with_stub();
    let progress = RecordingProgress::new(true);

    let mut req = request(
        &server.uri(),
        project.path(),
        DeployOptions {
            skip_push: false,
            dry_run: false,
            pre_sync: None,
        },
    );
    req.secrets_path = project.path().join("secrets.json");
    req.signing_key_path = project.path().join("signing-key.pem");

    let report = orchestrator.deploy(&req, &progress).await.expect("deploy");
    assert!(matches!(report.outcome, DeployOutcome::Deployed { .. }));

    let labels = progress.labels();
    assert!(labels.contains(&"SecretsSyncStarted".to_owned()));
    assert!(labels.contains(&"SecretsSynced".to_owned()));
    assert!(!labels.contains(&"SecretsFileMissing".to_owned()));

    let requests = server.received_requests().await.unwrap();
    let secret_bodies: Vec<serde_json::Value> = requests
        .iter()
        .filter(|r| r.url.path().ends_with("/secrets"))
        .map(|r| r.body_json().unwrap())
        .collect();
    let env_body = &secret_bodies[0]["secrets"];
    assert_eq!(env_body["ANTHROPIC_API_KEY"], "sk-ant");
    assert_eq!(env_body["MY_CUSTOM"], "v1");
    assert_eq!(env_body["SYSTEMPROMPT_CUSTOM_SECRETS"], "MY_CUSTOM");
    let pem = env_body["SIGNING_KEY_PEM"].as_str().unwrap();
    assert!(!pem.is_empty());
    assert_ne!(pem, "PEM-BYTES");
}

#[tokio::test]
async fn secrets_json_signing_key_wins_over_pem_file() {
    let server = MockServer::start().await;
    mount_deploy_mocks(&server).await;
    let project = scaffold_project("prod");
    fs::write(
        project.path().join("secrets.json"),
        json!({"signing_key_pem": "from-secrets"}).to_string(),
    )
    .unwrap();
    fs::write(project.path().join("signing-key.pem"), "from-file").unwrap();

    let (orchestrator, _docker_calls) = orchestrator_with_stub();
    let progress = RecordingProgress::new(true);

    let mut req = request(
        &server.uri(),
        project.path(),
        DeployOptions {
            skip_push: false,
            dry_run: false,
            pre_sync: None,
        },
    );
    req.secrets_path = project.path().join("secrets.json");
    req.signing_key_path = project.path().join("signing-key.pem");

    orchestrator.deploy(&req, &progress).await.expect("deploy");

    let requests = server.received_requests().await.unwrap();
    let env_body: serde_json::Value = requests
        .iter()
        .find(|r| r.url.path().ends_with("/secrets"))
        .map(|r| r.body_json().unwrap())
        .unwrap();
    assert_eq!(env_body["secrets"]["SIGNING_KEY_PEM"], "from-secrets");
}
