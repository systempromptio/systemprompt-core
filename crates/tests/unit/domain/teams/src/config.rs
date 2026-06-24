//! Tests for the declarative Teams app config.

use systemprompt_models::services::{ServicesConfig, TeamsAppConfig};

fn yaml(doc: &str) -> TeamsAppConfig {
    serde_yaml::from_str(doc).unwrap()
}

const FULL: &str = r#"
tenant_id: "tenant-1"
app_id: "app-1"
app_password_ref: "teams_app_password"
enabled: true
default_agent: "support-agent"
routing:
  "19:abc@thread.v2": "triage-agent"
  "/ask": "qa-agent"
authz:
  allowed_roles: ["teams-user"]
"#;

#[test]
fn deserializes_a_full_app() {
    let app = yaml(FULL);
    assert_eq!(app.tenant_id.as_str(), "tenant-1");
    assert_eq!(app.app_id, "app-1");
    assert!(app.enabled);
    assert_eq!(app.authz.allowed_roles, vec!["teams-user"]);
    app.validate("primary").unwrap();
}

#[test]
fn enabled_defaults_to_true() {
    let app = yaml(
        r#"
tenant_id: "t"
app_id: "a"
app_password_ref: "ref"
default_agent: "agent"
"#,
    );
    assert!(app.enabled);
}

#[test]
fn agent_for_prefers_routing_then_falls_back_to_default() {
    let app = yaml(FULL);
    assert_eq!(app.agent_for("/ask").unwrap().as_str(), "qa-agent");
    assert_eq!(
        app.agent_for("19:abc@thread.v2").unwrap().as_str(),
        "triage-agent"
    );
    assert_eq!(
        app.agent_for("19:unknown").unwrap().as_str(),
        "support-agent"
    );
}

#[test]
fn validate_rejects_empty_tenant() {
    let app = yaml(
        r#"
tenant_id: ""
app_id: "a"
app_password_ref: "ref"
default_agent: "agent"
"#,
    );
    assert!(app.validate("primary").is_err());
}

#[test]
fn validate_rejects_empty_app_id() {
    let app = yaml(
        r#"
tenant_id: "t"
app_id: ""
app_password_ref: "ref"
default_agent: "agent"
"#,
    );
    assert!(app.validate("primary").is_err());
}

#[test]
fn validate_requires_default_or_routing() {
    let app = yaml(
        r#"
tenant_id: "t"
app_id: "a"
app_password_ref: "ref"
"#,
    );
    assert!(app.validate("primary").is_err());
}

#[test]
fn unknown_fields_are_rejected() {
    let err = serde_yaml::from_str::<TeamsAppConfig>(
        r#"
tenant_id: "t"
app_id: "a"
app_password_ref: "ref"
default_agent: "agent"
surprise: true
"#,
    );
    assert!(err.is_err());
}

#[test]
fn services_manifest_with_teams_apps_validates() {
    let yaml = r#"
teams_apps:
  support:
    tenant_id: "tenant-1"
    app_id: "app-1"
    app_password_ref: "teams_app_password"
    default_agent: "support-agent"
"#;
    let cfg: ServicesConfig = serde_yaml::from_str(yaml).expect("valid services manifest");
    assert_eq!(cfg.teams_apps.len(), 1);
    assert!(cfg.validate().is_ok());
}

#[test]
fn services_manifest_rejects_invalid_teams_app() {
    let yaml = r#"
teams_apps:
  broken:
    tenant_id: "tenant-1"
    app_id: ""
    app_password_ref: "teams_app_password"
    default_agent: "support-agent"
"#;
    let cfg: ServicesConfig = serde_yaml::from_str(yaml).expect("deserializes");
    assert!(cfg.validate().is_err());
}
