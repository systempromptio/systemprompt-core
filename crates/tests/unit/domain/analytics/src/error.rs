//! Tests for analytics error types.

use systemprompt_analytics::AnalyticsError;

mod analytics_error_tests {
    use super::*;

    #[test]
    fn session_expired_displays_message() {
        let err = AnalyticsError::SessionExpired;
        let display = format!("{}", err);

        assert!(display.contains("Session expired"));
    }
}
