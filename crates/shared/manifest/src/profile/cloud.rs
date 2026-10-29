//! Cloud configuration and the hosting provider a profile deploys to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::TenantId;
use systemprompt_models::subprocess::{DEPLOYMENT_HOST_ENV, FLY_APP_NAME_ENV};

use super::deploy::{DeployConfig, OperatorAuthConfig};

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CloudConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<TenantId>,

    #[serde(default)]
    pub provider: HostingProvider,

    #[serde(default)]
    pub validation: CloudValidationMode,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploy: Option<DeployConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<OperatorAuthConfig>,
}

impl CloudConfig {
    #[must_use]
    pub const fn is_local_trial(&self) -> bool {
        self.provider.is_docker_local()
            || matches!(
                self.validation,
                CloudValidationMode::Warn | CloudValidationMode::Skip
            )
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum CloudValidationMode {
    #[default]
    Strict,
    Warn,
    Skip,
}

/// Where a tenant runs and who operates it.
///
/// `Systemprompt` is the managed cloud reached through the control plane;
/// every other provider is self-managed and never synced with it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HostingProvider {
    #[default]
    Systemprompt,
    Kubernetes,
    Gke,
    DockerLocal,
}

impl HostingProvider {
    pub const ALL: [Self; 4] = [
        Self::Systemprompt,
        Self::Kubernetes,
        Self::Gke,
        Self::DockerLocal,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Systemprompt => "systemprompt",
            Self::Kubernetes => "kubernetes",
            Self::Gke => "gke",
            Self::DockerLocal => "docker_local",
        }
    }

    #[must_use]
    pub const fn is_systemprompt(self) -> bool {
        matches!(self, Self::Systemprompt)
    }

    #[must_use]
    pub const fn is_docker_local(self) -> bool {
        matches!(self, Self::DockerLocal)
    }

    #[must_use]
    pub const fn is_self_managed(self) -> bool {
        matches!(self, Self::Kubernetes | Self::Gke)
    }

    #[must_use]
    pub const fn is_hosted(self) -> bool {
        !self.is_docker_local()
    }

    #[must_use]
    pub const fn deployment_host_markers(self) -> &'static [&'static str] {
        match self {
            Self::Systemprompt => &[DEPLOYMENT_HOST_ENV, FLY_APP_NAME_ENV],
            Self::Kubernetes | Self::Gke | Self::DockerLocal => &[DEPLOYMENT_HOST_ENV],
        }
    }

    pub fn is_deployment_host(self, lookup: impl Fn(&str) -> Option<String>) -> bool {
        systemprompt_models::subprocess::is_deployment_host(self.deployment_host_markers(), lookup)
    }
}

impl std::fmt::Display for HostingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for HostingProvider {
    type Err = UnknownHostingProvider;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|provider| provider.as_str() == value)
            .ok_or_else(|| UnknownHostingProvider {
                value: value.to_owned(),
            })
    }
}

#[derive(Debug, thiserror::Error)]
#[error(
    "unknown hosting provider '{value}' (expected systemprompt, kubernetes, gke or \
     docker_local)"
)]
pub struct UnknownHostingProvider {
    pub value: String,
}
