//! Canonical manifest view and assembly/signing service.
//!
//! [`CanonicalView`] is the JCS-canonicalised payload that is signed and then
//! verified bridge-side. [`ManifestService`] assembles a scoped, filtered
//! [`MarketplaceCandidate`] from the on-disk catalogue and signs a built view.

use std::path::Path;

use serde::Serialize;
use systemprompt_identifiers::{TenantId, UserId};
use systemprompt_models::bridge::ids::ManifestSignature;
use systemprompt_models::bridge::manifest::{
    AgentEntry, HookEntry, ManagedMcpServer, PluginEntry, SkillEntry, UserInfo,
};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_models::services::ServicesConfig;
use systemprompt_security::manifest_signing;

use crate::candidate::MarketplaceCandidate;
use crate::catalog::{
    load_agents, load_hooks, load_managed_mcp_servers, load_plugins, load_skills,
};
use crate::error::MarketplaceError;
use crate::filter::MarketplaceFilter;
use crate::scope::{active_marketplace, scope_to_marketplace};

// Why: must mirror the field set and order (alphabetical, after JCS sort) of
// the verifier-side `CanonicalView` in `bin/bridge/src/gateway/manifest.rs` so
// signer + verifier produce identical canonical bytes.
#[derive(Debug, Serialize)]
pub struct CanonicalView<'a> {
    pub manifest_version: &'a ManifestVersion,
    pub issued_at: &'a str,
    pub not_before: &'a str,
    pub user_id: &'a UserId,
    pub tenant_id: Option<&'a TenantId>,
    pub user: Option<&'a UserInfo>,
    pub plugins: &'a [PluginEntry],
    pub skills: &'a [SkillEntry],
    pub agents: &'a [AgentEntry],
    pub hooks: &'a [HookEntry],
    pub managed_mcp_servers: &'a [ManagedMcpServer],
    pub revocations: &'a [String],
    pub enabled_hosts: &'a [String],
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ManifestService;

impl ManifestService {
    /// Load the catalogue, scope it to the active marketplace, then apply the
    /// per-user filter, yielding the candidate to be signed.
    pub async fn assemble_candidate(
        services: &ServicesConfig,
        services_root: &Path,
        api_external_url: &str,
        filter: &dyn MarketplaceFilter,
        user_id: &UserId,
    ) -> Result<MarketplaceCandidate, MarketplaceError> {
        let skills = load_skills(services_root)?;
        let agents = load_agents(services, api_external_url);
        let hooks = load_hooks(services_root)?;
        let plugins = load_plugins(services_root, services);
        let managed_mcp_servers = load_managed_mcp_servers(services, api_external_url)?;

        let active = active_marketplace(services);
        let (skills, agents, plugins, managed_mcp_servers) = match active {
            Some(mp) => (
                scope_to_marketplace(skills, &mp.skills.include, |s| s.id.as_str()),
                scope_to_marketplace(agents, &mp.agents.include, |a| a.id.as_str()),
                scope_to_marketplace(plugins, &mp.plugins.include, |p| p.id.as_str()),
                scope_to_marketplace(managed_mcp_servers, &mp.mcp_servers.include, |m| {
                    m.name.as_str()
                }),
            ),
            None => (skills, agents, plugins, managed_mcp_servers),
        };

        let mut candidate =
            MarketplaceCandidate::new(plugins, skills, agents, hooks, managed_mcp_servers);
        if let Some(mp) = active {
            candidate = candidate.with_marketplace(mp.id.clone(), Some(mp.access.clone()));
        }
        Ok(filter.filter(user_id, candidate).await?)
    }

    pub fn sign(view: &CanonicalView<'_>) -> Result<ManifestSignature, MarketplaceError> {
        let signature = manifest_signing::sign_value(view)
            .map_err(|e| MarketplaceError::Signing(e.to_string()))?;
        Ok(ManifestSignature::new(signature))
    }
}
