use systemprompt_ai::{GatewayPolicySpec, QuotaWindow, SafetyConfig, SafetyHistoryMode};

#[test]
fn permissive_is_default() {
    let p = GatewayPolicySpec::permissive();
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
        ..QuotaWindow::default()
    };
    let yaml = serde_yaml::to_string(&qw).expect("ser");
    let back: QuotaWindow = serde_yaml::from_str(&yaml).expect("de");
    assert_eq!(back.window_seconds, 60);
    assert_eq!(back.max_requests, Some(100));
}

#[test]
fn a_quota_window_written_before_subjects_existed_still_deserializes_as_user() {
    let yaml = "window_seconds: 3600\nmax_requests: 50";
    let qw: QuotaWindow = serde_yaml::from_str(yaml).expect("de");
    assert_eq!(qw.subject, systemprompt_ai::USER_QUOTA_SUBJECT);
    assert_eq!(qw.max_cost_microdollars, None);
}

#[test]
fn a_quota_window_can_key_on_an_extension_subject_with_a_cost_ceiling() {
    let yaml = "window_seconds: 2592000\nsubject: organization\nmax_cost_microdollars: 500000000";
    let qw: QuotaWindow = serde_yaml::from_str(yaml).expect("de");
    assert_eq!(qw.subject, "organization");
    assert_eq!(qw.max_cost_microdollars, Some(500_000_000));
    assert_eq!(qw.max_requests, None);
}

#[test]
fn safety_config_defaults_are_empty() {
    let s = SafetyConfig::default();
    assert!(s.scanners.is_empty());
    assert!(s.block_categories.is_empty());
}

#[test]
fn spec_yaml_unknown_field_rejected() {
    let yaml = "quota_windows: []\nzz: 5";
    let r: Result<GatewayPolicySpec, _> = serde_yaml::from_str(yaml);
    assert!(r.is_err());
}

#[test]
fn safety_history_defaults_to_off() {
    assert_eq!(SafetyConfig::default().history, SafetyHistoryMode::Off);
}

#[test]
fn a_policy_written_before_history_existed_still_deserializes() {
    let yaml = "scanners: [heuristic]\nblock_categories: [jailbreak]";
    let s: SafetyConfig = serde_yaml::from_str(yaml).expect("de");
    assert_eq!(s.history, SafetyHistoryMode::Off);
    assert_eq!(s.scanners, vec!["heuristic".to_owned()]);
}

#[test]
fn safety_history_modes_round_trip_as_lowercase() {
    for (text, mode) in [
        ("off", SafetyHistoryMode::Off),
        ("audit", SafetyHistoryMode::Audit),
        ("block", SafetyHistoryMode::Block),
    ] {
        let yaml = format!("scanners: []\nblock_categories: []\nhistory: {text}");
        let s: SafetyConfig = serde_yaml::from_str(&yaml).expect("de");
        assert_eq!(s.history, mode);
    }
}
