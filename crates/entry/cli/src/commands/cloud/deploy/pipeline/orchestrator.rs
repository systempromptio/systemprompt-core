//! Deploy pipeline sequencing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, bail};
use systemprompt_cloud::constants::{container, paths};
use systemprompt_cloud::deploy::provider::{
    BuiltImage, DeployProvider, SecretBatch, SecretPurpose, SecretsOutcome, SecretsTransport,
};
use systemprompt_cloud::deploy::{find_services_config, validate_profile_dockerfile};
use systemprompt_cloud::{CloudCredentials, DockerCli, secrets_env};
use systemprompt_loader::ConfigLoader;

use super::artifacts::DeployArtifacts;
use super::progress::{DeployEvent, DeployProgress};
use super::request::{DeployReport, DeployRequest, DeploySecretsSource};

use super::secrets::{provision_secrets, pushed_count};

#[derive(Debug, Default)]
pub struct DeployOrchestrator {
    docker: DockerCli,
}

impl DeployOrchestrator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_docker(mut self, docker: DockerCli) -> Self {
        self.docker = docker;
        self
    }

    pub async fn deploy<P: DeployProvider>(
        &self,
        provider: &P,
        request: &DeployRequest,
        progress: &dyn DeployProgress,
    ) -> Result<DeployReport> {
        let artifacts = DeployArtifacts::resolve(&request.project_root, &request.profile_name)?;
        progress.event(&DeployEvent::ArtifactsResolved {
            tenant_name: &request.tenant_name,
            binary: &artifacts.binary,
            dockerfile: &artifacts.dockerfile,
        });

        let services_config_path = find_services_config(&request.project_root)?;
        let services_config = ConfigLoader::load_from_path(&services_config_path, None)?;
        validate_profile_dockerfile(
            &artifacts.dockerfile,
            &request.project_root,
            &services_config,
        )?;

        let image = self
            .build_and_push(provider, request, &artifacts, progress)
            .await?;
        provision_secrets(provider, request, progress).await?;

        progress.event(&DeployEvent::DeployStarted);
        let rollout = provider
            .rollout(&BuiltImage {
                reference: image,
                pushed: !request.options.skip_push,
            })
            .await?;
        if let Some(commit) = rollout.commit.as_deref() {
            progress.event(&DeployEvent::RolloutCommitted {
                commit,
                image: &rollout.image,
            });
        }
        progress.event(&DeployEvent::Deployed {
            status: &rollout.status,
            app_url: rollout.app_url.as_deref(),
        });

        if request.options.wait_ready {
            progress.event(&DeployEvent::ReadinessWaitStarted);
            let health = provider.status().await?;
            progress.event(&DeployEvent::TenantReady {
                version: health.version.as_deref(),
            });
        }

        Ok(DeployReport {
            image: rollout.image,
            status: rollout.status,
            app_url: rollout.app_url,
        })
    }

    async fn build_and_push<P: DeployProvider>(
        &self,
        provider: &P,
        request: &DeployRequest,
        artifacts: &DeployArtifacts,
        progress: &dyn DeployProgress,
    ) -> Result<String> {
        progress.event(&DeployEvent::RegistryAuthStarted);
        let target = provider.registry_credentials().await?;
        progress.event(&DeployEvent::RegistryAuthFinished);

        let image = target.image;
        progress.event(&DeployEvent::ImageResolved { image: &image });

        progress.event(&DeployEvent::BuildStarted);
        self.docker
            .build_image(&request.project_root, &artifacts.dockerfile, &image)?;
        progress.event(&DeployEvent::BuildFinished);

        if request.options.skip_push {
            progress.event(&DeployEvent::PushSkipped);
        } else {
            progress.event(&DeployEvent::PushStarted);
            if let Some(login) = &target.login {
                self.docker
                    .login(&login.registry, &login.username, &login.token)?;
            }
            self.docker.push(&image)?;
            progress.event(&DeployEvent::PushFinished);
        }

        Ok(image)
    }
}
