//! Reads, parses, and merges the active services configuration.
//!
//! [`ConfigLoader`] is the only public entry point. It resolves the active
//! profile (via [`systemprompt_config::ProfileBootstrap`]) to a YAML path,
//! parses the root file, recursively resolves the `includes:` graph
//! (rejecting cycles and duplicate definitions), inlines `!include`
//! references inside agent system prompts and skill instructions, and
//! finally validates the merged configuration before returning it to the
//! caller.
//!
//! [`ConfigLoader::load`] — the active-profile entry point — memoises its
//! result for the lifetime of the process. The explicit
//! [`ConfigLoader::load_from_path`] and [`ConfigLoader::validate_file`] forms
//! are one-shot reads of a caller-supplied file and are never cached.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod discovery;
mod includes;
mod merge;
mod types;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, PoisonError, RwLock};

use systemprompt_config::ProfileBootstrap;
use systemprompt_models::services::ServicesConfig;

use crate::error::{ConfigLoadError, ConfigLoadResult};

use discovery::{discover_marketplaces, discover_plugins, discover_skills};
use includes::resolve_includes_recursively;
use merge::{resolve_skill_instruction_includes, resolve_system_prompt_includes};
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

    /// Loads the active profile's services configuration.
    ///
    /// The result is cached for the lifetime of the process, keyed by the
    /// resolved configuration path. Boot alone calls this dozens of times —
    /// startup validation, the scheduler, the agent registry, and every MCP
    /// deployment accessor — and each uncached call re-read the YAML, re-walked
    /// the skills/plugins/marketplaces trees, and re-validated the whole file.
    /// Configuration is fixed for the life of a deployment, so an edit requires
    /// a restart to take effect.
    pub fn load() -> ConfigLoadResult<ServicesConfig> {
        Self::for_active_profile()?.run_cached()
    }

    /// Re-reads and re-validates the active profile's services configuration,
    /// bypassing the [`Self::load`] cache and refreshing it with the result.
    ///
    /// Required by any command that validates its own write: the agent
    /// create/edit/delete commands each load the configuration to resolve their
    /// target before writing, so a cached [`Self::load`] afterwards would
    /// return the pre-write state and the validation would silently pass.
    pub fn reload() -> ConfigLoadResult<ServicesConfig> {
        Self::for_active_profile()?.run_uncached()
    }

    fn run_uncached(&self) -> ConfigLoadResult<ServicesConfig> {
        let config = self.run()?;
        let key = fs::canonicalize(&self.config_path).unwrap_or_else(|_| self.config_path.clone());
        cache_store(key, &config);
        Ok(config)
    }

    /// [`Self::load`] against an explicit path, sharing the same process-wide
    /// cache.
    #[cfg(any(test, feature = "expose-internals"))]
    pub fn load_cached_from_path(path: &Path) -> ConfigLoadResult<ServicesConfig> {
        Self::new(path.to_path_buf()).run_cached()
    }

    /// [`Self::reload`] against an explicit path.
    #[cfg(any(test, feature = "expose-internals"))]
    pub fn reload_from_path(path: &Path) -> ConfigLoadResult<ServicesConfig> {
        Self::new(path.to_path_buf()).run_uncached()
    }

    fn run_cached(&self) -> ConfigLoadResult<ServicesConfig> {
        let key = fs::canonicalize(&self.config_path).unwrap_or_else(|_| self.config_path.clone());

        if let Some(cached) = cache_read(&key) {
            return Ok(cached);
        }

        let config = self.run()?;
        cache_store(key, &config);
        Ok(config)
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

        discover_skills(&self.base_path, &mut merged)?;
        discover_plugins(&self.base_path, &mut merged)?;
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

static CONFIG_CACHE: OnceLock<RwLock<HashMap<PathBuf, ServicesConfig>>> = OnceLock::new();

fn config_cache() -> &'static RwLock<HashMap<PathBuf, ServicesConfig>> {
    CONFIG_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn cache_read(key: &Path) -> Option<ServicesConfig> {
    config_cache()
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(key)
        .cloned()
}

fn cache_store(key: PathBuf, config: &ServicesConfig) {
    config_cache()
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, config.clone());
}
