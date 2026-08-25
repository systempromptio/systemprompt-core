//! Manifest assembly and sealing service.
//!
//! [`ManifestService`] assembles a scoped, filtered [`MarketplaceCandidate`]
//! from the on-disk catalogue and seals a built [`SignedManifest`] into a
//! [`SignedManifestEnvelope`]: the manifest's JCS-canonical serialization
//! carried verbatim as the envelope payload, signed byte-for-byte. The bridge
//! verifies over those exact bytes before parsing, so there is no second
//! canonical view to keep in sync with the manifest struct.
//!
//! Manifest skills are derived, never configured: the skills array is the
//! union of skills the enabled, marketplace-included plugins actually ship
//! (plugin `skills` refs plus their included agents' skill refs), so it cannot
//! diverge from what the plugin bundles deliver.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::ids::{LibraryArtifactId, ManifestSignature, SkillId};
use systemprompt_models::bridge::manifest::{
    ArtifactEntry, SignedManifest, SignedManifestEnvelope, SkillEntry,
};
use systemprompt_models::services::ServicesConfig;
use systemprompt_security::manifest_signing;

use crate::candidate::MarketplaceCandidate;
use crate::catalog::{CatalogContent, artifact_owners, load_hooks, load_plugins};
use crate::error::MarketplaceError;
use crate::filter::MarketplaceFilter;
use crate::service::MarketplaceService;
use crate::trace::{NoopTrace, TraceEvent, TraceKind, TraceSink, TraceStage};

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
        let selected_skills = crate::catalog::selected_skill_ids(services, &catalog.as_content())?;
        let (skills, agents, managed_mcp_servers, artifacts) = catalog.into_parts();

        let active = MarketplaceService::new(services).resolve_active()?;
        let (agents, managed_mcp_servers, artifacts) =
            scope_all(active, agents, managed_mcp_servers, artifacts, trace);

        let mut diagnostics = plugin_inclusion_diagnostics(services, &skills, &agents)?;
        let skills = gate_skills_by_plugin(skills, &selected_skills, trace);

        let owners = artifact_owners(services, &artifacts)?;
        let selected_artifacts: BTreeSet<LibraryArtifactId> = owners.keys().cloned().collect();
        let artifacts = gate_artifacts_by_plugin(artifacts, &selected_artifacts, trace);

        let mut candidate = MarketplaceCandidate {
            plugins,
            skills,
            agents,
            hooks,
            managed_mcp_servers,
            artifacts,
            ..MarketplaceCandidate::default()
        }
        .with_artifact_owners(owners);
        if let Some(mp) = active {
            candidate = candidate.with_marketplace(mp.id.clone(), Some(mp.access.clone()));
        }

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

type ScopedSections = (
    Vec<systemprompt_models::bridge::manifest::AgentEntry>,
    Vec<systemprompt_models::bridge::manifest::ManagedMcpServer>,
    Vec<ArtifactEntry>,
);

fn scope_all(
    active: Option<&systemprompt_models::services::MarketplaceConfig>,
    agents: Vec<systemprompt_models::bridge::manifest::AgentEntry>,
    managed_mcp_servers: Vec<systemprompt_models::bridge::manifest::ManagedMcpServer>,
    artifacts: Vec<ArtifactEntry>,
    trace: &mut dyn TraceSink,
) -> ScopedSections {
    match active {
        Some(mp) => (
            scope_traced(
                agents,
                &mp.agents.include,
                |a| a.id.as_str(),
                TraceKind::Agent,
                trace,
            ),
            scope_traced(
                managed_mcp_servers,
                &mp.mcp_servers.include,
                |m| m.name.as_str(),
                TraceKind::McpServer,
                trace,
            ),
            scope_traced(
                artifacts,
                &mp.artifacts.include,
                |a| a.id.as_str(),
                TraceKind::Artifact,
                trace,
            ),
        ),
        None => (agents, managed_mcp_servers, artifacts),
    }
}

fn prune_traced(filtered: &mut MarketplaceCandidate, trace: &mut dyn TraceSink) {
    let pre_prune: Vec<String> = filtered
        .artifacts
        .iter()
        .map(|a| a.id.as_str().to_owned())
        .collect();
    filtered.prune_orphaned_artifacts();
    for id in pre_prune {
        if !filtered.artifacts.iter().any(|a| a.id.as_str() == id) {
            trace.record(TraceEvent {
                kind: TraceKind::Artifact,
                id,
                stage: TraceStage::OrphanPrune,
                reason: "every plugin shipping this artifact was filtered out".to_owned(),
            });
        }
    }
}

fn scope_traced<T, F>(
    items: Vec<T>,
    include: &[String],
    id_of: F,
    kind: TraceKind,
    trace: &mut dyn TraceSink,
) -> Vec<T>
where
    F: Fn(&T) -> &str,
{
    if include.is_empty() {
        return items;
    }
    let (kept, dropped): (Vec<T>, Vec<T>) = items
        .into_iter()
        .partition(|item| include.iter().any(|inc| inc == id_of(item)));
    for item in &dropped {
        trace.record(TraceEvent {
            kind,
            id: id_of(item).to_owned(),
            stage: TraceStage::MarketplaceScope,
            reason: "not in the active marketplace's include list".to_owned(),
        });
    }
    kept
}

