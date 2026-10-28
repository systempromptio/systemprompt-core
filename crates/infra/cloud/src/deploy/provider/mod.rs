//! The provider seam behind `cloud deploy`.
//!
//! [`DeployProvider`] is the part of a deploy that differs by where the
//! tenant runs: where the image goes, how secrets reach the instance, what
//! "roll out" means, and how readiness is read back. The image build, the
//! artifact checks and the secret collection stay in the caller's pipeline.
//! [`SystempromptCloud`] drives the managed control plane; [`GitOpsDeploy`]
//! pushes to an operator-owned registry and pins the image digest in an ops
//! repository's values file for the cluster's controller to apply.
//! [`DeployProviderKind::for_provider`] is the one place a
//! [`HostingProvider`] selects its pipeline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod gitops;
mod health;
mod systemprompt;
mod values_file;

use std::collections::HashMap;
use std::future::Future;

use systemprompt_manifest::HostingProvider;

use crate::error::{CloudError, CloudResult};

pub use gitops::{GitOpsDeploy, GitOpsDeployParams};
pub use health::{TenantHealth, probe_tenant, tenant_base_url, wait_until_ready};
pub use systemprompt::SystempromptCloud;
pub use values_file::{ValuesFileError, set_yaml_scalar};

/// One tenant's deploy target. Every method acts on the tenant the provider
/// was built for.
pub trait DeployProvider: Send + Sync {
    fn provider(&self) -> HostingProvider;

    fn secrets_transport(&self) -> SecretsTransport;

    fn registry_credentials(&self) -> impl Future<Output = CloudResult<RegistryTarget>> + Send;

    fn push_secrets(
        &self,
        batch: SecretBatch,
    ) -> impl Future<Output = CloudResult<SecretsOutcome>> + Send;

    fn rollout(
        &self,
        image: &BuiltImage,
    ) -> impl Future<Output = CloudResult<RolloutReport>> + Send;

    fn status(&self) -> impl Future<Output = CloudResult<TenantHealth>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployProviderKind {
    SystempromptCloud,
    GitOps,
}

impl DeployProviderKind {
    pub const fn for_provider(provider: HostingProvider) -> CloudResult<Self> {
        match provider {
            HostingProvider::Systemprompt => Ok(Self::SystempromptCloud),
            HostingProvider::Kubernetes | HostingProvider::Gke => Ok(Self::GitOps),
            HostingProvider::DockerLocal => Err(CloudError::ProviderNotDeployable { provider }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretsTransport {
    Push,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryTarget {
    pub image: String,
    pub login: Option<RegistryLogin>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RegistryLogin {
    pub registry: String,
    pub username: String,
    pub token: String,
}

impl std::fmt::Debug for RegistryLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryLogin")
            .field("registry", &self.registry)
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretPurpose {
    Profile,
    VaultBootstrap,
    PlatformCredentials,
    ProfilePath,
}

#[derive(Clone, PartialEq, Eq)]
pub enum SecretBatch {
    Values {
        purpose: SecretPurpose,
        entries: HashMap<String, String>,
    },
    Declared {
        purpose: SecretPurpose,
        keys: Vec<String>,
    },
}

impl SecretBatch {
    #[must_use]
    pub const fn purpose(&self) -> SecretPurpose {
        match self {
            Self::Values { purpose, .. } | Self::Declared { purpose, .. } => *purpose,
        }
    }

    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = match self {
            Self::Values { entries, .. } => entries.keys().cloned().collect(),
            Self::Declared { keys, .. } => keys.clone(),
        };
        keys.sort();
        keys.dedup();
        keys
    }
}

impl std::fmt::Debug for SecretBatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretBatch")
            .field("purpose", &self.purpose())
            .field("keys", &self.keys())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretsOutcome {
    Pushed { keys: Vec<String> },
    External { required: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltImage {
    pub reference: String,
    pub pushed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutReport {
    pub image: String,
    pub status: String,
    pub app_url: Option<String>,
    pub commit: Option<String>,
}
