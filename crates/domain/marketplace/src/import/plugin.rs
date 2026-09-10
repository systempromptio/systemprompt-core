//! One `plugins/<id>/` Anthropic bundle → `plugins/<id>/config.yaml` plus the
//! skills, hooks, rules and scripts it ships.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use systemprompt_identifiers::PluginId;
use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_RELPATH, PluginManifest};
use systemprompt_models::services::plugin::{
    ComponentSource, PluginAuthor, PluginComponentRef, PluginConfig, PluginConfigFile,
};

use crate::error::MarketplaceError;

use super::anthropic::MarketplacePluginEntry;
use super::marketplace::{DEFAULT_LICENSE, DEFAULT_VERSION};
use super::rules::import_rules_dir;
use super::sidecar::{PluginSidecar, SIDECAR_RELPATH, load_plugin_sidecar};
use super::skill::{discover_skill_dirs, import_skill};
use super::warning::ImportWarning;
use super::writer::Sink;
use super::{hooks, scripts};

pub(super) const FALLBACK_CATEGORY: &str = "general";

pub(super) struct PluginImport {
    pub id: PluginId,
    pub skills: Vec<String>,
    pub rules: Vec<String>,
    pub hooks: Vec<String>,
    pub warnings: Vec<ImportWarning>,
}

pub(super) struct PluginScope<'a> {
    pub seen_skills: &'a mut BTreeSet<String>,
    pub seen_rules: &'a mut BTreeSet<String>,
}

pub(super) fn plugin_dir(
    from: &Path,
    entry: &MarketplacePluginEntry,
    plugin_root: Option<&str>,
) -> PathBuf {
    entry.local_path().map_or_else(
        || {
            let root = plugin_root.unwrap_or("./plugins");
            from.join(strip_dot(root)).join(&entry.name)
        },
        |path| from.join(strip_dot(path)),
    )
}

fn strip_dot(path: &str) -> &str {
    path.strip_prefix("./").unwrap_or(path)
}

pub(super) fn import_plugin(
    entry: &MarketplacePluginEntry,
    dir: &Path,
    scope: &mut PluginScope<'_>,
    sink: &Sink,
) -> Result<PluginImport, MarketplaceError> {
    let manifest_path = dir.join(PLUGIN_MANIFEST_RELPATH);
    let manifest = read_manifest(&manifest_path)?;
    let sidecar = load_plugin_sidecar(&dir.join(SIDECAR_RELPATH))?;

    let id = PluginId::new(manifest.name.trim());
    let mut warnings = Vec::new();
    collect_manifest_warnings(id.as_str(), &manifest, dir, &mut warnings);

    let category = resolve_category(id.as_str(), &sidecar, entry, &mut warnings);

    let skills = import_skills(dir, &category, scope, sink)?;
    if skills.is_empty() {
        warnings.push(ImportWarning::NoSkills {
            plugin: id.as_str().to_owned(),
        });
    }

    let mut rules = Vec::new();
    import_rules_dir(&dir.join("rules"), scope.seen_rules, sink, &mut rules)?;

    let imported_hooks = hooks::import_plugin_hooks(id.as_str(), dir, sink)?;
    warnings.extend(imported_hooks.warnings);

    scripts::copy_plugin_scripts(id.as_str(), dir, &sidecar.plugin.scripts, sink)?;

    let mut hooks_ref = sidecar.plugin.hooks.clone();
    for hook_id in &imported_hooks.ids {
        if !hooks_ref.include.contains(hook_id) {
            hooks_ref.include.push(hook_id.clone());
        }
    }

    let config = PluginConfig {
        id: id.clone(),
        name: sidecar
            .plugin
            .title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| id.as_str().to_owned()),
        description: description(&manifest, entry),
        version: version(&manifest, entry),
        enabled: sidecar.plugin.enabled,
        author: author(&manifest, entry),
        keywords: keywords(&manifest, entry),
        license: manifest
            .license
            .clone()
            .or_else(|| entry.license.clone())
            .unwrap_or_else(|| DEFAULT_LICENSE.to_owned()),
        category,
        skills: PluginComponentRef {
            source: ComponentSource::Explicit,
            filter: None,
            include: skills.clone(),
            exclude: Vec::new(),
        },
        agents: sidecar.plugin.agents.clone(),
        rules: rules_ref(&sidecar, &rules),
        mcp_servers: sidecar.plugin.mcp_servers.clone(),
        content_sources: sidecar.plugin.content_sources.clone(),
        artifacts: sidecar.plugin.artifacts.clone(),
        hooks: hooks_ref,
        scripts: sidecar.plugin.scripts.clone(),
    };

    config
        .validate(id.as_str())
        .map_err(|e| MarketplaceError::Import {
            path: manifest_path.display().to_string(),
            message: e.to_string(),
        })?;

    let rel = Path::new("plugins").join(id.as_str()).join("config.yaml");
    sink.write_yaml(&rel, &PluginConfigFile { plugin: config })?;

    Ok(PluginImport {
        id,
        skills,
        rules,
        hooks: imported_hooks.ids,
        warnings,
    })
}

