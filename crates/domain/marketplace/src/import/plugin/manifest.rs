//! The plugin manifest an import reads, and the skill paths it names.
//!
//! A plugin normally carries `.claude-plugin/plugin.json`. Claude Code's
//! `strict: false` makes the marketplace entry the whole definition instead,
//! which is how an upstream folder with no Claude manifest is re-listed; that
//! entry then supplies the manifest's name and its `skills` override: a path
//! or list of paths inside the plugin, refused rather than ignored when one
//! would leave it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_RELPATH, PluginManifest};

use crate::error::MarketplaceError;

use super::super::anthropic::MarketplacePluginEntry;

pub(super) fn resolve_manifest(
    entry: &MarketplacePluginEntry,
    dir: &Path,
) -> Result<PluginManifest, MarketplaceError> {
    let path = dir.join(PLUGIN_MANIFEST_RELPATH);
    if entry.strict == Some(false) && !path.is_file() {
        return Ok(PluginManifest {
            name: entry.name.clone(),
            skills: entry.skills.clone(),
            ..PluginManifest::default()
        });
    }
    let text =
        std::fs::read_to_string(&path).map_err(|e| MarketplaceError::import(&path, "read", e))?;
    let mut manifest: PluginManifest = serde_json::from_str(&text)
        .map_err(|e| MarketplaceError::import(&path, "plugin.json is not valid", e))?;
    if manifest.skills.is_none() {
        manifest.skills.clone_from(&entry.skills);
    }
    Ok(manifest)
}

pub(super) fn skill_paths(
    manifest: &PluginManifest,
    dir: &Path,
) -> Result<Vec<PathBuf>, MarketplaceError> {
    let values: Vec<&str> = match manifest.skills.as_ref() {
        None => Vec::new(),
        Some(serde_json::Value::String(one)) => vec![one.as_str()],
        Some(serde_json::Value::Array(many)) => {
            many.iter().filter_map(serde_json::Value::as_str).collect()
        },
        Some(_) => {
            return Err(MarketplaceError::Import {
                path: dir.display().to_string(),
                message: "`skills` must be a path or a list of paths".to_owned(),
            });
        },
    };
    values
        .into_iter()
        .map(|raw| {
            let relative = raw.trim_start_matches("./").trim_end_matches('/');
            if relative.is_empty() || relative == "." {
                return Ok(dir.to_path_buf());
            }
            systemprompt_models::managed::validate_path(relative)
                .map(|()| dir.join(relative))
                .map_err(|error| {
                    MarketplaceError::import(
                        &dir.join(relative),
                        format!("skills path {raw:?} must stay inside the plugin"),
                        error,
                    )
                })
        })
        .collect()
}
