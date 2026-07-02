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
    AgentEntry, ArtifactEntry, HookEntry, ManagedMcpServer, PluginEntry, SkillEntry, UserInfo,
};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_models::services::ServicesConfig;
use systemprompt_security::manifest_signing;

use crate::candidate::MarketplaceCandidate;
use crate::catalog::{CatalogContent, load_hooks, load_plugins};
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
    pub host_model_protocols: &'a std::collections::BTreeMap<String, Vec<String>>,
    pub artifacts: &'a [ArtifactEntry],
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ManifestService;

impl ManifestService {
    pub async fn assemble_candidate(
        services: &ServicesConfig,
        services_root: &Path,
        api_external_url: &str,
        filter: &dyn MarketplaceFilter,
        user_id: &UserId,
    ) -> Result<MarketplaceCandidate, MarketplaceError> {
        let catalog = CatalogContent::load(services, services_root, api_external_url)?;
        let hooks = load_hooks(services_root)?;
        let plugins = load_plugins(services, &catalog.as_content())?;
        let (skills, agents, managed_mcp_servers, artifacts) = catalog.into_parts();

        let active = active_marketplace(services);
        let (skills, agents, managed_mcp_servers, artifacts) = match active {
            Some(mp) => (
                scope_to_marketplace(skills, &mp.skills.include, |s| s.id.as_str()),
                scope_to_marketplace(agents, &mp.agents.include, |a| a.id.as_str()),
                scope_to_marketplace(managed_mcp_servers, &mp.mcp_servers.include, |m| {
                    m.name.as_str()
                }),
                scope_to_marketplace(artifacts, &mp.artifacts.include, |a| a.id.as_str()),
            ),
            None => (skills, agents, managed_mcp_servers, artifacts),
        };

        let artifacts = gate_artifacts_by_plugin(artifacts, &plugins);

        let mut candidate = MarketplaceCandidate::new(
            plugins,
            skills,
            agents,
            hooks,
            managed_mcp_servers,
            artifacts,
        );
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

fn gate_artifacts_by_plugin(
    artifacts: Vec<ArtifactEntry>,
    plugins: &[PluginEntry],
) -> Vec<ArtifactEntry> {
    let plugin_ids: std::collections::HashSet<&str> =
        plugins.iter().map(|p| p.id.as_str()).collect();
    artifacts
        .into_iter()
        .filter(|a| {
            let kept = plugin_ids.contains(a.plugin_id.as_str());
            if !kept {
                tracing::warn!(
                    artifact_id = %a.id.as_str(),
                    plugin_id = %a.plugin_id.as_str(),
                    "marketplace: artifact's owning plugin is not enabled/selected; skipping"
                );
            }
            kept
        })
        .collect()
}
