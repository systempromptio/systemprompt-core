//! Unit tests for [`AccessControlConfig`] parsing and validation.
//!
//! These cover the pure CPU/serde paths only — DB-backed ingestion is
//! exercised separately under `crates/tests/integration/`.

use systemprompt_security::authz::AccessControlConfig;

const VALID_YAML: &str = r#"
departments:
  - name: Engineering
    manager_email: ed@example.com
  - name: Marketing
rules:
  - entity_type: gateway_route
    entity_id: claude-star
    access: allow
    roles: [developer, admin]
  - entity_type: mcp_server
    entity_id: systemprompt
    access: allow
    departments: [Engineering]
    justification: "ICP team needs MCP for daily work"
"#;

#[test]
fn parses_and_validates_full_example() {
    let cfg: AccessControlConfig = serde_yaml::from_str(VALID_YAML).expect("yaml parses");
    assert_eq!(cfg.departments.len(), 2);
    assert_eq!(cfg.rules.len(), 2);
    cfg.validate().expect("valid baseline");
}

#[test]
fn rejects_unknown_keys() {
    let bad = r#"
departments:
  - name: Engineering
rules:
  - entity_type: agent
    entity_id: foo
    access: allow
    roles: [user]
    nonsense: true
"#;
    let err = serde_yaml::from_str::<AccessControlConfig>(bad)
        .expect_err("unknown field nonsense should fail");
    assert!(err.to_string().contains("nonsense"), "got: {err}");
}

#[test]
fn rejects_rule_with_no_roles_or_departments() {
    let bad = r#"
rules:
  - entity_type: agent
    entity_id: foo
    access: allow
"#;
    let cfg: AccessControlConfig = serde_yaml::from_str(bad).expect("yaml parses");
    let err = cfg
        .validate()
        .expect_err("rule with neither roles nor departments must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("roles[]") && msg.contains("departments[]"),
        "got: {msg}"
    );
}

#[test]
fn rejects_rule_referencing_undeclared_department() {
    let bad = r#"
departments:
  - name: Engineering
rules:
  - entity_type: agent
    entity_id: foo
    access: allow
    departments: [Marketing]
"#;
    let cfg: AccessControlConfig = serde_yaml::from_str(bad).expect("yaml parses");
    let err = cfg.validate().expect_err("undeclared department must fail");
    assert!(
        err.to_string()
            .contains("undeclared department 'Marketing'"),
        "got: {err}"
    );
}

#[test]
fn rejects_duplicate_department_names() {
    let bad = r#"
departments:
  - name: Engineering
  - name: Engineering
rules: []
"#;
    let cfg: AccessControlConfig = serde_yaml::from_str(bad).expect("yaml parses");
    let err = cfg.validate().expect_err("duplicate names must fail");
    assert!(err.to_string().contains("duplicate"), "got: {err}");
}

#[test]
fn rejects_empty_entity_id() {
    let bad = r#"
rules:
  - entity_type: agent
    entity_id: ""
    access: allow
    roles: [admin]
"#;
    let cfg: AccessControlConfig = serde_yaml::from_str(bad).expect("yaml parses");
    let err = cfg.validate().expect_err("empty entity_id must fail");
    assert!(err.to_string().contains("entity_id is empty"), "got: {err}");
}

#[test]
fn empty_config_validates() {
    let cfg = AccessControlConfig::default();
    cfg.validate().expect("empty config is valid");
}

#[test]
fn round_trips_through_serde() {
    let cfg: AccessControlConfig = serde_yaml::from_str(VALID_YAML).unwrap();
    let serialized = serde_yaml::to_string(&cfg).unwrap();
    let reparsed: AccessControlConfig = serde_yaml::from_str(&serialized).unwrap();
    reparsed.validate().expect("round-trip remains valid");
    assert_eq!(reparsed.departments.len(), cfg.departments.len());
    assert_eq!(reparsed.rules.len(), cfg.rules.len());
}
