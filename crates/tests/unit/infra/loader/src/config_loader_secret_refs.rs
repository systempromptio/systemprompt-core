//! The services loader refuses a Slack or Teams app whose secret ref names a
//! built-in platform secret: `ConfigLoader` runs `ServicesConfig::validate`
//! on every interpolated load, so the typed `BuiltInSecretRef` surfaces on
//! the path CLI validation and child hosts take, not only through the entry
//! crates' extension config check.

use systemprompt_loader::{ConfigLoadError, ConfigLoader};
use systemprompt_models::errors::ServicesValidationError;
use tempfile::TempDir;

const HEAD: &str = r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#;

fn load(apps: &str) -> Result<systemprompt_manifest::services::ServicesConfig, ConfigLoadError> {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("services.yaml");
    ConfigLoader::load_from_content(&format!("{HEAD}{apps}"), &config_path, None)
}

fn refused(err: &ConfigLoadError) -> Option<(&'static str, &str)> {
    match err {
        ConfigLoadError::Validation(ServicesValidationError::BuiltInSecretRef {
            field,
            secret,
            ..
        }) => Some((field, secret.as_str())),
        _ => None,
    }
}

#[test]
fn a_slack_signing_secret_ref_naming_a_built_in_secret_fails_the_load() {
    let err = load(
        r#"
slack_apps:
  ws_main:
    workspace_id: T12345
    signing_secret_ref: signing_key_pem
    bot_token_ref: slack_bot
    default_agent: helper
"#,
    )
    .expect_err("a built-in secret ref is refused by services validation");
    assert_eq!(
        refused(&err),
        Some(("signing_secret_ref", "signing_key_pem")),
        "got {err:?}"
    );
}

#[test]
fn a_slack_bot_token_ref_naming_a_built_in_alias_fails_the_load() {
    let err = load(
        r#"
slack_apps:
  ws_main:
    workspace_id: T12345
    signing_secret_ref: slack_signing
    bot_token_ref: GITHUB_TOKEN
    default_agent: helper
"#,
    )
    .expect_err("a built-in secret alias is refused by services validation");
    assert_eq!(
        refused(&err),
        Some(("bot_token_ref", "GITHUB_TOKEN")),
        "got {err:?}"
    );
}

#[test]
fn a_teams_app_password_ref_naming_a_built_in_secret_fails_the_load() {
    let err = load(
        r#"
teams_apps:
  tenant_main:
    tenant_id: tid-0001
    app_id: app-0001
    app_password_ref: oauth_at_rest_pepper
    default_agent: helper
"#,
    )
    .expect_err("a built-in secret ref is refused by services validation");
    assert_eq!(
        refused(&err),
        Some(("app_password_ref", "oauth_at_rest_pepper")),
        "got {err:?}"
    );
}

#[test]
fn custom_secret_refs_load() {
    let config = load(
        r#"
slack_apps:
  ws_main:
    workspace_id: T12345
    signing_secret_ref: slack_signing
    bot_token_ref: slack_bot
    default_agent: helper
teams_apps:
  tenant_main:
    tenant_id: tid-0001
    app_id: app-0001
    app_password_ref: teams_pw
    default_agent: helper
"#,
    )
    .expect("custom secret refs pass services validation");
    assert!(config.slack_apps.contains_key("ws_main"));
    assert!(config.teams_apps.contains_key("tenant_main"));
}
