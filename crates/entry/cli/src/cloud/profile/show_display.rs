use std::collections::HashMap;
use systemprompt_core_logging::CliService;
use systemprompt_models::{
    AgentConfig, AiConfig, ContentConfigRaw, Deployment, SkillsConfig, WebConfig,
};

use super::show_types::{EnvironmentConfig, FullConfig, SettingsOutput};

pub fn print_formatted_config(config: &FullConfig) {
    if let Some(env) = &config.environment {
        print_environment_section(env);
    }
    if let Some(settings) = &config.settings {
        print_settings_section(settings);
    }
    if let Some(agents) = &config.agents {
        print_agents_section(agents);
    }
    if let Some(mcp_servers) = &config.mcp_servers {
        print_mcp_section(mcp_servers);
    }
    if let Some(skills) = &config.skills {
        print_skills_section(skills);
    }
    if let Some(ai) = &config.ai {
        print_ai_section(ai);
    }
    if let Some(web) = &config.web {
        print_web_section(web);
    }
    if let Some(content) = &config.content {
        print_content_section(content);
    }
}

fn print_environment_section(env: &EnvironmentConfig) {
    CliService::section("Environment Configuration");

    CliService::info("Core Settings:");
    CliService::key_value("  sitename", &env.core.sitename);
    CliService::key_value("  host", &env.core.host);
    CliService::key_value("  port", &env.core.port.to_string());
    CliService::key_value("  api_server_url", &env.core.api_server_url);
    CliService::key_value("  api_external_url", &env.core.api_external_url);
    CliService::key_value("  use_https", &env.core.use_https.to_string());

    CliService::info("Database:");
    CliService::key_value("  type", &env.database.database_type);
    CliService::key_value("  url", &env.database.database_url);

    CliService::info("JWT:");
    CliService::key_value("  issuer", &env.jwt.issuer);
    CliService::key_value("  secret", &env.jwt.secret);
}

fn print_settings_section(settings: &SettingsOutput) {
    CliService::section("Services Settings");
    CliService::key_value(
        "  agent_port_range",
        &format!(
            "{}-{}",
            settings.agent_port_range.0, settings.agent_port_range.1
        ),
    );
    CliService::key_value(
        "  mcp_port_range",
        &format!(
            "{}-{}",
            settings.mcp_port_range.0, settings.mcp_port_range.1
        ),
    );
    CliService::key_value(
        "  auto_start_enabled",
        &settings.auto_start_enabled.to_string(),
    );
}

fn print_agents_section(agents: &HashMap<String, AgentConfig>) {
    CliService::section(&format!("Agents ({})", agents.len()));
    for (name, agent) in agents {
        CliService::info(&format!(
            "  {} (port: {}, enabled: {})",
            name, agent.port, agent.enabled
        ));
        CliService::key_value("    endpoint", &agent.endpoint);
        CliService::key_value("    display_name", &agent.card.display_name);
    }
}

fn print_mcp_section(mcp_servers: &HashMap<String, Deployment>) {
    CliService::section(&format!("MCP Servers ({})", mcp_servers.len()));
    for (name, mcp) in mcp_servers {
        CliService::info(&format!(
            "  {} (port: {}, enabled: {})",
            name, mcp.port, mcp.enabled
        ));
        CliService::key_value("    endpoint", &mcp.endpoint);
        CliService::key_value("    binary", &mcp.binary);
    }
}

fn print_skills_section(skills: &SkillsConfig) {
    CliService::section(&format!("Skills ({})", skills.skills.len()));
    CliService::key_value("  enabled", &skills.enabled.to_string());
    for (name, skill) in &skills.skills {
        CliService::info(&format!("  {} (enabled: {})", name, skill.enabled));
        CliService::key_value("    id", &skill.id);
        CliService::key_value("    name", &skill.name);
    }
}

fn print_ai_section(ai: &AiConfig) {
    CliService::section("AI Configuration");
    if !ai.default_provider.is_empty() {
        CliService::key_value("  default_provider", &ai.default_provider);
    }
    for (name, provider) in &ai.providers {
        CliService::info(&format!("  {} (enabled: {})", name, provider.enabled));
        if !provider.default_model.is_empty() {
            CliService::key_value("    default_model", &provider.default_model);
        }
    }
}

fn print_web_section(web: &WebConfig) {
    CliService::section("Web Configuration");
    if let Some(name) = &web.branding.site_name {
        CliService::key_value("  site_name", name);
    }
}

fn print_content_section(content: &ContentConfigRaw) {
    CliService::section(&format!(
        "Content Sources ({})",
        content.content_sources.len()
    ));
    for (name, source) in &content.content_sources {
        CliService::info(&format!("  {} (enabled: {})", name, source.enabled));
        CliService::key_value("    path", &source.path);
    }
}
