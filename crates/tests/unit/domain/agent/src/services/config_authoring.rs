//! Tests for the agent config authoring service: create/delete of agent YAML
//! files in a temp services dir, input validation, and edit application.

use std::fs;
use std::path::Path;

use systemprompt_agent::services::config_authoring::{
    AgentConfigAuthoringService, AgentCreateRequest, AgentEditRequest, ConfigAuthoringError,
};
use systemprompt_identifiers::AgentId;
use systemprompt_models::AgentConfig;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::services::ServicesConfig;

fn create_request(name: &str, port: u16) -> AgentCreateRequest {
    AgentCreateRequest {
        name: name.to_owned(),
        port,
        display_name: "Test Agent".to_owned(),
        description: "A test agent".to_owned(),
        system_prompt: "You are Test Agent.".to_owned(),
        enabled: true,
        ..Default::default()
    }
}

fn create_and_load(services_dir: &Path, name: &str, port: u16) -> AgentConfig {
    let service = AgentConfigAuthoringService::new(services_dir);
    let path = service
        .create(create_request(name, port))
        .expect("create agent");
    load_agent(&path, name)
}

fn load_agent(path: &Path, name: &str) -> AgentConfig {
    let text = fs::read_to_string(path).expect("read agent yaml");
    let value: serde_yaml::Value = serde_yaml::from_str(&text).expect("parse agent yaml");
    serde_yaml::from_value(value["agents"][name].clone()).expect("agent config")
}

fn services_config_with_server(name: &str) -> ServicesConfig {
    let mut config = ServicesConfig::default();
    config.mcp_servers.insert(
        name.to_owned(),
        systemprompt_models::mcp::Deployment {
            server_type: systemprompt_models::mcp::McpServerType::Internal,
            binary: "test-bin".to_owned(),
            package: None,
            port: 5001,
            endpoint: None,
            enabled: true,
            display_in_web: false,
            dev_only: false,
            schemas: vec![],
            oauth: systemprompt_models::mcp::OAuthRequirement {
                required: false,
                scopes: vec![],
                audience: systemprompt_models::auth::JwtAudience::Mcp,
                client_id: None,
            },
            tools: std::collections::HashMap::new(),
            model_config: None,
            env_vars: vec![],
            external_auth: None,
            headers: Default::default(),
        },
    );
    config
}

#[test]
fn validate_agent_name_accepts_lowercase_alphanumeric_underscores() {
    AgentConfigAuthoringService::validate_agent_name("agent_01").expect("valid name");
}

#[test]
fn validate_agent_name_rejects_short_and_long_names() {
    let short = AgentConfigAuthoringService::validate_agent_name("ab").expect_err("too short");
    assert_eq!(
        short.to_string(),
        "Agent name must be between 3 and 50 characters"
    );

    let long_name = "a".repeat(51);
    AgentConfigAuthoringService::validate_agent_name(&long_name).expect_err("too long");
}

#[test]
fn validate_agent_name_rejects_invalid_characters() {
    let err =
        AgentConfigAuthoringService::validate_agent_name("Agent-One").expect_err("invalid charset");
    assert_eq!(
        err.to_string(),
        "Agent name must be lowercase alphanumeric with underscores only"
    );
}

#[test]
fn validate_port_rejects_zero_and_privileged() {
    let zero = AgentConfigAuthoringService::validate_port(0).expect_err("zero port");
    assert_eq!(zero.to_string(), "Port cannot be 0");

    let privileged = AgentConfigAuthoringService::validate_port(80).expect_err("privileged port");
    assert_eq!(
        privileged.to_string(),
        "Port must be >= 1024 (non-privileged)"
    );

    AgentConfigAuthoringService::validate_port(1024).expect("non-privileged port");
}

#[test]
fn resolve_system_prompt_prefers_file_over_inline() {
    let dir = tempfile::tempdir().expect("tempdir");
    let prompt_path = dir.path().join("prompt.txt");
    fs::write(&prompt_path, "File prompt").expect("write prompt");

    let resolved = AgentConfigAuthoringService::resolve_system_prompt(
        Some(prompt_path.to_str().expect("utf8 path")),
        Some("Inline prompt".to_owned()),
        "Agent",
        "",
    )
    .expect("resolve from file");

    assert_eq!(resolved, "File prompt");
}

#[test]
fn resolve_system_prompt_uses_inline_then_default() {
    let inline = AgentConfigAuthoringService::resolve_system_prompt(
        None,
        Some("Inline prompt".to_owned()),
        "Agent",
        "desc",
    )
    .expect("inline prompt");
    assert_eq!(inline, "Inline prompt");

    let with_description =
        AgentConfigAuthoringService::resolve_system_prompt(None, None, "Agent", "Helps out.")
            .expect("default prompt");
    assert_eq!(with_description, "You are Agent. Helps out.");

    let without_description =
        AgentConfigAuthoringService::resolve_system_prompt(None, None, "Agent", "")
            .expect("default prompt");
    assert_eq!(without_description, "You are Agent.");
}

