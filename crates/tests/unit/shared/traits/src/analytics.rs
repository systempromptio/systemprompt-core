use systemprompt_traits::analytics::{AnalyticsProviderError, SessionAnalytics};

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
        preferred_locale: Some("en-US".to_owned()),
        ..Default::default()
    };
    let b = a.clone();
    let fp_a = a.compute_fingerprint();
    let fp_b = b.compute_fingerprint();
    assert_eq!(fp_a, fp_b);
    assert!(fp_a.starts_with("fp_"));
}

#[test]
fn compute_fingerprint_differs_with_preferred_locale() {
    let a = SessionAnalytics {
        user_agent: Some("UA".to_owned()),
        preferred_locale: Some("fr-FR".to_owned()),
        ..Default::default()
    };
    let b = SessionAnalytics {
        user_agent: Some("UA".to_owned()),
        preferred_locale: Some("en-US".to_owned()),
        ..Default::default()
    };
    assert_ne!(a.compute_fingerprint(), b.compute_fingerprint());
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