fn gate_skills_by_plugin(
    skills: Vec<SkillEntry>,
    selected: &BTreeSet<SkillId>,
    trace: &mut dyn TraceSink,
) -> Vec<SkillEntry> {
    for id in selected {
        if !skills.iter().any(|s| &s.id == id) {
            tracing::warn!(
                skill_id = %id,
                "marketplace: a plugin selects a skill that does not exist or was dropped as \
                 invalid"
            );
        }
    }

    skills
        .into_iter()
        .filter(|s| {
            let kept = selected.contains(&s.id);
            if !kept {
                tracing::warn!(
                    skill_id = %s.id.as_str(),
                    "marketplace: skill is not selected by any enabled plugin; it will not be \
                     installed on any host; skipping"
                );
                trace.record(TraceEvent {
                    kind: TraceKind::Skill,
                    id: s.id.as_str().to_owned(),
                    stage: TraceStage::PluginSelection,
                    reason: "no enabled, marketplace-included plugin selects this skill".to_owned(),
                });
            }
            kept
        })
        .collect()
}

fn plugin_inclusion_diagnostics(
    services: &ServicesConfig,
    skills: &[SkillEntry],
    agents: &[systemprompt_models::bridge::manifest::AgentEntry],
) -> Result<Vec<String>, MarketplaceError> {
    use systemprompt_models::services::ComponentSource;

    let mut diagnostics = Vec::new();
    let mut selected_agents: BTreeSet<&str> = BTreeSet::new();
    for config in crate::catalog::selected_configs(services)? {
        if config.skills.source == ComponentSource::Explicit {
            for raw in &config.skills.include {
                if !skills.iter().any(|s| s.id.as_str() == raw.as_str()) {
                    diagnostics.push(format!(
                        "plugin '{}' skills.include names '{raw}', which does not exist on disk \
                         or is disabled",
                        config.id
                    ));
                }
            }
        }
        match config.agents.source {
            ComponentSource::Explicit => {
                for raw in &config.agents.include {
                    if !agents.iter().any(|a| a.id.as_str() == raw.as_str()) {
                        diagnostics.push(format!(
                            "plugin '{}' agents.include names '{raw}', which does not exist or \
                             is outside the marketplace agents scope",
                            config.id
                        ));
                    }
                    selected_agents.insert(raw.as_str());
                }
            },
            ComponentSource::Instance => {
                for a in agents {
                    if !config.agents.exclude.iter().any(|ex| ex == a.id.as_str()) {
                        selected_agents.insert(a.id.as_str());
                    }
                }
            },
        }
    }

    for a in agents {
        if !selected_agents.contains(a.id.as_str()) {
            diagnostics.push(format!(
                "agent '{}' is in the marketplace scope but no enabled plugin includes it; it \
                 will not be installed by any plugin bundle",
                a.id.as_str()
            ));
        }
    }

    Ok(diagnostics)
}

struct CandidateSnapshot {
    skills: Vec<String>,
    agents: Vec<String>,
    mcp_servers: Vec<String>,
    artifacts: Vec<String>,
    plugins: Vec<String>,
}

fn snapshot(candidate: &MarketplaceCandidate) -> CandidateSnapshot {
    CandidateSnapshot {
        skills: candidate
            .skills
            .iter()
            .map(|s| s.id.as_str().to_owned())
            .collect(),
        agents: candidate
            .agents
            .iter()
            .map(|a| a.id.as_str().to_owned())
            .collect(),
        mcp_servers: candidate
            .managed_mcp_servers
            .iter()
            .map(|m| m.name.as_str().to_owned())
            .collect(),
        artifacts: candidate
            .artifacts
            .iter()
            .map(|a| a.id.as_str().to_owned())
            .collect(),
        plugins: candidate
            .plugins
            .iter()
            .map(|p| p.id.as_str().to_owned())
            .collect(),
    }
}

fn record_removed(
    before: &CandidateSnapshot,
    after: &CandidateSnapshot,
    stage: TraceStage,
    reason: &str,
    trace: &mut dyn TraceSink,
) {
    let sections: [(TraceKind, &Vec<String>, &Vec<String>); 5] = [
        (TraceKind::Skill, &before.skills, &after.skills),
        (TraceKind::Agent, &before.agents, &after.agents),
        (
            TraceKind::McpServer,
            &before.mcp_servers,
            &after.mcp_servers,
        ),
        (TraceKind::Artifact, &before.artifacts, &after.artifacts),
        (TraceKind::Plugin, &before.plugins, &after.plugins),
    ];
    for (kind, before_ids, after_ids) in sections {
        for id in before_ids {
            if !after_ids.contains(id) {
                trace.record(TraceEvent {
                    kind,
                    id: id.clone(),
                    stage,
                    reason: reason.to_owned(),
                });
            }
        }
    }
}

fn gate_artifacts_by_plugin(
    artifacts: Vec<ArtifactEntry>,
    selected: &BTreeSet<LibraryArtifactId>,
    trace: &mut dyn TraceSink,
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
                trace.record(TraceEvent {
                    kind: TraceKind::Artifact,
                    id: a.id.as_str().to_owned(),
                    stage: TraceStage::PluginSelection,
                    reason: "no enabled, marketplace-included plugin selects this artifact"
                        .to_owned(),
                });
            }
            kept
        })
        .collect()
}