fn import_skills(
    dir: &Path,
    category: &str,
    scope: &mut PluginScope<'_>,
    sink: &Sink,
) -> Result<Vec<String>, MarketplaceError> {
    let mut skills = Vec::new();
    for (skill_id, skill_dir) in discover_skill_dirs(dir) {
        if !scope.seen_skills.insert(skill_id.clone()) {
            return Err(MarketplaceError::Import {
                path: skill_dir.display().to_string(),
                message: format!(
                    "skill '{skill_id}' is shipped by more than one plugin; skill ids are unique \
                     across the whole tree"
                ),
            });
        }
        import_skill(&skill_id, &skill_dir, Some(category), sink)?;
        skills.push(skill_id);
    }
    Ok(skills)
}

fn rules_ref(sidecar: &PluginSidecar, imported: &[String]) -> PluginComponentRef {
    let mut out = sidecar.plugin.rules.clone();
    out.source = ComponentSource::Explicit;
    for id in imported {
        if !out.include.contains(id) {
            out.include.push(id.clone());
        }
    }
    out
}

fn read_manifest(path: &Path) -> Result<PluginManifest, MarketplaceError> {
    let text = std::fs::read_to_string(path).map_err(|e| MarketplaceError::Import {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;
    serde_json::from_str(&text).map_err(|e| MarketplaceError::Import {
        path: path.display().to_string(),
        message: format!("plugin.json is not valid: {e}"),
    })
}

fn collect_manifest_warnings(
    id: &str,
    manifest: &PluginManifest,
    dir: &Path,
    warnings: &mut Vec<ImportWarning>,
) {
    if manifest.mcp_servers.is_some() || dir.join(".mcp.json").is_file() {
        warnings.push(ImportWarning::InlineMcpServers {
            plugin: id.to_owned(),
        });
    }
    if dir.join("commands").is_dir() || manifest.commands.is_some() {
        warnings.push(ImportWarning::CommandsDirectory {
            plugin: id.to_owned(),
        });
    }
    let agents = count_agent_files(&dir.join("agents"));
    if agents > 0 {
        warnings.push(ImportWarning::AgentsDirectory {
            plugin: id.to_owned(),
            count: agents,
        });
    }
}

fn count_agent_files(dir: &Path) -> usize {
    std::fs::read_dir(dir).map_or(0, |read| {
        read.filter_map(Result::ok)
            .filter(|e| {
                let p = e.path();
                p.is_file() && p.extension().is_some_and(|x| x == "md")
            })
            .count()
    })
}

fn resolve_category(
    id: &str,
    sidecar: &PluginSidecar,
    entry: &MarketplacePluginEntry,
    warnings: &mut Vec<ImportWarning>,
) -> String {
    sidecar
        .plugin
        .category
        .clone()
        .or_else(|| entry.category.clone())
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| {
            warnings.push(ImportWarning::MissingCategory {
                plugin: id.to_owned(),
                applied: FALLBACK_CATEGORY.to_owned(),
            });
            FALLBACK_CATEGORY.to_owned()
        })
}

fn description(manifest: &PluginManifest, entry: &MarketplacePluginEntry) -> String {
    if manifest.description.trim().is_empty() {
        entry.description.clone().unwrap_or_default()
    } else {
        manifest.description.clone()
    }
}

fn version(manifest: &PluginManifest, entry: &MarketplacePluginEntry) -> String {
    if manifest.version.trim().is_empty() {
        entry
            .version
            .clone()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_VERSION.to_owned())
    } else {
        manifest.version.clone()
    }
}

fn author(manifest: &PluginManifest, entry: &MarketplacePluginEntry) -> PluginAuthor {
    manifest.author.as_ref().map_or_else(
        || PluginAuthor {
            name: entry.author_name().unwrap_or_default(),
            email: entry.author_email().unwrap_or_default(),
        },
        |a| PluginAuthor {
            name: a.name.clone(),
            email: a.email.clone(),
        },
    )
}

fn keywords(manifest: &PluginManifest, entry: &MarketplacePluginEntry) -> Vec<String> {
    if manifest.keywords.is_empty() {
        let mut out = entry.keywords.clone();
        out.extend(entry.tags.iter().cloned());
        out.sort();
        out.dedup();
        out
    } else {
        manifest.keywords.clone()
    }
}
