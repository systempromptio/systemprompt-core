//! Unit tests for ConfigLoader

use std::path::PathBuf;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::services::ServicesConfig;
use tempfile::TempDir;

fn create_minimal_config() -> String {
    r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#
    .to_string()
}

#[test]
fn test_load_from_path_minimal_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    let config = ConfigLoader::load_from_path(&config_path).expect("Should load config");
    assert!(config.agents.is_empty());
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn test_load_from_path_nonexistent() {
    let path = PathBuf::from("/nonexistent/services.yaml");
    let result = ConfigLoader::load_from_path(&path);
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Failed to read"));
}

#[test]
fn test_load_from_path_invalid_yaml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, "invalid: yaml: : :").expect("Failed to write config");

    ConfigLoader::load_from_path(&config_path).unwrap_err();
}

#[test]
fn test_load_from_content_minimal() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    ConfigLoader::load_from_content(&create_minimal_config(), &config_path)
        .expect("result should succeed");
}

#[test]
fn test_load_from_content_with_includes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let include_content = r#"
agents: {}
mcp_servers: {}
"#;
    std::fs::write(temp_dir.path().join("agents.yaml"), include_content)
        .expect("Failed to write include");

    let main_content = r#"
includes:
  - agents.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    ConfigLoader::load_from_content(main_content, &config_path).expect("result should succeed");
}

#[test]
fn test_validate_file_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    ConfigLoader::validate_file(&config_path).expect("result should succeed");
}

#[test]
fn test_loader_new_base_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let subdir = temp_dir.path().join("config");
    std::fs::create_dir(&subdir).expect("Failed to create subdir");
    let config_path = subdir.join("services.yaml");

    let loader = ConfigLoader::new(config_path);
    assert_eq!(loader.base_path(), subdir);
}

#[test]
fn test_loader_get_includes_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(&config_path, create_minimal_config()).expect("Failed to write config");

    let loader = ConfigLoader::new(config_path);
    let includes = loader.get_includes().expect("Should get includes");
    assert!(includes.is_empty());
}

#[test]
fn test_loader_get_includes_with_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let content = r#"
includes:
  - agents.yaml
  - mcp-servers.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    std::fs::write(&config_path, content).expect("Failed to write config");

    let loader = ConfigLoader::new(config_path);
    let includes = loader.get_includes().expect("Should get includes");
    assert_eq!(includes.len(), 2);
    assert!(includes.contains(&"agents.yaml".to_string()));
    assert!(includes.contains(&"mcp-servers.yaml".to_string()));
}

#[test]
fn test_loader_list_all_includes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    std::fs::write(
        temp_dir.path().join("existing.yaml"),
        "agents: {}\nmcp_servers: {}",
    )
    .expect("Failed to write include");

    let content = r#"
includes:
  - existing.yaml
  - missing.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    std::fs::write(&config_path, content).expect("Failed to write config");

    let loader = ConfigLoader::new(config_path);
    let includes = loader.list_all_includes().expect("Should list includes");
    assert_eq!(includes.len(), 2);

    let existing = includes
        .iter()
        .find(|(name, _)| name == "existing.yaml")
        .expect("existing should be present");
    assert!(existing.1);

    let missing = includes
        .iter()
        .find(|(name, _)| name == "missing.yaml")
        .expect("missing should be present");
    assert!(!missing.1);
}

#[test]
fn test_merge_multiple_includes_empty() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let empty_partial = "agents: {}\nmcp_servers: {}\n";
    std::fs::write(temp_dir.path().join("agents1.yaml"), empty_partial)
        .expect("Failed to write agents1");
    std::fs::write(temp_dir.path().join("agents2.yaml"), empty_partial)
        .expect("Failed to write agents2");

    let main_content = r#"
includes:
  - agents1.yaml
  - agents2.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    std::fs::write(&config_path, main_content).expect("Failed to write config");

    let config = ConfigLoader::load_from_path(&config_path).expect("Should load merged config");
    assert!(config.agents.is_empty());
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn test_recursive_three_level_includes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let skill_foo = r#"
agents: {}
mcp_servers: {}
"#;
    std::fs::write(temp_dir.path().join("skill-foo.yaml"), skill_foo)
        .expect("Failed to write skill-foo");

    let skills = r#"
