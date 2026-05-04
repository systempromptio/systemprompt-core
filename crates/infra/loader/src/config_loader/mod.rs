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
use systemprompt_models::services::{AiConfig, ServicesConfig};

use crate::error::{ConfigLoadError, ConfigLoadResult};

use includes::resolve_includes_recursively;
use merge::{resolve_skill_instruction_includes, resolve_system_prompt_includes};
use types::{IncludeResolveCtx, RootConfig};

/// Loader for the services-config tree rooted at a single YAML file.
#[derive(Debug)]
pub struct ConfigLoader {
    base_path: PathBuf,
    config_path: PathBuf,
}

impl ConfigLoader {
    /// Constructs a loader for the YAML file at `config_path`.
    ///
    /// `base_path` is taken from the parent directory of `config_path`
    /// (or `.` if there is no parent), and is used as the root for
    /// relative `includes:` and `!include` paths in the top-level file.
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

    /// Constructs a loader from the active profile bootstrap.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigLoadError::ProfileBootstrap`] if the profile has
    /// not been initialised yet.
    pub fn from_env() -> ConfigLoadResult<Self> {
        let profile = ProfileBootstrap::get()
            .map_err(|e| ConfigLoadError::ProfileBootstrap(e.to_string()))?;
        let config_path = PathBuf::from(profile.paths.config());
        Ok(Self::new(config_path))
    }

    /// Loads the active profile's services config in one call.
    ///
    /// # Errors
    ///
    /// Returns any [`ConfigLoadError`] raised by [`Self::from_env`] or
    /// while reading and parsing the resolved YAML file.
    pub fn load() -> ConfigLoadResult<ServicesConfig> {
        Self::from_env()?.run()
    }

    /// Loads a services config from the YAML file at `path`.
    ///
    /// # Errors
    ///
    /// Returns any [`ConfigLoadError`] raised while reading, parsing, or
    /// validating the file or its include graph.
    pub fn load_from_path(path: &Path) -> ConfigLoadResult<ServicesConfig> {
        Self::new(path.to_path_buf()).run()
    }

    /// Loads a services config from in-memory YAML content for tests and
    /// dry-run validation.
    ///
    /// # Errors
    ///
    /// Same as [`Self::load_from_path`], minus the initial file read.
    pub fn load_from_content(content: &str, path: &Path) -> ConfigLoadResult<ServicesConfig> {
        Self::new(path.to_path_buf()).run_from_content(content)
    }

    /// Validates that the file at `path` parses, includes resolve, and
    /// the merged configuration passes semantic validation.
    ///
    /// # Errors
    ///
    /// Same as [`Self::load_from_path`].
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
        let root: RootConfig =
            serde_yaml::from_str(content).map_err(|e| ConfigLoadError::Yaml {
                path: self.config_path.clone(),
                source: e,
            })?;

        let mut merged = ServicesConfig {
            agents: root.agents,
            mcp_servers: root.mcp_servers,
            settings: root.settings,
            scheduler: root.scheduler,
            ai: root.ai.unwrap_or_else(AiConfig::default),
            web: root.web,
            plugins: root.plugins,
            skills: root.skills,
            content: root.content,
        };

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
            for include_path in &root.includes {
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

        merged.settings.apply_env_overrides();

        merged
            .validate()
            .map_err(|e| ConfigLoadError::Validation(e.to_string()))?;

        Ok(merged)
    }

    /// Reads the top-level `includes:` list from the configured file
    /// without performing any merging or validation.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigLoadError::Io`] or [`ConfigLoadError::Yaml`] if
    /// the file cannot be read or parsed.
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

    /// Lists every top-level include with a flag indicating whether the
    /// referenced file currently exists on disk.
    ///
    /// # Errors
    ///
    /// Same as [`Self::get_includes`].
    pub fn list_all_includes(&self) -> ConfigLoadResult<Vec<(String, bool)>> {
        self.get_includes()?
            .into_iter()
            .map(|include| {
                let exists = self.base_path.join(&include).exists();
                Ok((include, exists))
            })
            .collect()
    }

    /// Returns the directory used as the base for relative include paths
    /// in the top-level config.
    #[must_use]
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}
