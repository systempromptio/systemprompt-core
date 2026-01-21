//! Tests for analytics error types.

use systemprompt_analytics::AnalyticsError;

mod analytics_error_tests {
    use super::*;

    #[test]
    fn session_not_found_displays_session_id() {
        let err = AnalyticsError::SessionNotFound("sess_123".to_string());
        let display = format!("{}", err);

        assert!(display.contains("Session not found"));
        assert!(display.contains("sess_123"));
    }

    #[test]
    fn invalid_fingerprint_displays_hash() {
        let err = AnalyticsError::InvalidFingerprint("invalid_hash".to_string());
        let display = format!("{}", err);

        assert!(display.contains("Invalid fingerprint hash"));
        assert!(display.contains("invalid_hash"));
    }

    #[test]
    fn session_expired_displays_message() {
        let err = AnalyticsError::SessionExpired;
        let display = format!("{}", err);

        assert!(display.contains("Session expired"));
    }

    #[test]
    fn throttle_level_exceeded_displays_message() {
        let err = AnalyticsError::ThrottleLevelExceeded;
        let display = format!("{}", err);

        assert!(display.contains("Throttle level exceeded"));
    }

    #[test]
    fn behavioral_bot_detected_displays_reason() {
        let err = AnalyticsError::BehavioralBotDetected("high_request_count".to_string());
        let display = format!("{}", err);

        assert!(display.contains("Behavioral bot detected"));
        assert!(display.contains("high_request_count"));
    }

    #[test]
    fn anomaly_detection_failed_displays_reason() {
        let err = AnalyticsError::AnomalyDetectionFailed("Threshold exceeded".to_string());
        let display = format!("{}", err);

        assert!(display.contains("Anomaly detection failed"));
        assert!(display.contains("Threshold exceeded"));
    }

    #[test]
    fn analytics_error_is_std_error() {
        let err = AnalyticsError::SessionExpired;
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn analytics_error_is_debug() {
        let err = AnalyticsError::SessionNotFound("test".to_string());
        let debug_str = format!("{:?}", err);

        assert!(debug_str.contains("SessionNotFound"));
        assert!(debug_str.contains("test"));
    }
}
