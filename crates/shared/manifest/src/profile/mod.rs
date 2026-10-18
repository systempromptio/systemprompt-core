//! Profile configuration models — the deserialized shape of a
//! `.systemprompt/profiles/<name>/profile.yaml` document.
//!
//! Covers server, database, paths, secrets, security, rate limits,
//! governance, and runtime sections, plus validation rules and
//! environment-variable interpolation. The provider catalog and gateway
//! routes are not profile sections: they live in the services tree
//! (`crate::services::{ProviderRegistry, GatewayState}`), and a profile that
//! still carries them fails to parse with [`ProfileError::MovedToServices`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cloud;
mod config_planes;
mod database;
mod error;
mod external_secrets;
mod governance;
mod info;
mod judge;
mod masking;
mod node_role;
mod observability;
mod oci_reference;
mod paths;
mod rate_limits;
mod retention;
mod runtime;
mod secrets;
mod security;
mod server;
mod services;
mod shutdown;
mod site;
mod storage;
mod style;
mod validation;
mod vault;

pub use cloud::{CloudConfig, CloudValidationMode};
pub use config_planes::{ConfigPlanesConfig, PlaneAuthority};
pub use database::{DatabaseConfig, PoolConfig, RoleMaxConnections};
pub use error::{ProfileError, ProfileResult};
pub use external_secrets::{ExternalSecretsConfig, MIN_EXTERNAL_REFRESH_SECS};
pub use governance::{
    AuditConfig, AuthzConfig, AuthzHookConfig, AuthzMode, GovernanceConfig, QuotaConfig,
    UNRESTRICTED_ACKNOWLEDGEMENT,
};
pub use info::ProfileInfo;
pub use judge::JudgeProfile;
pub use node_role::NodeRole;
pub use observability::{
    LogFormat, ObservabilityConfig, OtlpExportConfig, OtlpProtocol, OtlpSignal,
};
pub use oci_reference::{OciReference, OciReferenceError};
pub use paths::{PathsConfig, expand_home, resolve_path, resolve_with_home};
pub use rate_limits::{
    RateLimitsConfig, default_agent_registry, default_agents, default_artifacts,
    default_bridge_auth, default_burst, default_content, default_contexts, default_gateway,
    default_mcp, default_mcp_registry, default_oauth_auth, default_oauth_public, default_stream,
    default_tasks,
};
pub use retention::RetentionConfig;
pub use runtime::{Environment, LogLevel, OutputFormat, RuntimeConfig};
pub use secrets::{SecretsConfig, SecretsSource, SecretsValidationMode};
pub use security::{
    ApiKeyPolicyConfig, DEFAULT_ID_JAG_TTL_SECS, GATEWAY_REQUIRED_RESOURCE_AUDIENCES,
    SecurityConfig, TrustedIssuer, default_resource_audiences,
};
pub use server::{
    ContentNegotiationConfig, FrameOptions, ReferrerPolicy, SecurityHeadersConfig, ServerConfig,
};
pub use services::{
    BundleVerification, FetchFailurePolicy, HttpsServicesSource, OciServicesSource,
    ServicesProfileConfig, ServicesSource,
};
pub use shutdown::{
    MAX_DRAIN_GRACE_SECS, MAX_PRE_STOP_DELAY_SECS, STREAM_FLUSH_GRACE_SECS, ShutdownConfig,
};
pub use site::SiteConfig;
pub use storage::{GcsCredentials, StorageBackend, StorageConfig};
pub use style::ProfileStyle;
pub use vault::{
    DEFAULT_VAULT_RETRIES, DEFAULT_VAULT_TIMEOUT_SECS, MAX_VAULT_RETRIES, MAX_VAULT_TIMEOUT_SECS,
    VaultAuth, VaultKeyRef, VaultSecretsConfig,
};

use serde::{Deserialize, Serialize};
use std::path::Path;
use systemprompt_identifiers::ExtensionId;

use crate::env::{interpolate, read_env_optional};

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtensionsConfig {
    #[serde(default)]
    pub disabled: Vec<ExtensionId>,
}

