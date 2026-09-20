use std::path::Path;

use systemprompt_security::policy::{GovernanceConfig, GovernanceConfigError, PolicyMode};
use tempfile::{NamedTempFile, tempdir};

#[test]
fn defaults_declare_the_four_builtins_in_order() {
    let cfg = GovernanceConfig::defaults();
    let ids: Vec<&str> = cfg.policies.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        ["scope_check", "secret_scan", "tool_blocklist", "rate_limit"]
    );
    assert!(cfg.policies.iter().all(|p| p.enabled));
    assert!(cfg.policies.iter().all(|p| p.mode == PolicyMode::Warn));
    assert_eq!(cfg.mode, PolicyMode::Warn);
    assert!(cfg.enabled);
}

#[test]
fn defaults_have_no_vendor_catalog() {
    let cfg = GovernanceConfig::defaults();
    let engine = systemprompt_security::policy::GovernanceEngine::from_config(&cfg).unwrap();
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
fn load_falls_back_to_the_warn_only_chain_only_when_the_file_is_absent() {
    let cfg = GovernanceConfig::load(Path::new("/nonexistent/governance/config.yaml"))
        .expect("an absent file is the documented fallback");
    assert_eq!(cfg.mode, PolicyMode::Warn);
    assert_eq!(
        cfg.policies.len(),
        GovernanceConfig::defaults().policies.len()
    );
}

#[test]
fn load_rejects_a_present_but_broken_file_instead_of_downgrading_enforcement() {
    let dir = std::env::temp_dir().join(format!("gov-cfg-load-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let malformed = dir.join("malformed.yaml");
    std::fs::write(&malformed, ": : :").unwrap();
    assert!(matches!(
        GovernanceConfig::load(&malformed),
        Err(GovernanceConfigError::Yaml(_))
    ));

    let no_policies = dir.join("no-policies.yaml");
    std::fs::write(&no_policies, "governance: {}").unwrap();
    assert!(matches!(
        GovernanceConfig::load(&no_policies),
        Err(GovernanceConfigError::MissingPolicies)
    ));

    let bad_mode = dir.join("bad-mode.yaml");
    std::fs::write(
        &bad_mode,
        "governance:\n  mode: warnn\n  policies:\n    - id: secret_scan\n",
    )
    .unwrap();
    assert!(matches!(
        GovernanceConfig::load(&bad_mode),
        Err(GovernanceConfigError::InvalidMode { .. })
    ));

    let bad_regex = dir.join("bad-regex.yaml");
    std::fs::write(
        &bad_regex,
        "governance:\n  policies:\n    - id: secret_scan\n      patterns:\n        - id: broken\n          name: Broken\n          regex: '('\n",
    )
    .unwrap();
    assert!(matches!(
        GovernanceConfig::load(&bad_regex),
        Err(GovernanceConfigError::InvalidSecretPatterns(_))
    ));

    let good = dir.join("good.yaml");
    std::fs::write(&good, "governance:\n  policies:\n    - id: secret_scan\n").unwrap();
    let cfg = GovernanceConfig::load(&good).expect("valid file loads");
    assert_eq!(cfg.mode, PolicyMode::Enforce);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn engine_uses_defaults_when_services_root_has_no_governance_config() {
    let root = tempdir().unwrap();

    let engine = systemprompt_security::policy::GovernanceEngine::from_services_root(root.path())
        .expect("an absent config uses the documented warn-only defaults");
    let policies: Vec<_> = engine
        .policies()
        .filter(|(config, _)| config.enabled)
        .map(|(config, _)| (config.id.as_str(), config.enabled, config.mode))
        .collect();
    assert_eq!(
        policies,
        vec![
            ("scope_check", true, PolicyMode::Warn),
            ("secret_scan", true, PolicyMode::Warn),
            ("tool_blocklist", true, PolicyMode::Warn),
            ("rate_limit", true, PolicyMode::Warn),
        ]
    );
}

#[test]
fn engine_reports_non_directory_services_root_without_falling_back() {
    let root = NamedTempFile::new().unwrap();

    let error = systemprompt_security::policy::GovernanceEngine::from_services_root(root.path())
        .expect_err("a non-directory services root must not silently use defaults");
    assert!(matches!(
        error,
        systemprompt_security::policy::GovernanceEngineError::ConfigRejected { .. }
    ));
}
