//! Reads, parses, and merges the active services configuration.
//!
//! [`ConfigLoader`] is the only public entry point. It resolves the active
//! profile (via [`systemprompt_config::ProfileBootstrap`]) to a YAML path,
//! parses the root file, recursively resolves the `includes:` graph
//! (rejecting cycles and duplicate definitions), inlines `!include`
//! references inside agent system prompts, skill instructions and gateway
//! prompt overrides, projects the gateway spec to its resolved runtime form,
//! and finally validates the merged configuration — provider registry and
//! gateway references included — before returning it to the caller.
//!
//! [`ConfigLoader::load`] — the active-profile entry point — memoises its
//! result for the lifetime of the process. The explicit
//! [`ConfigLoader::load_from_path`] and [`ConfigLoader::validate_file`] forms
//! are one-shot reads of a caller-supplied file and are never cached.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod discovery;
pub mod gateway;
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

    pub fn load() -> ConfigLoadResult<ServicesConfig> {
        Self::for_active_profile()?.run_cached()
    }

    pub fn reload() -> ConfigLoadResult<ServicesConfig> {
        Self::for_active_profile()?.run_uncached()
    }

    fn run_uncached(&self) -> ConfigLoadResult<ServicesConfig> {
        let config = self.run()?;
        let key = fs::canonicalize(&self.config_path).unwrap_or_else(|_| self.config_path.clone());
        cache_store(key, &config);
        Ok(config)
    }

    #[cfg(any(test, feature = "expose-internals"))]
    pub fn load_cached_from_path(path: &Path) -> ConfigLoadResult<ServicesConfig> {
        Self::new(path.to_path_buf()).run_cached()
    }

    #[cfg(any(test, feature = "expose-internals"))]
    pub fn reload_from_path(path: &Path) -> ConfigLoadResult<ServicesConfig> {
        Self::new(path.to_path_buf()).run_uncached()
    }

    fn run_cached(&self) -> ConfigLoadResult<ServicesConfig> {
        let key = self.cache_key();

        if let Some(cached) = cache_read(&key) {
            return Ok(cached);
        }

        let config = self.run()?;
        cache_store(key, &config);
        Ok(config)
    }

    // Why: canonicalising the file itself makes the cache key depend on the file
    // still existing — `fs::canonicalize` fails once it is removed and the key
    // silently falls back to the uncanonicalised path, missing the entry at the
    // exact moment the cache is meant to cover for the missing file. Only the
    // directory is resolved, which survives the file's removal and still folds
    // away symlinked roots (on macOS `/var` vs `/private/var`).
    fn cache_key(&self) -> PathBuf {
        let Some(parent) = self.config_path.parent() else {
            return self.config_path.clone();
        };
        let Some(name) = self.config_path.file_name() else {
            return self.config_path.clone();
        };
        fs::canonicalize(parent).map_or_else(|_| self.config_path.clone(), |dir| dir.join(name))
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
        gateway::resolve_file_gateway_includes(&self.base_path, &mut merged)?;
        gateway::project_gateway(&mut merged);

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

        if let Ok(profile) = ProfileBootstrap::get()
            && !profile.services.is_identity()
        {
            merged
                .apply_port_offset(profile.services.port_offset)
                .map_err(|e| ConfigLoadError::Validation(e.to_string()))?;
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
