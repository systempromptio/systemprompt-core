//! Manifest assembly and sealing service.
//!
//! [`ManifestService`] assembles a scoped, filtered [`MarketplaceCandidate`]
//! from the on-disk catalogue and seals a built [`SignedManifest`] into a
//! [`SignedManifestEnvelope`]: the manifest's JCS-canonical serialization
//! carried verbatim as the envelope payload, signed byte-for-byte. The bridge
//! verifies over those exact bytes before parsing, so there is no second
//! canonical view to keep in sync with the manifest struct.
//!
//! The manifest is the union of every enabled marketplace: an entry any one of
//! them includes is assembled, and the per-user filter decides who sees it.
//!
//! Manifest skills are derived, never configured: the skills array is the
//! union of skills the enabled, marketplace-included plugins actually ship
//! (plugin `skills` refs plus their included agents' skill refs), so it cannot
//! diverge from what the plugin bundles deliver.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod diagnostics;
mod scoping;

use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::ids::{LibraryArtifactId, ManifestSignature, PluginId, SkillId};
use systemprompt_models::bridge::manifest::{
    ManifestMarketplace, SignedManifest, SignedManifestEnvelope,
};
use systemprompt_models::services::{MarketplaceConfig, ServicesConfig};
use systemprompt_security::manifest_signing;

use crate::candidate::MarketplaceCandidate;
use crate::catalog::{CatalogContent, artifact_owners, load_hooks, load_plugins, skill_owners};
use crate::error::MarketplaceError;
use crate::filter::MarketplaceFilter;
use crate::membership::MarketplaceMembership;
use crate::trace::{NoopTrace, TraceSink, TraceStage};
use diagnostics::{plugin_inclusion_diagnostics, record_removed, snapshot};
use scoping::{gate_artifacts_by_plugin, gate_skills_by_plugin, prune_traced, scope_all};

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
        Self::assemble_candidate_traced(
            services,
            services_root,
            api_external_url,
            filter,
            user_id,
            &mut NoopTrace,
        )
        .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "assemble_candidate's parameter list plus the trace sink; a wrapper struct \
                  would only relocate the same fan-in"
    )]
    pub async fn assemble_candidate_traced(
        services: &ServicesConfig,
        services_root: &Path,
        api_external_url: &str,
        filter: &dyn MarketplaceFilter,
        user_id: &UserId,
        trace: &mut dyn TraceSink,
    ) -> Result<MarketplaceCandidate, MarketplaceError> {
        let catalog =
            CatalogContent::load_traced(services, services_root, api_external_url, trace)?;
        let hooks = load_hooks(services_root)?;
        let plugins = load_plugins(services, &catalog.as_content())?;
        let skill_owners = skill_owners(services, &catalog.as_content())?;
        let selected_skills: BTreeSet<SkillId> = skill_owners.keys().cloned().collect();
        let (skills, agents, managed_mcp_servers, artifacts) = catalog.into_parts();

        let enabled = services.enabled_marketplaces();
        let marketplaces = listed_marketplaces(services, &enabled)?;
        let (agents, managed_mcp_servers, artifacts) =
            scope_all(&enabled, agents, managed_mcp_servers, artifacts, trace);
        let membership =
            MarketplaceMembership::from_services(services, &agents, &managed_mcp_servers);

        let mut diagnostics = plugin_inclusion_diagnostics(services, &skills, &agents);
        let skills = gate_skills_by_plugin(skills, &selected_skills, trace);

        let owners = artifact_owners(services, &artifacts)?;
        let selected_artifacts: BTreeSet<LibraryArtifactId> = owners.keys().cloned().collect();
        let artifacts = gate_artifacts_by_plugin(artifacts, &selected_artifacts, trace);

        let candidate = MarketplaceCandidate {
            plugins,
            skills,
            agents,
            hooks,
            managed_mcp_servers,
            artifacts,
            marketplaces,
            ..MarketplaceCandidate::default()
        }
        .with_artifact_owners(owners)
        .with_skill_owners(skill_owners)
        .with_membership(membership);

        for d in &diagnostics {
            tracing::warn!(diagnostic = %d, "marketplace: manifest diagnostic");
        }

        let pre_filter = snapshot(&candidate);
        let mut filtered = filter.filter(user_id, candidate).await?;
        record_removed(
            &pre_filter,
            &snapshot(&filtered),
            TraceStage::AccessFilter,
            "removed by the per-user marketplace filter",
            trace,
        );

        prune_traced(&mut filtered, trace);

        diagnostics.extend(std::mem::take(&mut filtered.diagnostics));
        filtered.diagnostics = diagnostics;
        Ok(filtered)
    }

    pub fn seal(manifest: &SignedManifest) -> Result<SignedManifestEnvelope, MarketplaceError> {
        let payload = manifest_signing::canonicalize(manifest)
            .map_err(|e| MarketplaceError::Signing(e.to_string()))?;
        let signature = manifest_signing::sign_bytes(payload.as_bytes())
            .map_err(|e| MarketplaceError::Signing(e.to_string()))?;
        Ok(SignedManifestEnvelope {
            payload,
            signature: ManifestSignature::new(signature),
        })
    }
}

// Why: seeded from the same plugin selection `MarketplaceMembership` reads, so
// the listed plugin ids and the authz membership cannot disagree; the
// intersection with the plugins that survive filtering happens at
// `into_manifest_parts`, once the final plugin list is known.
fn listed_marketplaces(
    services: &ServicesConfig,
    enabled: &[&MarketplaceConfig],
) -> Result<Vec<ManifestMarketplace>, MarketplaceError> {
    enabled
        .iter()
        .map(|marketplace| {
            let plugin_ids = services
                .marketplace_plugin_configs(marketplace)
                .iter()
                .map(|p| {
                    PluginId::try_new(p.id.as_str())
                        .map_err(|e| MarketplaceError::Catalog(e.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ManifestMarketplace {
                id: marketplace.id.clone(),
                name: marketplace.name.clone(),
                plugin_ids,
            })
        })
        .collect()
}
