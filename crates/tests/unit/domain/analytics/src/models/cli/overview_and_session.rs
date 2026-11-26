//! Tests for session rows.

use chrono::Utc;
use systemprompt_analytics::models::{LiveSessionRow, SessionStatsRow, SessionTrendRow};
use systemprompt_identifiers::SessionId;
use systemprompt_test_fixtures::unique_user_id;

mod session_row_tests {
    use super::*;

    #[test]
    fn session_stats_row_stores_values() {
        let row = SessionStatsRow {
            total_sessions: 10000,
            unique_users: 5000,
            avg_duration: Some(300.5),
            avg_requests: Some(15.2),
            conversions: 500,
        };

        assert_eq!(row.total_sessions, 10000);
        assert_eq!(row.unique_users, 5000);
        assert!((row.avg_duration.unwrap() - 300.5).abs() < f64::EPSILON);
        assert!((row.avg_requests.unwrap() - 15.2).abs() < f64::EPSILON);
        assert_eq!(row.conversions, 500);
    }

    #[test]
    fn session_stats_row_handles_none() {
        let row = SessionStatsRow {
            total_sessions: 50,
            unique_users: 25,
            avg_duration: None,
            avg_requests: None,
            conversions: 5,
        };

        assert!(row.avg_duration.is_none());
        assert!(row.avg_requests.is_none());
    }

    #[test]
    fn live_session_row_stores_values() {
        let now = Utc::now();
        let row = LiveSessionRow {
            session_id: SessionId::new("sess_live".to_string()),
            user_type: Some("authenticated".to_string()),
            started_at: now,
            duration_seconds: Some(300),
            request_count: Some(25),
            last_activity_at: now,
        };

        assert_eq!(row.session_id.as_str(), "sess_live");
        assert_eq!(row.user_type, Some("authenticated".to_string()));
        assert_eq!(row.duration_seconds, Some(300));
        assert_eq!(row.request_count, Some(25));
    }

    #[test]
    fn session_trend_row_stores_values() {
        let now = Utc::now();
        let row = SessionTrendRow {
            started_at: now,
            user_id: Some(unique_user_id("overview-and-session")),
            duration_seconds: Some(180),
        };

        assert_eq!(row.started_at, now);
        row.user_id.as_ref().expect("user_id should be present");
        assert_eq!(row.duration_seconds, Some(180));
    }
}
