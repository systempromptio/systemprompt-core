//! Tests for behavioral bot detector service.

use chrono::{Duration, Utc};
use systemprompt_analytics::{
    BehavioralAnalysisInput, BehavioralBotDetector, BehavioralSignal, SignalType,
    BEHAVIORAL_BOT_THRESHOLD,
};
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
            request_timestamps: vec![],
            has_javascript_events: true,
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
    fn input_is_clone() {
        let input = create_input(10, vec!["/page1".to_string()], 100, 1);
        let cloned = input.clone();
        assert_eq!(input.session_id, cloned.session_id);
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
        assert_eq!(format!("{}", SignalType::HighRequestCount), "high_request_count");
    }

    #[test]
    fn signal_type_display_high_page_coverage() {
        assert_eq!(format!("{}", SignalType::HighPageCoverage), "high_page_coverage");
    }

    #[test]
    fn signal_type_display_sequential_navigation() {
        assert_eq!(format!("{}", SignalType::SequentialNavigation), "sequential_navigation");
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
        assert_eq!(format!("{}", SignalType::HighPagesPerMinute), "high_pages_per_minute");
    }

    #[test]
    fn signal_type_display_outdated_browser() {
        assert_eq!(format!("{}", SignalType::OutdatedBrowser), "outdated_browser");
    }

    #[test]
    fn signal_type_is_eq() {
        assert_eq!(SignalType::HighRequestCount, SignalType::HighRequestCount);
        assert_ne!(SignalType::HighRequestCount, SignalType::HighPageCoverage);
    }

    #[test]
    fn signal_type_is_copy() {
        let signal = SignalType::RegularTiming;
        let copied = signal;
        assert_eq!(signal, copied);
    }

    #[test]
    fn signal_type_serializes() {
        let signal = SignalType::HighRequestCount;
        let json = serde_json::to_string(&signal).unwrap();
        assert!(json.contains("HighRequestCount"));
    }

    #[test]
    fn signal_type_deserializes() {
        let json = "\"HighPageCoverage\"";
        let signal: SignalType = serde_json::from_str(json).unwrap();
        assert_eq!(signal, SignalType::HighPageCoverage);
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
        let signal = create_signal(SignalType::HighRequestCount, 30, "Request count 100 exceeds 50");
        assert!(signal.details.contains("100"));
    }

    #[test]
    fn signal_is_clone() {
        let signal = create_signal(SignalType::HighPageCoverage, 25, "High coverage");
        let cloned = signal.clone();
        assert_eq!(signal.signal_type, cloned.signal_type);
        assert_eq!(signal.points, cloned.points);
    }

    #[test]
    fn signal_serializes() {
        let signal = create_signal(SignalType::RegularTiming, 15, "Suspicious timing");
        let json = serde_json::to_string(&signal).unwrap();
        assert!(json.contains("RegularTiming"));
        assert!(json.contains("15"));
        assert!(json.contains("Suspicious timing"));
    }

    #[test]
    fn signal_deserializes() {
        let json = r#"{"signal_type":"HighRequestCount","points":30,"details":"test"}"#;
        let signal: BehavioralSignal = serde_json::from_str(json).unwrap();
        assert_eq!(signal.signal_type, SignalType::HighRequestCount);
        assert_eq!(signal.points, 30);
    }
}

mod behavioral_bot_detector_tests {
    use super::*;

    fn create_input(
        request_count: i64,
        endpoints: Vec<String>,
        total_pages: i64,
        fingerprint_sessions: i64,
        duration_minutes: i64,
    ) -> BehavioralAnalysisInput {
        let now = Utc::now();
        BehavioralAnalysisInput {
            session_id: SessionId::new("sess_123".to_string()),
            fingerprint_hash: Some("fp_hash".to_string()),
            user_agent: Some("Mozilla/5.0 Chrome/120.0".to_string()),
            request_count,
            started_at: now - Duration::minutes(duration_minutes),
            last_activity_at: now,
            endpoints_accessed: endpoints,
            total_site_pages: total_pages,
            fingerprint_session_count: fingerprint_sessions,
            request_timestamps: vec![],
            has_javascript_events: true,
        }
    }

    fn create_input_with_timestamps(
        request_count: i64,
        timestamps: Vec<chrono::DateTime<Utc>>,
    ) -> BehavioralAnalysisInput {
        let now = Utc::now();
        BehavioralAnalysisInput {
            session_id: SessionId::new("sess_123".to_string()),
            fingerprint_hash: Some("fp_hash".to_string()),
            user_agent: Some("Mozilla/5.0 Chrome/120.0".to_string()),
            request_count,
            started_at: now - Duration::minutes(10),
            last_activity_at: now,
            endpoints_accessed: vec![],
            total_site_pages: 100,
            fingerprint_session_count: 1,
            request_timestamps: timestamps,
            has_javascript_events: true,
        }
    }

