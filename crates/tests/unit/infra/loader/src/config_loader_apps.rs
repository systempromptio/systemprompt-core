//! Behavioural tests for the remaining `merge_into` branches (Slack/Teams
//! apps, `web` carry-over), the sanctioned env-var overrides, and the
//! bootstrap-dependent entry points.

use systemprompt_loader::{ConfigLoadError, ConfigLoader};
use tempfile::TempDir;

const ROOT_HEAD: &str = r#"
includes:
  - include.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#;

fn write_include(temp: &TempDir, body: &str) {
    std::fs::write(temp.path().join("include.yaml"), body).expect("write include");
}

fn slack_app_block(agent: &str) -> String {
    format!(
        r#"
slack_apps:
  ws_main:
    workspace_id: T12345
    signing_secret_ref: slack_signing
    bot_token_ref: slack_bot
    default_agent: {agent}
"#
    )
}

fn teams_app_block(agent: &str) -> String {
    format!(
        r#"
teams_apps:
  tenant_main:
    tenant_id: tid-0001
    app_id: app-0001
    app_password_ref: teams_pw
    default_agent: {agent}
"#
    )
}

#[test]
fn slack_app_merged_from_include() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        &format!(
            "agents: {{}}\nmcp_servers: {{}}\n{}",
            slack_app_block("helper")
        ),
    );

    let root = format!("{ROOT_HEAD}\nai:\n  default_provider: anthropic\n");
    let config = ConfigLoader::load_from_content(&root, &config_path).expect("should load");

    let app = config
        .slack_apps
        .get("ws_main")
        .expect("slack app from include must be merged");
    assert_eq!(app.workspace_id.as_str(), "T12345");
    assert_eq!(
        app.default_agent.as_ref().map(|a| a.as_str()),
        Some("helper")
    );
}

#[test]
fn duplicate_slack_app_across_root_and_include_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        &format!(
            "agents: {{}}\nmcp_servers: {{}}\n{}",
            slack_app_block("helper")
        ),
    );

    let root = format!(
        "{ROOT_HEAD}\nai:\n  default_provider: anthropic\n{}",
        slack_app_block("other")
    );
    let err = ConfigLoader::load_from_content(&root, &config_path)
        .expect_err("duplicate slack app must error");

    assert!(
        matches!(err, ConfigLoadError::DuplicateSlackApp(ref name) if name == "ws_main"),
        "expected DuplicateSlackApp(ws_main), got: {err}"
    );
}

#[test]
fn teams_app_merged_from_include() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        &format!(
            "agents: {{}}\nmcp_servers: {{}}\n{}",
            teams_app_block("helper")
        ),
    );

    let root = format!("{ROOT_HEAD}\nai:\n  default_provider: anthropic\n");
    let config = ConfigLoader::load_from_content(&root, &config_path).expect("should load");

    let app = config
        .teams_apps
        .get("tenant_main")
        .expect("teams app from include must be merged");
    assert_eq!(app.tenant_id.as_str(), "tid-0001");
    assert_eq!(app.app_id, "app-0001");
}

#[test]
fn duplicate_teams_app_across_root_and_include_errors() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        &format!(
            "agents: {{}}\nmcp_servers: {{}}\n{}",
            teams_app_block("helper")
        ),
    );

    let root = format!(
        "{ROOT_HEAD}\nai:\n  default_provider: anthropic\n{}",
        teams_app_block("other")
    );
    let err = ConfigLoader::load_from_content(&root, &config_path)
        .expect_err("duplicate teams app must error");

    assert!(
        matches!(err, ConfigLoadError::DuplicateTeamsApp(ref name) if name == "tenant_main"),
        "expected DuplicateTeamsApp(tenant_main), got: {err}"
    );
}

#[test]
fn web_block_carried_from_include_when_root_has_none() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    write_include(
        &temp,
        &format!(
            "agents: {{}}\nmcp_servers: {{}}\n{}",
            crate::services_loader::FULL_WEB_BLOCK
        ),
    );

    let root = format!("{ROOT_HEAD}\nai:\n  default_provider: anthropic\n");
    let config = ConfigLoader::load_from_content(&root, &config_path).expect("should load");

    let web = config.web.expect("web block from include must be carried");
    assert_eq!(web.branding.name, "fixture-brand");
}

#[test]
fn env_path_overrides_are_applied_to_settings() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let root = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    unsafe {
        std::env::set_var("SYSTEMPROMPT_SERVICES_PATH", "/env/services");
        std::env::set_var("SYSTEMPROMPT_SKILLS_PATH", "/env/skills");
        std::env::set_var("SYSTEMPROMPT_CONFIG_PATH", "/env/config");
    }
    let result = ConfigLoader::load_from_content(root, &config_path);
    unsafe {
        std::env::remove_var("SYSTEMPROMPT_SERVICES_PATH");
        std::env::remove_var("SYSTEMPROMPT_SKILLS_PATH");
        std::env::remove_var("SYSTEMPROMPT_CONFIG_PATH");
    }

    let config = result.expect("should load with env overrides");
    assert_eq!(
        config.settings.services_path.as_deref(),
        Some("/env/services")
    );
    assert_eq!(config.settings.skills_path.as_deref(), Some("/env/skills"));
    assert_eq!(config.settings.config_path.as_deref(), Some("/env/config"));
}

#[test]
fn system_prompt_include_inside_included_file_resolves_relative_to_include() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");

    let sub = temp.path().join("sub");
    std::fs::create_dir_all(&sub).expect("create sub dir");
    std::fs::write(sub.join("prompt.md"), "prompt from include dir").expect("write prompt");

    let include = r#"
agents:
  prompted_agent:
    name: prompted_agent
    port: 4020
    endpoint: http://localhost:4020/prompted_agent
    enabled: true
    card:
      protocolVersion: "0.2.3"
      displayName: Prompted Agent
      description: Agent whose prompt lives beside the include
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
      systemPrompt: "!include prompt.md"
    oauth: {required: false, scopes: [], audience: a2a}
mcp_servers: {}
"#;
    std::fs::write(sub.join("include.yaml"), include).expect("write include");

    let root = r#"
includes:
  - sub/include.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;
    let config = ConfigLoader::load_from_content(root, &config_path).expect("should load");

    let agent = config.agents.get("prompted_agent").expect("agent merged");
    assert_eq!(
        agent.metadata.system_prompt.as_deref(),
        Some("prompt from include dir"),
        "system_prompt !include must resolve relative to the include's own directory"
    );
}

#[test]
fn load_errors_when_profile_bootstrap_not_initialized() {
    let err = ConfigLoader::load().expect_err("no profile bootstrap in a unit-test process");
    assert!(
        matches!(err, ConfigLoadError::ProfileBootstrap(_)),
        "expected ProfileBootstrap error, got: {err}"
    );
}

#[test]
fn for_active_profile_errors_when_bootstrap_not_initialized() {
    let err = ConfigLoader::for_active_profile()
        .expect_err("no profile bootstrap in a unit-test process");
    assert!(err.to_string().contains("profile bootstrap unavailable"));
}
