//! Terminal rendering of profile sections for `cloud profile show`.
//!
//! Each section renders to [`DisplayLine`]s; `print_formatted_config` emits
//! them through `CliService`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use systemprompt_logging::CliService;
use systemprompt_models::{
    AgentConfig, AiConfig, ContentConfigRaw, Deployment, SkillsConfig, WebConfig,
};

use super::show_types::{EnvironmentConfig, FullConfig, SettingsOutput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayLine {
    Section(String),
    Info(String),
    KeyValue(String, String),
}

impl DisplayLine {
    fn section(title: impl Into<String>) -> Self {
        Self::Section(title.into())
    }

    fn info(text: impl Into<String>) -> Self {
        Self::Info(text.into())
    }

    fn key_value(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::KeyValue(label.into(), value.into())
    }
}

pub(super) fn print_formatted_config(config: &FullConfig) {
    for line in render_formatted_config(config) {
        match line {
            DisplayLine::Section(title) => CliService::section(&title),
            DisplayLine::Info(text) => CliService::info(&text),
            DisplayLine::KeyValue(label, value) => CliService::key_value(&label, &value),
        }
    }
}

pub fn render_formatted_config(config: &FullConfig) -> Vec<DisplayLine> {
    let mut lines = Vec::new();
    if let Some(env) = &config.environment {
        lines.extend(render_environment_section(env));
    }
    if let Some(settings) = &config.settings {
        lines.extend(render_settings_section(settings));
    }
    if let Some(agents) = &config.agents {
        lines.extend(render_agents_section(agents));
    }
    if let Some(mcp_servers) = &config.mcp_servers {
        lines.extend(render_mcp_section(mcp_servers));
    }
    if let Some(skills) = &config.skills {
        lines.extend(render_skills_section(skills));
    }
    if let Some(ai) = &config.ai {
        lines.extend(render_ai_section(ai));
    }
    if let Some(web) = &config.web {
        lines.extend(render_web_section(web));
    }
    if let Some(content) = &config.content {
        lines.extend(render_content_section(content));
    }
    lines
}

pub fn render_environment_section(env: &EnvironmentConfig) -> Vec<DisplayLine> {
    vec![
        DisplayLine::section("Environment Configuration"),
        DisplayLine::info("Core Settings:"),
        DisplayLine::key_value("  sitename", &env.core.sitename),
        DisplayLine::key_value("  host", &env.core.host),
        DisplayLine::key_value("  port", env.core.port.to_string()),
        DisplayLine::key_value("  api_server_url", &env.core.api_server_url),
        DisplayLine::key_value("  api_external_url", &env.core.api_external_url),
        DisplayLine::key_value("  use_https", env.core.use_https.to_string()),
        DisplayLine::info("Database:"),
        DisplayLine::key_value("  type", &env.database.database_type),
        DisplayLine::key_value("  url", &env.database.database_url),
        DisplayLine::info("JWT:"),
        DisplayLine::key_value("  issuer", &env.jwt.issuer),
        DisplayLine::key_value("  secret", &env.jwt.secret),
    ]
}

pub fn render_settings_section(settings: &SettingsOutput) -> Vec<DisplayLine> {
    vec![
        DisplayLine::section("Services Settings"),
        DisplayLine::key_value(
            "  agent_port_range",
            format!(
                "{}-{}",
                settings.agent_port_range.0, settings.agent_port_range.1
            ),
        ),
        DisplayLine::key_value(
            "  mcp_port_range",
            format!(
                "{}-{}",
                settings.mcp_port_range.0, settings.mcp_port_range.1
            ),
        ),
        DisplayLine::key_value(
            "  auto_start_enabled",
            settings.auto_start_enabled.to_string(),
        ),
    ]
}

pub fn render_agents_section(agents: &HashMap<String, AgentConfig>) -> Vec<DisplayLine> {
    let mut lines = vec![DisplayLine::section(format!("Agents ({})", agents.len()))];
    for (name, agent) in agents {
        lines.push(DisplayLine::info(format!(
            "  {} (port: {}, enabled: {})",
            name, agent.port, agent.enabled
        )));
        lines.push(DisplayLine::key_value("    endpoint", &agent.endpoint));
        lines.push(DisplayLine::key_value(
            "    display_name",
            &agent.card.display_name,
        ));
    }
    lines
}

pub fn render_mcp_section(mcp_servers: &HashMap<String, Deployment>) -> Vec<DisplayLine> {
    let mut lines = vec![DisplayLine::section(format!(
        "MCP Servers ({})",
        mcp_servers.len()
    ))];
    for (name, mcp) in mcp_servers {
        lines.push(DisplayLine::info(format!(
            "  {} (port: {}, enabled: {})",
            name, mcp.port, mcp.enabled
        )));
        lines.push(DisplayLine::key_value(
            "    endpoint",
            mcp.endpoint
                .as_deref()
                .unwrap_or("<derived from api_external_url>"),
        ));
        lines.push(DisplayLine::key_value("    binary", &mcp.binary));
    }
    lines
}

pub fn render_skills_section(skills: &SkillsConfig) -> Vec<DisplayLine> {
    let mut lines = vec![
        DisplayLine::section(format!("Skills ({})", skills.skills.len())),
        DisplayLine::key_value("  enabled", skills.enabled.to_string()),
    ];
    for (name, skill) in &skills.skills {
        lines.push(DisplayLine::info(format!(
            "  {} (enabled: {})",
            name, skill.enabled
        )));
        lines.push(DisplayLine::key_value("    id", skill.id.as_str()));
        lines.push(DisplayLine::key_value("    name", &skill.name));
    }
    lines
}

pub fn render_ai_section(ai: &AiConfig) -> Vec<DisplayLine> {
    let mut lines = vec![DisplayLine::section("AI Configuration")];
    if !ai.default_provider.is_empty() {
        lines.push(DisplayLine::key_value(
            "  default_provider",
            &ai.default_provider,
        ));
    }
    for (name, provider) in &ai.providers {
        lines.push(DisplayLine::info(format!(
            "  {} (enabled: {})",
            name, provider.enabled
        )));
        if !provider.default_model.is_empty() {
            lines.push(DisplayLine::key_value(
                "    default_model",
                &provider.default_model,
            ));
        }
    }
    lines
}

pub fn render_web_section(web: &WebConfig) -> Vec<DisplayLine> {
    vec![
        DisplayLine::section("Web Configuration"),
        DisplayLine::key_value("  site_name", &web.branding.name),
        DisplayLine::key_value("  title", &web.branding.title),
    ]
}

pub fn render_content_section(content: &ContentConfigRaw) -> Vec<DisplayLine> {
    let mut lines = vec![DisplayLine::section(format!(
        "Content Sources ({})",
        content.content_sources.len()
    ))];
    for (name, source) in &content.content_sources {
        lines.push(DisplayLine::info(format!(
            "  {} (enabled: {})",
            name, source.enabled
        )));
        lines.push(DisplayLine::key_value("    path", &source.path));
    }
    lines
}
