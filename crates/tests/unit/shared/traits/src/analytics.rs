use systemprompt_traits::analytics::{AnalyticsProviderError, SessionAnalytics};

fn ua(value: &str) -> SessionAnalytics {
    SessionAnalytics {
        user_agent: Some(value.to_owned()),
        ..Default::default()
    }
}

#[test]
fn is_ai_crawler_detects_known_tokens_case_insensitively() {
    for token in [
        "ChatGPT-User/1.0",
        "Mozilla/5.0 GPTBot/1.0",
        "PerplexityBot/2",
        "ClaudeBot",
        "anthropic-ai/1",
        "applebot-extended",
        "amazonbot",
        "Mozilla/5.0 (compatible; CCBot/2.0)",
    ] {
        assert!(ua(token).is_ai_crawler(), "{token}");
    }
}

#[test]
fn is_ai_crawler_false_for_browser_user_agent() {
    assert!(
        !ua("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15").is_ai_crawler()
    );
    assert!(!SessionAnalytics::default().is_ai_crawler());
}

#[test]
fn is_bot_true_for_generic_bot_user_agent() {
    assert!(ua("MyCustomBot/1.0").is_bot());
    assert!(ua("Googlebot/2.1").is_bot());
    assert!(ua("Mozilla/5.0 (Linux) Crawler").is_bot());
    assert!(ua("Spider/1").is_bot());
    assert!(ua("HeadlessChrome/120").is_bot());
}

#[test]
fn is_bot_false_for_ai_crawler_user_agent() {
    assert!(ua("ChatGPT-User").is_ai_crawler());
    assert!(!ua("ChatGPT-User").is_bot());
}

#[test]
fn is_bot_false_for_default_and_human_user_agent() {
    assert!(!SessionAnalytics::default().is_bot());
    assert!(
        !ua("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 Safari/605.1.15")
            .is_bot()
    );
}

#[test]
fn compute_fingerprint_uses_existing_hash_when_present() {
    let analytics = SessionAnalytics {
        fingerprint_hash: Some("fp_abc".to_owned()),
        user_agent: Some("anything".to_owned()),
        ..Default::default()
    };
    assert_eq!(analytics.compute_fingerprint(), "fp_abc");
}

#[test]
fn compute_fingerprint_is_deterministic_when_derived() {
    let a = SessionAnalytics {
        user_agent: Some("Mozilla/5.0".to_owned()),
        accept_language: Some("en-US".to_owned()),
        ..Default::default()
    };
    let b = a.clone();
    let fp_a = a.compute_fingerprint();
    let fp_b = b.compute_fingerprint();
    assert_eq!(fp_a, fp_b);
    assert!(fp_a.starts_with("fp_"));
}

#[test]
fn compute_fingerprint_falls_back_to_preferred_locale_when_no_accept_language() {
    let a = SessionAnalytics {
        user_agent: Some("UA".to_owned()),
        preferred_locale: Some("fr-FR".to_owned()),
        ..Default::default()
    };
    let b = SessionAnalytics {
        user_agent: Some("UA".to_owned()),
        accept_language: Some("fr-FR".to_owned()),
        ..Default::default()
    };
    assert_eq!(a.compute_fingerprint(), b.compute_fingerprint());
}

#[test]
fn compute_fingerprint_differs_with_user_agent() {
    let a = SessionAnalytics {
        user_agent: Some("AgentA".to_owned()),
        ..Default::default()
    };
    let b = SessionAnalytics {
        user_agent: Some("AgentB".to_owned()),
        ..Default::default()
    };
    assert_ne!(a.compute_fingerprint(), b.compute_fingerprint());
}

#[test]
fn analytics_provider_error_messages_are_descriptive() {
    let e = AnalyticsProviderError::SessionNotFound;
    assert!(format!("{e}").contains("Session"));
    let e = AnalyticsProviderError::FingerprintNotFound;
    assert!(format!("{e}").contains("Fingerprint"));
    let e = AnalyticsProviderError::Internal("boom".to_owned());
    assert!(format!("{e}").contains("boom"));
}