#[test]
fn resolve_system_prompt_missing_file_errors() {
    let err = AgentConfigAuthoringService::resolve_system_prompt(
        Some("/nonexistent/prompt.txt"),
        None,
        "Agent",
        "",
    )
    .expect_err("missing file");

    assert_eq!(
        err.to_string(),
        "Failed to read system prompt file: /nonexistent/prompt.txt"
    );
}

#[test]
fn create_writes_agent_yaml_with_defaults() {
    let dir = tempfile::tempdir().expect("tempdir");
    let agent = create_and_load(dir.path(), "demo_agent", 8101);

    assert_eq!(agent.name, "demo_agent");
    assert_eq!(agent.port, 8101);
    assert!(agent.enabled);
    assert_eq!(
        agent.endpoint,
        ApiPaths::agent_endpoint(&AgentId::new("demo_agent"))
    );
    assert_eq!(
        agent.card.protocol_version,
        systemprompt_agent::A2A_PROTOCOL_VERSION
    );
    assert_eq!(agent.card.display_name, "Test Agent");
    assert_eq!(agent.card.version, "1.0.0");
    assert!(agent.card.capabilities.streaming);
    assert!(!agent.card.capabilities.push_notifications);
    assert_eq!(agent.metadata.provider.as_deref(), Some("anthropic"));
    assert!(agent.metadata.model.is_some());
    assert_eq!(
        agent.metadata.system_prompt.as_deref(),
        Some("You are Test Agent.")
    );
}

#[test]
fn create_respects_explicit_overrides() {
    let dir = tempfile::tempdir().expect("tempdir");
    let service = AgentConfigAuthoringService::new(dir.path());

    let mut request = create_request("custom_agent", 8102);
    request.endpoint = Some("/custom/endpoint".to_owned());
    request.provider = Some("openai".to_owned());
    request.model = Some("gpt-test".to_owned());
    request.streaming = Some(false);
    request.mcp_servers = vec!["tools".to_owned()];

    let path = service.create(request).expect("create agent");
    let agent = load_agent(&path, "custom_agent");

    assert_eq!(agent.endpoint, "/custom/endpoint");
    assert_eq!(agent.metadata.provider.as_deref(), Some("openai"));
    assert_eq!(agent.metadata.model.as_deref(), Some("gpt-test"));
    assert!(!agent.card.capabilities.streaming);
    assert_eq!(agent.metadata.mcp_servers.include, vec!["tools".to_owned()]);
}

#[test]
fn create_rejects_invalid_name_and_port() {
    let dir = tempfile::tempdir().expect("tempdir");
    let service = AgentConfigAuthoringService::new(dir.path());

    let bad_name = service
        .create(create_request("ab", 8103))
        .expect_err("name too short");
    assert!(matches!(bad_name, ConfigAuthoringError::NameLength));

    let bad_port = service
        .create(create_request("demo_agent", 0))
        .expect_err("port zero");
    assert!(matches!(bad_port, ConfigAuthoringError::PortZero));
}

#[test]
fn create_rejects_duplicate_agent_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let service = AgentConfigAuthoringService::new(dir.path());

    service
        .create(create_request("dup_agent", 8104))
        .expect("first create");
    let err = service
        .create(create_request("dup_agent", 8104))
        .expect_err("duplicate create");

    assert!(err.to_string().contains("already exists"));
}

#[test]
fn delete_removes_agent_file_and_include_entry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_dir = dir.path().join("config");
    fs::create_dir_all(&config_dir).expect("config dir");
    fs::write(
        config_dir.join("config.yaml"),
        "includes:\n  - ../agents/gone_agent.yaml\n",
    )
    .expect("write config.yaml");

    let service = AgentConfigAuthoringService::new(dir.path());
    let path = service
        .create(create_request("gone_agent", 8105))
        .expect("create agent");
    assert!(path.exists());

    service.delete("gone_agent").expect("delete agent");

    assert!(!path.exists());
    let config_text = fs::read_to_string(config_dir.join("config.yaml")).expect("read config.yaml");
    assert!(!config_text.contains("gone_agent"));
}

#[test]
fn delete_missing_agent_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let service = AgentConfigAuthoringService::new(dir.path());

    let err = service.delete("ghost_agent").expect_err("missing agent");
    assert_eq!(
        err.to_string(),
        "Agent 'ghost_agent' not found in any configuration file"
    );
}

