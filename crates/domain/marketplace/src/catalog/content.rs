//! Resolved catalogue shared by the manifest and byte-serving paths.
//!
//! [`CatalogContent`] owns the loaded skills, agents, and managed MCP servers
//! plus the plugins root, and is the single place the three loaders run for
//! bundle assembly. Both the signed-manifest projection and the gateway
//! byte-serving path build their [`BundleContent`] from one of these, so the
//! two paths cannot resolve the catalogue two different ways and drift.

use std::path::{Path, PathBuf};

use systemprompt_models::bridge::manifest::{AgentEntry, ManagedMcpServer, SkillEntry};
use systemprompt_models::services::ServicesConfig;

use crate::bundle::BundleContent;
use crate::catalog::{load_agents, load_managed_mcp_servers, load_skills};
use crate::error::MarketplaceError;

#[derive(Debug)]
pub struct CatalogContent {
    skills: Vec<SkillEntry>,
    agents: Vec<AgentEntry>,
    managed_mcp_servers: Vec<ManagedMcpServer>,
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
            plugins_root: services_root.join("plugins"),
        })
    }

    #[must_use]
    pub fn as_content(&self) -> BundleContent<'_> {
        BundleContent {
            skills: &self.skills,
            agents: &self.agents,
            mcp_servers: &self.managed_mcp_servers,
            plugins_root: &self.plugins_root,
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (Vec<SkillEntry>, Vec<AgentEntry>, Vec<ManagedMcpServer>) {
        (self.skills, self.agents, self.managed_mcp_servers)
    }
}
