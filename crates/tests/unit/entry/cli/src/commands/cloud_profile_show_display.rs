//! Tests for the `cloud profile show` section renderers.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;

use systemprompt_cli::cloud::profile::show_display::{
    DisplayLine, render_agents_section, render_ai_section, render_content_section,
    render_environment_section, render_formatted_config, render_mcp_section,
    render_settings_section, render_skills_section,
};
use systemprompt_cli::cloud::profile::show_types::{
    CoreEnvVars, DatabaseEnvVars, EnvironmentConfig, FullConfig, JwtEnvVars, PathsEnvVars,
    RateLimitEnvVars, SettingsOutput, SystempromptEnvVars,
};
use systemprompt_models::services::{AgentConfig, SkillsConfig};
use systemprompt_models::{AiConfig, ContentConfigRaw, Deployment};

fn env() -> EnvironmentConfig {
    EnvironmentConfig {
        core: CoreEnvVars {
            sitename: "Site".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 8080,
            api_server_url: "http://localhost:8080".to_owned(),
            api_external_url: "https://example.com".to_owned(),
            use_https: true,
            github_link: String::new(),
            github_token: None,
            cors_allowed_origins: vec![],
        },
        systemprompt: SystempromptEnvVars {
            env: "local".to_owned(),
            verbosity: "info".to_owned(),
            services_path: None,
            skills_path: None,
            config_path: None,
        },
        database: DatabaseEnvVars {
            database_type: "postgres".to_owned(),
            database_url: "postgres://***@localhost/db".to_owned(),
        },
        jwt: JwtEnvVars {
            issuer: "https://issuer.test".to_owned(),
            secret: "[redacted]".to_owned(),
            access_token_expiration: 3600,
            refresh_token_expiration: 86400,
        },
        rate_limits: RateLimitEnvVars {
            disabled: false,
            burst_multiplier: 3,
        },
        paths: PathsEnvVars {
            system_path: "/x".to_owned(),
            services: "/x/services".to_owned(),
            skills: "/x/skills".to_owned(),
            services_config: "/x/services/config".to_owned(),
        },
    }
}

fn settings() -> SettingsOutput {
    SettingsOutput {
        agent_port_range: (9000, 9100),
        mcp_port_range: (5000, 5100),
        auto_start_enabled: true,
        validation_strict: false,
        schema_validation_mode: "warn".to_owned(),
    }
}

