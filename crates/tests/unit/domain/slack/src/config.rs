use systemprompt_models::services::ServicesConfig;
use systemprompt_slack::SlackAppConfig;

fn parse(yaml: &str) -> SlackAppConfig {
    serde_yaml::from_str(yaml).expect("valid slack app config")
}

const FULL: &str = r#"
workspace_id: "T0123456789"
signing_secret_ref: "slack_signing_secret"
bot_token_ref: "slack_bot_token"
enabled: true
default_agent: "support-agent"
routing:
  "C0ABC": "triage-agent"
  "/ask": "qa-agent"
authz:
  allowed_roles: ["slack-user"]
"#;

#[test]
fn deserializes_full_config() {
    let app = parse(FULL);
    assert_eq!(app.workspace_id.as_str(), "T0123456789");
    assert!(app.enabled);
    assert_eq!(app.authz.allowed_roles, vec!["slack-user".to_owned()]);
}

#[test]
fn enabled_defaults_true() {
    let app = parse(
        r#"
workspace_id: "T1"
signing_secret_ref: "s"
bot_token_ref: "b"
default_agent: "a"
"#,
    );
    assert!(app.enabled);
}

#[test]
fn routing_overrides_default_then_falls_back() {
    let app = parse(FULL);
    assert_eq!(app.agent_for("/ask").map(|a| a.as_str()), Some("qa-agent"));
    assert_eq!(
        app.agent_for("C0ABC").map(|a| a.as_str()),
        Some("triage-agent")
    );
    assert_eq!(
        app.agent_for("C0UNKNOWN").map(|a| a.as_str()),
        Some("support-agent")
    );
}

#[test]
fn validate_accepts_default_agent_only() {
    let app = parse(
        r#"
workspace_id: "T1"
signing_secret_ref: "s"
bot_token_ref: "b"
default_agent: "a"
"#,
    );
    assert!(app.validate("app").is_ok());
}

#[test]
fn validate_rejects_no_agent_no_routing() {
    let app = parse(
        r#"
workspace_id: "T1"
signing_secret_ref: "s"
bot_token_ref: "b"
"#,
    );
    assert!(app.validate("app").is_err());
}

#[test]
fn unknown_field_rejected() {
    let res: Result<SlackAppConfig, _> = serde_yaml::from_str(
        r#"
workspace_id: "T1"
signing_secret_ref: "s"
bot_token_ref: "b"
default_agent: "a"
bogus: true
"#,
    );
    assert!(res.is_err());
}

#[test]
fn services_manifest_with_slack_apps_validates() {
    let yaml = r#"
slack_apps:
  support:
    workspace_id: "T0123456789"
    signing_secret_ref: "slack_signing_secret"
    bot_token_ref: "slack_bot_token"
    default_agent: "support-agent"
"#;
    let cfg: ServicesConfig = serde_yaml::from_str(yaml).expect("valid services manifest");
    assert_eq!(cfg.slack_apps.len(), 1);
    assert!(cfg.validate().is_ok());
}

#[test]
fn services_manifest_rejects_invalid_slack_app() {
    let yaml = r#"
slack_apps:
  broken:
    workspace_id: "T1"
    signing_secret_ref: "s"
    bot_token_ref: "b"
"#;
    let cfg: ServicesConfig = serde_yaml::from_str(yaml).expect("deserializes");
    assert!(cfg.validate().is_err());
}
