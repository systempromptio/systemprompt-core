//! The signed manifest (PROTOCOL section 9, "The manifest").
//!
//! `GET /v1/bridge/manifest` answers a
//! [`SignedManifestEnvelope`](crate::wire::manifest::SignedManifestEnvelope):
//! `payload` is the JCS-canonical manifest as a string, `signature` a detached
//! ed25519 signature (base64) over its exact UTF-8 bytes. The signature is
//! checked over the raw string before anything is parsed;
//! [`Manifest`](crate::wire::manifest::Manifest) is then decoded from
//! it. Unknown fields are ignored, so fields a newer instance adds never break
//! this client; a semantic break is declared by raising `min_schema_version`
//! above [`SUPPORTED_SCHEMA_VERSION`](crate::wire::manifest::SUPPORTED_SCHEMA_VERSION).
//!
//! The manifest body is defined here rather than taken from
//! `systemprompt-models`: the harness reads host lists as plain strings (a host
//! id the models crate does not know must not fail the whole manifest) and
//! reads only the settings block of its own host from a marketplace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod entries;
mod settings;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
pub use entries::{
    AgentEntry, ManagedMcpServer, PluginEntry, PluginFile, PluginHooks, RuleEntry, Selection,
    SkillEntry, SkillSource,
};
use serde::{Deserialize, Serialize};
pub use settings::{HostSettings, Marketplace, ThemeMode, ThemeRef};
pub use systemprompt_models::bridge::ids::{ManifestSignature, Sha256Digest, ToolPolicy};
pub use systemprompt_models::bridge::manifest::SignedManifestEnvelope;

use crate::ids::{ManagedResourceId, PublicationId, ResourceRevisionId, TenantId, UserId};
use crate::version::ManifestVersion;

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[must_use]
pub fn probe_schema_version(payload: &str) -> Option<u32> {
    #[derive(Deserialize)]
    struct Probe {
        #[serde(default)]
        min_schema_version: Option<u32>,
    }
    serde_json::from_str::<Probe>(payload)
        .ok()?
        .min_schema_version
}

/// The manifest body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[expect(
    clippy::struct_field_names,
    reason = "manifest_version is the instance's wire field name"
)]
pub struct Manifest {
    #[serde(default)]
    pub min_schema_version: u32,
    #[serde(default)]
    pub min_bridge_version: Option<String>,
    #[serde(default)]
    pub manifest_version: Option<ManifestVersion>,
    #[serde(default)]
    pub issued_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub not_before: Option<DateTime<Utc>>,
    pub user_id: UserId,
    #[serde(default)]
    pub tenant_id: Option<TenantId>,
    #[serde(default)]
    pub user: Option<ManifestUser>,
    #[serde(default)]
    pub rules: Vec<RuleEntry>,
    #[serde(default)]
    pub skills: Vec<SkillEntry>,
    #[serde(default)]
    pub agents: Vec<AgentEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
    pub plugins: Vec<PluginEntry>,
    #[serde(default)]
    pub marketplaces: Vec<Marketplace>,
    #[serde(default)]
    pub enabled_hosts: Vec<String>,
    #[serde(default)]
    pub host_model_protocols: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub host_extensions: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub revocations: Vec<String>,
    // JSON: instance branding is free-form display data the instance owns.
    #[serde(default)]
    pub branding: Option<serde_json::Map<String, serde_json::Value>>,
}

impl Manifest {
    #[must_use]
    pub const fn schema_too_new(&self) -> bool {
        self.min_schema_version > SUPPORTED_SCHEMA_VERSION
    }

    #[must_use]
    pub fn governance_plugin(&self) -> Option<&PluginEntry> {
        self.plugins.iter().find(|plugin| plugin.hooks.governance)
    }

    #[must_use]
    pub fn model_families(&self, host: &str) -> Option<&[String]> {
        self.host_model_protocols.get(host).map(Vec::as_slice)
    }

    #[must_use]
    pub fn extensions_for(&self, host: &str) -> Option<&[String]> {
        self.host_extensions.get(host).map(Vec::as_slice)
    }

    #[must_use]
    pub fn host_settings(&self) -> Option<&HostSettings> {
        self.marketplaces
            .iter()
            .find_map(|marketplace| marketplace.harness.as_ref())
    }
}

/// The signed-in user as the manifest names them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestUser {
    #[serde(default)]
    pub id: Option<UserId>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
}

/// The signed publication whose bundle carries a skill's runtime files.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[expect(
    clippy::struct_field_names,
    reason = "publication_id is the instance's wire field name"
)]
pub struct Publication {
    pub publication_id: PublicationId,
    pub resource_id: ManagedResourceId,
    pub revision_id: ResourceRevisionId,
    pub generation: i64,
    pub bundle_digest: Sha256Digest,
}
