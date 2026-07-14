//! End-to-end "build crate, push docker image, deploy" flow used by
//! `systemprompt cloud deploy` for the rust-side container image.
//!
//! All process execution flows through the
//! [`CommandRunner`] seam so tests can
//! substitute a stub instead of spawning real `cargo`/`git`/`docker`.

use std::env;
use std::path::{Path, PathBuf};

use systemprompt_cloud::{CommandRunner, CommandSpec, SystemCommandRunner};

use crate::api_client::SyncApiClient;
use crate::error::{SyncError, SyncResult};
use crate::{SyncConfig, SyncOperationResult};

pub struct CrateDeployService {
    config: SyncConfig,
    api_client: SyncApiClient,
    runner: Box<dyn CommandRunner>,
}

impl std::fmt::Debug for CrateDeployService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrateDeployService")
            .field("config", &self.config)
            .field("api_client", &self.api_client)
            .finish_non_exhaustive()
    }
}

impl CrateDeployService {
    #[must_use]
    pub fn new(config: SyncConfig, api_client: SyncApiClient) -> Self {
        Self {
            config,
            api_client,
            runner: Box::new(SystemCommandRunner),
        }
    }

    #[must_use]
    pub fn with_runner(mut self, runner: Box<dyn CommandRunner>) -> Self {
        self.runner = runner;
        self
    }

    pub async fn deploy(
        &self,
        skip_build: bool,
        custom_tag: Option<String>,
    ) -> SyncResult<SyncOperationResult> {
        let project_root = Self::get_project_root()?;
        let app_id = self.get_app_id().await?;

        let tag = if let Some(t) = custom_tag {
            t
        } else {
            let timestamp = chrono::Utc::now().timestamp();
            let git_sha = self.get_git_sha()?;
            format!("deploy-{timestamp}-{git_sha}")
        };

        let image = format!("registry.fly.io/{app_id}:{tag}");

        if !skip_build {
            self.build_release(&project_root)?;
        }

        self.build_docker(&project_root, &image)?;

        let token = self
            .api_client
            .get_registry_token(&self.config.tenant_id)
            .await?;
        self.docker_login(&token.registry, &token.username, &token.token)?;
        self.docker_push(&image)?;

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

    fn get_git_sha(&self) -> SyncResult<String> {
        let spec = CommandSpec {
            program: "git".to_owned(),
            args: vec![
                "rev-parse".to_owned(),
                "--short".to_owned(),
                "HEAD".to_owned(),
            ],
            current_dir: None,
        };
        let output = self
            .runner
            .output(&spec)
            .map_err(|source| SyncError::CommandSpawnFailed {
                command: spec.rendered(),
                source,
            })?;

        String::from_utf8(output.stdout)
            .map(|sha| sha.trim().to_owned())
            .map_err(|_e| SyncError::GitShaUnavailable)
    }

    fn build_release(&self, project_root: &Path) -> SyncResult<()> {
        self.run_command(
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

    fn build_docker(&self, project_root: &Path, image: &str) -> SyncResult<()> {
        self.run_command(
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

    fn docker_login(&self, registry: &str, username: &str, token: &str) -> SyncResult<()> {
        let spec = CommandSpec {
            program: "docker".to_owned(),
            args: vec![
                "login".to_owned(),
                registry.to_owned(),
                "-u".to_owned(),
                username.to_owned(),
                "--password-stdin".to_owned(),
            ],
            current_dir: None,
        };
        let status = self
            .runner
            .status_with_stdin(&spec, token.as_bytes())
            .map_err(|source| SyncError::CommandSpawnFailed {
                command: format!("docker login {registry}"),
                source,
            })?;

        if !status.success() {
            return Err(SyncError::DockerLoginFailed);
        }
        Ok(())
    }

    fn docker_push(&self, image: &str) -> SyncResult<()> {
        self.run_command("docker", &["push", image], &env::current_dir()?)
    }

    fn run_command(&self, cmd: &str, args: &[&str], dir: &Path) -> SyncResult<()> {
        let spec = CommandSpec {
            program: cmd.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            current_dir: Some(dir.to_path_buf()),
        };
        let command_str = spec.rendered();
        let status = self
            .runner
            .status(&spec)
            .map_err(|source| SyncError::CommandSpawnFailed {
                command: command_str.clone(),
                source,
            })?;

        if !status.success() {
            return Err(SyncError::CommandFailed {
                command: command_str,
            });
        }
        Ok(())
    }
}
