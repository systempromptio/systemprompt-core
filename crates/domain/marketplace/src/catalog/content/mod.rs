//! Resolved catalogue shared by the manifest and byte-serving paths.
//!
//! [`CatalogContent`] owns the loaded skills, rules, agents, and managed MCP
//! servers plus the plugins root, and is the single place the three loaders run
//! for bundle assembly. Both the signed-manifest projection and the gateway
//! byte-serving path build their [`BundleContent`] from one of these, so the
//! two paths cannot resolve the catalogue two different ways and drift.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::ids::SkillId;
use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, ManagedMcpServer, RuleEntry, SkillEntry,
};
use systemprompt_models::services::ServicesConfig;

use crate::bundle::BundleContent;
use crate::catalog::fingerprint::hash_dir_metadata;
use crate::catalog::{
    disabled_mcp_server_names, load_agents, load_artifacts, load_managed_mcp_servers,
    validate_artifact_tools,
};
use crate::error::MarketplaceError;
use crate::managed::RevisionFiles;

mod managed;

/// The owned entry lists a [`CatalogContent`] yields, in the order
/// [`CatalogContent::into_parts`] returns them: skills, rules, agents, managed
/// MCP servers, artifacts.
pub type CatalogParts = (
    Vec<SkillEntry>,
    Vec<RuleEntry>,
    Vec<AgentEntry>,
    Vec<ManagedMcpServer>,
    Vec<ArtifactEntry>,
);

#[derive(Debug, Clone)]
pub struct CatalogContent {
    skills: Vec<SkillEntry>,
    rules: Vec<RuleEntry>,
    agents: Vec<AgentEntry>,
    managed_mcp_servers: Vec<ManagedMcpServer>,
    disabled_mcp_servers: BTreeSet<String>,
    artifacts: Vec<ArtifactEntry>,
    plugins_root: PathBuf,
    managed_files: BTreeMap<SkillId, RevisionFiles>,
}

impl CatalogContent {
    pub fn load(
        services: &ServicesConfig,
        services_root: &Path,
        api_external_url: &str,
    ) -> Result<Self, MarketplaceError> {
        Self::load_traced(
            services,
            services_root,
            api_external_url,
            &mut crate::trace::NoopTrace,
        )
    }

    pub fn load_traced(
        services: &ServicesConfig,
        services_root: &Path,
        api_external_url: &str,
        trace: &mut dyn crate::trace::TraceSink,
    ) -> Result<Self, MarketplaceError> {
        Ok(Self {
            skills: crate::catalog::load_skills_traced(services_root, trace)?,
            rules: crate::catalog::load_rules_traced(services_root, trace)?,
            agents: load_agents(services, api_external_url),
            managed_mcp_servers: load_managed_mcp_servers(services, api_external_url)?,
            disabled_mcp_servers: disabled_mcp_server_names(services),
            artifacts: {
                let artifacts = load_artifacts(services_root)?;
                validate_artifact_tools(services, &artifacts)?;
                artifacts
            },
            plugins_root: services_root.join("plugins"),
            managed_files: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn as_content(&self) -> BundleContent<'_> {
        BundleContent {
            skills: &self.skills,
            rules: &self.rules,
            agents: &self.agents,
            mcp_servers: &self.managed_mcp_servers,
            disabled_mcp_servers: &self.disabled_mcp_servers,
            artifacts: &self.artifacts,
            plugins_root: &self.plugins_root,
            managed_files: &self.managed_files,
        }
    }

    #[must_use]
    pub fn into_parts(self) -> CatalogParts {
        (
            self.skills,
            self.rules,
            self.agents,
            self.managed_mcp_servers,
            self.artifacts,
        )
    }
}

pub(super) fn catalog_fingerprint(
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
    hash_dir_metadata(&mut hasher, &services_root.join("rules"));
    hash_dir_metadata(&mut hasher, &services_root.join("artifacts"));
    Ok(hasher.finalize().into())
}
