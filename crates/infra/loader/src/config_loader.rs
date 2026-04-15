use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_models::AppPaths;
use systemprompt_models::mcp::Deployment;
use systemprompt_models::services::{
    AgentConfig, AiConfig, ContentConfig, PartialServicesConfig, PluginConfig, SchedulerConfig,
    ServicesConfig, Settings as ServicesSettings, SkillsConfig, WebConfig,
};

use crate::ConfigWriter;

#[derive(Debug)]
pub struct ConfigLoader {
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
#[serde(deny_unknown_fields)]
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
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub content: ContentConfig,
}

#[derive(serde::Deserialize)]
struct PartialServicesFile {
    #[serde(default)]
    includes: Vec<String>,
    #[serde(flatten)]
    config: PartialServicesConfig,
}

struct IncludeResolveCtx<'a> {
    visited: &'a mut HashSet<PathBuf>,
    merged: &'a mut ServicesConfig,
    chain: Vec<PathBuf>,
}

impl ConfigLoader {
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

    pub fn load() -> Result<ServicesConfig> {
        Self::from_env()?.run()
    }

    pub fn load_from_path(path: &Path) -> Result<ServicesConfig> {
        Self::new(path.to_path_buf()).run()
    }

    pub fn load_from_content(content: &str, path: &Path) -> Result<ServicesConfig> {
        Self::new(path.to_path_buf()).run_from_content(content)
    }

    pub fn validate_file(path: &Path) -> Result<()> {
        let _ = Self::load_from_path(path)?;
        Ok(())
    }

    fn run(&self) -> Result<ServicesConfig> {
        let content = fs::read_to_string(&self.config_path)
            .with_context(|| format!("Failed to read config: {}", self.config_path.display()))?;
        self.run_from_content(&content)
    }

    fn run_from_content(&self, content: &str) -> Result<ServicesConfig> {
        let root: RootConfig = serde_yaml::from_str(content)
            .with_context(|| format!("Failed to parse config: {}", self.config_path.display()))?;

        let mut merged = ServicesConfig {
            agents: root.config.agents,
            mcp_servers: root.config.mcp_servers,
            settings: root.config.settings,
            scheduler: root.config.scheduler,
            ai: root.config.ai.unwrap_or_else(AiConfig::default),
            web: root.config.web,
            plugins: root.config.plugins,
            skills: root.config.skills,
            content: root.config.content,
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
                self.resolve_includes_recursively(include_path, &self.config_path, &mut ctx)?;
            }
        }

        self.discover_and_load_agents(&root.includes, &mut merged)?;

        self.resolve_system_prompt_includes(&mut merged)?;

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

    fn resolve_includes_recursively(
        &self,
        include_path: &str,
        referrer: &Path,
        ctx: &mut IncludeResolveCtx<'_>,
    ) -> Result<()> {
        let referrer_dir = referrer.parent().unwrap_or(&self.base_path);
        let full_path = referrer_dir.join(include_path);

        if !full_path.exists() {
            anyhow::bail!(
                "Include file not found: {}\nReferenced in: {}\nEither create the file or remove \
                 it from the includes list.",
                full_path.display(),
                referrer.display()
            );
        }

        let canonical = fs::canonicalize(&full_path).with_context(|| {
            format!(
                "while loading include {} referenced from {}",
                full_path.display(),
                referrer.display()
            )
        })?;

        if ctx.visited.contains(&canonical) {
            let mut chain: Vec<String> =
                ctx.chain.iter().map(|p| p.display().to_string()).collect();
            chain.push(canonical.display().to_string());
            anyhow::bail!("Include cycle detected: {}", chain.join(" -> "));
        }
        ctx.visited.insert(canonical.clone());

        let content = fs::read_to_string(&canonical).with_context(|| {
            format!(
                "while loading include {} referenced from {}",
                canonical.display(),
                referrer.display()
            )
        })?;

        let partial_file: PartialServicesFile =
            serde_yaml::from_str(&content).with_context(|| {
                format!(
                    "while loading include {} referenced from {}",
                    canonical.display(),
                    referrer.display()
                )
            })?;

        ctx.chain.push(canonical.clone());
        for nested in &partial_file.includes {
            self.resolve_includes_recursively(nested, &canonical, ctx)?;
        }
        ctx.chain.pop();

        Self::merge_partial(ctx.merged, partial_file.config)?;

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
                    target.ai.providers.insert(name, provider);
                }
            }
        }

        if partial.web.is_some() {
            target.web = partial.web;
        }

        for (name, plugin) in partial.plugins {
            if target.plugins.contains_key(&name) {
                anyhow::bail!("Duplicate plugin definition: {name}");
            }
            target.plugins.insert(name, plugin);
        }

        Self::merge_skills(target, partial.skills)?;
        Self::merge_content(&mut target.content, partial.content)?;

        Ok(())
    }

    fn merge_skills(target: &mut ServicesConfig, partial: SkillsConfig) -> Result<()> {
        if partial.auto_discover {
            target.skills.auto_discover = true;
        }
        if partial.skills_path.is_some() {
            target.skills.skills_path = partial.skills_path;
        }
        for (id, skill) in partial.skills {
            if target.skills.skills.contains_key(&id) {
                anyhow::bail!("Duplicate skill definition: {id}");
            }
            target.skills.skills.insert(id, skill);
        }
        Ok(())
    }

    fn merge_content(target: &mut ContentConfig, partial: ContentConfig) -> Result<()> {
        for (name, source) in partial.sources {
            if target.sources.contains_key(&name) {
                anyhow::bail!("Duplicate content source definition: {name}");
            }
            target.sources.insert(name, source);
        }

        for (name, source) in partial.raw.content_sources {
            if target.raw.content_sources.contains_key(&name) {
                anyhow::bail!("Duplicate content source definition: {name}");
            }
            target.raw.content_sources.insert(name, source);
        }

        for (name, category) in partial.raw.categories {
            target.raw.categories.entry(name).or_insert(category);
        }

        if !partial.raw.metadata.default_author.is_empty() {
            target.raw.metadata = partial.raw.metadata;
        }

        Ok(())
    }

    fn resolve_system_prompt_includes(&self, config: &mut ServicesConfig) -> Result<()> {
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
