use systemprompt_ai::{GatewayPolicySpec, QuotaWindow, SafetyConfig};

#[test]
fn permissive_is_default() {
    let p = GatewayPolicySpec::permissive();
    assert!(p.max_input_tokens_per_call.is_none());
    assert!(p.max_tool_depth.is_none());
    assert!(p.quota_windows.is_empty());
    assert!(p.safety.scanners.is_empty());
    assert!(p.safety.block_categories.is_empty());
}

#[test]
fn quota_window_serde_roundtrip() {
    let qw = QuotaWindow {
        window_seconds: 60,
        max_requests: Some(100),
        max_input_tokens: Some(10_000),
        max_output_tokens: None,
    };
    let yaml = serde_yaml::to_string(&qw).expect("ser");
    let back: QuotaWindow = serde_yaml::from_str(&yaml).expect("de");
    assert_eq!(back.window_seconds, 60);
    assert_eq!(back.max_requests, Some(100));
}

#[test]
fn safety_config_defaults_are_empty() {
    let s = SafetyConfig::default();
    assert!(s.scanners.is_empty());
    assert!(s.block_categories.is_empty());
}

#[test]
fn spec_yaml_unknown_field_rejected() {
    let yaml = "max_input_tokens_per_call: 100\nzz: 5";
    let r: Result<GatewayPolicySpec, _> = serde_yaml::from_str(yaml);
    assert!(r.is_err());
}