includes:
  - skill-foo.yaml
agents: {}
mcp_servers: {}
"#;
    std::fs::write(temp_dir.path().join("skills.yaml"), skills).expect("Failed to write skills");

    let main_content = r#"
includes:
  - skills.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;
    std::fs::write(&config_path, main_content).expect("Failed to write config");

    ConfigLoader::load_from_path(&config_path)
        .expect("Three-level nested includes should load successfully");
}

#[test]
fn test_include_cycle_detected() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let a_yaml = r#"
includes:
  - b.yaml
agents: {}
mcp_servers: {}
"#;
    let b_yaml = r#"
includes:
  - a.yaml
agents: {}
mcp_servers: {}
"#;
    std::fs::write(temp_dir.path().join("a.yaml"), a_yaml).expect("Failed to write a.yaml");
    std::fs::write(temp_dir.path().join("b.yaml"), b_yaml).expect("Failed to write b.yaml");

    let main_content = r#"
includes:
  - a.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;
    std::fs::write(&config_path, main_content).expect("Failed to write config");

    let err = ConfigLoader::load_from_path(&config_path).expect_err("cycle should error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("cycle detected"),
        "expected cycle detected error, got: {msg}"
    );
}

const FULL_WEB_BLOCK: &str = r#"web:
  branding:
    name: fixture-brand
    title: "Fixture Brand"
    description: Fixture brand for tests
    copyright: 2026 fixture
    themeColor: '#ff0000'
    display_sitename: true
    twitter_handle: '@fixture'
    logo:
      primary:
        svg: /images/logo.svg
    favicon: /images/favicon.ico
  fonts:
    body:
      family: OpenSans
      fallback: sans-serif
      files: []
    heading:
      family: Inter
      fallback: sans-serif
      files: []
  colors:
    light:
      primary:
        hsl: hsl(0, 100%, 50%)
        rgb: [255, 0, 0]
      secondary:
        hsl: hsl(0, 100%, 50%)
        rgb: [255, 0, 0]
      success: '#10b981'
      warning: '#f59e0b'
      error: '#ef4444'
      surface:
        default: '#FFFFFF'
        dark: '#FAFAF9'
        variant: '#F5F5F4'
        secondaryContainer: '#FFF7ED'
        errorContainer: '#FEE2E2'
      text:
        primary: '#0F172A'
        secondary: '#475569'
        inverted: '#FFFFFF'
        disabled: '#A8A29E'
      background:
        default: '#FAFAF9'
        dark: '#F5F5F4'
      border:
        default: '#D6D3D1'
        dark: '#A8A29E'
        outline: '#A8A29E'
    dark:
      primary:
        hsl: hsl(0, 100%, 50%)
        rgb: [255, 0, 0]
      secondary:
        hsl: hsl(0, 100%, 50%)
        rgb: [255, 0, 0]
      success: '#34d399'
      warning: '#fbbf24'
      error: '#f87171'
      surface:
        default: '#1C1917'
        dark: '#0C0A09'
        variant: '#292524'
        secondaryContainer: '#000000'
        errorContainer: '#000000'
      text:
        primary: '#FAFAF9'
        secondary: '#D6D3D1'
        inverted: '#000000'
        disabled: '#78716C'
      background:
        default: '#1C1917'
        dark: '#0C0A09'
      border:
        default: '#000000'
        dark: '#44403C'
        outline: '#78716C'
  typography:
    sizes:
      xs: 12px
      sm: 14px
      md: 16px
      lg: 18px
      xl: 20px
      xxl: 30px
    weights:
      regular: 400
      medium: 500
      semibold: 600
      bold: 700
  spacing:
    xs: 4px
    sm: 8px
    md: 16px
    lg: 24px
    xl: 32px
    xxl: 48px
  radius:
    xs: 2px
    sm: 4px
    md: 8px
    lg: 12px
    xl: 16px
    xxl: 24px
    round: 9999px
  shadows:
    light:
      sm: s
      md: m
      lg: l
      accent: a
    dark:
      sm: s
      md: m
      lg: l
      accent: a
  animation:
    fast: 150ms
    normal: 250ms
    slow: 400ms
  zIndex:
    base: 1
    content: 10
    navigation: 100
    modal: 1000
    tooltip: 2000
  layout:
    headerHeight: 72px
    sidebarLeft:
      width: 20%
      minWidth: 240px
      maxWidth: 320px
    sidebarRight:
      width: 15%
      minWidth: 200px
      maxWidth: 280px
    navHeight: 72px
    contentMaxWidth: 1200px
  card:
    radius:
      default: 16px
      cut: 4px
    padding:
      sm: 12px
      md: 16px
      lg: 24px
    gradient:
      start: s
      mid: m
      end: e
  mobile:
    spacing:
      xs: 4px
      sm: 8px
      md: 12px
      lg: 16px
      xl: 24px
      xxl: 32px
    typography:
      sizes:
        xs: 11px
        sm: 13px
        md: 15px
        lg: 17px
        xl: 19px
        xxl: 26px
    layout:
      headerHeight: 64px
      navHeight: 64px
    card:
      padding:
        sm: 10px
        md: 12px
        lg: 16px
  touchTargets:
    default: 44px
    sm: 40px
    lg: 48px
  paths:
    templates: services/web/templates
    assets: services/web/assets
