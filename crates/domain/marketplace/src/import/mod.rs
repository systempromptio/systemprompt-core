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
//! ## Upstream plugins
//!
//! A marketplace entry may name a `github`, `url` or `git-subdir` source, as
//! Claude Code allows, to re-list a plugin published elsewhere. The importer
//! fetches that commit and imports it like a local plugin (`remote`), so the
//! services tree, and the bundle packed from it, carries the upstream files
//! and needs no network at boot. An entry without a `sha` is imported from
//! whatever its ref points at and flagged, which `strict` refuses.
//!
//! An entry marked `"mode": "pass_through"` is the exception: it is not
//! fetched but kept as authored on the marketplace config
//! (`external_plugins`), and Claude Code fetches it itself. Such an entry must
//! pin a `sha`, and its name may not repeat a vendored plugin's
//! (`pass_through`).
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
mod pass_through;
mod plugin;
mod remote;
mod rules;
mod scripts;
pub mod sidecar;
mod skill;
mod warning;
mod writer;

use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_identifiers::{MarketplaceId, PluginId};

use crate::dev_files::DevFileFilter;
use crate::error::MarketplaceError;
use crate::managed::{GitSourceCapture, NativeGitSourceCapture};

pub use anthropic::{
    MarketplaceJson, MarketplacePluginEntry, PluginEntryMode, PluginSource, RemotePluginSource,
};
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
    pub upstream: Vec<String>,
    pub warnings: Vec<ImportWarning>,
}

pub fn import_anthropic_tree(
    from: &Path,
    into: &Path,
    opts: &ImportOptions,
) -> Result<ImportReport, MarketplaceError> {
    import_anthropic_tree_with(from, into, opts, &NativeGitSourceCapture)
}

pub fn import_anthropic_tree_with(
    from: &Path,
    into: &Path,
    opts: &ImportOptions,
    capture: &dyn GitSourceCapture,
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

    let dev_files = DevFileFilter::load(from)
        .map_err(|e| MarketplaceError::import(from, "load dev-file filter", e))?;
    let sink = writer::Sink::new(into, opts.dry_run, dev_files);
    let mut report = ImportReport {
        copied_base_dirs: base::copy_base_tree(from, &sink)?,
        ..ImportReport::default()
    };

    let manifest_path = from.join(MARKETPLACE_MANIFEST_RELPATH);
    if manifest_path.is_file() {
        import_marketplace_tree(from, &manifest_path, &sink, capture, &mut report)?;
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
    capture: &dyn GitSourceCapture,
    report: &mut ImportReport,
) -> Result<(), MarketplaceError> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|e| MarketplaceError::import(manifest_path, "read", e))?;
    let manifest: MarketplaceJson = serde_json::from_str(&text)
        .map_err(|e| MarketplaceError::import(manifest_path, "marketplace.json is not valid", e))?;

    let sidecar = sidecar::load_marketplace_sidecar(&from.join(SIDECAR_RELPATH))?;
    let (vendored, external_plugins) = pass_through::split(&manifest.plugins, manifest_path)?;
    let id = marketplace::import_marketplace(
        &manifest,
        &sidecar,
        external_plugins,
        manifest_path,
        sink,
    )?;
    report.marketplaces.push(id);

    let mut seen_skills: BTreeSet<String> = BTreeSet::new();
    let mut seen_rules: BTreeSet<String> = BTreeSet::new();
    let plugin_root = manifest.metadata.plugin_root.as_deref();

    for entry in vendored {
        let source = entry.plugin_source().map_err(|e| {
            MarketplaceError::import(manifest_path, format!("plugin '{}' source", entry.name), e)
        })?;
        let fetched = match source {
            PluginSource::Unsupported(_) => {
                report.warnings.push(ImportWarning::RemotePluginSource {
                    plugin: entry.name.clone(),
                });
                continue;
            },
            PluginSource::Remote(remote) => {
                if remote.commit.is_none() {
                    report.warnings.push(ImportWarning::RemotePluginUnpinned {
                        plugin: entry.name.clone(),
                    });
                }
                let fetched = remote::fetch_plugin(capture, &entry.name, &remote)?;
                report.upstream.push(format!(
                    "{} {}{}@{}",
                    entry.name,
                    remote.repository,
                    remote
                        .subdirectory
                        .as_deref()
                        .map_or_else(String::new, |path| format!("/{path}")),
                    fetched.commit
                ));
                Some(fetched)
            },
            PluginSource::Default | PluginSource::Local(_) => None,
        };
        let dir = match &fetched {
            Some(fetched) => fetched.dir.path().to_path_buf(),
            None => plugin::plugin_dir(from, entry, plugin_root)?,
        };
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
    let mut read = std::fs::read_dir(into)
        .map_err(|e| MarketplaceError::import(into, "read destination", e))?;
    if read.next().is_some() {
        return Err(MarketplaceError::Import {
            path: into.display().to_string(),
            message: "destination is not empty; remove it before importing".to_owned(),
        });
    }
    Ok(())
}
