use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_models::services::{PartialServicesConfig, ServicesConfig};
use systemprompt_models::AppPaths;

use crate::ConfigWriter;

#[derive(Debug, Clone, Copy)]
pub struct ConfigLoader;

#[derive(serde::Deserialize)]
struct ConfigWithIncludes {
    #[serde(default)]
    includes: Vec<String>,
    #[serde(flatten)]
    config: ServicesConfig,
}

impl ConfigLoader {
    pub fn load() -> Result<ServicesConfig> {
        let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
        let path = paths.system().settings();
        Self::load_from_path(path)
    }

    pub fn load_from_path(config_path: &Path) -> Result<ServicesConfig> {
        let content = fs::read_to_string(config_path).with_context(|| {
            format!("Failed to read services config: {}", config_path.display())
        })?;

        Self::load_from_content(&content, config_path)
    }

    pub fn load_from_content(content: &str, config_path: &Path) -> Result<ServicesConfig> {
        let root_config: ConfigWithIncludes = serde_yaml::from_str(content)
            .with_context(|| format!("Failed to parse config: {}", config_path.display()))?;

        let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));

        let mut merged_config = root_config.config;

        for include_path in &root_config.includes {
            let full_path = config_dir.join(include_path);
            let include_config = Self::load_include_file(&full_path)?;
            Self::merge_include(&mut merged_config, include_config);
        }

        Self::discover_and_load_agents(
            config_dir,
            config_path,
            &root_config.includes,
            &mut merged_config,
        )?;

        merged_config.settings.apply_env_overrides();

        merged_config
            .validate()
            .map_err(|e| anyhow::anyhow!("Services config validation failed: {}", e))?;

        Ok(merged_config)
    }

    fn discover_and_load_agents(
        config_dir: &Path,
        config_path: &Path,
        existing_includes: &[String],
        merged_config: &mut ServicesConfig,
    ) -> Result<()> {
        let agents_dir = config_dir.join("../agents");

        if !agents_dir.exists() {
            return Ok(());
        }

        let included_files: HashSet<String> = existing_includes
            .iter()
            .filter_map(|inc| {
                Path::new(inc)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
            })
            .collect();

        let entries = fs::read_dir(&agents_dir).with_context(|| {
            format!("Failed to read agents directory: {}", agents_dir.display())
        })?;

        for entry in entries {
            let path = entry
                .with_context(|| format!("Failed to read entry in: {}", agents_dir.display()))?
                .path();

            let is_yaml = path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml");

            if !is_yaml {
                continue;
            }

            let file_name = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .ok_or_else(|| anyhow::anyhow!("Invalid file path: {}", path.display()))?;

            if included_files.contains(&file_name) {
                continue;
            }

            let include_config = Self::load_include_file(&path)?;
            Self::merge_include(merged_config, include_config);

            let relative_path = format!("../agents/{}", file_name);
            ConfigWriter::add_include(&relative_path, config_path).with_context(|| {
                format!(
                    "Failed to add discovered agent to includes: {}",
                    relative_path
                )
            })?;
        }

        Ok(())
    }

    fn load_include_file(path: &PathBuf) -> Result<PartialServicesConfig> {
        if !path.exists() {
            anyhow::bail!(
                "Include file not found: {}\nEither create the file or remove it from the \
                 includes list.",
                path.display()
            );
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read include: {}", path.display()))?;

        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse include: {}", path.display()))
    }

    fn merge_include(target: &mut ServicesConfig, partial: PartialServicesConfig) {
        for (name, agent) in partial.agents {
            target.agents.insert(name, agent);
        }

        for (name, mcp_server) in partial.mcp_servers {
            target.mcp_servers.insert(name, mcp_server);
        }

        if partial.scheduler.is_some() {
            target.scheduler = partial.scheduler;
        }

        if let Some(ai) = partial.ai {
            target.ai = ai;
        }

        if let Some(web) = partial.web {
            target.web = web;
        }
    }

    pub fn validate_file(path: &Path) -> Result<()> {
        let config = Self::load_from_path(path)?;
        config
            .validate()
            .map_err(|e| anyhow::anyhow!("Config validation failed: {}", e))?;
        Ok(())
    }
}
