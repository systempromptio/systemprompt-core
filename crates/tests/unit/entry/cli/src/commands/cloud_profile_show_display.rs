//! Tests for the `cloud profile show` section renderers.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;

use systemprompt_cli::cloud::profile::show_display::{
    DisplayLine, render_ai_section, render_environment_section, render_formatted_config,
    render_mcp_section, render_settings_section,
};
use systemprompt_cli::cloud::profile::show_types::{
    CoreEnvVars, DatabaseEnvVars, EnvironmentConfig, FullConfig, JwtEnvVars, PathsEnvVars,
    RateLimitEnvVars, SettingsOutput, SystempromptEnvVars,
};
use systemprompt_models::{AiConfig, Deployment};

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
