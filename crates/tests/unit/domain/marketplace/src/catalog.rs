use std::collections::HashMap;
use std::fs;

use systemprompt_marketplace::catalog::{
    load_agents, load_hooks, load_managed_mcp_servers, load_plugins, load_skills,
};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::Deployment;
use systemprompt_models::mcp::deployment::OAuthRequirement;
use systemprompt_models::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, OAuthConfig, ServicesConfig,
};

use crate::helpers::config_with;

fn make_agent_config(name: &str) -> AgentConfig {
    AgentConfig {
        name: name.to_owned(),
        port: 8080,
        endpoint: String::new(),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        tags: vec![],
        card: AgentCardConfig {
            protocol_version: "0.2.5".into(),
            name: Some(name.to_owned()),
            display_name: name.to_owned(),
            description: format!("{name} agent"),
            version: "1.0.0".into(),
            preferred_transport: "http".into(),
            icon_url: None,
            documentation_url: None,
            provider: None,
            capabilities: Default::default(),
            default_input_modes: vec!["text".into()],
            default_output_modes: vec!["text".into()],
            security_schemes: None,
            security: None,
            skills: vec![],
            supports_authenticated_extended_card: false,
        },
        metadata: AgentMetadataConfig::default(),
        oauth: OAuthConfig::default(),
    }
}

fn make_deployment(_name: &str, enabled: bool, endpoint: Option<&str>) -> Deployment {
    Deployment {
        server_type: Default::default(),
        binary: "server".into(),
        package: None,
        port: 3000,
        endpoint: endpoint.map(ToOwned::to_owned),
        enabled,
        display_in_web: true,
        dev_only: false,
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
    }
}

#[test]
fn load_agents_empty_config_returns_empty() {
    let config = config_with(vec![]);
    let agents = load_agents(&config, "https://api.example.com");
    assert!(agents.is_empty());
}

#[test]
fn load_agents_single_enabled_agent() {
    let mut config = ServicesConfig::default();
    config
        .agents
        .insert("my-agent".to_owned(), make_agent_config("my-agent"));
    let agents = load_agents(&config, "https://api.example.com");
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].id.as_str(), "my-agent");
}

#[test]
fn load_agents_disabled_agents_excluded() {
    let mut config = ServicesConfig::default();
    let mut disabled = make_agent_config("disabled-agent");
    disabled.enabled = false;
    config
        .agents
        .insert("disabled-agent".to_owned(), disabled);
    let agents = load_agents(&config, "https://api.example.com");
    assert!(agents.is_empty());
}

#[test]
fn load_agents_endpoint_built_from_base_when_empty() {
    let mut config = ServicesConfig::default();
    let agent = make_agent_config("search");
    config.agents.insert("search".to_owned(), agent);
    let agents = load_agents(&config, "https://api.example.com/");
    assert_eq!(agents.len(), 1);
    assert!(
        agents[0].endpoint.starts_with("https://api.example.com"),
        "endpoint must be derived from base url",
    );
}

#[test]
fn load_agents_absolute_endpoint_passed_through() {
    let mut config = ServicesConfig::default();
    let mut agent = make_agent_config("remote");
    agent.endpoint = "https://remote.example.com/a2a".into();
    config.agents.insert("remote".to_owned(), agent);
    let agents = load_agents(&config, "https://api.example.com");
    assert_eq!(agents[0].endpoint, "https://remote.example.com/a2a");
}

#[test]
fn load_agents_sorted_alphabetically() {
    let mut config = ServicesConfig::default();
    config
        .agents
        .insert("bravo".to_owned(), make_agent_config("bravo"));
    config
        .agents
        .insert("alpha".to_owned(), make_agent_config("alpha"));
    let agents = load_agents(&config, "https://api.example.com");
    assert_eq!(agents[0].id.as_str(), "alpha");
    assert_eq!(agents[1].id.as_str(), "bravo");
}

#[test]
fn load_skills_no_skills_dir_returns_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    let result = load_skills(dir.path()).expect("no error when skills dir absent");
    assert!(result.is_empty());
}

