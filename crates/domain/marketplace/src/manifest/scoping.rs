//! Marketplace-scope filtering and plugin gating of manifest sections.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_models::bridge::ids::{LibraryArtifactId, SkillId};
use systemprompt_models::bridge::manifest::{ArtifactEntry, SkillEntry};

use crate::candidate::MarketplaceCandidate;
use crate::trace::{TraceEvent, TraceKind, TraceSink, TraceStage};

pub(super) type ScopedSections = (
    Vec<systemprompt_models::bridge::manifest::AgentEntry>,
    Vec<systemprompt_models::bridge::manifest::ManagedMcpServer>,
    Vec<ArtifactEntry>,
);

pub(super) fn scope_all(
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

pub(super) fn prune_traced(filtered: &mut MarketplaceCandidate, trace: &mut dyn TraceSink) {
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

pub(super) fn gate_skills_by_plugin(
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

pub(super) fn gate_artifacts_by_plugin(
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
