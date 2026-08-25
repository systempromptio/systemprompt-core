//! Per-stage drop tracing for manifest assembly.
//!
//! [`ManifestService::assemble_candidate_traced`](crate::ManifestService::assemble_candidate_traced)
//! records a [`TraceEvent`] for every catalogue entry dropped between the disk
//! scan and the sealed manifest, so a diagnostic caller can answer "why is
//! skill X missing" without reading server logs. The production path pays
//! nothing: [`NoopTrace`] discards events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TraceStage {
    DiskScan,
    Parse,
    Disabled,
    MarketplaceScope,
    PluginSelection,
    AccessFilter,
    OrphanPrune,
}

impl fmt::Display for TraceStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::DiskScan => "disk-scan",
            Self::Parse => "parse",
            Self::Disabled => "disabled",
            Self::MarketplaceScope => "marketplace-scope",
            Self::PluginSelection => "plugin-selection",
            Self::AccessFilter => "access-filter",
            Self::OrphanPrune => "orphan-prune",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TraceKind {
    Skill,
    Agent,
    McpServer,
    Artifact,
    Plugin,
}

impl fmt::Display for TraceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Skill => "skill",
            Self::Agent => "agent",
            Self::McpServer => "mcp-server",
            Self::Artifact => "artifact",
            Self::Plugin => "plugin",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceEvent {
    pub kind: TraceKind,
    pub id: String,
    pub stage: TraceStage,
    pub reason: String,
}

pub trait TraceSink: Send {
    fn record(&mut self, event: TraceEvent);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopTrace;

impl TraceSink for NoopTrace {
    fn record(&mut self, _event: TraceEvent) {}
}

#[derive(Debug, Default, Clone)]
pub struct ManifestTrace {
    pub events: Vec<TraceEvent>,
}

impl TraceSink for ManifestTrace {
    fn record(&mut self, event: TraceEvent) {
        self.events.push(event);
    }
}
