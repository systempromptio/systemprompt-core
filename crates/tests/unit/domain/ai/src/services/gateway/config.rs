use systemprompt_ai::{GatewayPolicyConfig, GatewayPolicyEntry, GatewayPolicySpec};

#[test]
fn empty_config_validates() {
    let cfg = GatewayPolicyConfig::default();
    assert!(cfg.validate().is_ok());
}

#[test]
fn single_named_policy_validates() {
    let cfg = GatewayPolicyConfig {
        policies: vec![GatewayPolicyEntry {
            name: "default".into(),
            enabled: true,
            spec: GatewayPolicySpec::default(),
        }],
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn empty_name_is_rejected() {
    let cfg = GatewayPolicyConfig {
        policies: vec![GatewayPolicyEntry {
            name: "  ".into(),
            enabled: true,
            spec: GatewayPolicySpec::default(),
        }],
    };
    let err = cfg.validate().unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("must not be empty"), "got: {msg}");
}

#[test]
fn duplicate_names_are_rejected() {
    let cfg = GatewayPolicyConfig {
        policies: vec![
            GatewayPolicyEntry {
                name: "p1".into(),
                enabled: true,
                spec: GatewayPolicySpec::default(),
            },
            GatewayPolicyEntry {
                name: "p1".into(),
                enabled: true,
                spec: GatewayPolicySpec::default(),
            },
        ],
    };
    let err = cfg.validate().unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("duplicate"), "got: {msg}");
}

#[test]
fn yaml_parses_minimal_policy() {
    let yaml = r#"
policies:
  - name: default
    enabled: true
    spec:
      safety:
        scanners: [heuristic]
        block_categories: [secret]
"#;
    let cfg: GatewayPolicyConfig = serde_yaml::from_str(yaml).expect("yaml parses");
    assert_eq!(cfg.policies.len(), 1);
    assert_eq!(cfg.policies[0].name, "default");
    assert_eq!(cfg.policies[0].spec.safety.scanners, vec!["heuristic"]);
    assert_eq!(cfg.policies[0].spec.safety.block_categories, vec!["secret"]);
}

#[test]
fn yaml_rejects_unknown_fields() {
    let yaml = r#"
policies:
  - name: default
    enabled: true
    unknown: 5
"#;
    let result: Result<GatewayPolicyConfig, _> = serde_yaml::from_str(yaml);
    assert!(result.is_err());
}

#[test]
fn default_enabled_is_true() {
    let yaml = "policies:\n  - name: default";
    let cfg: GatewayPolicyConfig = serde_yaml::from_str(yaml).expect("parses");
    assert!(cfg.policies[0].enabled);
}
