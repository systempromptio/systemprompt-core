use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_models::mcp::Deployment;
use systemprompt_models::services::{
    AgentConfig, AiConfig, PartialServicesConfig, SchedulerConfig, ServicesConfig,
    Settings as ServicesSettings, SkillsConfig, WebConfig,
};
use systemprompt_models::AppPaths;

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

        merged_config
            .validate()
            .with_context(|| "Services config validation failed")?;

        Ok(merged_config)
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
            .with_context(|| "Config validation failed")?;
        Ok(())
    }
}

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
    #[allow(dead_code)]
    #[serde(default)]
    pub skills: Option<SkillsConfig>,
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
            ai: root.config.ai.unwrap_or_default(),
            web: root.config.web.unwrap_or_default(),
        };

        for include_path in &root.includes {
            let partial = self.load_include(include_path)?;
            Self::merge_partial(&mut merged, partial)?;
        }

        self.resolve_includes(&mut merged)?;

        merged
            .validate()
            .with_context(|| "Services config validation failed")?;

        Ok(merged)
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
