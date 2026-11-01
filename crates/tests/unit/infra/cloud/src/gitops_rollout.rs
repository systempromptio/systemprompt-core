use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use systemprompt_cloud::deploy::provider::{
    BuiltImage, DeployProvider, GitOpsDeploy, GitOpsDeployParams,
};
use systemprompt_cloud::{CloudError, CommandRunner, CommandSpec, DockerCli};
use systemprompt_manifest::{DeployConfig, GitOpsConfig, HostingProvider};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DIGEST: &str = "sha256:fresh";
const VALUES: &str = "image:\n  digest: \"sha256:old\"\n";

type Calls = Arc<Mutex<Vec<String>>>;

#[derive(Clone, Copy)]
enum Fail {
    None,
    Exit(&'static str),
    Spawn(&'static str),
}

struct Runner {
    calls: Calls,
    fail: Fail,
}

impl CommandRunner for Runner {
    fn output(&self, spec: &CommandSpec) -> io::Result<Output> {
        let args = spec.args.join(" ");
        self.calls.lock().unwrap().push(args.clone());
        let stdout = if spec.program == "docker" {
            format!("[\"reg.example/acme/sp@{DIGEST}\"]")
        } else if args == "rev-parse --abbrev-ref HEAD" {
            "main".to_owned()
        } else if args == "rev-parse HEAD" {
            "c0ffee".to_owned()
        } else {
            String::new()
        };
        let code = match self.fail {
            Fail::Exit(prefix) if args.starts_with(prefix) => 1,
            Fail::Spawn(prefix) if args.starts_with(prefix) => {
                return Err(io::Error::other("git missing"));
            },
            _ => 0,
        };
        Ok(Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: stdout.into_bytes(),
            stderr: b"remote rejected".to_vec(),
        })
    }

    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus> {
        self.output(spec).map(|o| o.status)
    }

    fn status_with_stdin(&self, spec: &CommandSpec, _stdin: &[u8]) -> io::Result<ExitStatus> {
        self.status(spec)
    }
}

fn params(root: &Path, repo: PathBuf, push: bool, hostname: String) -> GitOpsDeployParams {
    GitOpsDeployParams {
        provider: HostingProvider::Kubernetes,
        tenant_name: "acme".to_owned(),
        hostname,
        deploy: DeployConfig {
            registry: "reg.example".to_owned(),
            repository: "acme/sp".to_owned(),
            gitops: GitOpsConfig {
                repo_path: repo,
                values_file: PathBuf::from("values.yaml"),
                image_digest_key: "image.digest".to_owned(),
                branch: "main".to_owned(),
                commit_message: "deploy({tenant}): {digest}".to_owned(),
                push,
            },
        },
        project_root: root.to_path_buf(),
        rollout_timeout: Duration::from_secs(1),
    }
}

fn provider(root: &Path, push: bool, fail: Fail, hostname: String) -> (GitOpsDeploy, Calls) {
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let docker = DockerCli::with_runner(Box::new(Runner {
        calls: Arc::new(Mutex::new(Vec::new())),
        fail: Fail::None,
    }));
    let git = Box::new(Runner {
        calls: Arc::clone(&calls),
        fail,
    });
    let deploy = GitOpsDeploy::new(
        params(root, PathBuf::from("ops"), push, hostname),
        docker,
        git,
    )
    .unwrap();
    (deploy, calls)
}

fn seed(root: &Path, values: &str) -> PathBuf {
    let file = root.join("ops/values.yaml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, values).unwrap();
    file
}

fn pushed() -> BuiltImage {
    BuiltImage {
        reference: "reg.example/acme/sp:1".to_owned(),
        pushed: true,
    }
}

#[tokio::test]
async fn a_relative_repo_resolves_under_the_project_root_and_pushes() {
    let dir = tempfile::tempdir().unwrap();
    let values = seed(dir.path(), VALUES);
    let (deploy, calls) = provider(dir.path(), true, Fail::None, "acme.example".to_owned());
    assert_eq!(deploy.repo(), dir.path().join("ops"));
    assert_eq!(deploy.provider(), HostingProvider::Kubernetes);

    let report = deploy.rollout(&pushed()).await.unwrap();
    assert_eq!(report.status, "pushed");
    assert_eq!(report.commit.as_deref(), Some("c0ffee"));
    assert!(std::fs::read_to_string(values).unwrap().contains(DIGEST));
    let calls = calls.lock().unwrap();
    assert!(
        calls.contains(&"pull --ff-only origin main".to_owned()),
        "{calls:?}"
    );
    assert!(calls.contains(&"push origin main".to_owned()), "{calls:?}");
}

#[tokio::test]
async fn an_already_pinned_digest_is_reported_unchanged_without_a_commit() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), &format!("image:\n  digest: \"{DIGEST}\"\n"));
    let (deploy, calls) = provider(dir.path(), false, Fail::None, "a.example".to_owned());
    let report = deploy.rollout(&pushed()).await.unwrap();
    assert_eq!(report.status, "unchanged");
    assert_eq!(report.commit.as_deref(), Some("c0ffee"));
    assert!(
        !calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.starts_with("commit"))
    );
}