#[test]
fn apply_enabled_flags_records_changes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut agent = create_and_load(dir.path(), "edit_agent", 8106);

    let mut changes = Vec::new();
    let request = AgentEditRequest {
        disable: true,
        ..Default::default()
    };
    AgentConfigAuthoringService::apply_enabled_flags(&mut agent, &request, &mut changes);

    assert!(!agent.enabled);
    assert_eq!(changes, vec!["enabled: false".to_owned()]);
}

#[test]
fn apply_runtime_fields_validates_port() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut agent = create_and_load(dir.path(), "edit_agent", 8106);

    let mut changes = Vec::new();
    let request = AgentEditRequest {
        port: Some(9000),
        endpoint: Some("/new/endpoint".to_owned()),
        ..Default::default()
    };
    AgentConfigAuthoringService::apply_runtime_fields(&mut agent, &request, &mut changes)
        .expect("apply runtime fields");

    assert_eq!(agent.port, 9000);
    assert_eq!(agent.endpoint, "/new/endpoint");
    assert_eq!(
        changes,
        vec![
            "port: 9000".to_owned(),
            "endpoint: /new/endpoint".to_owned()
        ]
    );

    let bad = AgentEditRequest {
        port: Some(0),
        ..Default::default()
    };
    let err = AgentConfigAuthoringService::apply_runtime_fields(&mut agent, &bad, &mut Vec::new())
        .expect_err("port zero");
    assert_eq!(err.to_string(), "Port cannot be 0");
}

#[test]
fn apply_card_and_capability_fields_record_changes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut agent = create_and_load(dir.path(), "edit_agent", 8106);

    let mut changes = Vec::new();
    let request = AgentEditRequest {
        display_name: Some("Renamed".to_owned()),
        streaming: Some(false),
        ..Default::default()
    };
    AgentConfigAuthoringService::apply_card_fields(&mut agent, &request, &mut changes);
    AgentConfigAuthoringService::apply_capability_fields(&mut agent, &request, &mut changes);

    assert_eq!(agent.card.display_name, "Renamed");
    assert!(!agent.card.capabilities.streaming);
    assert_eq!(
        changes,
        vec![
            "card.display_name: Renamed".to_owned(),
            "card.capabilities.streaming: false".to_owned(),
        ]
    );
}

#[test]
fn apply_metadata_fields_sets_prompt_from_inline_and_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut agent = create_and_load(dir.path(), "edit_agent", 8106);

    let mut changes = Vec::new();
    let inline = AgentEditRequest {
        provider: Some("gemini".to_owned()),
        system_prompt: Some("New prompt".to_owned()),
        ..Default::default()
    };
    AgentConfigAuthoringService::apply_metadata_fields(&mut agent, &inline, &mut changes)
        .expect("inline prompt");
    assert_eq!(agent.metadata.system_prompt.as_deref(), Some("New prompt"));
    assert_eq!(
        changes,
        vec![
            "metadata.provider: gemini".to_owned(),
            "system_prompt: 10 chars".to_owned(),
        ]
    );

    let prompt_path = dir.path().join("prompt.txt");
    fs::write(&prompt_path, "From file").expect("write prompt");
    let from_file = AgentEditRequest {
        system_prompt_file: Some(prompt_path.to_str().expect("utf8 path").to_owned()),
        ..Default::default()
    };
    AgentConfigAuthoringService::apply_metadata_fields(&mut agent, &from_file, &mut Vec::new())
        .expect("file prompt");
    assert_eq!(agent.metadata.system_prompt.as_deref(), Some("From file"));
}

#[test]
fn apply_mcp_server_changes_validates_and_skips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut agent = create_and_load(dir.path(), "edit_agent", 8106);
    let services_config = services_config_with_server("tools");

    let mut changes = Vec::new();
    let add = AgentEditRequest {
        mcp_servers: vec!["tools".to_owned()],
        ..Default::default()
    };
    let skipped = AgentConfigAuthoringService::apply_mcp_server_changes(
        &mut agent,
        &add,
        &services_config,
        &mut changes,
    )
    .expect("add known server");
    assert!(skipped.is_empty());
    assert_eq!(agent.metadata.mcp_servers.include, vec!["tools".to_owned()]);
    assert_eq!(changes, vec!["added mcp_server: tools".to_owned()]);

    let unknown = AgentEditRequest {
        mcp_servers: vec!["missing".to_owned()],
        ..Default::default()
    };
    let err = AgentConfigAuthoringService::apply_mcp_server_changes(
        &mut agent,
        &unknown,
        &services_config,
        &mut Vec::new(),
    )
    .expect_err("unknown server");
    assert_eq!(
        err.to_string(),
        "MCP server 'missing' not found in configuration. Available servers: tools"
    );

    let remove = AgentEditRequest {
        remove_mcp_servers: vec!["tools".to_owned(), "absent".to_owned()],
        ..Default::default()
    };
    let mut removal_changes = Vec::new();
    let skipped = AgentConfigAuthoringService::apply_mcp_server_changes(
        &mut agent,
        &remove,
        &services_config,
        &mut removal_changes,
    )
    .expect("remove servers");
    assert!(agent.metadata.mcp_servers.include.is_empty());
    assert_eq!(
        removal_changes,
        vec!["removed mcp_server: tools".to_owned()]
    );
    assert_eq!(skipped, vec!["absent".to_owned()]);
}