    fn create_input_with_user_agent(user_agent: Option<String>) -> BehavioralAnalysisInput {
        let now = Utc::now();
        BehavioralAnalysisInput {
            session_id: SessionId::new("sess_123".to_string()),
            fingerprint_hash: Some("fp_hash".to_string()),
            user_agent,
            request_count: 10,
            started_at: now - Duration::minutes(10),
            last_activity_at: now,
            endpoints_accessed: vec![],
            total_site_pages: 100,
            fingerprint_session_count: 1,
            request_timestamps: vec![],
            has_javascript_events: true,
        }
    }

    #[test]
    fn new_creates_detector() {
        let detector = BehavioralBotDetector::new();
        let _ = format!("{:?}", detector);
    }

    #[test]
    fn detector_is_default() {
        let detector = BehavioralBotDetector::default();
        let _ = format!("{:?}", detector);
    }

    #[test]
    fn detector_is_copy() {
        let detector = BehavioralBotDetector::new();
        let copied = detector;
        let _ = format!("{:?}", copied);
    }

    #[test]
    fn analyze_normal_traffic_returns_low_score() {
        let input = create_input(10, vec!["/page1".to_string()], 100, 1, 10);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(!result.is_suspicious);
        assert!(result.score < BEHAVIORAL_BOT_THRESHOLD);
        assert!(result.reason.is_none());
    }

    #[test]
    fn analyze_high_request_count_adds_signal() {
        let input = create_input(100, vec![], 100, 1, 10);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(result.signals.iter().any(|s| s.signal_type == SignalType::HighRequestCount));
    }

    #[test]
    fn analyze_high_page_coverage_adds_signal() {
        let pages: Vec<String> = (0..70).map(|i| format!("/page{}", i)).collect();
        let input = create_input(70, pages, 100, 1, 30);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(result.signals.iter().any(|s| s.signal_type == SignalType::HighPageCoverage));
    }

    #[test]
    fn analyze_multiple_fingerprint_sessions_adds_signal() {
        let input = create_input(10, vec![], 100, 10, 10);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(result
            .signals
            .iter()
            .any(|s| s.signal_type == SignalType::MultipleFingerPrintSessions));
    }

    #[test]
    fn analyze_high_pages_per_minute_adds_signal() {
        // 30 pages in 1 minute = 30 pages/min (threshold is 5)
        let pages: Vec<String> = (0..30).map(|i| format!("/page{}", i)).collect();
        let input = create_input(30, pages, 100, 1, 1);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(result.signals.iter().any(|s| s.signal_type == SignalType::HighPagesPerMinute));
    }

    #[test]
    fn analyze_outdated_chrome_adds_signal() {
        let input = create_input_with_user_agent(Some("Mozilla/5.0 Chrome/80.0".to_string()));
        let result = BehavioralBotDetector::analyze(&input);

        assert!(result.signals.iter().any(|s| s.signal_type == SignalType::OutdatedBrowser));
    }

    #[test]
    fn analyze_outdated_firefox_adds_signal() {
        let input = create_input_with_user_agent(Some("Mozilla/5.0 Firefox/80".to_string()));
        let result = BehavioralBotDetector::analyze(&input);

        assert!(result.signals.iter().any(|s| s.signal_type == SignalType::OutdatedBrowser));
    }

    #[test]
    fn analyze_modern_chrome_no_outdated_signal() {
        let input = create_input_with_user_agent(Some("Mozilla/5.0 Chrome/120.0".to_string()));
        let result = BehavioralBotDetector::analyze(&input);

        assert!(!result.signals.iter().any(|s| s.signal_type == SignalType::OutdatedBrowser));
    }

    #[test]
    fn analyze_regular_timing_adds_signal() {
        let now = Utc::now();
        // Create timestamps with very regular intervals (exactly 1 second apart)
        let timestamps: Vec<chrono::DateTime<Utc>> = (0..10)
            .map(|i| now - Duration::seconds((10 - i) * 1000))
            .collect();
        let input = create_input_with_timestamps(10, timestamps);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(result.signals.iter().any(|s| s.signal_type == SignalType::RegularTiming));
    }

    #[test]
    fn analyze_bot_pattern_is_suspicious() {
        // Create a pattern that should trigger multiple signals
        let pages: Vec<String> = (0..70).map(|i| format!("/page{}", i)).collect();
        let input = create_input(100, pages, 100, 10, 5);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(result.is_suspicious);
        assert!(result.score >= BEHAVIORAL_BOT_THRESHOLD);
        assert!(result.reason.is_some());
    }

