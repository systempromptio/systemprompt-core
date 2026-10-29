//! Unit tests for the deploy-provider seam: provider selection, the GitOps
//! values-file digest rewrite, and the GitOps rollout over stubbed commands.

use std::collections::HashMap;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{ExitStatus, Output};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use systemprompt_cloud::deploy::provider::{
    BuiltImage, DeployProvider, DeployProviderKind, GitOpsDeploy, GitOpsDeployParams, SecretBatch,
    SecretPurpose, SecretsOutcome, SecretsTransport, ValuesFileError, set_yaml_scalar,
    tenant_base_url,
};
use systemprompt_cloud::docker::{image_repository, select_repo_digest};
use systemprompt_cloud::{CloudError, CommandRunner, CommandSpec, DockerCli};
use systemprompt_manifest::{DeployConfig, GitOpsConfig, HostingProvider};

#[test]
fn the_managed_provider_selects_the_systemprompt_cloud_pipeline() {
    assert_eq!(
        DeployProviderKind::for_provider(HostingProvider::Systemprompt).unwrap(),
        DeployProviderKind::SystempromptCloud
    );
}

#[test]
fn kubernetes_and_gke_select_the_gitops_pipeline() {
    for provider in [HostingProvider::Kubernetes, HostingProvider::Gke] {
        assert_eq!(
            DeployProviderKind::for_provider(provider).unwrap(),
            DeployProviderKind::GitOps,
            "{provider}"
        );
    }
}

#[test]
fn docker_local_has_no_deploy_pipeline() {
    let err = DeployProviderKind::for_provider(HostingProvider::DockerLocal).unwrap_err();
    assert!(
        matches!(
            err,
            CloudError::ProviderNotDeployable {
                provider: HostingProvider::DockerLocal
            }
        ),
        "{err:?}"
    );
}

const VALUES: &str = "\
# Tenant values for acme
replicaCount: 2
image:
  repository: europe-docker.pkg.dev/acme/systemprompt # set by CI
  digest: \"sha256:old\" # pinned by cloud deploy
  pullPolicy: IfNotPresent
ingress:
  enabled: true
";

#[test]
fn the_digest_rewrite_changes_only_the_target_line() {
    let updated = set_yaml_scalar(VALUES, &["image", "digest"], "sha256:new").unwrap();
    let expected = VALUES.replace(
        "  digest: \"sha256:old\" # pinned by cloud deploy",
        "  digest: \"sha256:new\" # pinned by cloud deploy",
    );
    assert_eq!(updated, expected);
}

#[test]
fn the_digest_rewrite_is_idempotent() {
    let once = set_yaml_scalar(VALUES, &["image", "digest"], "sha256:new").unwrap();
    let twice = set_yaml_scalar(&once, &["image", "digest"], "sha256:new").unwrap();
    assert_eq!(once, twice);
}

#[test]
fn a_missing_leaf_is_appended_to_its_parent_mapping() {
    let updated = set_yaml_scalar(VALUES, &["image", "tag"], "v1").unwrap();
    assert!(
        updated.contains("  pullPolicy: IfNotPresent\n  tag: \"v1\"\ningress:\n"),
        "{updated}"
    );
    assert!(updated.contains("  digest: \"sha256:old\" # pinned by cloud deploy"));
}

#[test]
fn a_missing_path_is_created_at_the_end_of_the_document() {
    let updated =
        set_yaml_scalar(VALUES, &["systemprompt", "image", "digest"], "sha256:x").unwrap();
    assert!(
        updated.ends_with("  enabled: true\nsystemprompt:\n  image:\n    digest: \"sha256:x\"\n"),
        "{updated}"
    );
    assert!(updated.starts_with("# Tenant values for acme\nreplicaCount: 2\n"));
}

#[test]
fn a_nested_key_with_the_same_name_elsewhere_is_not_touched() {
    let doc = "sidecar:\n  digest: keep\nimage:\n  digest: old\n";
    let updated = set_yaml_scalar(doc, &["image", "digest"], "sha256:n").unwrap();
    assert_eq!(
        updated,
        "sidecar:\n  digest: keep\nimage:\n  digest: \"sha256:n\"\n"
    );
}

