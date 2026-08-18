//! Manifest assembly and sealing service.
//!
//! [`ManifestService`] assembles a scoped, filtered [`MarketplaceCandidate`]
//! from the on-disk catalogue and seals a built [`SignedManifest`] into a
//! [`SignedManifestEnvelope`]: the manifest's JCS-canonical serialization
//! carried verbatim as the envelope payload, signed byte-for-byte. The bridge
//! verifies over those exact bytes before parsing, so there is no second
//! canonical view to keep in sync with the manifest struct.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::ids::{LibraryArtifactId, ManifestSignature};
use systemprompt_models::bridge::manifest::{
    ArtifactEntry, SignedManifest, SignedManifestEnvelope,
};
use systemprompt_models::services::ServicesConfig;
use systemprompt_security::manifest_signing;

use crate::candidate::MarketplaceCandidate;
use crate::catalog::{CatalogContent, artifact_owners, load_hooks, load_plugins};
use crate::error::MarketplaceError;
use crate::filter::MarketplaceFilter;
use crate::scope::{active_marketplace, scope_to_marketplace};

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

        let owners = artifact_owners(services, &artifacts)?;
        let selected_artifacts: std::collections::BTreeSet<LibraryArtifactId> =
            owners.keys().cloned().collect();
        let artifacts = gate_artifacts_by_plugin(artifacts, &selected_artifacts);

        let mut candidate = MarketplaceCandidate::new(
            plugins,
            skills,
            agents,
            hooks,
            managed_mcp_servers,
            artifacts,
        )
        .with_artifact_owners(owners);
        if let Some(mp) = active {
            candidate = candidate.with_marketplace(mp.id.clone(), Some(mp.access.clone()));
        }
        let mut filtered = filter.filter(user_id, candidate).await?;
        filtered.prune_orphaned_artifacts();
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

fn gate_artifacts_by_plugin(
    artifacts: Vec<ArtifactEntry>,
    selected: &std::collections::BTreeSet<LibraryArtifactId>,
) -> Vec<ArtifactEntry> {
    for id in selected {
        if !artifacts.iter().any(|a| &a.id == id) {
            tracing::warn!(
                artifact_id = %id,
                "marketplace: plugin artifacts.include names an artifact that does not exist \
                 or was dropped as invalid"
            );
        }
    }

    artifacts
        .into_iter()
        .filter(|a| {
            let kept = selected.contains(&a.id);
            if !kept {
                tracing::warn!(
                    artifact_id = %a.id.as_str(),
                    "marketplace: artifact is not selected by any enabled plugin; skipping"
                );
            }
            kept
        })
        .collect()
}
