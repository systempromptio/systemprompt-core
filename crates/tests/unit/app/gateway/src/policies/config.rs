use systemprompt_gateway::{GatewayPolicyConfig, GatewayPolicyEntry, GatewayPolicySpec};

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
            priority: 0,
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
            priority: 0,
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
                priority: 0,
                spec: GatewayPolicySpec::default(),
            },
            GatewayPolicyEntry {
                name: "p1".into(),
                enabled: true,
                priority: 0,
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

#[test]
fn heuristic_scanner_with_empty_effective_list_is_rejected() {
    use systemprompt_gateway::HeuristicConfig;
    let mut spec = GatewayPolicySpec::default();
    spec.safety.scanners = vec!["heuristic".to_owned()];
    spec.safety.heuristic = HeuristicConfig {
        disable_builtin: true,
        ..HeuristicConfig::default()
    };
    let cfg = GatewayPolicyConfig {
        policies: vec![GatewayPolicyEntry {
            name: "strict".to_owned(),
            enabled: true,
            priority: 0,
            spec,
        }],
    };
    let err = cfg
        .validate()
        .expect_err("empty phrase list must be rejected");
    assert!(err.to_string().contains("heuristic"), "{err}");
}

#[test]
fn heuristic_scanner_with_custom_phrases_validates() {
    use systemprompt_gateway::HeuristicConfig;
    let mut spec = GatewayPolicySpec::default();
    spec.safety.scanners = vec!["heuristic".to_owned()];
    spec.safety.heuristic = HeuristicConfig {
        phrases: Some(vec!["duck".to_owned()]),
        ..HeuristicConfig::default()
    };
    let cfg = GatewayPolicyConfig {
        policies: vec![GatewayPolicyEntry {
            name: "strict".to_owned(),
            enabled: true,
            priority: 0,
            spec,
        }],
    };
    cfg.validate().expect("custom phrase list must validate");
}

#[test]
fn yaml_parses_heuristic_block() {
    let yaml = r#"
policies:
  - name: guarded
    spec:
      safety:
        scanners: [heuristic]
        heuristic:
          extra_phrases: [duck]
"#;
    let cfg: GatewayPolicyConfig = serde_yaml::from_str(yaml).expect("yaml parses");
    assert_eq!(
        cfg.policies[0].spec.safety.heuristic.extra_phrases,
        vec!["duck"]
    );
    cfg.validate().expect("validates");
}

fn policy_with_safety(yaml: &str) -> GatewayPolicyConfig {
    let mut indented = String::new();
    for line in yaml.lines() {
        indented.push_str("        ");
        indented.push_str(line);
        indented.push('\n');
    }
    serde_yaml::from_str(&format!(
        "policies:\n  - name: p\n    spec:\n      safety:\n{indented}"
    ))
    .expect("policy yaml parses")
}

#[test]
fn scanner_settings_for_an_unlisted_scanner_are_rejected() {
    let config =
        policy_with_safety("scanners: [null]\nscanner_settings:\n  vendor:\n    fail_mode: open");
    let msg = config
        .validate()
        .expect_err("unlisted scanner settings")
        .to_string();
    assert!(msg.contains("scanner_settings.vendor"), "{msg}");
    assert!(msg.contains("not listed"), "{msg}");
}

#[test]
fn a_zero_scanner_timeout_is_rejected() {
    let config =
        policy_with_safety("scanners: [null]\nscanner_settings:\n  null:\n    timeout_ms: 0");
    let msg = config.validate().expect_err("zero timeout").to_string();
    assert!(msg.contains("scanner_settings.null.timeout_ms"), "{msg}");
}

#[test]
fn settings_for_a_listed_scanner_validate() {
    let config = policy_with_safety(
        "scanners: [null]\nscanner_settings:\n  null:\n    fail_mode: open\n    timeout_ms: \
         1\n    config: {anything: [1, 2]}",
    );
    config.validate().expect("valid settings");
}

#[test]
fn a_category_both_blocked_and_redacted_is_rejected() {
    let config = policy_with_safety(
        "scanners: [heuristic]\nblock_categories: [jailbreak, pii_email]\nredact_categories: \
         [pii_email]",
    );
    let msg = config.validate().expect_err("overlap").to_string();
    assert!(msg.contains("redact_categories"), "{msg}");
    assert!(msg.contains("pii_email"), "{msg}");
}

#[test]
fn disjoint_block_and_redact_categories_validate() {
    let config = policy_with_safety(
        "scanners: [heuristic]\nblock_categories: [jailbreak]\nredact_categories: [pii_email, \
         pii_credit_card]",
    );
    config.validate().expect("disjoint lists");
}
