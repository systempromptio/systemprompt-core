use systemprompt_manifest::profile::{AutoConfirm, BrainProfile, CaptureSource, StalePolicy};
use systemprompt_models::brain::ScopeLevel;
use systemprompt_models::origin::ClientKind;

#[test]
fn defaults_match_the_contract() {
    let profile = BrainProfile::default();
    assert!(!profile.enabled);
    assert_eq!(profile.recall.budget_tokens, 800);
    assert_eq!(profile.recall.deadline_ms, 200);
    assert_eq!(profile.recall.scopes, ScopeLevel::ALL.to_vec());
    assert_eq!(profile.recall.stale, StalePolicy::Withhold);
    assert!(profile.recall.client_enabled(ClientKind::ClaudeCode));
    assert!(profile.recall.client_enabled(ClientKind::ClaudeDesktop));
    assert!(!profile.recall.client_enabled(ClientKind::OpenCode));
    assert!(!profile.recall.client_enabled(ClientKind::Internal));
    assert_eq!(
        profile.capture.auto_confirm.document,
        AutoConfirm::ProjectPolicy
    );
    assert_eq!(profile.capture.auto_confirm.others, AutoConfirm::Never);
    assert!(profile.capture.sources.contains(&CaptureSource::Document));
    assert_eq!(profile.extraction.max_output_tokens, 2048);
    assert_eq!(profile.extraction.timeout_ms, 60_000);
    assert_eq!(profile.extraction.daily_token_budget, 1_000_000);
    assert_eq!(profile.lifecycle.review_after_days, 90);
    assert_eq!(profile.lifecycle.retired_retention_days, 365);
    assert_eq!(profile.lifecycle.recalls_retention_days, 90);
    assert_eq!(profile.lifecycle.results_retention_days, 30);
    assert_eq!(profile.lifecycle.correction_window_days, 14);
    assert_eq!(profile.consumer.batch, 32);
    assert_eq!(profile.consumer.poll_secs, 5);
    assert_eq!(profile.consumer.sweep_secs, 300);
    assert_eq!(profile.consumer.source_timeout_ms, 5000);
}

#[test]
fn session_capture_and_consolidation_are_off_by_default() {
    let profile = BrainProfile::default();
    assert!(!profile.capture.sources.contains(&CaptureSource::Session));
    assert!(!profile.capture.session.enabled);
    assert_eq!(profile.capture.session.max_chars, 4000);
    assert!(!profile.capture.session_capture_on());
    assert!(!profile.consolidation.enabled);
    assert_eq!(profile.consolidation.daily_token_budget, None);
}

#[test]
fn session_capture_needs_both_the_source_and_the_switch() {
    let only_switch: BrainProfile =
        serde_yaml::from_str("capture:\n  session: { enabled: true }\n").expect("switch");
    assert!(!only_switch.capture.session_capture_on());
    let only_source: BrainProfile =
        serde_yaml::from_str("capture:\n  sources: [session]\n").expect("source");
    assert!(!only_source.capture.session_capture_on());
    let both: BrainProfile = serde_yaml::from_str(
        "capture:\n  sources: [document, session]\n  session: { enabled: true, max_chars: 10 }\n",
    )
    .expect("both");
    assert!(both.capture.session_capture_on());
    assert_eq!(both.capture.session.max_chars, 10);
    let consolidation: BrainProfile =
        serde_yaml::from_str("consolidation: { enabled: true, daily_token_budget: 5 }\n")
            .expect("consolidation");
    assert_eq!(
        consolidation
            .consolidation
            .budget_tokens(&consolidation.extraction),
        5
    );
}

#[test]
fn empty_block_is_the_default() {
    let parsed: BrainProfile = serde_yaml::from_str("{}").expect("empty block");
    assert_eq!(parsed, BrainProfile::default());
}

#[test]
fn partial_block_keeps_other_defaults() {
    let parsed: BrainProfile =
        serde_yaml::from_str("enabled: true\nrecall:\n  clients: { opencode: true }\n")
            .expect("partial block");
    assert!(parsed.enabled);
    assert!(parsed.recall.client_enabled(ClientKind::OpenCode));
    assert!(!parsed.recall.client_enabled(ClientKind::ClaudeCode));
    assert_eq!(parsed.recall.budget_tokens, 800);
}

#[test]
fn unknown_fields_are_refused() {
    assert!(serde_yaml::from_str::<BrainProfile>("enable: true\n").is_err());
    assert!(serde_yaml::from_str::<BrainProfile>("recall: { budget: 10 }\n").is_err());
    assert!(serde_yaml::from_str::<BrainProfile>("consumer: { batch: 1, x: 2 }\n").is_err());
}

#[test]
fn yaml_on_is_not_a_bool() {
    assert!(serde_yaml::from_str::<BrainProfile>("enabled: on\n").is_err());
    assert!(
        serde_yaml::from_str::<BrainProfile>("recall: { clients: { claude-code: on } }\n").is_err()
    );
}
