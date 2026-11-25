//! The bridge catalogue contract: one user's resolved marketplace catalogue,
//! as the bridge manifest and plugin routes serve it.
//!
//! The `managed_resources` member owns the resolution (the disk catalogue with
//! the managed overlay, scoped and filtered for the user, memoised per user)
//! and provides it as [`DynBridgeCatalogSource`] after governance, open as
//! the repository, cache and seams it reads are. The `gateway-http` member
//! builds its bridge routes over it, and issues an enrolled device's consumer
//! credential through it; a composition without that member provides none,
//! and no bridge route is built.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use systemprompt_identifiers::{DeviceCertId, DeviceId, ManagedResourceId, PluginId, UserId};
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, HookEntry, ManagedMcpServer, ManifestMarketplace, PluginEntry,
    RuleEntry, SkillEntry,
};
use systemprompt_traits::BoxedSource;

/// The entry lists a candidate contributes to the signed wire manifest.
#[derive(Debug, Clone, Default)]
pub struct ManifestEntries {
    pub plugins: Vec<PluginEntry>,
    pub skills: Vec<SkillEntry>,
    pub rules: Vec<RuleEntry>,
    pub agents: Vec<AgentEntry>,
    pub hooks: Vec<HookEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
    pub artifacts: Vec<ArtifactEntry>,
    pub marketplaces: Vec<ManifestMarketplace>,
    pub diagnostics: Vec<String>,
}

/// Whether a resolution may be served from the per-user memo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogFreshness {
    Memo,
    Fresh,
}

/// The installation facts a resolution is scoped by.
#[derive(Debug, Clone, Copy)]
pub struct CatalogScope<'a> {
    pub services: &'a ServicesConfig,
    pub services_root: &'a Path,
    pub api_external_url: &'a str,
    pub owner: &'a UserId,
}

#[derive(Debug, thiserror::Error)]
pub enum BridgeCatalogError {
    #[error("catalogue resolution failed")]
    Resolve(#[source] BoxedSource),
    #[error("catalogue grants not recorded")]
    Grants(#[source] BoxedSource),
    #[error("plugin packing failed")]
    Pack(#[source] BoxedSource),
    #[error("device credential issue failed")]
    Credential(#[source] BoxedSource),
}

/// The `sp_device_` consumer credential issued to an enrolled device.
#[derive(Serialize, schemars::JsonSchema)]
pub struct IssuedConsumerCredential {
    pub device_id: DeviceId,
    pub consumer_id: UserId,
    pub credential: String,
}

impl std::fmt::Debug for IssuedConsumerCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuedConsumerCredential")
            .field("device_id", &self.device_id)
            .field("consumer_id", &self.consumer_id)
            .finish_non_exhaustive()
    }
}

/// A plugin file of a resolved catalogue, or which half of the lookup missed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginFile<'a> {
    NoPlugin,
    NoFile,
    Bytes(&'a [u8]),
}

/// One user's resolved catalogue.
pub trait ResolvedBridgeCatalog: Send + Sync {
    fn plugins(&self) -> &[PluginEntry];

    fn skill_publications(&self) -> Vec<ManagedResourceId>;

    fn manifest_entries(&self) -> ManifestEntries;

    fn plugin_file(&self, plugin: &PluginId, path: &str) -> PluginFile<'_>;

    fn pack_plugin(&self, plugin: &PluginId) -> Option<Result<Vec<u8>, BridgeCatalogError>>;
}

pub type DynResolvedBridgeCatalog = Arc<dyn ResolvedBridgeCatalog>;

/// Resolves and memoises users' catalogues and records the grants a served
/// manifest carries.
///
/// `#[async_trait]` because the source is held as `Arc<dyn
/// BridgeCatalogSource>`.
#[async_trait]
pub trait BridgeCatalogSource: Send + Sync {
    async fn resolve(
        &self,
        scope: CatalogScope<'_>,
        user: &UserId,
        freshness: CatalogFreshness,
    ) -> Result<DynResolvedBridgeCatalog, BridgeCatalogError>;

    fn latest(&self, user: &UserId) -> Option<DynResolvedBridgeCatalog>;

    async fn retain_catalog_grants(
        &self,
        owner: &UserId,
        resources: &[ManagedResourceId],
        user: &UserId,
    ) -> Result<(), BridgeCatalogError>;

    async fn issue_consumer_credential(
        &self,
        cert: &DeviceCertId,
    ) -> Result<IssuedConsumerCredential, BridgeCatalogError>;
}

pub type DynBridgeCatalogSource = Arc<dyn BridgeCatalogSource>;
