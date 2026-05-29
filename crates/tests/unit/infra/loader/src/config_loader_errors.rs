use systemprompt_loader::ConfigLoader;
use tempfile::TempDir;

#[test]
fn include_with_settings_block_is_rejected() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let include_with_settings = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#;
    std::fs::write(temp.path().join("bad_include.yaml"), include_with_settings)
        .expect("write include");

    let main = r#"
includes:
  - bad_include.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err = ConfigLoader::load_from_content(main, &config_path)
        .expect_err("settings in include must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("settings"),
        "expected settings-in-include error, got: {msg}"
    );
}

#[test]
fn duplicate_agent_in_include_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let include = r#"
agents:
  clash_agent:
    name: clash_agent
    port: 4001
    endpoint: http://localhost:4001/clash_agent
    enabled: true
    card:
      protocolVersion: "0.2.3"
      displayName: Clash Agent
      description: Duplicate agent
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: false
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      skills: []
      supportsAuthenticatedExtendedCard: false
    metadata: {}
    oauth: {required: false, scopes: [], audience: a2a}
mcp_servers: {}
"#;
    std::fs::write(temp.path().join("extra.yaml"), include).expect("write extra");

    let main = format!(
        r#"
includes:
  - extra.yaml
agents:
  clash_agent:
    name: clash_agent
    port: 4002
    endpoint: http://localhost:4002/clash_agent
    enabled: true
    card:
      protocolVersion: "0.2.3"
      displayName: Clash Agent 2
      description: Also duplicate
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: false
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      skills: []
      supportsAuthenticatedExtendedCard: false
    metadata: {{}}
    oauth: {{required: false, scopes: [], audience: a2a}}
mcp_servers: {{}}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#
    );

    let err = ConfigLoader::load_from_content(&main, &config_path)
        .expect_err("duplicate agent must error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("duplicate agent") || msg.contains("clash_agent"),
        "expected duplicate agent error, got: {msg}"
    );
}

#[test]
fn duplicate_mcp_server_in_include_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let include = r#"
agents: {}
mcp_servers:
  clash_mcp:
    type: internal
    binary: clash-bin
    package: clash
    port: 5001
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
"#;
    std::fs::write(temp.path().join("mcp_extra.yaml"), include).expect("write include");

    let main = r#"
includes:
  - mcp_extra.yaml
agents: {}
mcp_servers:
  clash_mcp:
    type: internal
    binary: clash-bin-2
    package: clash
    port: 5002
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err = ConfigLoader::load_from_content(main, &config_path)
        .expect_err("duplicate mcp server must error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("duplicate MCP server") || msg.contains("clash_mcp"),
        "expected duplicate MCP server error, got: {msg}"
    );
}

#[test]
fn include_not_found_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let main = r#"
includes:
  - does_not_exist.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err = ConfigLoader::load_from_content(main, &config_path)
        .expect_err("missing include must error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("not found") || msg.contains("include"),
        "expected include-not-found error, got: {msg}"
    );
}

#[test]
fn system_prompt_include_resolved() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let prompt_content = "You are a helpful assistant for testing.";
    std::fs::write(temp.path().join("prompt.txt"), prompt_content).expect("write prompt");

    let main = r#"
agents:
  prompt_agent:
    name: prompt_agent
    port: 4010
    endpoint: http://localhost:4010/prompt_agent
    enabled: true
    card:
      protocolVersion: "0.2.3"
      displayName: Prompt Agent
      description: Agent with !include system prompt
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: false
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      skills: []
      supportsAuthenticatedExtendedCard: false
    metadata:
      systemPrompt: "!include prompt.txt"
    oauth: {required: false, scopes: [], audience: a2a}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let config = ConfigLoader::load_from_content(main, &config_path)
        .expect("should load config with !include system prompt");

    let agent = config.agents.get("prompt_agent").expect("agent present");
    let sp = agent.metadata.system_prompt.as_deref().expect("system_prompt set");
    assert_eq!(sp, prompt_content);
}

