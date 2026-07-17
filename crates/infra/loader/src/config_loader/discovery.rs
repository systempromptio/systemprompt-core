//! On-disk catalog discovery for the loader.
//!
//! After the `includes:` graph is merged, the loader walks the sibling
//! `<services>/{skills,plugins,marketplaces}/*/config.yaml` directories and
//! folds each entry into the merged [`ServicesConfig`] so that
//! `skills.include` / `plugins.include` references resolve at validation time.
//!
//! `base_path` is the parent of the root `config.yaml` (i.e.
//! `<services>/config`), so each catalog directory is its grandparent's child —
//! resolved via [`catalog_dir`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_models::services::{
    MarketplaceConfigFile, PluginComponentRef, PluginConfigFile, ServicesConfig, SkillConfig,
};
use systemprompt_models::{DiskSkillConfig, SKILL_CONFIG_FILENAME};

use crate::error::{ConfigLoadError, ConfigLoadResult};

fn catalog_dir(base_path: &Path, name: &str) -> Option<PathBuf> {
    let dir = base_path.parent()?.join(name);
    dir.exists().then_some(dir)
}

fn read_entry_config(dir: &Path, filename: &str) -> ConfigLoadResult<Option<(PathBuf, String)>> {
    if !dir.is_dir() {
        return Ok(None);
    }
    let config_path = dir.join(filename);
    if !config_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&config_path).map_err(|e| ConfigLoadError::Io {
        path: config_path.clone(),
        source: e,
    })?;
    Ok(Some((config_path, content)))
}

fn read_catalog(dir: &Path) -> ConfigLoadResult<fs::ReadDir> {
    fs::read_dir(dir).map_err(|e| ConfigLoadError::Io {
        path: dir.to_path_buf(),
        source: e,
    })
}

fn next_entry(dir: &Path, entry: std::io::Result<fs::DirEntry>) -> ConfigLoadResult<fs::DirEntry> {
    entry.map_err(|e| ConfigLoadError::Io {
        path: dir.to_path_buf(),
        source: e,
    })
}

pub(super) fn discover_skills(
    base_path: &Path,
    merged: &mut ServicesConfig,
) -> ConfigLoadResult<()> {
    let Some(skills_dir) = catalog_dir(base_path, "skills") else {
        return Ok(());
    };

    for entry in read_catalog(&skills_dir)? {
        let entry = next_entry(&skills_dir, entry)?;
        let Some((config_path, content)) = read_entry_config(&entry.path(), SKILL_CONFIG_FILENAME)?
        else {
            continue;
        };

        let disk: DiskSkillConfig =
            serde_yaml::from_str(&content).map_err(|e| ConfigLoadError::Yaml {
                path: config_path,
                source: e,
            })?;

        let key = disk.id.as_str().to_owned();
        if merged.skills.skills.contains_key(&key) {
            continue;
        }
        merged.skills.skills.insert(key, skill_from_disk(disk));
    }

    Ok(())
}

fn skill_from_disk(disk: DiskSkillConfig) -> SkillConfig {
    SkillConfig {
        id: disk.id,
        name: disk.name,
        description: disk.description,
        enabled: disk.enabled,
        tags: disk.tags,
        instructions: None,
        assigned_agents: PluginComponentRef::default(),
        mcp_servers: PluginComponentRef::default(),
        model_config: None,
    }
}

pub(super) fn discover_plugins(
    base_path: &Path,
    merged: &mut ServicesConfig,
) -> ConfigLoadResult<()> {
    let Some(plugins_dir) = catalog_dir(base_path, "plugins") else {
        return Ok(());
    };

    for entry in read_catalog(&plugins_dir)? {
        let entry = next_entry(&plugins_dir, entry)?;
        let Some((config_path, content)) = read_entry_config(&entry.path(), "config.yaml")? else {
            continue;
        };

        let file: PluginConfigFile =
            serde_yaml::from_str(&content).map_err(|e| ConfigLoadError::Yaml {
                path: config_path,
                source: e,
            })?;

        let id = file.plugin.id.as_str().to_owned();
        if merged.plugins.contains_key(&id) {
            continue;
        }
        merged.plugins.insert(id, file.plugin);
    }

    Ok(())
}

pub(super) fn discover_marketplaces(
    base_path: &Path,
    merged: &mut ServicesConfig,
) -> ConfigLoadResult<()> {
    let Some(marketplaces_dir) = catalog_dir(base_path, "marketplaces") else {
        return Ok(());
    };

    for entry in read_catalog(&marketplaces_dir)? {
        let entry = next_entry(&marketplaces_dir, entry)?;
        let Some((config_path, content)) = read_entry_config(&entry.path(), "config.yaml")? else {
            continue;
        };

        let file: MarketplaceConfigFile =
            serde_yaml::from_str(&content).map_err(|e| ConfigLoadError::Yaml {
                path: config_path,
                source: e,
            })?;

        let id = file.marketplace.id.clone();
        if merged.marketplaces.contains_key(&id) {
            return Err(ConfigLoadError::DuplicateMarketplace(
                id.as_str().to_owned(),
            ));
        }
        merged.marketplaces.insert(id, file.marketplace);
    }

    Ok(())
}
