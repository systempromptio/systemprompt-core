//! Targeted tests for ghost-session and no-JavaScript-events checks,
//! exercising the branches that were not directly covered elsewhere.

use chrono::{Duration, Utc};
use systemprompt_analytics::{BehavioralAnalysisInput, BehavioralBotDetector, SignalType};
use systemprompt_identifiers::SessionId;

fn base_input() -> BehavioralAnalysisInput {
    let now = Utc::now();
    BehavioralAnalysisInput {
        session_id: SessionId::new("ghost_sess".to_string()),
        fingerprint_hash: Some("fp".to_string()),
        user_agent: Some("Mozilla/5.0 Chrome/121.0".to_string()),
        request_count: 0,
        started_at: now - Duration::minutes(5),
        last_activity_at: now,
        endpoints_accessed: vec![],
        total_site_pages: 100,
        fingerprint_session_count: 1,
        fingerprint_unique_ip_count: 1,
        fingerprint_engagement_event_count: 0,
        fingerprint_session_starts: vec![],
        request_timestamps: vec![],
        has_javascript_events: true,
        landing_page: Some("/".to_string()),
        entry_url: Some("/".to_string()),
    }
}

mod ghost_session_tests {
    use super::*;

    #[test]
    fn ghost_session_flags_when_no_landing_entry_and_old() {
        let now = Utc::now();
        let mut input = base_input();
        input.landing_page = None;
        input.entry_url = None;
        input.request_count = 0;
        input.started_at = now - Duration::seconds(60);
        input.last_activity_at = now;

        let result = BehavioralBotDetector::analyze(&input);

        assert!(
            result
                .signals
                .iter()
                .any(|s| s.signal_type == SignalType::GhostSession),
            "Expected GhostSession signal; got {:?}",
            result.signals
        );
    }

    #[test]
    fn ghost_session_not_flagged_when_has_landing_page() {
        let now = Utc::now();
        let mut input = base_input();
        input.landing_page = Some("/home".to_string());
        input.entry_url = None;
        input.request_count = 0;
        input.started_at = now - Duration::seconds(60);
        input.last_activity_at = now;

        let result = BehavioralBotDetector::analyze(&input);

        assert!(
            !result
                .signals
                .iter()
                .any(|s| s.signal_type == SignalType::GhostSession)
        );
    }

    #[test]
    fn ghost_session_not_flagged_when_has_entry_url() {
        let now = Utc::now();
        let mut input = base_input();
        input.landing_page = None;
        input.entry_url = Some("/docs".to_string());
        input.request_count = 0;
        input.started_at = now - Duration::seconds(60);
        input.last_activity_at = now;

        let result = BehavioralBotDetector::analyze(&input);

        assert!(
            !result
                .signals
                .iter()
                .any(|s| s.signal_type == SignalType::GhostSession)
        );
    }

    #[test]
    fn ghost_session_not_flagged_when_session_too_young() {
        let now = Utc::now();
        let mut input = base_input();
        input.landing_page = None;
        input.entry_url = None;
        input.request_count = 0;
        input.started_at = now - Duration::seconds(5);
        input.last_activity_at = now;

        let result = BehavioralBotDetector::analyze(&input);

        assert!(
            !result
                .signals
                .iter()
                .any(|s| s.signal_type == SignalType::GhostSession)
        );
    }

    #[test]
    fn ghost_session_not_flagged_when_has_requests() {
        let now = Utc::now();
        let mut input = base_input();
        input.landing_page = None;
        input.entry_url = None;
        input.request_count = 1;
        input.started_at = now - Duration::seconds(60);
        input.last_activity_at = now;

        let result = BehavioralBotDetector::analyze(&input);

        assert!(
            !result
                .signals
                .iter()
                .any(|s| s.signal_type == SignalType::GhostSession)
        );
    }
}

mod no_javascript_events_tests {
    use super::*;

    #[test]
    fn no_js_events_flagged_when_requests_present_and_no_js() {
        let mut input = base_input();
        input.request_count = 5;
        input.has_javascript_events = false;

        let result = BehavioralBotDetector::analyze(&input);

        assert!(
            result
                .signals
                .iter()
                .any(|s| s.signal_type == SignalType::NoJavaScriptEvents),
            "Expected NoJavaScriptEvents signal"
        );
    }

    #[test]
    fn no_js_events_not_flagged_when_js_present() {
        let mut input = base_input();
        input.request_count = 5;
        input.has_javascript_events = true;

        let result = BehavioralBotDetector::analyze(&input);

        assert!(
            !result
                .signals
                .iter()
                .any(|s| s.signal_type == SignalType::NoJavaScriptEvents)
        );
    }

    #[test]
    fn no_js_events_not_flagged_when_zero_requests() {
        let mut input = base_input();
        input.request_count = 0;
        input.has_javascript_events = false;

        let result = BehavioralBotDetector::analyze(&input);

        assert!(
            !result
                .signals
                .iter()
                .any(|s| s.signal_type == SignalType::NoJavaScriptEvents)
        );
    }

    #[test]
    fn no_js_events_not_flagged_when_one_request_no_js() {
        let mut input = base_input();
        input.request_count = 1;
        input.has_javascript_events = false;

        let result = BehavioralBotDetector::analyze(&input);

        assert!(
            !result
                .signals
                .iter()
                .any(|s| s.signal_type == SignalType::NoJavaScriptEvents)
        );
    }

    #[test]
    fn no_js_events_flagged_when_exactly_2_requests() {
        let mut input = base_input();
        input.request_count = 2;
        input.has_javascript_events = false;

        let result = BehavioralBotDetector::analyze(&input);

        assert!(
            result
                .signals
                .iter()
                .any(|s| s.signal_type == SignalType::NoJavaScriptEvents)
        );
    }
}

mod error_helper_tests {
    use systemprompt_analytics::AnalyticsError;

    #[test]
    fn missing_field_constructs_error() {
        let err = AnalyticsError::missing_field("session_id");
        assert!(format!("{}", err).contains("Missing field"));
        assert!(format!("{}", err).contains("session_id"));
    }

    #[test]
    fn invalid_argument_constructs_error() {
        let err = AnalyticsError::invalid_argument("bad value");
        assert!(format!("{}", err).contains("Invalid argument"));
        assert!(format!("{}", err).contains("bad value"));
    }

    #[test]
    fn missing_field_error_is_debug() {
        let err = AnalyticsError::missing_field("fingerprint");
        let dbg = format!("{:?}", err);
        assert!(dbg.contains("MissingField"));
    }

    #[test]
    fn invalid_argument_error_is_debug() {
        let err = AnalyticsError::invalid_argument("out of range");
        let dbg = format!("{:?}", err);
        assert!(dbg.contains("InvalidArgument"));
    }
}
