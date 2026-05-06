use systemprompt_models::profile::{
    AuthzMode, GovernanceConfig, UNRESTRICTED_ACKNOWLEDGEMENT,
};

#[test]
fn governance_config_default_has_no_authz_block() {
    let cfg = GovernanceConfig::default();
    assert!(
        cfg.authz.is_none(),
        "default governance has no authz block — runtime installs DenyAllHook"
    );
}

#[test]
fn webhook_mode_yaml_round_trips() {
    let yaml = r"
authz:
  hook:
    mode: webhook
    url: http://localhost:8080/govern/authz
    timeout_ms: 250
";
    let cfg: GovernanceConfig = serde_yaml::from_str(yaml).expect("yaml parses");
    let authz = cfg.authz.expect("authz block parsed");
    assert!(matches!(authz.hook.mode, AuthzMode::Webhook));
    assert_eq!(authz.hook.timeout_ms, 250);
    assert_eq!(
        authz.hook.url.as_deref(),
        Some("http://localhost:8080/govern/authz"),
    );
    assert!(authz.hook.acknowledgement.is_none());
}

#[test]
fn disabled_mode_yaml_parses() {
    let yaml = r"
authz:
  hook:
    mode: disabled
";
    let cfg: GovernanceConfig = serde_yaml::from_str(yaml).expect("yaml parses");
    let authz = cfg.authz.expect("authz block parsed");
    assert!(matches!(authz.hook.mode, AuthzMode::Disabled));
    assert_eq!(authz.hook.timeout_ms, 500, "default timeout applies");
}

#[test]
fn unrestricted_mode_with_acknowledgement_parses() {
    let yaml = format!(
        r#"
authz:
  hook:
    mode: unrestricted
    acknowledgement: "{}"
"#,
        UNRESTRICTED_ACKNOWLEDGEMENT
    );
    let cfg: GovernanceConfig = serde_yaml::from_str(&yaml).expect("yaml parses");
    let authz = cfg.authz.expect("authz block parsed");
    assert!(matches!(authz.hook.mode, AuthzMode::Unrestricted));
    assert_eq!(
        authz.hook.acknowledgement.as_deref(),
        Some(UNRESTRICTED_ACKNOWLEDGEMENT),
    );
}

#[test]
fn empty_governance_yaml_has_no_authz() {
    let cfg: GovernanceConfig = serde_yaml::from_str("{}").expect("yaml parses");
    assert!(cfg.authz.is_none());
}
