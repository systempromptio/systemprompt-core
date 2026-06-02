//! Unit tests for [`AccessControlConfig`] parsing and validation.
//!
//! These cover the pure CPU/serde paths only — DB-backed ingestion is
//! exercised separately under `crates/tests/integration/`.

use systemprompt_security::authz::AccessControlConfig;

const VALID_YAML: &str = r#"
rules:
  - entity_type: gateway_route
    entity_id: claude-star
    access: allow
    roles: [developer, admin]
  - entity_type: mcp_server
    entity_id: systemprompt
    access: allow
    roles: [developer]
    justification: "ICP team needs MCP for daily work"
"#;

#[test]
fn parses_and_validates_full_example() {
    let cfg: AccessControlConfig = serde_yaml::from_str(VALID_YAML).expect("yaml parses");
    assert_eq!(cfg.rules.len(), 2);
    cfg.validate().expect("valid baseline");
}

#[test]
fn rejects_unknown_keys() {
    let bad = r#"
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
fn rejects_rule_with_no_roles() {
    let bad = r#"
rules:
  - entity_type: agent
    entity_id: foo
    access: allow
"#;
    let cfg: AccessControlConfig = serde_yaml::from_str(bad).expect("yaml parses");
    let err = cfg.validate().expect_err("rule with no roles must fail");
    let msg = err.to_string();
    assert!(msg.contains("at least one role"), "got: {msg}");
}

#[test]
fn rejects_empty_role_string() {
    let bad = r#"
rules:
  - entity_type: agent
    entity_id: foo
    access: allow
    roles: ["   "]
"#;
    let cfg: AccessControlConfig = serde_yaml::from_str(bad).expect("yaml parses");
    let err = cfg.validate().expect_err("empty role string must fail");
    assert!(err.to_string().contains("empty role"), "got: {err}");
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
fn rejects_both_entity_id_and_match() {
    let bad = r#"
rules:
  - entity_type: gateway_route
    entity_id: claude-star
    entity_match: "claude-*"
    roles: [user]
"#;
    let err = serde_yaml::from_str::<AccessControlConfig>(bad)
        .expect_err("setting both entity_id and entity_match must fail");
    assert!(err.to_string().contains("both"), "got: {err}");
}

#[test]
fn rejects_neither_entity_id_nor_match() {
    let bad = r#"
rules:
  - entity_type: gateway_route
    roles: [user]
"#;
    let err = serde_yaml::from_str::<AccessControlConfig>(bad)
        .expect_err("setting neither entity_id nor entity_match must fail");
    assert!(err.to_string().contains("neither"), "got: {err}");
}

#[test]
fn parses_entity_match_glob_rule() {
    let yaml = r#"
rules:
  - entity_type: gateway_route
    entity_match: "*"
    default_included: true
    roles: [user]
"#;
    let cfg: AccessControlConfig = serde_yaml::from_str(yaml).expect("yaml parses");
    assert_eq!(cfg.rules.len(), 1);
    cfg.validate().expect("glob rule is valid");
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
    assert_eq!(reparsed.rules.len(), cfg.rules.len());
}
