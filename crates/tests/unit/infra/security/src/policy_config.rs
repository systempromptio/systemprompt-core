use systemprompt_governance::policy::{GovernanceConfig, GovernanceConfigError};
use systemprompt_manifest::EnforcementMode;

#[test]
fn defaults_declare_the_four_builtins_in_order() {
    let cfg = GovernanceConfig::defaults();
    let ids: Vec<&str> = cfg.policies.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        ["scope_check", "secret_scan", "tool_blocklist", "rate_limit"]
    );
    assert!(cfg.policies.iter().all(|p| p.enabled));
    assert!(cfg.policies.iter().all(|p| p.mode == EnforcementMode::Warn));
    assert_eq!(cfg.mode, EnforcementMode::Warn);
    assert!(cfg.enabled);
}

#[test]
fn defaults_have_no_vendor_catalog() {
    let cfg = GovernanceConfig::defaults();
    let engine = systemprompt_governance::policy::GovernanceEngine::from_config(
        &cfg,
        Some(crate::offline_rate_windows()),
    )
    .unwrap();
    assert_eq!(engine.secret_scanner().unwrap().pattern_count(), 0);
}

#[test]
fn the_master_switch_parses_and_defaults_to_on() {
    let off = GovernanceConfig::parse(
        "governance:\n  enabled: false\n  policies:\n    - id: secret_scan\n",
    )
    .unwrap();
    assert!(!off.enabled);
    assert!(
        off.policies[0].enabled,
        "the master switch must not rewrite the per-policy declarations"
    );

    let absent =
        GovernanceConfig::parse("governance:\n  policies:\n    - id: secret_scan\n").unwrap();
    assert!(absent.enabled);
}

#[test]
fn parse_reads_ids_enabled_flags_and_params() {
    let cfg = GovernanceConfig::parse(
        "governance:\n  policies:\n    - id: rate_limit\n      enabled: false\n      window_secs: 5\n    - id: secret_scan\n",
    )
    .unwrap();
    assert_eq!(cfg.policies.len(), 2);
    assert_eq!(cfg.policies[0].id, "rate_limit");
    assert!(!cfg.policies[0].enabled);
    assert_eq!(
        cfg.policies[0]
            .params
            .get("window_secs")
            .and_then(serde_yaml::Value::as_u64),
        Some(5)
    );
    assert_eq!(cfg.policies[1].id, "secret_scan");
    assert!(cfg.policies[1].enabled);
}

#[test]
fn parse_rejects_documents_without_a_policies_sequence() {
    assert!(matches!(
        GovernanceConfig::parse("governance: {}"),
        Err(GovernanceConfigError::MissingPolicies)
    ));
    assert!(matches!(
        GovernanceConfig::parse("unrelated: true"),
        Err(GovernanceConfigError::MissingPolicies)
    ));
}

#[test]
fn parse_rejects_entries_without_an_id() {
    let err = GovernanceConfig::parse(
        "governance:\n  policies:\n    - id: secret_scan\n    - enabled: true\n",
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GovernanceConfigError::MissingPolicyId { index: 1 }
    ));
}

#[test]
fn parse_rejects_entries_with_a_blank_id() {
    let err = GovernanceConfig::parse(
        "governance:\n  policies:\n    - id: secret_scan\n    - id: \"  \"\n",
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GovernanceConfigError::MissingPolicyId { index: 1 }
    ));
}

#[test]
fn parse_rejects_non_string_and_unknown_policy_modes() {
    for mode in ["7", "enforce-ish"] {
        let yaml = format!("governance:\n  mode: {mode}\n  policies:\n    - id: secret_scan\n");
        assert!(matches!(
            GovernanceConfig::parse(&yaml),
            Err(GovernanceConfigError::InvalidMode { .. })
        ));
    }
}

#[test]
fn parse_rejects_invalid_yaml() {
    assert!(matches!(
        GovernanceConfig::parse(": : :"),
        Err(GovernanceConfigError::Yaml(_))
    ));
}

#[test]
fn the_defaults_are_the_warn_only_chain() {
    let cfg = GovernanceConfig::defaults();
    assert_eq!(cfg.mode, EnforcementMode::Warn);
    assert!(cfg.policies.iter().all(|p| p.mode == EnforcementMode::Warn));
}

#[test]
fn parse_rejects_a_broken_declaration_instead_of_downgrading_enforcement() {
    assert!(matches!(
        GovernanceConfig::parse("governance: {}"),
        Err(GovernanceConfigError::MissingPolicies)
    ));
    assert!(matches!(
        GovernanceConfig::parse("governance:\n  mode: warnn\n  policies:\n    - id: secret_scan\n"),
        Err(GovernanceConfigError::InvalidMode { .. })
    ));
    assert!(matches!(
        GovernanceConfig::parse(
            "governance:\n  policies:\n    - id: secret_scan\n      patterns:\n        - id: broken\n          name: Broken\n          regex: '('\n"
        ),
        Err(GovernanceConfigError::InvalidSecretPatterns(_))
    ));
    let cfg = GovernanceConfig::parse("governance:\n  policies:\n    - id: secret_scan\n")
        .expect("valid declaration parses");
    assert_eq!(cfg.mode, EnforcementMode::Enforce);
}

#[test]
fn engine_built_from_the_defaults_runs_every_builtin_in_warn() {
    let engine = systemprompt_governance::policy::GovernanceEngine::from_config(
        &GovernanceConfig::defaults(),
        Some(crate::offline_rate_windows()),
    )
    .expect("the defaults build");
    let policies: Vec<_> = engine
        .policies()
        .filter(|(config, _)| config.enabled)
        .map(|(config, _)| (config.id.as_str(), config.enabled, config.mode))
        .collect();
    assert_eq!(
        policies,
        vec![
            ("scope_check", true, EnforcementMode::Warn),
            ("secret_scan", true, EnforcementMode::Warn),
            ("tool_blocklist", true, EnforcementMode::Warn),
            ("rate_limit", true, EnforcementMode::Warn),
        ]
    );
}