"#;

fn full_services_fixture() -> String {
    let mut yaml = String::new();
    yaml.push_str(
        r#"
agents:
  alpha_agent:
    name: alpha_agent
    port: 9100
    endpoint: http://localhost:8080/api/v1/agents/alpha_agent
    enabled: true
    dev_only: false
    is_primary: true
    default: true
    card:
      protocolVersion: "0.3.0"
      displayName: Alpha Agent
      description: Fixture alpha agent
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: false
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      skills:
        - id: general_assistance
          name: General Assistance
          description: General help
          tags: [help]
      supportsAuthenticatedExtendedCard: false
    metadata:
      mcpServers: [fixture_mcp]
      skills: [general_assistance]
      toolModelOverrides: {}
    oauth:
      required: false
      scopes: []
      audience: a2a
mcp_servers:
  fixture_mcp:
    type: internal
    binary: fixture-mcp-binary
    package: fixture
    port: 5100
    endpoint: http://localhost:8080/api/v1/mcp/fixture/mcp
    enabled: true
    display_in_web: true
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
settings:
  agent_port_range: [9000, 9999]
  mcp_port_range: [5000, 5999]
  auto_start_enabled: true
  validation_strict: true
  schema_validation_mode: warn
scheduler:
  enabled: true
  jobs:
    - name: cleanup_anonymous_users
      extension: core
      schedule: "0 0 3 * * *"
      enabled: true
  bootstrap_jobs:
    - database_cleanup
ai:
  default_provider: anthropic
  sampling:
    enable_smart_routing: false
    fallback_enabled: true
  providers:
    anthropic:
      enabled: true
      api_key: test-key
      default_model: claude-sonnet-4-20250514
  mcp:
    auto_discover: true
    connect_timeout_ms: 5000
    execution_timeout_ms: 30000
    retry_attempts: 3
  history:
    retention_days: 30
    log_tool_executions: true
plugins:
  fixture-plugin:
    id: fixture-plugin
    name: fixture-plugin
    description: Fixture plugin
    version: 1.0.0
    enabled: true
    author:
      name: fixture
      email: fixture@example.com
    keywords: [fixture]
    license: proprietary
    category: platform
    skills:
      source: explicit
      include:
        - general_assistance
    agents:
      source: explicit
      include:
        - alpha_agent
    mcp_servers:
      - fixture_mcp
    content_sources: []
    scripts: []
skills:
  enabled: true
  auto_discover: false
  skills:
    general_assistance:
      id: general_assistance
      name: General Assistance
      description: General help skill
      enabled: true
      tags: [help, general]
      instructions: "You are a helpful assistant."
      assigned_agents: [alpha_agent]
      mcp_servers: [fixture_mcp]
content:
  content_sources:
    blog:
      path: content/blog
      source_id: blog
      category_id: blog
      enabled: true
      description: Fixture blog
      allowed_content_types: [blog]
      branding:
        name: Blog
        description: Fixture blog
        image: /images/blog.png
        keywords: blog
      indexing:
        clear_before: false
        recursive: true
      sitemap:
        enabled: true
        url_pattern: /blog/{slug}
        priority: 0.8
        changefreq: weekly
        fetch_from: database
"#,
    );
    yaml.push_str(FULL_WEB_BLOCK);
    yaml
}

