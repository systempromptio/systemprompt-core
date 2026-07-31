use std::path::Path;

use systemprompt_security::policy::{GovernanceConfig, GovernanceConfigError};

#[test]
fn defaults_declare_the_four_builtins_in_order() {
    let cfg = GovernanceConfig::defaults();
    let ids: Vec<&str> = cfg.policies.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        ["secret_scan", "scope_check", "tool_blocklist", "rate_limit"]
    );
    assert!(cfg.policies.iter().all(|p| p.enabled));
    assert!(cfg.enabled);
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

    let absent = GovernanceConfig::parse("governance:\n  policies:\n    - id: secret_scan\n")
        .unwrap();
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
fn parse_rejects_invalid_yaml() {
    assert!(matches!(
        GovernanceConfig::parse(": : :"),
        Err(GovernanceConfigError::Yaml(_))
    ));
}

#[test]
fn load_falls_back_to_defaults_when_the_file_is_absent() {
    let cfg = GovernanceConfig::load(Path::new("/nonexistent/governance/config.yaml"));
    assert_eq!(
        cfg.policies.len(),
        GovernanceConfig::defaults().policies.len()
    );
}

#[test]
fn load_falls_back_to_defaults_on_malformed_yaml() {
    let dir = std::env::temp_dir().join(format!("gov-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.yaml");
    std::fs::write(&path, ": : :").unwrap();
    let cfg = GovernanceConfig::load(&path);
    assert_eq!(cfg.policies.len(), 4);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn validate_accepts_an_absent_file() {
    assert!(GovernanceConfig::validate(Path::new("/nonexistent/governance/config.yaml")).is_ok());
}

#[test]
fn validate_rejects_what_load_would_silently_swallow() {
    let dir = std::env::temp_dir().join(format!("gov-cfg-validate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let malformed = dir.join("malformed.yaml");
    std::fs::write(&malformed, ": : :").unwrap();
    assert!(matches!(
        GovernanceConfig::validate(&malformed),
        Err(GovernanceConfigError::Yaml(_))
    ));

    let no_policies = dir.join("no-policies.yaml");
    std::fs::write(&no_policies, "governance: {}").unwrap();
    assert!(matches!(
        GovernanceConfig::validate(&no_policies),
        Err(GovernanceConfigError::MissingPolicies)
    ));

    let good = dir.join("good.yaml");
    std::fs::write(&good, "governance:\n  policies:\n    - id: secret_scan\n").unwrap();
    assert!(GovernanceConfig::validate(&good).is_ok());

    std::fs::remove_dir_all(&dir).ok();
}