#[test]
fn system_prompt_include_missing_file_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let main = r#"
agents:
  missing_prompt_agent:
    name: missing_prompt_agent
    port: 4011
    endpoint: http://localhost:4011/missing_prompt_agent
    enabled: true
    card:
      protocolVersion: "0.2.3"
      displayName: Missing Prompt
      description: Agent with missing !include
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: false
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      skills: []
      supportsAuthenticatedExtendedCard: false
    metadata:
      systemPrompt: "!include nonexistent_prompt.txt"
    oauth: {required: false, scopes: [], audience: a2a}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err = ConfigLoader::load_from_content(main, &config_path)
        .expect_err("missing !include file must error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("io error") || msg.contains("No such file"),
        "expected io error for missing !include, got: {msg}"
    );
}

#[test]
fn skill_instruction_include_resolved() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let instructions = "Follow these steps to assist the user.";
    std::fs::write(temp.path().join("instructions.md"), instructions).expect("write instructions");

    let main = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
skills:
  enabled: true
  auto_discover: false
  skills:
    my_skill:
      id: my_skill
      name: My Skill
      description: Skill with include instructions
      enabled: true
      tags: []
      instructions: "!include instructions.md"
      assigned_agents: {include: []}
      mcp_servers: {include: []}
"#;

    let config = ConfigLoader::load_from_content(main, &config_path)
        .expect("should load config with skill instruction include");

    let skill = config.skills.skills.get("my_skill").expect("skill present");
    let instr = skill.instructions.as_ref().expect("instructions set");
    let text = match instr {
        systemprompt_models::services::IncludableString::Inline(s) => s.as_str(),
        systemprompt_models::services::IncludableString::Include { path } => {
            panic!("expected Inline after resolution, got Include({path})")
        },
    };
    assert_eq!(text, instructions);
}

#[test]
fn skill_instruction_include_missing_file_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let main = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
skills:
  enabled: true
  auto_discover: false
  skills:
    broken_skill:
      id: broken_skill
      name: Broken Skill
      description: Skill with missing include
      enabled: true
      tags: []
      instructions: "!include no_such_instructions.md"
      assigned_agents: {include: []}
      mcp_servers: {include: []}
"#;

    let err = ConfigLoader::load_from_content(main, &config_path)
        .expect_err("missing skill instruction include must error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("io error") || msg.contains("No such file"),
        "expected io error for missing skill include, got: {msg}"
    );
}

#[test]
fn base_path_is_parent_of_config() {
    let temp = TempDir::new().expect("tempdir");
    let sub = temp.path().join("config");
    std::fs::create_dir_all(&sub).expect("create subdir");
    let config_path = sub.join("services.yaml");

    let loader = ConfigLoader::new(config_path);
    assert_eq!(loader.base_path(), sub.as_path());
}

#[test]
fn base_path_with_root_level_path() {
    let config_path = std::path::PathBuf::from("/services.yaml");
    let loader = ConfigLoader::new(config_path);
    let bp = loader.base_path();
    assert!(!bp.as_os_str().is_empty());
}

#[test]
fn get_includes_from_nonexistent_file_errors() {
    let loader = ConfigLoader::new(std::path::PathBuf::from("/nonexistent/services.yaml"));
    let err = loader.get_includes().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("io error"));
}

#[test]
fn list_all_includes_nonexistent_file_errors() {
    let loader = ConfigLoader::new(std::path::PathBuf::from("/nonexistent/services.yaml"));
    let err = loader.list_all_includes().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("io error"));
}

#[test]
fn validate_file_invalid_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("bad.yaml");
    std::fs::write(&config_path, "not: valid: yaml: : :").expect("write bad yaml");

    let err = ConfigLoader::validate_file(&config_path)
        .expect_err("invalid YAML must fail validation");
    let msg = format!("{err:#}");
    assert!(!msg.is_empty());
}

#[test]
fn include_with_yaml_error_propagates() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    std::fs::write(temp.path().join("broken.yaml"), "broken: yaml: : :")
        .expect("write broken include");

    let main = r#"
includes:
  - broken.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err = ConfigLoader::load_from_content(main, &config_path)
        .expect_err("broken include YAML must error");
    let msg = format!("{err:#}");
    assert!(!msg.is_empty());
}
