//! Deploy pipeline sequencing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_cloud::constants::{container, paths};
use systemprompt_cloud::deploy::{find_services_config, validate_profile_dockerfile};
use systemprompt_cloud::{CloudApiClient, CloudCredentials, DockerCli, secrets_env};
use systemprompt_loader::ConfigLoader;

use crate::api_client::SyncApiClient;
use crate::error::SyncResult;

use super::artifacts::DeployArtifacts;
use super::pre_sync::{self, PreSyncOutcome};
use super::progress::{DeployEvent, DeployProgress};
use super::request::{DeployOutcome, DeployReport, DeployRequest};

const SIGNING_KEY_ENV: &str = "SIGNING_KEY_PEM";
const PROFILE_ENV: &str = "SYSTEMPROMPT_PROFILE";

#[derive(Debug, Default)]
pub struct DeployOrchestrator {
    docker: DockerCli,
    sync_client: Option<SyncApiClient>,
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

    /// Replaces the pre-sync HTTP client; direct sync always targets
    /// `https://{hostname}`, so tests must substitute a relay-mode client.
    #[must_use]
    pub fn with_sync_client(mut self, client: SyncApiClient) -> Self {
        self.sync_client = Some(client);
        self
    }

    pub async fn deploy(
        &self,
        request: &DeployRequest,
        progress: &dyn DeployProgress,
    ) -> SyncResult<DeployReport> {
        if let Some(options) = &request.options.pre_sync {
            let outcome =
                pre_sync::run(request, options, self.sync_client.clone(), progress).await?;
            if outcome == PreSyncOutcome::DryRun {
                return Ok(DeployReport {
                    outcome: DeployOutcome::DryRun,
                });
            }
        }

        let artifacts = DeployArtifacts::resolve(&request.project_root, &request.profile_name)?;
        progress.event(&DeployEvent::ArtifactsResolved {
            tenant_name: &request.tenant_name,
            binary: &artifacts.binary,
            dockerfile: &artifacts.dockerfile,
        });

        let services_config_path = find_services_config(&request.project_root)?;
        let services_config = ConfigLoader::load_from_path(&services_config_path)?;
        validate_profile_dockerfile(
            &artifacts.dockerfile,
            &request.project_root,
            &services_config,
        )?;

        let api_client = CloudApiClient::new(
            &request.credentials.api_url,
            request.credentials.api_token.as_str(),
        )?;

        let image = self
            .build_and_push(&api_client, request, &artifacts, progress)
            .await?;
        provision_secrets(&api_client, request, progress).await?;

        progress.event(&DeployEvent::DeployStarted);
        let response = api_client.deploy(&request.tenant_id, &image).await?;
        progress.event(&DeployEvent::Deployed {
            status: &response.status,
            app_url: response.app_url.as_deref(),
        });

        Ok(DeployReport {
            outcome: DeployOutcome::Deployed {
                image,
                status: response.status,
                app_url: response.app_url,
            },
        })
    }

    async fn build_and_push(
        &self,
        api_client: &CloudApiClient,
        request: &DeployRequest,
        artifacts: &DeployArtifacts,
        progress: &dyn DeployProgress,
    ) -> SyncResult<String> {
        progress.event(&DeployEvent::RegistryAuthStarted);
        let registry_token = api_client.get_registry_token(&request.tenant_id).await?;
        progress.event(&DeployEvent::RegistryAuthFinished);

        let image = format!(
            "{}/{}:{}",
            registry_token.registry, registry_token.repository, registry_token.tag
        );
        progress.event(&DeployEvent::ImageResolved { image: &image });

        progress.event(&DeployEvent::BuildStarted);
        self.docker
            .build_image(&request.project_root, &artifacts.dockerfile, &image)?;
        progress.event(&DeployEvent::BuildFinished);

        if request.options.skip_push {
            progress.event(&DeployEvent::PushSkipped);
        } else {
            progress.event(&DeployEvent::PushStarted);
            self.docker.login(
                &registry_token.registry,
                &registry_token.username,
                &registry_token.token,
            )?;
            self.docker.push(&image)?;
            progress.event(&DeployEvent::PushFinished);
        }

        Ok(image)
    }
}

async fn provision_secrets(
    api_client: &CloudApiClient,
    request: &DeployRequest,
    progress: &dyn DeployProgress,
) -> SyncResult<()> {
    progress.event(&DeployEvent::SecretsPhaseStarted);

    let mut env_secrets = if request.secrets_path.exists() {
        secrets_env::map_secrets_to_env_vars(secrets_env::load_secrets_json(&request.secrets_path)?)
    } else {
        progress.event(&DeployEvent::SecretsFileMissing);
        HashMap::new()
    };

    if !env_secrets.contains_key(SIGNING_KEY_ENV)
        && let Some(pem) = secrets_env::read_signing_key_pem(&request.signing_key_path)?
    {
        env_secrets.insert(SIGNING_KEY_ENV.to_owned(), pem);
    }

    if !env_secrets.is_empty() {
        progress.event(&DeployEvent::SecretsSyncStarted);
        let keys = api_client
            .set_secrets(&request.tenant_id, env_secrets)
            .await?;
        progress.event(&DeployEvent::SecretsSynced { count: keys.len() });
    }

    progress.event(&DeployEvent::CredentialsSyncStarted);
    let keys = api_client
        .set_secrets(&request.tenant_id, credentials_env(&request.credentials))
        .await?;
    progress.event(&DeployEvent::CredentialsSynced { count: keys.len() });

    let profile_env_path = format!(
        "{}/{}/{}",
        container::PROFILES,
        request.profile_name,
        paths::PROFILE_CONFIG
    );
    let mut profile_secret = HashMap::new();
    profile_secret.insert(PROFILE_ENV.to_owned(), profile_env_path);
    api_client
        .set_secrets(&request.tenant_id, profile_secret)
        .await?;
    progress.event(&DeployEvent::ProfilePathConfigured);

    Ok(())
}

fn credentials_env(creds: &CloudCredentials) -> HashMap<String, String> {
    HashMap::from([
        (
            "SYSTEMPROMPT_API_TOKEN".to_owned(),
            creds.api_token.as_str().to_owned(),
        ),
        (
            "SYSTEMPROMPT_USER_EMAIL".to_owned(),
            creds.user_email.as_str().to_owned(),
        ),
        ("SYSTEMPROMPT_CLI_REMOTE".to_owned(), "true".to_owned()),
    ])
}
