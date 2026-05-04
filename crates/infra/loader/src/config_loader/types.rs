use std::collections::HashMap;
use std::path::PathBuf;

use systemprompt_models::mcp::Deployment;
use systemprompt_models::services::{
    AgentConfig, AiConfig, ContentConfig, PartialServicesConfig, PluginConfig, SchedulerConfig,
    ServicesConfig, Settings as ServicesSettings, SkillsConfig, WebConfig,
};

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct RootConfig {
    #[serde(default)]
    pub includes: Vec<String>,
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

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct PartialServicesFile {
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub mcp_servers: HashMap<String, Deployment>,
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

impl PartialServicesFile {
    pub(super) fn into_partial_config(self) -> PartialServicesConfig {
        PartialServicesConfig {
            agents: self.agents,
            mcp_servers: self.mcp_servers,
            scheduler: self.scheduler,
            ai: self.ai,
            web: self.web,
            plugins: self.plugins,
            skills: self.skills,
            content: self.content,
        }
    }
}

pub(super) struct IncludeResolveCtx<'a> {
    pub visited: &'a mut std::collections::HashSet<PathBuf>,
    pub merged: &'a mut ServicesConfig,
    pub chain: Vec<PathBuf>,
}
