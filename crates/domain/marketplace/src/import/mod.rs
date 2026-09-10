//! Importer for Anthropic-format authoring repositories.
//!
//! [`import_anthropic_tree`] reads a repository authored for Claude Code —
//! `.claude-plugin/marketplace.json`, one
//! `plugins/<id>/.claude-plugin/plugin.json` per plugin,
//! `skills/<id>/SKILL.md`, `rules/*.md`, `hooks/hooks.json` — and writes the
//! systemprompt services tree the loader discovers. The Anthropic tree stays
//! installable by Claude Code unchanged: nothing is written back into it.
//!
//! ## Two sources, no overlap
//!
//! Everything Anthropic's format can express is *derived* from the manifests
//! and the directory layout. Everything it cannot — visibility, access rules,
//! MCP server and agent references, hook selection, scripts — comes from the
//! optional `.claude-plugin/systemprompt.yaml` sidecars in
//! [`sidecar`]. The two sets are disjoint and the sidecar rejects any key that
//! would restate a derived fact, so no field ever has two authors.
//!
//! ## Destination
//!
//! `into` must not exist or must be empty. The importer composes a whole tree
//! (including the verbatim base tree at `<from>/systemprompt/`) and cannot
//! reason about what a partial previous import left behind; a caller wanting to
//! re-import removes the directory first. `dry_run` lifts that check because
//! nothing is written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod aggregator;
mod anthropic;
mod base;
mod disk;
mod hooks;
mod marketplace;
mod plugin;
mod rules;
mod scripts;
pub mod sidecar;
mod skill;
mod warning;
mod writer;

use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_identifiers::{MarketplaceId, PluginId};

use crate::error::MarketplaceError;

pub use anthropic::{MarketplaceJson, MarketplacePluginEntry};
pub use sidecar::{MarketplaceSidecar, PluginSidecar, SIDECAR_RELPATH};
pub use warning::ImportWarning;

pub const MARKETPLACE_MANIFEST_RELPATH: &str = ".claude-plugin/marketplace.json";

#[derive(Debug, Clone, Copy, Default)]
pub struct ImportOptions {
    pub strict: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub marketplaces: Vec<MarketplaceId>,
    pub plugins: Vec<PluginId>,
    pub skills: Vec<String>,
    pub rules: Vec<String>,
    pub hooks: Vec<String>,
    pub copied_base_dirs: Vec<String>,
    pub warnings: Vec<ImportWarning>,
}

pub fn import_anthropic_tree(
    from: &Path,
    into: &Path,
    opts: &ImportOptions,
) -> Result<ImportReport, MarketplaceError> {
    if !from.is_dir() {
        return Err(MarketplaceError::Import {
            path: from.display().to_string(),
            message: "source tree does not exist".to_owned(),
        });
    }
    if !opts.dry_run {
        ensure_empty(into)?;
    }

    let sink = writer::Sink::new(into, opts.dry_run);
    let mut report = ImportReport {
        copied_base_dirs: base::copy_base_tree(from, &sink)?,
        ..ImportReport::default()
    };

    let manifest_path = from.join(MARKETPLACE_MANIFEST_RELPATH);
    if manifest_path.is_file() {
        import_marketplace_tree(from, &manifest_path, &sink, &mut report)?;
    } else {
        report.warnings.push(ImportWarning::NoMarketplaceManifest);
    }

    let mut seen_rules: BTreeSet<String> = report.rules.iter().cloned().collect();
    let mut root_rules: Vec<String> = Vec::new();
    rules::import_rules_dir(&from.join("rules"), &mut seen_rules, &sink, &mut root_rules)?;
    if !root_rules.is_empty() {
        report.warnings.push(ImportWarning::UnattachedRootRules {
            rules: root_rules.clone(),
        });
        report.rules.extend(root_rules);
    }

    aggregator::write_aggregator(from, &report.copied_base_dirs, &sink)?;

    if opts.strict
        && let Some(warning) = report.warnings.iter().find(|w| w.is_strict_error())
    {
        return Err(MarketplaceError::Import {
            path: from.display().to_string(),
            message: format!("strict import refused: {warning}"),
        });
    }

    Ok(report)
}

fn import_marketplace_tree(
    from: &Path,
    manifest_path: &Path,
    sink: &writer::Sink,
    report: &mut ImportReport,
) -> Result<(), MarketplaceError> {
    let text = std::fs::read_to_string(manifest_path).map_err(|e| MarketplaceError::Import {
        path: manifest_path.display().to_string(),
        message: e.to_string(),
    })?;
    let manifest: MarketplaceJson =
        serde_json::from_str(&text).map_err(|e| MarketplaceError::Import {
            path: manifest_path.display().to_string(),
            message: format!("marketplace.json is not valid: {e}"),
        })?;

    let sidecar = sidecar::load_marketplace_sidecar(&from.join(SIDECAR_RELPATH))?;
    let id = marketplace::import_marketplace(&manifest, &sidecar, manifest_path, sink)?;
    report.marketplaces.push(id);

    let mut seen_skills: BTreeSet<String> = BTreeSet::new();
    let mut seen_rules: BTreeSet<String> = BTreeSet::new();
    let plugin_root = manifest.metadata.plugin_root.as_deref();

    for entry in &manifest.plugins {
        if entry.source_is_remote() {
            report.warnings.push(ImportWarning::RemotePluginSource {
                plugin: entry.name.clone(),
            });
            continue;
        }
        let dir = plugin::plugin_dir(from, entry, plugin_root);
        let mut scope = plugin::PluginScope {
            seen_skills: &mut seen_skills,
            seen_rules: &mut seen_rules,
        };
        let imported = plugin::import_plugin(entry, &dir, &mut scope, sink)?;
        report.plugins.push(imported.id);
        report.skills.extend(imported.skills);
        report.rules.extend(imported.rules);
        report.hooks.extend(imported.hooks);
        report.warnings.extend(imported.warnings);
    }

    Ok(())
}

fn ensure_empty(into: &Path) -> Result<(), MarketplaceError> {
    if !into.exists() {
        return Ok(());
    }
    if !into.is_dir() {
        return Err(MarketplaceError::Import {
            path: into.display().to_string(),
            message: "destination exists and is not a directory".to_owned(),
        });
    }
    let mut read = std::fs::read_dir(into).map_err(|e| MarketplaceError::Import {
        path: into.display().to_string(),
        message: e.to_string(),
    })?;
    if read.next().is_some() {
        return Err(MarketplaceError::Import {
            path: into.display().to_string(),
            message: "destination is not empty; remove it before importing".to_owned(),
        });
    }
    Ok(())
}