impl ExtensionsConfig {
    pub fn is_disabled(&self, extension_id: &ExtensionId) -> bool {
        self.disabled.contains(extension_id)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ProfileType {
    #[default]
    Local,
    Cloud,
}

impl ProfileType {
    pub const fn is_cloud(&self) -> bool {
        matches!(self, Self::Cloud)
    }

    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,

    pub display_name: String,

    #[serde(default)]
    pub target: ProfileType,

    pub site: SiteConfig,

    pub database: DatabaseConfig,

    pub server: ServerConfig,

    pub paths: PathsConfig,

    pub security: SecurityConfig,

    pub rate_limits: RateLimitsConfig,

    pub system_admin: crate::services::SystemAdminConfig,

    #[serde(default)]
    pub runtime: RuntimeConfig,

    #[serde(default)]
    pub cloud: Option<CloudConfig>,

    #[serde(default)]
    pub secrets: Option<SecretsConfig>,

    #[serde(default)]
    pub extensions: ExtensionsConfig,

    #[serde(default)]
    pub governance: Option<GovernanceConfig>,

    // Why: renamed from `evaluation` in 5c8d3ae9e; the profile denies unknown
    // fields, so a deployed profile.yaml still on the old key would refuse to
    // boot. See the same alias on PluginHooksRef.
    #[serde(default, alias = "evaluation")]
    pub judge: JudgeProfile,

    #[serde(default)]
    pub retention: RetentionConfig,

    #[serde(default)]
    pub services: ServicesProfileConfig,

    #[serde(default)]
    pub storage: StorageConfig,

    #[serde(default)]
    pub observability: ObservabilityConfig,

    #[serde(default)]
    pub config_planes: ConfigPlanesConfig,
}

const MOVED_SECTIONS: &[(&str, &str)] = &[
    ("providers", "services/ai/providers.yaml"),
    ("gateway", "services/ai/gateway.yaml"),
];

fn reject_moved_sections(content: &str, profile_path: &Path) -> ProfileResult<()> {
    let Ok(serde_yaml::Value::Mapping(map)) = serde_yaml::from_str::<serde_yaml::Value>(content)
    else {
        return Ok(());
    };
    for (key, destination) in MOVED_SECTIONS {
        if map.contains_key(serde_yaml::Value::String((*key).to_owned())) {
            return Err(ProfileError::MovedToServices {
                path: profile_path.to_path_buf(),
                key: (*key).to_owned(),
                destination: (*destination).to_owned(),
            });
        }
    }
    Ok(())
}

impl Profile {
    #[must_use]
    pub fn is_local_trial(&self) -> bool {
        self.cloud
            .as_ref()
            .map_or_else(|| self.target.is_local(), CloudConfig::is_local_trial)
    }

    #[must_use]
    pub const fn path_resolution(&self) -> crate::paths::PathResolution {
        if self.target.is_cloud() {
            crate::paths::PathResolution::Lexical
        } else {
            crate::paths::PathResolution::Canonicalize
        }
    }

    pub fn from_yaml(content: &str, profile_path: &Path) -> ProfileResult<Self> {
        let content = interpolate(content, &|name| read_env_optional(name));

        reject_moved_sections(&content, profile_path)?;

        let mut profile: Self =
            serde_yaml::from_str(&content).map_err(|source| ProfileError::ParseYaml {
                path: profile_path.to_path_buf(),
                source,
            })?;

        let profile_dir =
            profile_path
                .parent()
                .ok_or_else(|| ProfileError::InvalidProfilePath {
                    path: profile_path.to_path_buf(),
                })?;

        profile.paths.resolve_relative_to(profile_dir);

        if let Some(secrets) = profile.secrets.as_ref() {
            secrets.validate()?;
        }

        Ok(profile)
    }

    pub fn to_yaml(&self) -> ProfileResult<String> {
        serde_yaml::to_string(self).map_err(ProfileError::SerializeYaml)
    }

    pub fn payload_cap_bytes(&self) -> usize {
        self.governance
            .as_ref()
            .map_or(AuditConfig::DEFAULT_PAYLOAD_CAP_BYTES, |governance| {
                governance.audit.payload_cap_bytes
            })
    }

    pub fn quota_config(&self) -> QuotaConfig {
        self.governance
            .as_ref()
            .map_or_else(QuotaConfig::default, |governance| governance.quota)
    }

    pub fn profile_style(&self) -> ProfileStyle {
        match self.name.to_lowercase().as_str() {
            "dev" | "development" | "local" => ProfileStyle::Development,
            "prod" | "production" => ProfileStyle::Production,
            "staging" | "stage" => ProfileStyle::Staging,
            "test" | "testing" => ProfileStyle::Test,
            _ => ProfileStyle::Custom,
        }
    }
}