#[test]
fn a_path_through_a_scalar_is_refused() {
    let err = set_yaml_scalar(VALUES, &["replicaCount", "digest"], "x").unwrap_err();
    assert_eq!(
        err,
        ValuesFileError::Scalar {
            path: "replicaCount".to_owned()
        }
    );
}

#[test]
fn replacing_a_mapping_with_a_scalar_is_refused() {
    let err = set_yaml_scalar(VALUES, &["image"], "x").unwrap_err();
    assert_eq!(
        err,
        ValuesFileError::Mapping {
            path: "image".to_owned()
        }
    );
}

#[test]
fn a_path_through_a_sequence_is_refused() {
    let doc = "hosts:\n  - a\n  - b\n";
    let err = set_yaml_scalar(doc, &["hosts", "digest"], "x").unwrap_err();
    assert_eq!(
        err,
        ValuesFileError::Sequence {
            path: "hosts".to_owned()
        }
    );
}

#[test]
fn an_empty_key_path_is_refused() {
    assert_eq!(
        set_yaml_scalar(VALUES, &[], "x").unwrap_err(),
        ValuesFileError::EmptyPath
    );
    assert_eq!(
        set_yaml_scalar(VALUES, &["image", ""], "x").unwrap_err(),
        ValuesFileError::EmptyPath
    );
}

#[test]
fn image_repository_strips_the_tag_but_keeps_a_registry_port() {
    assert_eq!(
        image_repository("reg.example/acme/sp:20260101"),
        "reg.example/acme/sp"
    );
    assert_eq!(
        image_repository("reg.example:5000/acme/sp"),
        "reg.example:5000/acme/sp"
    );
    assert_eq!(
        image_repository("reg.example:5000/acme/sp:v1"),
        "reg.example:5000/acme/sp"
    );
    assert_eq!(
        image_repository("reg.example/acme/sp@sha256:ab"),
        "reg.example/acme/sp"
    );
}

#[test]
fn the_repo_digest_is_the_one_for_the_pushed_repository() {
    let digests = vec![
        "other.example/acme/sp@sha256:other".to_owned(),
        "reg.example/acme/sp@sha256:mine".to_owned(),
    ];
    assert_eq!(
        select_repo_digest("reg.example/acme/sp:v1", &digests).as_deref(),
        Some("sha256:mine")
    );
    assert_eq!(select_repo_digest("unknown.example/x:v1", &digests), None);
}

#[test]
fn a_bare_hostname_is_reached_over_https() {
    assert_eq!(
        tenant_base_url("acme.example.com"),
        "https://acme.example.com"
    );
    assert_eq!(
        tenant_base_url("http://localhost:8080/"),
        "http://localhost:8080"
    );
}

type Calls = Arc<Mutex<Vec<CommandSpec>>>;

struct ScriptedRunner {
    calls: Calls,
    replies: HashMap<String, String>,
}

impl ScriptedRunner {
    fn reply(&self, spec: &CommandSpec) -> Output {
        self.calls.lock().unwrap().push(spec.clone());
        let key = format!(
            "{} {}",
            spec.program,
            spec.args.first().cloned().unwrap_or_default()
        );
        Output {
            status: ExitStatus::from_raw(0),
            stdout: self
                .replies
                .get(&key)
                .cloned()
                .unwrap_or_default()
                .into_bytes(),
            stderr: Vec::new(),
        }
    }
}

impl CommandRunner for ScriptedRunner {
    fn output(&self, spec: &CommandSpec) -> io::Result<Output> {
        Ok(self.reply(spec))
    }

    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus> {
        Ok(self.reply(spec).status)
    }

    fn status_with_stdin(&self, spec: &CommandSpec, _stdin: &[u8]) -> io::Result<ExitStatus> {
        Ok(self.reply(spec).status)
    }
}