    #[test]
    fn analyze_returns_reason_when_suspicious() {
        let pages: Vec<String> = (0..70).map(|i| format!("/page{}", i)).collect();
        let input = create_input(100, pages, 100, 10, 5);
        let result = BehavioralBotDetector::analyze(&input);

        if result.is_suspicious {
            let reason = result.reason.unwrap();
            // Reason should contain signal type names
            assert!(!reason.is_empty());
        }
    }

    #[test]
    fn analyze_sequential_navigation_with_sorted_endpoints() {
        // Create endpoints that are already sorted (sequential crawl pattern)
        let pages: Vec<String> = (0..10).map(|i| format!("/page{:02}", i)).collect();
        let input = create_input(10, pages, 100, 1, 10);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(result.signals.iter().any(|s| s.signal_type == SignalType::SequentialNavigation));
    }

    #[test]
    fn analyze_no_sequential_with_random_endpoints() {
        let pages = vec![
            "/about".to_string(),
            "/home".to_string(),
            "/contact".to_string(),
            "/products".to_string(),
            "/blog".to_string(),
        ];
        let input = create_input(5, pages, 100, 1, 10);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(!result.signals.iter().any(|s| s.signal_type == SignalType::SequentialNavigation));
    }

    #[test]
    fn analyze_result_is_clone() {
        let input = create_input(10, vec![], 100, 1, 10);
        let result = BehavioralBotDetector::analyze(&input);
        let cloned = result.clone();

        assert_eq!(result.score, cloned.score);
        assert_eq!(result.is_suspicious, cloned.is_suspicious);
    }

    #[test]
    fn analyze_result_serializes() {
        let input = create_input(100, vec![], 100, 10, 5);
        let result = BehavioralBotDetector::analyze(&input);
        let json = serde_json::to_string(&result).unwrap();

        assert!(json.contains("score"));
        assert!(json.contains("is_suspicious"));
        assert!(json.contains("signals"));
    }

    #[test]
    fn threshold_constant_is_30() {
        assert_eq!(BEHAVIORAL_BOT_THRESHOLD, 30);
    }

    #[test]
    fn analyze_zero_total_pages_skips_coverage_check() {
        let pages: Vec<String> = (0..10).map(|i| format!("/page{}", i)).collect();
        let input = create_input(10, pages, 0, 1, 10);
        let result = BehavioralBotDetector::analyze(&input);

        // Should not add high page coverage signal when total_site_pages is 0
        assert!(!result.signals.iter().any(|s| s.signal_type == SignalType::HighPageCoverage));
    }

    #[test]
    fn analyze_few_timestamps_skips_timing_check() {
        let now = Utc::now();
        let timestamps = vec![now - Duration::seconds(10), now];
        let input = create_input_with_timestamps(2, timestamps);
        let result = BehavioralBotDetector::analyze(&input);

        // With only 2 timestamps, should skip timing check (needs at least 5)
        assert!(!result.signals.iter().any(|s| s.signal_type == SignalType::RegularTiming));
    }

    #[test]
    fn analyze_zero_duration_skips_pages_per_minute_check() {
        let now = Utc::now();
        let pages: Vec<String> = (0..10).map(|i| format!("/page{}", i)).collect();
        let input = BehavioralAnalysisInput {
            session_id: SessionId::new("sess_123".to_string()),
            fingerprint_hash: Some("fp_hash".to_string()),
            user_agent: Some("Mozilla/5.0 Chrome/120.0".to_string()),
            request_count: 10,
            started_at: now,
            last_activity_at: now,
            endpoints_accessed: pages,
            total_site_pages: 100,
            fingerprint_session_count: 1,
            request_timestamps: vec![],
            has_javascript_events: true,
        };
        let result = BehavioralBotDetector::analyze(&input);

        assert!(!result.signals.iter().any(|s| s.signal_type == SignalType::HighPagesPerMinute));
    }

    #[test]
    fn analyze_no_user_agent_skips_browser_check() {
        let input = create_input_with_user_agent(None);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(!result.signals.iter().any(|s| s.signal_type == SignalType::OutdatedBrowser));
    }

    #[test]
    fn analyze_few_endpoints_skips_sequential_check() {
        let pages = vec!["/page1".to_string(), "/page2".to_string()];
        let input = create_input(2, pages, 100, 1, 10);
        let result = BehavioralBotDetector::analyze(&input);

        // With less than 5 endpoints, should skip sequential check
        assert!(!result.signals.iter().any(|s| s.signal_type == SignalType::SequentialNavigation));
    }
}
