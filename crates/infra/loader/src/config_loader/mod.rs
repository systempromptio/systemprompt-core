//! Reads, parses, and merges the active services configuration.
//!
//! [`ConfigLoader`] is the only public entry point. It resolves the active
//! profile's services root to a YAML path, parses the root file, recursively resolves the `includes:` graph
//! (rejecting cycles and duplicate definitions), reads the model catalog by
//! convention from `services/ai/` ([`catalog`]), inlines `!include`
//! references inside agent system prompts, skill instructions and gateway
//! prompt overrides, projects the gateway spec to its resolved runtime form,
//! and finally validates the merged configuration — the catalog resolved
//! with every reference checked — before returning it to the caller.
//!
//! A pre-0.73 tree is refused with the `systemprompt admin config migrate`
//! hint. A provider whose secret is absent is still loaded: a missing
//! credential is health state the gateway reports, never a rewrite of the
//! catalog.
//!
//! Every entry point is a fresh read; the process holds the parsed result in
//! [`crate::ServicesState`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod catalog;
mod discovery;
pub mod gateway;
mod includes;
mod merge;
mod types;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_manifest::profile::Profile;
use systemprompt_manifest::services::ServicesConfig;

use crate::error::{ConfigLoadError, ConfigLoadResult};

use discovery::{discover_marketplaces, discover_plugins, discover_skills};
use includes::resolve_includes_recursively;
use merge::{resolve_skill_instruction_includes, resolve_system_prompt_includes};
use types::IncludeResolveCtx;

#[derive(Debug)]
pub struct ConfigLoader {
    base_path: PathBuf,
    config_path: PathBuf,
    port_offset: Option<u16>,
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
            port_offset: None,
        }
    }

    #[must_use]
    pub fn for_profile(profile: &Profile, services_root: &Path) -> Self {
        Self::new(services_root.join("config").join("config.yaml")).with_profile(Some(profile))
    }

    #[must_use]
    pub fn with_profile(mut self, profile: Option<&Profile>) -> Self {
        self.port_offset = profile
            .filter(|profile| !profile.services.is_identity())
            .map(|profile| profile.services.port_offset);
        self
    }

    pub fn load(self) -> ConfigLoadResult<ServicesConfig> {
        self.run()
    }

    pub fn load_from_path(
        path: &Path,
        profile: Option<&Profile>,
    ) -> ConfigLoadResult<ServicesConfig> {
        Self::new(path.to_path_buf()).with_profile(profile).run()
    }

    pub fn load_from_content(
        content: &str,
        path: &Path,
        profile: Option<&Profile>,
    ) -> ConfigLoadResult<ServicesConfig> {
        Self::new(path.to_path_buf())
            .with_profile(profile)
            .run_from_content(content)
    }

    pub fn validate_file(path: &Path, profile: Option<&Profile>) -> ConfigLoadResult<()> {
        Self::load_from_path(path, profile).map(|_| ())
    }

    fn run(&self) -> ConfigLoadResult<ServicesConfig> {
        let content = fs::read_to_string(&self.config_path).map_err(|e| ConfigLoadError::Io {
            path: self.config_path.clone(),
            source: e,
        })?;
        self.run_from_content(&content)
    }

    fn run_from_content(&self, content: &str) -> ConfigLoadResult<ServicesConfig> {
        catalog::refuse_legacy(&self.config_path, content)?;
        let mut merged: ServicesConfig =
            serde_yaml::from_str(content).map_err(|e| ConfigLoadError::Yaml {
                path: self.config_path.clone(),
                source: e,
            })?;
        catalog::refuse_catalog_sections(&self.config_path, &merged)?;

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

        let document = catalog::read_catalog_dir(&catalog::catalog_dir(&self.base_path))?;
        catalog::install_catalog(&mut merged, document);

        resolve_system_prompt_includes(&self.base_path, &mut merged)?;
        resolve_skill_instruction_includes(&self.base_path, &mut merged)?;
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

        if let Some(port_offset) = self.port_offset {
            merged.apply_port_offset(port_offset)?;
        }

        merged.validate()?;

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
