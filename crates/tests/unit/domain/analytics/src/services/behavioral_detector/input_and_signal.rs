//! Tests for BehavioralAnalysisInput, SignalType, and BehavioralSignal.

use chrono::{Duration, Utc};
use systemprompt_analytics::{BehavioralAnalysisInput, BehavioralSignal, SignalType};
use systemprompt_identifiers::SessionId;

mod behavioral_analysis_input_tests {
    use super::*;

    fn create_input(
        request_count: i64,
        endpoints: Vec<String>,
        total_pages: i64,
        fingerprint_sessions: i64,
    ) -> BehavioralAnalysisInput {
        let now = Utc::now();
        BehavioralAnalysisInput {
            session_id: SessionId::new("sess_123".to_string()),
            fingerprint_hash: Some("fp_hash".to_string()),
            user_agent: Some("Mozilla/5.0 Chrome/120.0".to_string()),
            request_count,
            started_at: now - Duration::minutes(10),
            last_activity_at: now,
            endpoints_accessed: endpoints,
            total_site_pages: total_pages,
            fingerprint_session_count: fingerprint_sessions,
            fingerprint_unique_ip_count: 1,
            fingerprint_engagement_event_count: 1,
            fingerprint_session_starts: vec![],
            request_timestamps: vec![],
            has_javascript_events: true,
            landing_page: Some("/".to_string()),
            entry_url: Some("/".to_string()),
        }
    }

    #[test]
    fn input_stores_session_id() {
        let input = create_input(10, vec![], 100, 1);
        assert_eq!(input.session_id.as_str(), "sess_123");
    }

    #[test]
    fn input_stores_fingerprint_hash() {
        let input = create_input(10, vec![], 100, 1);
        assert_eq!(input.fingerprint_hash, Some("fp_hash".to_string()));
    }

    #[test]
    fn input_stores_user_agent() {
        let input = create_input(10, vec![], 100, 1);
        assert!(input.user_agent.as_ref().unwrap().contains("Chrome"));
    }

    #[test]
    fn input_is_debug() {
        let input = create_input(10, vec![], 100, 1);
        let debug_str = format!("{:?}", input);
        assert!(debug_str.contains("BehavioralAnalysisInput"));
    }
}

mod signal_type_tests {
    use super::*;

    #[test]
    fn signal_type_display_high_request_count() {
        assert_eq!(
            format!("{}", SignalType::HighRequestCount),
            "high_request_count"
        );
    }

    #[test]
    fn signal_type_display_high_page_coverage() {
        assert_eq!(
            format!("{}", SignalType::HighPageCoverage),
            "high_page_coverage"
        );
    }

    #[test]
    fn signal_type_display_sequential_navigation() {
        assert_eq!(
            format!("{}", SignalType::SequentialNavigation),
            "sequential_navigation"
        );
    }

    #[test]
    fn signal_type_display_multiple_fingerprint_sessions() {
        assert_eq!(
            format!("{}", SignalType::MultipleFingerPrintSessions),
            "multiple_fingerprint_sessions"
        );
    }

    #[test]
    fn signal_type_display_regular_timing() {
        assert_eq!(format!("{}", SignalType::RegularTiming), "regular_timing");
    }

    #[test]
    fn signal_type_display_high_pages_per_minute() {
        assert_eq!(
            format!("{}", SignalType::HighPagesPerMinute),
            "high_pages_per_minute"
        );
    }

    #[test]
    fn signal_type_display_outdated_browser() {
        assert_eq!(
            format!("{}", SignalType::OutdatedBrowser),
            "outdated_browser"
        );
    }

    #[test]
    fn signal_type_serializes() {
        let signal = SignalType::HighRequestCount;
        let json = serde_json::to_string(&signal).unwrap();
        assert!(json.contains("HighRequestCount"));
    }
}

mod behavioral_signal_tests {
    use super::*;

    fn create_signal(signal_type: SignalType, points: i32, details: &str) -> BehavioralSignal {
        BehavioralSignal {
            signal_type,
            points,
            details: details.to_string(),
        }
    }

    #[test]
    fn signal_stores_type() {
        let signal = create_signal(SignalType::HighRequestCount, 30, "High requests");
        assert_eq!(signal.signal_type, SignalType::HighRequestCount);
    }

    #[test]
    fn signal_stores_points() {
        let signal = create_signal(SignalType::HighRequestCount, 30, "High requests");
        assert_eq!(signal.points, 30);
    }

    #[test]
    fn signal_stores_details() {
        let signal = create_signal(
            SignalType::HighRequestCount,
            30,
            "Request count 100 exceeds 50",
        );
        assert!(signal.details.contains("100"));
    }

    #[test]
    fn signal_serializes() {
        let signal = create_signal(SignalType::RegularTiming, 15, "Suspicious timing");
        let json = serde_json::to_string(&signal).unwrap();
        assert!(json.contains("RegularTiming"));
        assert!(json.contains("15"));
        assert!(json.contains("Suspicious timing"));
    }
}
