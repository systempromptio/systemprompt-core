//! Plugin-inclusion diagnostics and filter-removal trace records.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_models::bridge::manifest::SkillEntry;
use systemprompt_models::services::ServicesConfig;

use crate::candidate::MarketplaceCandidate;
use crate::trace::{TraceEvent, TraceKind, TraceSink, TraceStage};

pub(super) fn plugin_inclusion_diagnostics(
    services: &ServicesConfig,
    skills: &[SkillEntry],
    agents: &[systemprompt_models::bridge::manifest::AgentEntry],
) -> Vec<String> {
    use systemprompt_models::services::ComponentSource;

    let mut diagnostics = Vec::new();
    let mut selected_agents: BTreeSet<&str> = BTreeSet::new();
    for config in crate::catalog::selected_configs(services) {
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
                "agent '{}' is in an enabled marketplace's scope but no enabled plugin includes it; it \
                 will not be installed by any plugin bundle",
                a.id.as_str()
            ));
        }
    }

    diagnostics
}

pub(super) struct CandidateSnapshot {
    skills: Vec<String>,
    agents: Vec<String>,
    mcp_servers: Vec<String>,
    artifacts: Vec<String>,
    plugins: Vec<String>,
}

pub(super) fn snapshot(candidate: &MarketplaceCandidate) -> CandidateSnapshot {
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

pub(super) fn record_removed(
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
