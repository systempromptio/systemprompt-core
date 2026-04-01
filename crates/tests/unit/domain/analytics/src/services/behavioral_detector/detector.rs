//! Tests for BehavioralBotDetector::analyze.

use chrono::{Duration, Utc};
use systemprompt_analytics::{
    BehavioralAnalysisInput, BehavioralBotDetector, SignalType, BEHAVIORAL_BOT_THRESHOLD,
};
use systemprompt_identifiers::SessionId;

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
            landing_page: Some("/".to_string()),
            entry_url: Some("/".to_string()),
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
            landing_page: Some("/".to_string()),
            entry_url: Some("/".to_string()),
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
            landing_page: Some("/".to_string()),
            entry_url: Some("/".to_string()),
        }
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
        let timestamps: Vec<chrono::DateTime<Utc>> = (0..10)
            .map(|i| now - Duration::seconds((10 - i) * 1000))
            .collect();
        let input = create_input_with_timestamps(10, timestamps);
        let result = BehavioralBotDetector::analyze(&input);

        assert!(result.signals.iter().any(|s| s.signal_type == SignalType::RegularTiming));
    }

    #[test]
    fn analyze_bot_pattern_is_suspicious() {
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
            assert!(!reason.is_empty());
        }
    }

    #[test]
    fn analyze_sequential_navigation_with_sorted_endpoints() {
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

        assert!(!result.signals.iter().any(|s| s.signal_type == SignalType::HighPageCoverage));
    }

    #[test]
    fn analyze_few_timestamps_skips_timing_check() {
        let now = Utc::now();
        let timestamps = vec![now - Duration::seconds(10), now];
        let input = create_input_with_timestamps(2, timestamps);
        let result = BehavioralBotDetector::analyze(&input);

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
            landing_page: Some("/".to_string()),
            entry_url: Some("/".to_string()),
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

        assert!(!result.signals.iter().any(|s| s.signal_type == SignalType::SequentialNavigation));
    }
}
