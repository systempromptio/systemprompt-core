//! Resolved catalogue shared by the manifest and byte-serving paths.
//!
//! [`CatalogContent`] owns the loaded skills, agents, and managed MCP servers
//! plus the plugins root, and is the single place the three loaders run for
//! bundle assembly. Both the signed-manifest projection and the gateway
//! byte-serving path build their [`BundleContent`] from one of these, so the
//! two paths cannot resolve the catalogue two different ways and drift.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, ManagedMcpServer, SkillEntry,
};
use systemprompt_models::services::ServicesConfig;

use crate::bundle::BundleContent;
use crate::catalog::fingerprint::hash_dir_metadata;
use crate::catalog::{
    disabled_mcp_server_names, load_agents, load_artifacts, load_managed_mcp_servers, load_skills,
};
use crate::error::MarketplaceError;

#[derive(Debug)]
pub struct CatalogContent {
    skills: Vec<SkillEntry>,
    agents: Vec<AgentEntry>,
    managed_mcp_servers: Vec<ManagedMcpServer>,
    disabled_mcp_servers: BTreeSet<String>,
    artifacts: Vec<ArtifactEntry>,
    plugins_root: PathBuf,
}

impl CatalogContent {
    pub fn load(
        services: &ServicesConfig,
        services_root: &Path,
        api_external_url: &str,
    ) -> Result<Self, MarketplaceError> {
        Ok(Self {
            skills: load_skills(services_root)?,
            agents: load_agents(services, api_external_url),
            managed_mcp_servers: load_managed_mcp_servers(services, api_external_url)?,
            disabled_mcp_servers: disabled_mcp_server_names(services),
            artifacts: load_artifacts(services_root)?,
            plugins_root: services_root.join("plugins"),
        })
    }

    /// Memoized [`load`](Self::load): reuses the cached catalogue while the
    /// fingerprint of the services config and the on-disk skills tree is
    /// unchanged, so the bridge's per-file sync requests stop re-reading and
    /// re-parsing every skill on each call.
    pub fn load_cached(
        services: &ServicesConfig,
        services_root: &Path,
        api_external_url: &str,
    ) -> Result<Arc<Self>, MarketplaceError> {
        let fingerprint = catalog_fingerprint(services, services_root, api_external_url)?;
        let cache = CATALOG_CACHE.get_or_init(|| RwLock::new(None));

        let hit = {
            let guard = cache
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard
                .as_ref()
                .filter(|(cached_fp, _)| *cached_fp == fingerprint)
                .map(|(_, catalog)| Arc::clone(catalog))
        };
        if let Some(catalog) = hit {
            return Ok(catalog);
        }

        let catalog = Arc::new(Self::load(services, services_root, api_external_url)?);
        let mut guard = cache
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some((fingerprint, Arc::clone(&catalog)));
        drop(guard);
        Ok(catalog)
    }

    #[must_use]
    pub fn as_content(&self) -> BundleContent<'_> {
        BundleContent {
            skills: &self.skills,
            agents: &self.agents,
            mcp_servers: &self.managed_mcp_servers,
            disabled_mcp_servers: &self.disabled_mcp_servers,
            plugins_root: &self.plugins_root,
        }
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Vec<SkillEntry>,
        Vec<AgentEntry>,
        Vec<ManagedMcpServer>,
        Vec<ArtifactEntry>,
    ) {
        (
            self.skills,
            self.agents,
            self.managed_mcp_servers,
            self.artifacts,
        )
    }
}

type CatalogCache = OnceLock<RwLock<Option<([u8; 32], Arc<CatalogContent>)>>>;

static CATALOG_CACHE: CatalogCache = OnceLock::new();

fn catalog_fingerprint(
    services: &ServicesConfig,
    services_root: &Path,
    api_external_url: &str,
) -> Result<[u8; 32], MarketplaceError> {
    let mut hasher = Sha256::new();
    let config =
        serde_json::to_vec(services).map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    hasher.update((config.len() as u64).to_le_bytes());
    hasher.update(&config);
    hasher.update(services_root.as_os_str().as_encoded_bytes());
    hasher.update(b"\0");
    hasher.update(api_external_url.as_bytes());
    hasher.update(b"\0");
    hash_dir_metadata(&mut hasher, &services_root.join("skills"));
    hash_dir_metadata(&mut hasher, &services_root.join("artifacts"));
    Ok(hasher.finalize().into())
}