fn key_values(lines: &[DisplayLine]) -> Vec<(String, String)> {
    lines
        .iter()
        .filter_map(|l| match l {
            DisplayLine::KeyValue(k, v) => Some((k.clone(), v.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn environment_section_renders_core_database_and_jwt() {
    let lines = render_environment_section(&env());

    assert_eq!(
        lines[0],
        DisplayLine::Section("Environment Configuration".to_owned())
    );
    let kvs = key_values(&lines);
    assert!(kvs.contains(&("  sitename".to_owned(), "Site".to_owned())));
    assert!(kvs.contains(&("  port".to_owned(), "8080".to_owned())));
    assert!(kvs.contains(&("  use_https".to_owned(), "true".to_owned())));
    assert!(kvs.contains(&("  type".to_owned(), "postgres".to_owned())));
    assert!(kvs.contains(&("  issuer".to_owned(), "https://issuer.test".to_owned())));
    assert!(kvs.contains(&("  secret".to_owned(), "[redacted]".to_owned())));
}

#[test]
fn settings_section_formats_port_ranges() {
    let kvs = key_values(&render_settings_section(&settings()));

    assert!(kvs.contains(&("  agent_port_range".to_owned(), "9000-9100".to_owned())));
    assert!(kvs.contains(&("  mcp_port_range".to_owned(), "5000-5100".to_owned())));
    assert!(kvs.contains(&("  auto_start_enabled".to_owned(), "true".to_owned())));
}

#[test]
fn mcp_section_marks_derived_endpoints() {
    let server: Deployment = serde_yaml::from_str(
        "binary: svc-bin\npackage: null\nport: 5010\nenabled: true\ndisplay_in_web: false\noauth:\n  required: false\n  scopes: []\n  audience: mcp\n  client_id: null\n",
    )
    .unwrap();
    let mut servers = HashMap::new();
    servers.insert("svc".to_owned(), server);

    let lines = render_mcp_section(&servers);
    assert_eq!(lines[0], DisplayLine::Section("MCP Servers (1)".to_owned()));
    let kvs = key_values(&lines);
    assert!(kvs.contains(&(
        "    endpoint".to_owned(),
        "<derived from api_external_url>".to_owned()
    )));
    assert!(kvs.contains(&("    binary".to_owned(), "svc-bin".to_owned())));
}

#[test]
fn ai_section_skips_empty_provider_and_model_fields() {
    let ai: AiConfig = serde_yaml::from_str(
        "default_provider: anthropic\nproviders:\n  anthropic:\n    enabled: true\n    default_model: claude\n  bare:\n    enabled: false\n",
    )
    .unwrap();

    let lines = render_ai_section(&ai);
    let kvs = key_values(&lines);
    assert!(kvs.contains(&("  default_provider".to_owned(), "anthropic".to_owned())));
    assert!(kvs.contains(&("    default_model".to_owned(), "claude".to_owned())));
    assert_eq!(kvs.len(), 2);

    let empty: AiConfig = serde_yaml::from_str("providers: {}\n").unwrap();
    let lines = render_ai_section(&empty);
    assert_eq!(
        lines,
        vec![DisplayLine::Section("AI Configuration".to_owned())]
    );
}

#[test]
fn formatted_config_renders_only_present_sections_in_order() {
    let empty = FullConfig::empty();
    assert!(render_formatted_config(&empty).is_empty());

    let config = FullConfig::empty()
        .with_environment(env())
        .with_settings(settings());
    let lines = render_formatted_config(&config);

    let sections: Vec<&str> = lines
        .iter()
        .filter_map(|l| match l {
            DisplayLine::Section(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        sections,
        vec!["Environment Configuration", "Services Settings"]
    );
}

#[test]
fn agents_section_lists_port_endpoint_and_display_name() {
    let agent: AgentConfig = serde_yaml::from_str(
        "name: helper\nport: 9001\nendpoint: /a2a\nenabled: true\ndev_only: false\nis_primary: false\ndefault: false\ntags: []\ncard:\n  protocolVersion: '1.0'\n  displayName: Helper Agent\n  description: Helps\n  version: 1.0.0\n  preferredTransport: JSONRPC\n  capabilities: {}\n  defaultInputModes: [text/plain]\n  defaultOutputModes: [text/plain]\n  supportsAuthenticatedExtendedCard: false\nmetadata: {}\noauth: {}\n",
    )
    .unwrap();
    let mut agents = HashMap::new();
    agents.insert("helper".to_owned(), agent);

    let lines = render_agents_section(&agents);
    assert_eq!(lines[0], DisplayLine::Section("Agents (1)".to_owned()));

    let kvs = key_values(&lines);
    assert!(kvs.contains(&("    endpoint".to_owned(), "/a2a".to_owned())));
    assert!(kvs.contains(&("    display_name".to_owned(), "Helper Agent".to_owned())));
    assert!(
        lines.iter().any(
            |l| matches!(l, DisplayLine::Info(s) if s.contains("port: 9001")
                && s.contains("enabled: true"))
        ),
        "the info line should carry port and enabled: {lines:?}"
    );
}

#[test]
fn agents_section_headline_counts_zero() {
    let agents: HashMap<String, AgentConfig> = HashMap::new();
    let lines = render_agents_section(&agents);
    assert_eq!(lines, vec![DisplayLine::Section("Agents (0)".to_owned())]);
}

#[test]
fn skills_section_reports_enabled_flag_and_each_skill() {
    let skills: SkillsConfig = serde_yaml::from_str(
        "enabled: true\nauto_discover: false\nskills:\n  writer:\n    id: skill-writer\n    name: Writer\n    description: Writes things\n    enabled: true\n",
    )
    .unwrap();

    let lines = render_skills_section(&skills);
    assert_eq!(lines[0], DisplayLine::Section("Skills (1)".to_owned()));

    let kvs = key_values(&lines);
    assert!(kvs.contains(&("  enabled".to_owned(), "true".to_owned())));
    assert!(kvs.contains(&("    id".to_owned(), "skill-writer".to_owned())));
    assert!(kvs.contains(&("    name".to_owned(), "Writer".to_owned())));
}

#[test]
fn skills_section_with_no_skills_still_reports_the_enabled_flag() {
    let skills: SkillsConfig = serde_yaml::from_str("enabled: false\nskills: {}\n").unwrap();
    let lines = render_skills_section(&skills);

    assert_eq!(lines[0], DisplayLine::Section("Skills (0)".to_owned()));
    assert_eq!(
        key_values(&lines),
        vec![("  enabled".to_owned(), "false".to_owned())]
    );
}

#[test]
fn content_section_lists_each_source_path() {
    let content: ContentConfigRaw = serde_yaml::from_str(
        "content_sources:\n  blog:\n    path: content/blog\n    enabled: true\n    source_id: blog\n    category_id: guides\n",
    )
    .unwrap();

    let lines = render_content_section(&content);
    assert_eq!(
        lines[0],
        DisplayLine::Section("Content Sources (1)".to_owned())
    );
    assert!(key_values(&lines).contains(&("    path".to_owned(), "content/blog".to_owned())));
}

#[test]
fn content_section_headline_counts_zero() {
    let content: ContentConfigRaw = serde_yaml::from_str("content_sources: {}\n").unwrap();
    assert_eq!(
        render_content_section(&content),
        vec![DisplayLine::Section("Content Sources (0)".to_owned())]
    );
}

#[test]
fn formatted_config_renders_every_section_when_all_are_present() {
    let agents: HashMap<String, AgentConfig> = HashMap::new();
    let mcp: HashMap<String, Deployment> = HashMap::new();
    let skills: SkillsConfig = serde_yaml::from_str("enabled: true\nskills: {}\n").unwrap();
    let ai: AiConfig = serde_yaml::from_str("providers: {}\n").unwrap();
    let content: ContentConfigRaw = serde_yaml::from_str("content_sources: {}\n").unwrap();

    let config = FullConfig::empty()
        .with_environment(env())
        .with_settings(settings())
        .with_agents(agents)
        .with_mcp_servers(mcp)
        .with_skills(skills)
        .with_ai(ai)
        .with_content(content);

    let sections: Vec<String> = render_formatted_config(&config)
        .iter()
        .filter_map(|l| match l {
            DisplayLine::Section(s) => Some(s.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(
        sections,
        vec![
            "Environment Configuration",
            "Services Settings",
            "Agents (0)",
            "MCP Servers (0)",
            "Skills (0)",
            "AI Configuration",
            "Content Sources (0)",
        ],
        "sections must render in declaration order, skipping absent ones"
    );
}
