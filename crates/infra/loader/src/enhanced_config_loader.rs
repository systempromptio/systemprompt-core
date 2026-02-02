use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_models::mcp::Deployment;
use systemprompt_models::services::{
    AgentConfig, AiConfig, PartialServicesConfig, SchedulerConfig, ServicesConfig,
    Settings as ServicesSettings, WebConfig,
};
use systemprompt_models::AppPaths;

use crate::ConfigWriter;

#[derive(Debug)]
pub struct EnhancedConfigLoader {
    base_path: PathBuf,
    config_path: PathBuf,
}

#[derive(serde::Deserialize)]
struct RootConfig {
    #[serde(default)]
    includes: Vec<String>,
    #[serde(flatten)]
    config: PartialServicesRootConfig,
}

#[derive(serde::Deserialize, Default)]
struct PartialServicesRootConfig {
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub mcp_servers: HashMap<String, Deployment>,
    #[serde(default)]
    pub settings: ServicesSettings,
    #[serde(default)]
    pub scheduler: Option<SchedulerConfig>,
    #[serde(default)]
    pub ai: Option<AiConfig>,
    #[serde(default)]
    pub web: Option<WebConfig>,
}

impl EnhancedConfigLoader {
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

    pub fn from_env() -> Result<Self> {
        let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
        let config_path = paths.system().settings().to_path_buf();
        Ok(Self::new(config_path))
    }

    pub fn load(&self) -> Result<ServicesConfig> {
        let content = fs::read_to_string(&self.config_path)
            .with_context(|| format!("Failed to read config: {}", self.config_path.display()))?;

        self.load_from_content(&content)
    }

    pub fn load_from_content(&self, content: &str) -> Result<ServicesConfig> {
        let root: RootConfig = serde_yaml::from_str(content)
            .with_context(|| format!("Failed to parse config: {}", self.config_path.display()))?;

        let mut merged = ServicesConfig {
            agents: root.config.agents,
            mcp_servers: root.config.mcp_servers,
            settings: root.config.settings,
            scheduler: root.config.scheduler,
            ai: root.config.ai.unwrap_or_else(AiConfig::default),
            web: root.config.web.unwrap_or_else(WebConfig::default),
        };

        for include_path in &root.includes {
            let partial = self.load_include(include_path)?;
            Self::merge_partial(&mut merged, partial)?;
        }

        self.discover_and_load_agents(&root.includes, &mut merged)?;

        self.resolve_includes(&mut merged)?;

        merged.settings.apply_env_overrides();

        merged
            .validate()
            .map_err(|e| anyhow::anyhow!("Services config validation failed: {}", e))?;

        Ok(merged)
    }

    fn discover_and_load_agents(
        &self,
        existing_includes: &[String],
        merged: &mut ServicesConfig,
    ) -> Result<()> {
        let agents_dir = self.base_path.join("../agents");

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

            let relative_path = format!("../agents/{}", file_name);
            let partial = self.load_include(&relative_path)?;
            Self::merge_partial(merged, partial)?;

            ConfigWriter::add_include(&relative_path, &self.config_path).with_context(|| {
                format!(
                    "Failed to add discovered agent to includes: {}",
                    relative_path
                )
            })?;
        }

        Ok(())
    }

    fn load_include(&self, path: &str) -> Result<PartialServicesConfig> {
        let full_path = self.base_path.join(path);

        if !full_path.exists() {
            anyhow::bail!(
                "Include file not found: {}\nReferenced in: {}/config.yaml\nEither create the \
                 file or remove it from the includes list.",
                full_path.display(),
                self.base_path.display()
            );
        }

        let content = fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read include: {}", full_path.display()))?;

        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse include: {}", full_path.display()))
    }

    fn merge_partial(target: &mut ServicesConfig, partial: PartialServicesConfig) -> Result<()> {
        for (name, agent) in partial.agents {
            if target.agents.contains_key(&name) {
                anyhow::bail!("Duplicate agent definition: {name}");
            }
            target.agents.insert(name, agent);
        }

        for (name, mcp) in partial.mcp_servers {
            if target.mcp_servers.contains_key(&name) {
                anyhow::bail!("Duplicate MCP server definition: {name}");
            }
            target.mcp_servers.insert(name, mcp);
        }

        if partial.scheduler.is_some() && target.scheduler.is_none() {
            target.scheduler = partial.scheduler;
        }

        if let Some(ai) = partial.ai {
            if target.ai.providers.is_empty() && !ai.providers.is_empty() {
                target.ai = ai;
            } else {
                for (name, provider) in ai.providers {
                    target.ai.providers.entry(name).or_insert(provider);
                }
            }
        }

        if let Some(web) = partial.web {
            target.web = web;
        }

        Ok(())
    }

    fn resolve_includes(&self, config: &mut ServicesConfig) -> Result<()> {
        for (name, agent) in &mut config.agents {
            if let Some(ref system_prompt) = agent.metadata.system_prompt {
                if let Some(include_path) = system_prompt.strip_prefix("!include ") {
                    let full_path = self.base_path.join(include_path.trim());
                    let resolved = fs::read_to_string(&full_path).with_context(|| {
                        format!(
                            "Failed to resolve system_prompt include for agent '{name}': {}",
                            full_path.display()
                        )
                    })?;
                    agent.metadata.system_prompt = Some(resolved);
                }
            }
        }

        Ok(())
    }

    pub fn validate_file(path: &Path) -> Result<()> {
        let loader = Self::new(path.to_path_buf());
        let _config = loader.load()?;
        Ok(())
    }

    pub fn get_includes(&self) -> Result<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct IncludesOnly {
            #[serde(default)]
            includes: Vec<String>,
        }

        let content = fs::read_to_string(&self.config_path)?;
        let parsed: IncludesOnly = serde_yaml::from_str(&content)?;
        Ok(parsed.includes)
    }

    pub fn list_all_includes(&self) -> Result<Vec<(String, bool)>> {
        self.get_includes()?
            .into_iter()
            .map(|include| {
                let exists = self.base_path.join(&include).exists();
                Ok((include, exists))
            })
            .collect()
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}
