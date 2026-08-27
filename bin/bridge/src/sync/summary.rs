//! The human-facing result of one sync run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gateway::manifest::SignedManifest;
use crate::sync::apply::{self, HostFailure};

#[derive(Debug, Clone)]
pub struct SyncSummary {
    pub identity: String,
    pub manifest_version: String,
    pub plugin_count: usize,
    pub skill_count: usize,
    pub agent_count: usize,
    pub hook_count: usize,
    pub mcp_count: usize,
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub malformed: Vec<String>,
    pub host_failures: Vec<HostFailure>,
    pub diagnostics: Vec<String>,
}

impl SyncSummary {
    #[must_use]
    pub fn one_line(&self) -> String {
        let status = if self.host_failures.is_empty() {
            "sync ok"
        } else {
            "sync PARTIAL"
        };
        let malformed_suffix = if self.malformed.is_empty() {
            String::new()
        } else {
            format!(
                " — WARNING: {} malformed plugin(s) missing claude-plugin/plugin.json: {}",
                self.malformed.len(),
                self.malformed.join(", "),
            )
        };
        let host_suffix = if self.host_failures.is_empty() {
            String::new()
        } else {
            let detail = self
                .host_failures
                .iter()
                .map(|f| format!("{} ({})", f.host_id, first_line(&f.error)))
                .collect::<Vec<_>>()
                .join("; ");
            format!(
                " — {} host(s) failed: {} — see bridge.log",
                self.host_failures.len(),
                detail,
            )
        };
        let diagnostics_suffix = if self.diagnostics.is_empty() {
            String::new()
        } else {
            format!(
                " — {} gateway diagnostic(s): {}",
                self.diagnostics.len(),
                self.diagnostics.join("; "),
            )
        };
        format!(
            "{status} ({}): {} plugins ({} new, {} updated, {} removed), {} skills installed, {} \
             agents, {} hooks, {} MCP — manifest {}{}{}{}",
            self.identity,
            self.plugin_count,
            self.installed.len(),
            self.updated.len(),
            self.removed.len(),
            self.skill_count,
            self.agent_count,
            self.hook_count,
            self.mcp_count,
            self.manifest_version,
            malformed_suffix,
            host_suffix,
            diagnostics_suffix,
        )
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).to_owned()
}

pub(super) fn build_summary(manifest: &SignedManifest, report: apply::ApplyReport) -> SyncSummary {
    let identity = manifest
        .user
        .as_ref()
        .map_or_else(|| manifest.user_id.to_string(), |u| u.email.clone());
    let bundled_skills = bundled_skill_count(manifest);
    let mut diagnostics = manifest.diagnostics.clone();
    if bundled_skills < manifest.skills.len() {
        diagnostics.push(format!(
            "manifest lists {} skill(s) but plugin bundles install only {}; a skill in the \
             marketplace scope is missing from every plugin's skills.include",
            manifest.skills.len(),
            bundled_skills,
        ));
    }
    for d in &diagnostics {
        tracing::warn!(diagnostic = %d, "sync: gateway diagnostic");
    }
    SyncSummary {
        identity,
        manifest_version: manifest.manifest_version.to_string(),
        plugin_count: manifest.plugins.len(),
        skill_count: bundled_skills,
        agent_count: manifest.agents.len(),
        hook_count: manifest.hooks.len(),
        mcp_count: manifest.managed_mcp_servers.len(),
        installed: report.installed,
        updated: report.updated,
        removed: report.removed,
        malformed: report.malformed,
        host_failures: report.host_failures,
        diagnostics,
    }
}

fn bundled_skill_count(manifest: &SignedManifest) -> usize {
    let mut dirs = std::collections::BTreeSet::new();
    for plugin in &manifest.plugins {
        for file in &plugin.files {
            if let Some(rest) = file.path.strip_prefix("skills/")
                && let Some((dir, _)) = rest.split_once('/')
            {
                dirs.insert(dir.to_owned());
            }
        }
    }
    dirs.len()
}
