//! Pre-sync apply-from-cloud paths of the deploy pipeline: download, backup,
//! diff, confirm/apply, already-clean short-circuit, and the fail-closed
//! error mapping for download and dry-run failures.

use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{ExitStatus, Output};
use std::sync::Mutex;
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

struct OkRunner;

impl CommandRunner for OkRunner {
    fn output(&self, _spec: &CommandSpec) -> io::Result<Output> {
        Ok(Output {
            status: ExitStatus::from_raw(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    fn status(&self, _spec: &CommandSpec) -> io::Result<ExitStatus> {
        Ok(ExitStatus::from_raw(0))
    }

    fn status_with_stdin(&self, _spec: &CommandSpec, _stdin: &[u8]) -> io::Result<ExitStatus> {
        Ok(ExitStatus::from_raw(0))
    }
}

struct ScriptedProgress {
    events: Mutex<Vec<String>>,
    answers: Mutex<Vec<bool>>,
}

impl ScriptedProgress {
    fn new(answers: &[bool]) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            answers: Mutex::new(answers.to_vec()),
        }
    }

    fn labels(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

impl DeployProgress for ScriptedProgress {
    fn event(&self, event: &DeployEvent<'_>) {
        let debug = format!("{event:?}");
        let label = debug
            .split([' ', '(', '{'])
            .next()
            .unwrap_or_default()
            .to_owned();
        self.events.lock().unwrap().push(label);
    }

    fn confirm(&self, _prompt: &DeployPrompt) -> SyncResult<bool> {
        let mut answers = self.answers.lock().unwrap();
        let answer = if answers.is_empty() {
            true
        } else {
            answers.remove(0)
        };
        self.events
            .lock()
            .unwrap()
            .push(format!("Confirm:{answer}"));
        Ok(answer)
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

fn pre_sync_options(assume_yes: bool, dry_run: bool) -> DeployOptions {
    DeployOptions {
        skip_push: false,
        dry_run,
        pre_sync: Some(PreSyncOptions {
            no_sync: false,
            assume_yes,
        }),
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

async fn mount_files_tarball(server: &MockServer, entries: &[(&str, &[u8])]) {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    for (name, content) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, *content).unwrap();
    }
    let tarball = builder.into_inner().unwrap().finish().unwrap();
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t-deploy/files"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
        .mount(server)
        .await;
}

fn orchestrator(server_uri: &str) -> DeployOrchestrator {
    let sync_client = SyncApiClient::new(server_uri, "tok").expect("client");
    DeployOrchestrator::new()
        .with_docker(DockerCli::with_runner(Box::new(OkRunner)))
        .with_sync_client(sync_client)
}

#[tokio::test]
async fn apply_from_cloud_writes_changed_files_and_backs_up() {
    let server = MockServer::start().await;
    mount_deploy_mocks(&server).await;
    mount_files_tarball(
        &server,
        &[
            ("config/config.yaml", b"{}"),
            ("agents/new.yaml", b"name: new-agent\n"),
        ],
    )
    .await;

    let project = scaffold_project("prod");
    let progress = ScriptedProgress::new(&[]);
    let req = request(&server.uri(), project.path(), pre_sync_options(true, false));

    let report = orchestrator(&server.uri())
        .deploy(&req, &progress)
        .await
        .expect("deploy");

    assert!(matches!(report.outcome, DeployOutcome::Deployed { .. }));
    let applied = project.path().join("services/agents/new.yaml");
    assert_eq!(
        fs::read(&applied).expect("applied file"),
        b"name: new-agent\n"
    );

    let backups: Vec<_> = fs::read_dir(project.path().join("backup"))
        .expect("backup dir")
        .collect();
    assert_eq!(backups.len(), 1);

    let labels = progress.labels();
    for expected in [
        "SyncDownloadStarted",
        "SyncDownloadFinished",
        "SyncBackupStarted",
        "SyncBackupFinished",
        "SyncDiff",
        "SyncApplied",
    ] {
        assert!(labels.contains(&expected.to_owned()), "missing {expected}");
    }
}

#[tokio::test]
async fn apply_declined_leaves_local_files_and_still_deploys() {
    let server = MockServer::start().await;
    mount_deploy_mocks(&server).await;
    mount_files_tarball(
        &server,
        &[
            ("config/config.yaml", b"{}"),
            ("agents/new.yaml", b"name: new-agent\n"),
        ],
    )
    .await;

    let project = scaffold_project("prod");
    let progress = ScriptedProgress::new(&[true, false]);
    let req = request(
        &server.uri(),
        project.path(),
        pre_sync_options(false, false),
    );

    let report = orchestrator(&server.uri())
        .deploy(&req, &progress)
        .await
        .expect("deploy");

    assert!(matches!(report.outcome, DeployOutcome::Deployed { .. }));
    assert!(!project.path().join("services/agents/new.yaml").exists());
    let labels = progress.labels();
    assert!(labels.contains(&"SyncCancelled".to_owned()));
    assert!(!labels.contains(&"SyncApplied".to_owned()));
}

#[tokio::test]
async fn already_clean_tree_skips_apply_and_prompt() {
    let server = MockServer::start().await;
    mount_deploy_mocks(&server).await;
    mount_files_tarball(&server, &[("config/config.yaml", b"{}")]).await;

    let project = scaffold_project("prod");
    let progress = ScriptedProgress::new(&[]);
    let req = request(&server.uri(), project.path(), pre_sync_options(true, false));

    let report = orchestrator(&server.uri())
        .deploy(&req, &progress)
        .await
        .expect("deploy");

    assert!(matches!(report.outcome, DeployOutcome::Deployed { .. }));
    let labels = progress.labels();
    assert!(labels.contains(&"SyncAlreadyClean".to_owned()));
    assert!(!labels.contains(&"SyncApplied".to_owned()));
    assert!(!labels.contains(&"SyncCancelled".to_owned()));
}

#[tokio::test]
async fn download_failure_maps_to_download_stage_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t-deploy/files"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let project = scaffold_project("prod");
    let progress = ScriptedProgress::new(&[]);
    let req = request(&server.uri(), project.path(), pre_sync_options(true, false));

    let err = orchestrator(&server.uri())
        .deploy(&req, &progress)
        .await
        .expect_err("must fail");
    match err {
        SyncError::PreSyncStage { stage, source } => {
            assert_eq!(stage, "Download");
            assert!(matches!(*source, SyncError::ApiError { status: 500, .. }));
        },
        other => panic!("expected PreSyncStage, got {other:?}"),
    }
    assert!(!project.path().join("backup").exists());
}

#[tokio::test]
async fn dry_run_download_failure_fails_closed_with_errors_event() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t-deploy/files"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let project = scaffold_project("prod");
    let progress = ScriptedProgress::new(&[]);
    let req = request(&server.uri(), project.path(), pre_sync_options(true, true));

    let err = orchestrator(&server.uri())
        .deploy(&req, &progress)
        .await
        .expect_err("must fail");
    assert!(matches!(err, SyncError::PreDeploySyncFailed));
    let labels = progress.labels();
    assert!(labels.contains(&"SyncDryRunStarted".to_owned()));
    assert!(labels.contains(&"SyncErrors".to_owned()));
}

#[tokio::test]
async fn without_injected_client_pre_sync_targets_hostname_directly() {
    let project = scaffold_project("prod");
    let progress = ScriptedProgress::new(&[]);
    let mut req = request(
        "http://127.0.0.1:1",
        project.path(),
        pre_sync_options(true, false),
    );
    req.hostname = Some("127.0.0.1:1".to_owned());

    let orchestrator =
        DeployOrchestrator::new().with_docker(DockerCli::with_runner(Box::new(OkRunner)));
    let err = orchestrator
        .deploy(&req, &progress)
        .await
        .expect_err("unreachable hostname must fail the download stage");
    match err {
        SyncError::PreSyncStage { stage, .. } => assert_eq!(stage, "Download"),
        other => panic!("expected PreSyncStage, got {other:?}"),
    }
}
