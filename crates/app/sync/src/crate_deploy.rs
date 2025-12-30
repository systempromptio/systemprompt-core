use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use crate::api_client::SyncApiClient;
use crate::error::{SyncError, SyncResult};
use crate::{SyncConfig, SyncOperationResult};

#[derive(Debug)]
pub struct CrateDeployService {
    config: SyncConfig,
    api_client: SyncApiClient,
}

impl CrateDeployService {
    pub const fn new(config: SyncConfig, api_client: SyncApiClient) -> Self {
        Self { config, api_client }
    }

    pub async fn deploy(
        &self,
        skip_build: bool,
        custom_tag: Option<String>,
    ) -> SyncResult<SyncOperationResult> {
        let project_root = Self::get_project_root()?;
        let app_id = self.get_app_id().await?;

        let tag = match custom_tag {
            Some(t) => t,
            None => {
                let timestamp = chrono::Utc::now().timestamp();
                let git_sha = Self::get_git_sha()?;
                format!("deploy-{timestamp}-{git_sha}")
            },
        };

        let image = format!("registry.fly.io/{app_id}:{tag}");

        if !skip_build {
            Self::build_release(&project_root)?;
            Self::build_web(&project_root)?;
        }

        Self::build_docker(&project_root, &image)?;

        let token = self
            .api_client
            .get_registry_token(&self.config.tenant_id)
            .await?;
        Self::docker_login(&token.registry, &token.username, &token.token)?;
        Self::docker_push(&image)?;

        let response = self
            .api_client
            .deploy(&self.config.tenant_id, &image)
            .await?;

        Ok(
            SyncOperationResult::success("crate_deploy", 1).with_details(serde_json::json!({
                "image": image,
                "status": response.status,
                "app_url": response.app_url,
            })),
        )
    }

    fn get_project_root() -> SyncResult<PathBuf> {
        let current = env::current_dir()?;
        if current.join("infrastructure").exists() {
            Ok(current)
        } else {
            Err(SyncError::NotProjectRoot)
        }
    }

    async fn get_app_id(&self) -> SyncResult<String> {
        self.api_client
            .get_tenant_app_id(&self.config.tenant_id)
            .await
    }

    fn get_git_sha() -> SyncResult<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()?;

        String::from_utf8(output.stdout)
            .map(|sha| sha.trim().to_string())
            .map_err(|_| SyncError::GitShaUnavailable)
    }

    fn build_release(project_root: &PathBuf) -> SyncResult<()> {
        Self::run_command(
            "cargo",
            &[
                "build",
                "--release",
                "--manifest-path=core/Cargo.toml",
                "--bin",
                "systemprompt",
            ],
            project_root,
        )
    }

    fn build_web(project_root: &PathBuf) -> SyncResult<()> {
        Self::run_command(
            "npm",
            &["run", "build", "--prefix", "core/web"],
            project_root,
        )
    }

    fn build_docker(project_root: &PathBuf, image: &str) -> SyncResult<()> {
        Self::run_command(
            "docker",
            &[
                "build",
                "-f",
                "infrastructure/docker/app.Dockerfile",
                "-t",
                image,
                ".",
            ],
            project_root,
        )
    }

    fn docker_login(registry: &str, username: &str, token: &str) -> SyncResult<()> {
        let mut command = Command::new("docker");
        command.args(["login", registry, "-u", username, "--password-stdin"]);
        command.stdin(std::process::Stdio::piped());

        let mut child = command.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(token.as_bytes())?;
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(SyncError::DockerLoginFailed);
        }
        Ok(())
    }

    fn docker_push(image: &str) -> SyncResult<()> {
        Self::run_command("docker", &["push", image], &env::current_dir()?)
    }

    fn run_command(cmd: &str, args: &[&str], dir: &PathBuf) -> SyncResult<()> {
        let status = Command::new(cmd).args(args).current_dir(dir).status()?;

        if !status.success() {
            return Err(SyncError::CommandFailed {
                command: format!("{cmd} {}", args.join(" ")),
            });
        }
        Ok(())
    }
}