#[tokio::test]
async fn a_missing_values_file_is_a_gitops_error() {
    let dir = tempfile::tempdir().unwrap();
    let (deploy, _) = provider(dir.path(), false, Fail::None, "a.example".to_owned());
    let err = deploy.rollout(&pushed()).await.unwrap_err();
    assert!(
        matches!(&err, CloudError::GitOps { message, source: Some(_) } if message.starts_with("Failed to read")),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_digest_key_holding_a_sequence_is_a_values_file_error() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), "image:\n  - one\n");
    let (deploy, _) = provider(dir.path(), false, Fail::None, "a.example".to_owned());
    let err = deploy.rollout(&pushed()).await.unwrap_err();
    assert!(matches!(err, CloudError::ValuesFile { .. }), "{err:?}");
}

#[tokio::test]
async fn a_failed_git_push_carries_the_stderr() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), VALUES);
    let (deploy, _) = provider(dir.path(), true, Fail::Exit("push"), "a.example".to_owned());
    let err = deploy.rollout(&pushed()).await.unwrap_err();
    let text = err.to_string();
    assert!(text.contains("git push origin main"), "{text}");
    assert!(text.contains("remote rejected"), "{text}");
}

#[tokio::test]
async fn a_git_that_cannot_spawn_is_reported_with_its_source() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), VALUES);
    let (deploy, _) = provider(
        dir.path(),
        false,
        Fail::Spawn("rev-parse"),
        "a.example".to_owned(),
    );
    let err = deploy.rollout(&pushed()).await.unwrap_err();
    assert!(
        matches!(&err, CloudError::GitOps { message, source: Some(_) } if message.starts_with("Failed to run: git rev-parse")),
        "{err:?}"
    );
}

#[tokio::test]
async fn status_returns_the_ready_tenant_health() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/readyz"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"status": "ready", "version": "1.0"})),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let (deploy, _) = provider(dir.path(), false, Fail::None, server.uri());
    let health = deploy.status().await.unwrap();
    assert!(health.ready);
    assert_eq!(health.version.as_deref(), Some("1.0"));
}

#[tokio::test]
async fn status_reports_an_unready_tenant_once_the_timeout_cannot_fit_a_poll() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/readyz"))
        .respond_with(
            ResponseTemplate::new(503).set_body_json(serde_json::json!({"status": "starting"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let (deploy, _) = provider(dir.path(), false, Fail::None, server.uri());
    let err = deploy.status().await.unwrap_err();
    assert!(
        matches!(&err, CloudError::TenantUnready { last, .. } if last == "starting"),
        "{err:?}"
    );
}