fn scripted(replies: &[(&str, &str)]) -> (Box<dyn CommandRunner>, Calls) {
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let runner = ScriptedRunner {
        calls: Arc::clone(&calls),
        replies: replies
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    };
    (Box::new(runner), calls)
}

fn gitops_params(repo: PathBuf, push: bool) -> GitOpsDeployParams {
    GitOpsDeployParams {
        provider: HostingProvider::Gke,
        tenant_name: "acme".to_owned(),
        hostname: "acme.example.com".to_owned(),
        deploy: DeployConfig {
            registry: "reg.example".to_owned(),
            repository: "acme/sp".to_owned(),
            gitops: GitOpsConfig {
                repo_path: repo,
                values_file: PathBuf::from("tenants/acme/values.yaml"),
                image_digest_key: "image.digest".to_owned(),
                branch: "main".to_owned(),
                commit_message: "deploy({tenant}): {digest}".to_owned(),
                push,
            },
        },
        project_root: PathBuf::from("/nonexistent"),
        rollout_timeout: Duration::from_secs(1),
    }
}

#[tokio::test]
async fn gitops_reports_secret_keys_instead_of_pushing_them() {
    let dir = tempfile::tempdir().unwrap();
    let (docker, _) = scripted(&[]);
    let (git, git_calls) = scripted(&[]);
    let provider = GitOpsDeploy::new(
        gitops_params(dir.path().to_path_buf(), false),
        DockerCli::with_runner(docker),
        git,
    )
    .unwrap();

    assert_eq!(provider.secrets_transport(), SecretsTransport::External);
    let outcome = provider
        .push_secrets(SecretBatch::Declared {
            purpose: SecretPurpose::Profile,
            keys: vec!["SYSTEMPROMPT_PROFILE".to_owned(), "JWT_SECRET".to_owned()],
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        SecretsOutcome::External {
            required: vec!["JWT_SECRET".to_owned(), "SYSTEMPROMPT_PROFILE".to_owned()]
        }
    );
    assert!(git_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn gitops_registry_target_uses_ambient_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let (docker, _) = scripted(&[]);
    let (git, _) = scripted(&[]);
    let provider = GitOpsDeploy::new(
        gitops_params(dir.path().to_path_buf(), false),
        DockerCli::with_runner(docker),
        git,
    )
    .unwrap();

    let target = provider.registry_credentials().await.unwrap();
    assert!(
        target.image.starts_with("reg.example/acme/sp:"),
        "{}",
        target.image
    );
    assert!(target.login.is_none());
}

#[tokio::test]
async fn gitops_rollout_pins_the_digest_and_commits_only_the_values_file() {
    let dir = tempfile::tempdir().unwrap();
    let values = dir.path().join("tenants/acme/values.yaml");
    std::fs::create_dir_all(values.parent().unwrap()).unwrap();
    std::fs::write(&values, VALUES).unwrap();

    let (docker, _) = scripted(&[("docker image", "[\"reg.example/acme/sp@sha256:fresh\"]")]);
    let (git, git_calls) = scripted(&[("git rev-parse", "main")]);
    let provider = GitOpsDeploy::new(
        gitops_params(dir.path().to_path_buf(), false),
        DockerCli::with_runner(docker),
        git,
    )
    .unwrap();

    let report = provider
        .rollout(&BuiltImage {
            reference: "reg.example/acme/sp:20261028".to_owned(),
            pushed: true,
        })
        .await
        .unwrap();

    assert_eq!(report.image, "reg.example/acme/sp@sha256:fresh");
    assert_eq!(report.status, "committed");
    assert_eq!(report.app_url.as_deref(), Some("https://acme.example.com"));
    let written = std::fs::read_to_string(&values).unwrap();
    assert!(written.contains("  digest: \"sha256:fresh\" # pinned by cloud deploy"));
    assert!(written.contains("  repository: europe-docker.pkg.dev/acme/systemprompt # set by CI"));

    let calls = git_calls.lock().unwrap();
    let rendered: Vec<String> = calls.iter().map(|c| c.args.join(" ")).collect();
    assert!(
        rendered.contains(
            &"commit -m deploy(acme): sha256:fresh -- tenants/acme/values.yaml".to_owned()
        ),
        "{rendered:?}"
    );
    assert!(
        !rendered.iter().any(|args| args.starts_with("push")),
        "{rendered:?}"
    );
    assert!(
        calls
            .iter()
            .all(|c| c.current_dir.as_deref() == Some(dir.path()))
    );
}

#[tokio::test]
async fn gitops_rollout_refuses_an_unpushed_image() {
    let dir = tempfile::tempdir().unwrap();
    let (docker, _) = scripted(&[]);
    let (git, git_calls) = scripted(&[]);
    let provider = GitOpsDeploy::new(
        gitops_params(dir.path().to_path_buf(), false),
        DockerCli::with_runner(docker),
        git,
    )
    .unwrap();

    let err = provider
        .rollout(&BuiltImage {
            reference: "reg.example/acme/sp:1".to_owned(),
            pushed: false,
        })
        .await
        .unwrap_err();
    assert!(matches!(err, CloudError::GitOps { .. }), "{err:?}");
    assert!(git_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn gitops_rollout_refuses_a_working_copy_on_another_branch() {
    let dir = tempfile::tempdir().unwrap();
    let (docker, _) = scripted(&[("docker image", "[\"reg.example/acme/sp@sha256:a\"]")]);
    let (git, _) = scripted(&[("git rev-parse", "feature")]);
    let provider = GitOpsDeploy::new(
        gitops_params(dir.path().to_path_buf(), false),
        DockerCli::with_runner(docker),
        git,
    )
    .unwrap();

    let err = provider
        .rollout(&BuiltImage {
            reference: "reg.example/acme/sp:1".to_owned(),
            pushed: true,
        })
        .await
        .unwrap_err();
    assert!(err.to_string().contains("'feature'"), "{err}");
}

#[test]
fn a_cloud_deploy_block_parses_with_defaults() {
    let yaml = "\
tenant_id: acme
provider: gke
deploy:
  registry: europe-docker.pkg.dev
  repository: acme/systemprompt
  gitops:
    repo_path: ../ops
    values_file: tenants/acme/values.yaml
    image_digest_key: image.digest
    branch: main
operator:
  issuer: https://login.acme.example
  client_id: systemprompt-cli
";
    let cloud: systemprompt_manifest::CloudConfig = serde_yaml::from_str(yaml).unwrap();
    let deploy = cloud.deploy.expect("deploy block");
    assert_eq!(
        deploy.image_repository(),
        "europe-docker.pkg.dev/acme/systemprompt"
    );
    assert!(!deploy.gitops.push);
    assert_eq!(
        deploy.gitops.render_commit_message("acme", "sha256:1"),
        "deploy(acme): sha256:1"
    );
    assert_eq!(deploy.gitops.digest_key_segments(), vec!["image", "digest"]);
    let operator = cloud.operator.expect("operator block");
    assert_eq!(operator.scopes, vec!["openid", "email", "profile"]);
}

#[test]
fn a_profile_without_deploy_settings_parses_as_before() {
    let cloud: systemprompt_manifest::CloudConfig =
        serde_yaml::from_str("tenant_id: acme\n").unwrap();
    assert!(cloud.deploy.is_none());
    assert!(cloud.operator.is_none());
    assert_eq!(cloud.provider, HostingProvider::Systemprompt);
}

#[test]
fn an_unknown_gitops_field_is_rejected() {
    let yaml = "\
provider: kubernetes
deploy:
  registry: r
  repository: x
  gitops:
    repo_path: ops
    values_file: v.yaml
    image_digest_key: image.digest
    branch: main
    clone_url: https://example.com/ops.git
";
    assert!(serde_yaml::from_str::<systemprompt_manifest::CloudConfig>(yaml).is_err());
}
