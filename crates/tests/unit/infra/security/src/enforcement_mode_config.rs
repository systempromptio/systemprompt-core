use systemprompt_governance::policy::{GovernanceConfig, GovernanceConfigError};
use systemprompt_manifest::EnforcementMode;

#[test]
fn governance_yaml_mode_parses_to_the_shared_enum() {
    let cfg = GovernanceConfig::parse(
        r"
governance:
  enabled: true
  mode: warn
  policies:
    - id: scope_check
    - id: rate_limit
      mode: enforce
",
    )
    .unwrap();
    assert_eq!(cfg.mode, EnforcementMode::Warn);
    assert_eq!(cfg.policies[0].mode, EnforcementMode::Warn);
    assert_eq!(cfg.policies[1].mode, EnforcementMode::Enforce);
}

#[test]
fn unknown_governance_mode_is_a_parse_error() {
    let err = GovernanceConfig::parse(
        r"
governance:
  policies:
    - id: scope_check
      mode: warnn
",
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GovernanceConfigError::InvalidMode { ref value, .. } if value == "warnn"
    ));
}

#[test]
fn defaults_enable_every_builtin_in_warn_mode() {
    let defaults = GovernanceConfig::defaults();
    let ids: Vec<&str> = defaults.policies.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        ["scope_check", "secret_scan", "tool_blocklist", "rate_limit"]
    );
    assert!(defaults.policies.iter().all(|p| p.mode.is_warn()));
}