#[test]
fn test_load_full_fixture_round_trip() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");
    let yaml = full_services_fixture();
    std::fs::write(&config_path, &yaml).expect("Failed to write config");

    let config = ConfigLoader::load_from_path(&config_path).expect("full fixture should load");

    assert_eq!(config.agents.len(), 1);
    assert!(config.agents.contains_key("alpha_agent"));
    assert_eq!(config.mcp_servers.len(), 1);
    assert!(config.mcp_servers.contains_key("fixture_mcp"));
    assert_eq!(config.plugins.len(), 1);
    assert!(config.plugins.contains_key("fixture-plugin"));
    assert_eq!(config.skills.skills.len(), 1);
    assert!(config.skills.skills.contains_key("general_assistance"));
    assert!(config.web.is_some());
    assert!(config.scheduler.is_some());
    assert_eq!(config.ai.default_provider, "anthropic");
    assert!(!config.ai.providers.is_empty());
    assert!(!config.content.raw.content_sources.is_empty());
    assert_eq!(config.settings.agent_port_range, (9000, 9999));
}

#[test]
fn test_deny_unknown_top_level_field() {
    let content = r#"
agents: {}
mcp_servers: {}
skills_typo:
  enabled: true
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err =
        serde_yaml::from_str::<ServicesConfig>(content).expect_err("unknown field should error");
    let msg = err.to_string();
    assert!(
        msg.contains("unknown field") && msg.contains("skills_typo"),
        "expected unknown field error mentioning skills_typo, got: {msg}"
    );
}

#[test]
fn test_loader_denies_unknown_top_level_field() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let content = r#"
agents: {}
mcp_servers: {}
skills_typo:
  enabled: true
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err = ConfigLoader::load_from_content(content, &config_path)
        .expect_err("unknown top-level field should fail through loader path");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("unknown field") && msg.contains("skills_typo"),
        "expected unknown field error mentioning skills_typo, got: {msg}"
    );
}

#[test]
fn test_loader_denies_unknown_field_in_include() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let include_content = r#"
agents: {}
mcp_servers: {}
mcp_servers_typo: {}
"#;
    std::fs::write(temp_dir.path().join("extra.yaml"), include_content)
        .expect("Failed to write include");

    let main_content = r#"
includes:
  - extra.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err = ConfigLoader::load_from_content(main_content, &config_path)
        .expect_err("unknown field in include should fail through loader path");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("unknown field") && msg.contains("mcp_servers_typo"),
        "expected unknown field error mentioning mcp_servers_typo, got: {msg}"
    );
}

#[test]
fn test_plugin_binding_references_unknown_agent() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("services.yaml");

    let content = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [9000, 9999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
skills:
  enabled: true
  auto_discover: false
  skills:
    sample_skill:
      id: sample_skill
      name: Sample Skill
      description: Sample
      enabled: true
      tags: []
      instructions: "hello"
      assigned_agents: []
      mcp_servers: []
plugins:
  broken-plugin:
    id: broken-plugin
    name: broken-plugin
    description: Plugin with dangling agent reference
    version: 1.0.0
    enabled: true
    author:
      name: fixture
      email: fixture@example.com
    keywords: []
    license: proprietary
    category: platform
    skills:
      source: explicit
      include:
        - sample_skill
    agents:
      source: explicit
      include:
        - ghost_agent
    mcp_servers: []
    content_sources: []
    scripts: []
"#;
    std::fs::write(&config_path, content).expect("Failed to write config");

    let err = ConfigLoader::load_from_path(&config_path)
        .expect_err("unknown agent reference should fail validation");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("Plugin 'broken-plugin'")
            && msg.contains("agents.include references unknown agent")
            && msg.contains("ghost_agent"),
        "expected plugin binding validation error, got: {msg}"
    );
}

#[test]
fn test_external_template_smoke() {
    let path = PathBuf::from("/var/www/html/systemprompt-template/services/config/config.yaml");
    if !path.exists() {
        return;
    }

    let _config = ConfigLoader::load_from_path(&path).expect("external template should load");
}