#[test]
fn apply_skill_changes_adds_removes_and_skips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut agent = create_and_load(dir.path(), "edit_agent", 8106);

    let mut changes = Vec::new();
    let add = AgentEditRequest {
        skills: vec!["search".to_owned(), "search".to_owned()],
        ..Default::default()
    };
    let skipped = AgentConfigAuthoringService::apply_skill_changes(&mut agent, &add, &mut changes);
    assert!(skipped.is_empty());
    assert_eq!(agent.metadata.skills.include, vec!["search".to_owned()]);
    assert_eq!(changes, vec!["added skill: search".to_owned()]);

    let remove = AgentEditRequest {
        remove_skills: vec!["search".to_owned(), "absent".to_owned()],
        ..Default::default()
    };
    let mut removal_changes = Vec::new();
    let skipped =
        AgentConfigAuthoringService::apply_skill_changes(&mut agent, &remove, &mut removal_changes);
    assert!(agent.metadata.skills.include.is_empty());
    assert_eq!(removal_changes, vec!["removed skill: search".to_owned()]);
    assert_eq!(skipped, vec!["absent".to_owned()]);
}

#[test]
fn apply_set_value_changes_handles_supported_and_invalid_keys() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut agent = create_and_load(dir.path(), "edit_agent", 8106);

    let mut changes = Vec::new();
    let request = AgentEditRequest {
        set_values: vec![
            "card.displayName=Set Name".to_owned(),
            "is_primary=true".to_owned(),
        ],
        ..Default::default()
    };
    AgentConfigAuthoringService::apply_set_value_changes(&mut agent, &request, &mut changes)
        .expect("set values");
    assert_eq!(agent.card.display_name, "Set Name");
    assert!(agent.is_primary);
    assert_eq!(
        changes,
        vec![
            "card.displayName: Set Name".to_owned(),
            "is_primary: true".to_owned(),
        ]
    );

    let no_equals = AgentEditRequest {
        set_values: vec!["noequals".to_owned()],
        ..Default::default()
    };
    let err = AgentConfigAuthoringService::apply_set_value_changes(
        &mut agent,
        &no_equals,
        &mut Vec::new(),
    )
    .expect_err("missing equals");
    assert_eq!(
        err.to_string(),
        "Invalid --set format: 'noequals'. Expected key=value"
    );

    let unknown_key = AgentEditRequest {
        set_values: vec!["bogus=1".to_owned()],
        ..Default::default()
    };
    let err = AgentConfigAuthoringService::apply_set_value_changes(
        &mut agent,
        &unknown_key,
        &mut Vec::new(),
    )
    .expect_err("unknown key");
    assert!(
        err.to_string()
            .starts_with("Unknown configuration key: 'bogus'.")
    );

    let bad_bool = AgentEditRequest {
        set_values: vec!["dev_only=banana".to_owned()],
        ..Default::default()
    };
    let err = AgentConfigAuthoringService::apply_set_value_changes(
        &mut agent,
        &bad_bool,
        &mut Vec::new(),
    )
    .expect_err("bad bool");
    assert_eq!(
        err.to_string(),
        "Invalid boolean value for dev_only: 'banana'"
    );
}

#[test]
fn apply_set_value_changes_covers_description_version_endpoint_and_default() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut agent = create_and_load(dir.path(), "edit_agent", 8106);

    let request = AgentEditRequest {
        set_values: vec![
            "card.description=A described agent".to_owned(),
            "card.version=2.1.0".to_owned(),
            "endpoint=/set/endpoint".to_owned(),
            "default=true".to_owned(),
        ],
        ..Default::default()
    };
    AgentConfigAuthoringService::apply_set_value_changes(&mut agent, &request, &mut Vec::new())
        .expect("set values");

    assert_eq!(agent.card.description, "A described agent");
    assert_eq!(agent.card.version, "2.1.0");
    assert_eq!(agent.endpoint, "/set/endpoint");
    assert!(agent.default);
}
