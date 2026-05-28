//! Reads, parses, and merges the active services configuration.
//!
//! [`ConfigLoader`] is the only public entry point. It resolves the active
//! profile (via [`systemprompt_config::ProfileBootstrap`]) to a YAML path,
//! parses the root file, recursively resolves the `includes:` graph
//! (rejecting cycles and duplicate definitions), inlines `!include`
//! references inside agent system prompts and skill instructions, and
//! finally validates the merged configuration before returning it to the
//! caller.

mod includes;
mod merge;
mod types;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_config::ProfileBootstrap;
use systemprompt_models::services::{MarketplaceConfigFile, ServicesConfig};

use crate::error::{ConfigLoadError, ConfigLoadResult};

use includes::resolve_includes_recursively;
use merge::{
    resolve_skill_instruction_includes, resolve_system_prompt_includes,
    warn_on_authored_card_skills,
};
use types::IncludeResolveCtx;

#[derive(Debug)]
pub struct ConfigLoader {
    base_path: PathBuf,
    config_path: PathBuf,
}

impl ConfigLoader {
    #[must_use]
    pub fn new(config_path: PathBuf) -> Self {
        let base_path = config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Self {
            base_path,
            config_path,
        }
    }

    pub fn for_active_profile() -> ConfigLoadResult<Self> {
        let profile = ProfileBootstrap::get()?;
        let config_path = PathBuf::from(profile.paths.config());
        Ok(Self::new(config_path))
    }

    pub fn load() -> ConfigLoadResult<ServicesConfig> {
        Self::for_active_profile()?.run()
    }

    pub fn load_from_path(path: &Path) -> ConfigLoadResult<ServicesConfig> {
        Self::new(path.to_path_buf()).run()
    }

    #[cfg(any(test, feature = "expose-internals"))]
    pub fn load_from_content(content: &str, path: &Path) -> ConfigLoadResult<ServicesConfig> {
        Self::new(path.to_path_buf()).run_from_content(content)
    }

    pub fn validate_file(path: &Path) -> ConfigLoadResult<()> {
        Self::load_from_path(path).map(|_| ())
    }

    fn run(&self) -> ConfigLoadResult<ServicesConfig> {
        let content = fs::read_to_string(&self.config_path).map_err(|e| ConfigLoadError::Io {
            path: self.config_path.clone(),
            source: e,
        })?;
        self.run_from_content(&content)
    }

    fn run_from_content(&self, content: &str) -> ConfigLoadResult<ServicesConfig> {
        let mut merged: ServicesConfig =
            serde_yaml::from_str(content).map_err(|e| ConfigLoadError::Yaml {
                path: self.config_path.clone(),
                source: e,
            })?;

        let includes = std::mem::take(&mut merged.includes);

        let mut visited: HashSet<PathBuf> = HashSet::new();
        if let Ok(canonical_root) = fs::canonicalize(&self.config_path) {
            visited.insert(canonical_root);
        }
        {
            let mut ctx = IncludeResolveCtx {
                visited: &mut visited,
                merged: &mut merged,
                chain: vec![self.config_path.clone()],
            };
            for include_path in &includes {
                resolve_includes_recursively(
                    &self.base_path,
                    include_path,
                    &self.config_path,
                    &mut ctx,
                )?;
            }
        }

        resolve_system_prompt_includes(&self.base_path, &mut merged)?;
        resolve_skill_instruction_includes(&self.base_path, &mut merged)?;
        warn_on_authored_card_skills(&merged);

        discover_marketplaces(&self.base_path, &mut merged)?;

        if let Ok(val) = std::env::var("SYSTEMPROMPT_SERVICES_PATH") {
            merged.settings.services_path = Some(val);
        }
        if let Ok(val) = std::env::var("SYSTEMPROMPT_SKILLS_PATH") {
            merged.settings.skills_path = Some(val);
        }
        if let Ok(val) = std::env::var("SYSTEMPROMPT_CONFIG_PATH") {
            merged.settings.config_path = Some(val);
        }

        merged
            .validate()
            .map_err(|e| ConfigLoadError::Validation(e.to_string()))?;

        Ok(merged)
    }

    pub fn get_includes(&self) -> ConfigLoadResult<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct IncludesOnly {
            #[serde(default)]
            includes: Vec<String>,
        }

        let content = fs::read_to_string(&self.config_path).map_err(|e| ConfigLoadError::Io {
            path: self.config_path.clone(),
            source: e,
        })?;
        let parsed: IncludesOnly =
            serde_yaml::from_str(&content).map_err(|e| ConfigLoadError::Yaml {
                path: self.config_path.clone(),
                source: e,
            })?;
        Ok(parsed.includes)
    }

    pub fn list_all_includes(&self) -> ConfigLoadResult<Vec<(String, bool)>> {
        self.get_includes()?
            .into_iter()
            .map(|include| {
                let exists = self.base_path.join(&include).exists();
                Ok((include, exists))
            })
            .collect()
    }

    #[must_use]
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

/// Walks `<services>/marketplaces/*/config.yaml`, parses each as a
/// `MarketplaceConfigFile`, and inserts into `merged.marketplaces`.
///
/// `base_path` is the parent of the root `config.yaml` (i.e.
/// `<services>/config`), so the marketplaces directory is its sibling.
fn discover_marketplaces(base_path: &Path, merged: &mut ServicesConfig) -> ConfigLoadResult<()> {
    let Some(services_dir) = base_path.parent() else {
        return Ok(());
    };
    let marketplaces_dir = services_dir.join("marketplaces");
    if !marketplaces_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(&marketplaces_dir).map_err(|e| ConfigLoadError::Io {
        path: marketplaces_dir.clone(),
        source: e,
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| ConfigLoadError::Io {
            path: marketplaces_dir.clone(),
            source: e,
        })?;
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let config_path = dir.join("config.yaml");
        if !config_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&config_path).map_err(|e| ConfigLoadError::Io {
            path: config_path.clone(),
            source: e,
        })?;
        let file: MarketplaceConfigFile =
            serde_yaml::from_str(&content).map_err(|e| ConfigLoadError::Yaml {
                path: config_path.clone(),
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
