//! Profile configuration models — the deserialized shape of a
//! `.systemprompt/profiles/<name>/profile.yaml` document.
//!
//! A profile is `profiles/<env>/profile.yaml` layered over the optional
//! shared `profiles/base.yaml` (see [`layered`]). Covers server, database,
//! paths, secrets, security, rate limits, governance, and runtime sections,
//! plus validation rules and environment-variable interpolation. URLs that
//! follow from `server.external_url` are derived at load (see [`derived`]);
//! pre-0.73 shapes are refused with the `admin config migrate` hint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod brain;
mod cloud;
mod config_planes;
mod database;
mod deploy;
pub mod derived;
mod error;
mod external_secrets;
mod governance;
mod judge;
pub mod layered;
mod legacy;
mod licence;
mod masking;
mod node_role;
mod notifications;
mod observability;
mod oci_reference;
pub mod overlay;
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

pub use brain::{
    AutoConfirm, AutoConfirmPolicy, BrainCaptureProfile, BrainConsumerProfile,
    BrainExtractionProfile, BrainLifecycleProfile, BrainProfile, BrainRecallProfile, CaptureSource,
    StalePolicy,
};
pub use cloud::{CloudConfig, CloudValidationMode, HostingProvider, UnknownHostingProvider};
pub use config_planes::{ConfigPlanesConfig, PlaneAuthority};
pub use database::{DatabaseConfig, PoolConfig, RoleMaxConnections};
pub use deploy::{
    DEFAULT_GITOPS_COMMIT_MESSAGE, DeployConfig, GitOpsConfig, OperatorAuthConfig,
    default_operator_scopes,
};
pub use error::{ProfileError, ProfileResult};
pub use external_secrets::{ExternalSecretsConfig, MIN_EXTERNAL_REFRESH_SECS};
pub use governance::{
    AuditConfig, AuthzConfig, AuthzHookConfig, AuthzMode, GovernanceConfig, QuotaConfig,
    UNRESTRICTED_ACKNOWLEDGEMENT, UnappliedPlaneMode,
};
pub use judge::JudgeProfile;
pub use layered::{BASE_PROFILE_FILE, OVERLAY_PROFILE_FILE, merge_yaml};
pub use legacy::MIGRATE_COMMAND;
pub use licence::{LICENCE_SIGNING_SECRET, LicenceConfig};
pub use node_role::{NodeRole, NodeRoleParseError};
pub use notifications::{ConsumerProfile, NotificationsProfile};
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
pub use validation::committed_source_errors;
pub use vault::{
    DEFAULT_VAULT_RETRIES, DEFAULT_VAULT_TIMEOUT_SECS, MAX_VAULT_RETRIES, MAX_VAULT_TIMEOUT_SECS,
    VaultAuth, VaultKeyRef, VaultSecretsConfig,
};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ExtensionId;


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

    #[serde(default)]
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

    #[serde(default)]
    pub licence: LicenceConfig,

    #[serde(default)]
    pub notifications: NotificationsProfile,

    #[serde(default)]
    pub brain: BrainProfile,
}

impl Profile {
    #[must_use]
    pub fn is_local_trial(&self) -> bool {
        self.cloud
            .as_ref()
            .map_or_else(|| self.target.is_local(), CloudConfig::is_local_trial)
    }

    #[must_use]
    pub fn hosting_provider(&self) -> HostingProvider {
        self.cloud
            .as_ref()
            .map_or_else(HostingProvider::default, |cloud| cloud.provider)
    }

    pub fn is_deployment_host(&self, lookup: impl Fn(&str) -> Option<String>) -> bool {
        self.hosting_provider().is_deployment_host(lookup)
    }

    #[must_use]
    pub const fn path_resolution(&self) -> crate::paths::PathResolution {
        if self.target.is_cloud() {
            crate::paths::PathResolution::Lexical
        } else {
            crate::paths::PathResolution::Canonicalize
        }
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

    pub fn unapplied_plane_mode(&self) -> UnappliedPlaneMode {
        self.governance
            .as_ref()
            .map_or_else(UnappliedPlaneMode::default, |governance| {
                governance.unapplied_planes
            })
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
