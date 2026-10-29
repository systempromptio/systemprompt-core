//! [`DeployProvider`] for self-managed Kubernetes and GKE tenants.
//!
//! The image goes to the registry named in `cloud.deploy` with whatever
//! credentials the operator's docker client already holds. The rollout is a
//! commit to the ops repository's working copy that pins the pushed image's
//! registry digest at `cloud.deploy.gitops.image_digest_key`; the cluster's
//! `GitOps` controller applies it. Secrets reach the instance through the
//! cluster's secret store (External Secrets), never from the CLI, so
//! `push_secrets` only reports the keys the profile expects to find there.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use systemprompt_manifest::{DeployConfig, HostingProvider};
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT};

use super::health::{TenantHealth, tenant_base_url, wait_until_ready};
use super::values_file::set_yaml_scalar;
use super::{
    BuiltImage, DeployProvider, RegistryTarget, RolloutReport, SecretBatch, SecretsOutcome,
    SecretsTransport,
};
use crate::docker::{CommandRunner, CommandSpec, DockerCli, image_repository};
use crate::error::{CloudError, CloudResult};

const READY_POLL_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct GitOpsDeployParams {
    pub provider: HostingProvider,
    pub tenant_name: String,
    pub hostname: String,
    pub deploy: DeployConfig,
    pub project_root: PathBuf,
    pub rollout_timeout: Duration,
}

pub struct GitOpsDeploy {
    params: GitOpsDeployParams,
    repo: PathBuf,
    docker: DockerCli,
    git: Box<dyn CommandRunner>,
    http: reqwest::Client,
}

impl std::fmt::Debug for GitOpsDeploy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitOpsDeploy")
            .field("params", &self.params)
            .field("repo", &self.repo)
            .finish_non_exhaustive()
    }
}

impl GitOpsDeploy {
    pub fn new(
        params: GitOpsDeployParams,
        docker: DockerCli,
        git: Box<dyn CommandRunner>,
    ) -> CloudResult<Self> {
        let repo = resolve_repo(&params.project_root, &params.deploy.gitops.repo_path);
        let http = reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_DEFAULT_TIMEOUT)
            .build()?;
        Ok(Self {
            params,
            repo,
            docker,
            git,
            http,
        })
    }

    #[must_use]
    pub fn repo(&self) -> &Path {
        &self.repo
    }

    fn git(&self, args: &[&str]) -> CloudResult<String> {
        let spec = CommandSpec::git(&self.repo, args.iter().copied());
        let output = self.git.output(&spec).map_err(|e| {
            CloudError::gitops_with(format!("Failed to run: {}", spec.rendered()), e)
        })?;
        if !output.status.success() {
            return Err(CloudError::gitops(format!(
                "`{}` failed in {}: {}",
                spec.rendered(),
                self.repo.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn pin_digest(&self, digest: &str) -> CloudResult<(String, Option<String>)> {
        let gitops = &self.params.deploy.gitops;
        let branch = self.git(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if branch != gitops.branch {
            return Err(CloudError::gitops(format!(
                "Ops repository {} is on branch '{branch}', expected '{}'",
                self.repo.display(),
                gitops.branch
            )));
        }
        if gitops.push {
            self.git(&["pull", "--ff-only", "origin", gitops.branch.as_str()])?;
        }

        let values_path = self.repo.join(&gitops.values_file);
        let values_display = values_path.display().to_string();
        let current = std::fs::read_to_string(&values_path)
            .map_err(|e| CloudError::gitops_with(format!("Failed to read {values_display}"), e))?;
        let updated =
            set_yaml_scalar(&current, &gitops.digest_key_segments(), digest).map_err(|source| {
                CloudError::ValuesFile {
                    path: values_display.clone(),
                    source,
                }
            })?;
        if updated == current {
            let head = self.git(&["rev-parse", "HEAD"])?;
            return Ok(("unchanged".to_owned(), Some(head)));
        }
        std::fs::write(&values_path, updated)
            .map_err(|e| CloudError::gitops_with(format!("Failed to write {values_display}"), e))?;

        let relative = gitops.values_file.to_string_lossy().into_owned();
        let message = gitops.render_commit_message(&self.params.tenant_name, digest);
        self.git(&["add", "--", relative.as_str()])?;
        self.git(&["commit", "-m", message.as_str(), "--", relative.as_str()])?;
        let commit = self.git(&["rev-parse", "HEAD"])?;
        if gitops.push {
            self.git(&["push", "origin", gitops.branch.as_str()])?;
            return Ok(("pushed".to_owned(), Some(commit)));
        }
        Ok(("committed".to_owned(), Some(commit)))
    }
}

fn resolve_repo(project_root: &Path, repo_path: &Path) -> PathBuf {
    if repo_path.is_absolute() {
        repo_path.to_path_buf()
    } else {
        project_root.join(repo_path)
    }
}

impl DeployProvider for GitOpsDeploy {
    fn provider(&self) -> HostingProvider {
        self.params.provider
    }

    fn secrets_transport(&self) -> SecretsTransport {
        SecretsTransport::External
    }

    fn registry_credentials(&self) -> impl Future<Output = CloudResult<RegistryTarget>> + Send {
        let tag = Utc::now().format("%Y%m%d%H%M%S");
        std::future::ready(Ok(RegistryTarget {
            image: format!("{}:{tag}", self.params.deploy.image_repository()),
            login: None,
        }))
    }

    fn push_secrets(
        &self,
        batch: SecretBatch,
    ) -> impl Future<Output = CloudResult<SecretsOutcome>> + Send {
        std::future::ready(Ok(SecretsOutcome::External {
            required: batch.keys(),
        }))
    }

    fn rollout(
        &self,
        image: &BuiltImage,
    ) -> impl Future<Output = CloudResult<RolloutReport>> + Send {
        std::future::ready(self.pin_rollout(image))
    }

    async fn status(&self) -> CloudResult<TenantHealth> {
        wait_until_ready(
            &self.http,
            &tenant_base_url(&self.params.hostname),
            self.params.rollout_timeout,
            READY_POLL_INTERVAL,
        )
        .await
    }
}

impl GitOpsDeploy {
    fn pin_rollout(&self, image: &BuiltImage) -> CloudResult<RolloutReport> {
        if !image.pushed {
            return Err(CloudError::gitops(
                "A GitOps rollout pins the pushed image's registry digest; deploy without \
                 --skip-push",
            ));
        }
        let digest = self.docker.repo_digest(&image.reference)?;
        let (status, commit) = self.pin_digest(&digest)?;
        Ok(RolloutReport {
            image: format!("{}@{digest}", image_repository(&image.reference)),
            status,
            app_url: Some(tenant_base_url(&self.params.hostname)),
            commit,
        })
    }
}
