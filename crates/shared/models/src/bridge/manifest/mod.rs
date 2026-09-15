//! Signed manifest wire format.
//!
//! `GET /v1/bridge/manifest` returns a [`SignedManifestEnvelope`]: the
//! JCS-canonical serialization of a [`SignedManifest`] carried verbatim as
//! `payload`, plus a detached ed25519 signature over those exact bytes. The
//! bridge verifies the signature against the raw `payload` string *before*
//! deserialising it, so fields added to [`SignedManifest`] in newer gateways
//! never invalidate the signature on older bridges — unknown fields are
//! simply ignored at parse time. Semantic breaks that an older bridge cannot
//! safely ignore are declared by raising `min_schema_version` above
//! [`MANIFEST_SCHEMA_VERSION`] of the consuming bridge, which then refuses
//! with an upgrade message instead of a signature error.
//!
//! Signing, signature verification, and manifest construction live in
//! the bridge crate (`bin/bridge/src/gateway/manifest.rs`) alongside
//! the gateway client. Those layers pull in `ed25519-dalek` and
//! `serde_jcs` which are not appropriate dependencies for this
//! foundation crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod entries;
mod managed_mcp;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use crate::bridge::ids::ManifestSignature;
use crate::bridge::ids::PluginId;
use crate::bridge::manifest_version::ManifestVersion;
use crate::services::bridge_policy::AutoUpdatePolicy;
pub use crate::services::marketplace::{ExternalMarketplace, ExternalMarketplaceSource};
use systemprompt_identifiers::{ApiKeyId, MarketplaceId, TenantId, UserId};

pub use entries::{
    AgentEntry, ArtifactEntry, HookEntry, PluginEntry, PluginFile, RuleEntry, SkillEntry,
    SkillPublication,
};
pub use managed_mcp::ManagedMcpServer;

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[must_use]
pub const fn min_bridge_version() -> semver::Version {
    semver::Version::new(0, 28, 0)
}

#[must_use]
pub fn bridge_version_is_supported(reported: &str, floor: &semver::Version) -> bool {
    // Why: a version that cannot be parsed cannot be shown to meet the
    // floor, so it is refused rather than admitted.
    semver::Version::parse(reported).is_ok_and(|reported| reported >= *floor)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifestEnvelope {
    pub payload: String,
    pub signature: ManifestSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    #[serde(default)]
    pub min_schema_version: u32,
    #[serde(default)]
    pub min_bridge_version: Option<semver::Version>,
    pub manifest_version: ManifestVersion,
    pub issued_at: DateTime<Utc>,
    pub not_before: DateTime<Utc>,
    pub user_id: UserId,
    pub tenant_id: Option<TenantId>,
    #[serde(default)]
    pub user: Option<UserInfo>,
    pub plugins: Vec<PluginEntry>,
    #[serde(default)]
    pub skills: Vec<SkillEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleEntry>,
    #[serde(default)]
    pub agents: Vec<AgentEntry>,
    #[serde(default)]
    pub hooks: Vec<HookEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
    pub revocations: Vec<ApiKeyId>,
    #[serde(default)]
    pub enabled_hosts: Vec<String>,
    #[serde(default)]
    pub host_model_protocols: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactEntry>,
    #[serde(default)]
    pub allow_claude_ai_connectors: bool,
    #[serde(default)]
    pub auto_update: AutoUpdatePolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marketplaces: Vec<ManifestMarketplace>,
}

/// An enabled marketplace and the manifest plugins it carries, after the
/// per-user filter: `plugin_ids` never names a plugin absent from `plugins`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestMarketplace {
    pub id: MarketplaceId,
    pub name: String,
    pub plugin_ids: Vec<PluginId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_cross_marketplace_dependencies_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_marketplaces: Vec<ExternalMarketplace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: UserId,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
}
