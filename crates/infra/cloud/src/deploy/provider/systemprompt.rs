//! [`DeployProvider`] for the managed systemprompt.io cloud.
//!
//! The control plane issues the registry credentials and tag, stores the
//! tenant's secrets, and performs the rollout; readiness is the control
//! plane's own tenant status, polled until it reports a ready state, reports
//! a failed one, or the wait runs out; a status call that fails is retried
//! within the wait. The status does not name the running image, so a ready
//! tenant is reported with `release_confirmed: false`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_identifiers::TenantId;
use systemprompt_manifest::HostingProvider;

use super::{
    BuiltImage, DeployProvider, ReadinessWait, RegistryLogin, RegistryTarget, RolloutReport,
    SecretBatch, SecretsOutcome, SecretsTransport, TenantHealth,
};
use crate::CloudApiClient;
use crate::error::{CloudError, CloudResult};

#[derive(Debug)]
pub struct SystempromptCloud {
    client: CloudApiClient,
    tenant_id: TenantId,
}

impl SystempromptCloud {
    #[must_use]
    pub const fn new(client: CloudApiClient, tenant_id: TenantId) -> Self {
        Self { client, tenant_id }
    }

    #[must_use]
    pub const fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }
}

impl DeployProvider for SystempromptCloud {
    fn provider(&self) -> HostingProvider {
        HostingProvider::Systemprompt
    }

    fn secrets_transport(&self) -> SecretsTransport {
        SecretsTransport::Push
    }

    async fn registry_credentials(&self) -> CloudResult<RegistryTarget> {
        let token = self.client.get_registry_token(&self.tenant_id).await?;
        Ok(RegistryTarget {
            image: format!("{}/{}:{}", token.registry, token.repository, token.tag),
            login: Some(RegistryLogin {
                registry: token.registry,
                username: token.username,
                token: token.token,
            }),
        })
    }

    async fn push_secrets(&self, batch: SecretBatch) -> CloudResult<SecretsOutcome> {
        match batch {
            SecretBatch::Values { entries, .. } => {
                let keys = self.client.set_secrets(&self.tenant_id, entries).await?;
                Ok(SecretsOutcome::Pushed { keys })
            },
            SecretBatch::Declared { keys, .. } => Err(CloudError::deploy(format!(
                "The managed cloud stores secret values; {} were declared without values",
                keys.join(", ")
            ))),
        }
    }

    async fn rollout(&self, image: &BuiltImage) -> CloudResult<RolloutReport> {
        let response = self
            .client
            .deploy(&self.tenant_id, &image.reference)
            .await?;
        Ok(RolloutReport {
            image: image.reference.clone(),
            status: response.status,
            app_url: response.app_url,
            commit: None,
        })
    }

    async fn wait_until_ready(&self, wait: ReadinessWait) -> CloudResult<TenantHealth> {
        let started = Instant::now();
        loop {
            let health = match self.tenant_health().await {
                Ok(health) => health,
                Err(error) if !wait.exhausted(started) => {
                    tracing::warn!(error = %error, "Tenant status unavailable; retrying");
                    tokio::time::sleep(wait.interval).await;
                    continue;
                },
                Err(error) => return Err(error),
            };
            if health.ready {
                return Ok(health);
            }
            if is_failed_state(&health.status) || wait.exhausted(started) {
                return Err(CloudError::TenantUnready {
                    url: health.url,
                    waited_secs: started.elapsed().as_secs(),
                    last: health.status,
                });
            }
            tokio::time::sleep(wait.interval).await;
        }
    }
}

impl SystempromptCloud {
    async fn tenant_health(&self) -> CloudResult<TenantHealth> {
        let status = self.client.get_tenant_status(&self.tenant_id).await?;
        Ok(TenantHealth {
            url: status
                .app_url
                .unwrap_or_else(|| self.tenant_id.as_str().to_owned()),
            http_status: None,
            ready: matches!(
                status.status.to_ascii_lowercase().as_str(),
                "running" | "ready" | "healthy" | "active"
            ),
            status: status.status,
            version: None,
            release_confirmed: false,
        })
    }
}

fn is_failed_state(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "failed" | "error" | "crashed" | "deploy_failed"
    )
}