#[test]
fn load_skills_dir_with_valid_skill() {
    let dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = dir.path().join("skills").join("my-skill");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("config.yaml"),
        "id: my_skill\nname: My Skill\ndescription: test\nenabled: true\n",
    )
    .expect("write config");

    let skills = load_skills(dir.path()).expect("load skills");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].id.as_str(), "my_skill");
}

#[test]
fn load_skills_disabled_skill_excluded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let skill_dir = dir.path().join("skills").join("off-skill");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("config.yaml"),
        "id: off_skill\nname: Off\ndescription: disabled\nenabled: false\n",
    )
    .expect("write config");

    let skills = load_skills(dir.path()).expect("load skills");
    assert!(skills.is_empty());
}

#[test]
fn load_skills_sorted_alphabetically() {
    let dir = tempfile::tempdir().expect("temp dir");
    for name in &["zebra", "apple"] {
        let skill_dir = dir.path().join("skills").join(name);
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        fs::write(
            skill_dir.join("config.yaml"),
            format!("id: {name}\nname: {name}\ndescription: test\nenabled: true\n"),
        )
        .expect("write config");
    }
    let skills = load_skills(dir.path()).expect("load skills");
    assert_eq!(skills[0].id.as_str(), "apple");
    assert_eq!(skills[1].id.as_str(), "zebra");
}

#[test]
fn load_hooks_no_hooks_dir_returns_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    let result = load_hooks(dir.path()).expect("no error when hooks dir absent");
    assert!(result.is_empty());
}

#[test]
fn load_hooks_dir_with_valid_hook() {
    let dir = tempfile::tempdir().expect("temp dir");
    let hook_dir = dir.path().join("hooks").join("my-hook");
    fs::create_dir_all(&hook_dir).expect("create hook dir");
    fs::write(
        hook_dir.join("config.yaml"),
        "event: PreToolUse\nenabled: true\ncommand: echo hello\n",
    )
    .expect("write config");

    let hooks = load_hooks(dir.path()).expect("load hooks");
    assert_eq!(hooks.len(), 1);
}

#[test]
fn load_hooks_disabled_hook_excluded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let hook_dir = dir.path().join("hooks").join("off-hook");
    fs::create_dir_all(&hook_dir).expect("create hook dir");
    fs::write(
        hook_dir.join("config.yaml"),
        "event: PostToolUse\nenabled: false\ncommand: echo off\n",
    )
    .expect("write config");

    let hooks = load_hooks(dir.path()).expect("load hooks");
    assert!(hooks.is_empty());
}

#[test]
fn load_managed_mcp_servers_empty_config_returns_empty() {
    let config = ServicesConfig::default();
    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert!(servers.is_empty());
}

#[test]
fn load_managed_mcp_servers_single_enabled() {
    let mut config = ServicesConfig::default();
    config
        .mcp_servers
        .insert("my-mcp".to_owned(), make_deployment("my-mcp", true, None));
    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name.as_str(), "my-mcp");
}

#[test]
fn load_managed_mcp_servers_disabled_excluded() {
    let mut config = ServicesConfig::default();
    config
        .mcp_servers
        .insert("off-mcp".to_owned(), make_deployment("off-mcp", false, None));
    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert!(servers.is_empty());
}

#[test]
fn load_managed_mcp_servers_default_endpoint_synthesised() {
    let mut config = ServicesConfig::default();
    config
        .mcp_servers
        .insert("my-mcp".to_owned(), make_deployment("my-mcp", true, None));
    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert!(
        servers[0].url.as_str().contains("my-mcp"),
        "synthesised url must contain server name",
    );
}

#[test]
fn load_managed_mcp_servers_absolute_endpoint_used() {
    let mut config = ServicesConfig::default();
    config.mcp_servers.insert(
        "remote-mcp".to_owned(),
        make_deployment("remote-mcp", true, Some("https://remote.example.com/mcp")),
    );
    let servers =
        load_managed_mcp_servers(&config, "https://api.example.com").expect("load mcp servers");
    assert_eq!(
        servers[0].url.as_str(),
        "https://remote.example.com/mcp",
    );
}

#[test]
fn load_plugins_empty_config_returns_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = ServicesConfig::default();
    let plugins = load_plugins(dir.path(), &config);
    assert!(plugins.is_empty());
}
