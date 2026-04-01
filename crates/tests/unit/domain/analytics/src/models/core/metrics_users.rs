//! Tests for user metrics, session, and event model types.

use chrono::{Duration, Utc};
use systemprompt_analytics::{AnalyticsEvent, AnalyticsSession, UserMetricsWithTrends};
use systemprompt_identifiers::{SessionId, UserId};

mod user_metrics_with_trends_tests {
    use super::*;

    fn create_metrics(
        count_24h: i64,
        count_7d: i64,
        count_30d: i64,
        prev_24h: i64,
        prev_7d: i64,
        prev_30d: i64,
    ) -> UserMetricsWithTrends {
        UserMetricsWithTrends {
            count_24h,
            count_7d,
            count_30d,
            prev_24h,
            prev_7d,
            prev_30d,
        }
    }

    #[test]
    fn metrics_stores_24h_values() {
        let metrics = create_metrics(100, 500, 2000, 90, 480, 1900);
        assert_eq!(metrics.count_24h, 100);
        assert_eq!(metrics.prev_24h, 90);
    }

    #[test]
    fn metrics_stores_7d_values() {
        let metrics = create_metrics(100, 500, 2000, 90, 480, 1900);
        assert_eq!(metrics.count_7d, 500);
        assert_eq!(metrics.prev_7d, 480);
    }

    #[test]
    fn metrics_stores_30d_values() {
        let metrics = create_metrics(100, 500, 2000, 90, 480, 1900);
        assert_eq!(metrics.count_30d, 2000);
        assert_eq!(metrics.prev_30d, 1900);
    }

    #[test]
    fn metrics_is_copy() {
        let metrics = create_metrics(10, 50, 200, 8, 45, 180);
        let copied = metrics;
        assert_eq!(metrics.count_24h, copied.count_24h);
    }

    #[test]
    fn metrics_is_clone() {
        let metrics = create_metrics(10, 50, 200, 8, 45, 180);
        let cloned = metrics.clone();
        assert_eq!(metrics.count_7d, cloned.count_7d);
    }

    #[test]
    fn metrics_is_debug() {
        let metrics = create_metrics(1, 2, 3, 0, 1, 2);
        let debug_str = format!("{:?}", metrics);
        assert!(debug_str.contains("UserMetricsWithTrends"));
    }

    #[test]
    fn metrics_serializes_with_renamed_fields() {
        let metrics = create_metrics(100, 500, 2000, 90, 480, 1900);
        let json = serde_json::to_string(&metrics).unwrap();

        assert!(json.contains("users_24h"));
        assert!(json.contains("users_7d"));
        assert!(json.contains("users_30d"));
        assert!(json.contains("users_prev_24h"));
    }

    #[test]
    fn metrics_deserializes() {
        let json = r#"{
            "users_24h": 150,
            "users_7d": 700,
            "users_30d": 2500,
            "users_prev_24h": 140,
            "users_prev_7d": 680,
            "users_prev_30d": 2400
        }"#;

        let metrics: UserMetricsWithTrends = serde_json::from_str(json).unwrap();

        assert_eq!(metrics.count_24h, 150);
        assert_eq!(metrics.count_7d, 700);
        assert_eq!(metrics.prev_30d, 2400);
    }
}

mod analytics_session_tests {
    use super::*;

    fn create_session() -> AnalyticsSession {
        let now = Utc::now();
        AnalyticsSession {
            session_id: SessionId::new("sess_123".to_string()),
            user_id: Some(UserId::new("user_456".to_string())),
            fingerprint_hash: Some("fp_abc".to_string()),
            ip_address: Some("192.168.1.1".to_string()),
            user_agent: Some("Mozilla/5.0 Chrome/120".to_string()),
            device_type: Some("desktop".to_string()),
            browser: Some("Chrome".to_string()),
            os: Some("Windows".to_string()),
            country: Some("US".to_string()),
            city: Some("New York".to_string()),
            referrer_url: Some("https://google.com".to_string()),
            utm_source: Some("google".to_string()),
            utm_medium: Some("cpc".to_string()),
            utm_campaign: Some("summer_sale".to_string()),
            is_bot: false,
            is_scanner: Some(false),
            is_behavioral_bot: Some(false),
            behavioral_bot_reason: None,
            started_at: Some(now - Duration::hours(1)),
            last_activity_at: Some(now),
            ended_at: None,
            request_count: Some(25),
            task_count: Some(5),
            ai_request_count: Some(10),
            message_count: Some(15),
        }
    }

    #[test]
    fn session_stores_session_id() {
        let session = create_session();
        assert_eq!(session.session_id.as_str(), "sess_123");
    }

    #[test]
    fn session_stores_user_id() {
        let session = create_session();
        assert!(session.user_id.is_some());
        assert_eq!(session.user_id.unwrap().as_str(), "user_456");
    }

    #[test]
    fn session_stores_fingerprint() {
        let session = create_session();
        assert_eq!(session.fingerprint_hash, Some("fp_abc".to_string()));
    }

    #[test]
    fn session_stores_device_info() {
        let session = create_session();
        assert_eq!(session.device_type, Some("desktop".to_string()));
        assert_eq!(session.browser, Some("Chrome".to_string()));
        assert_eq!(session.os, Some("Windows".to_string()));
    }

    #[test]
    fn session_stores_location() {
        let session = create_session();
        assert_eq!(session.country, Some("US".to_string()));
        assert_eq!(session.city, Some("New York".to_string()));
    }

    #[test]
    fn session_stores_utm_params() {
        let session = create_session();
        assert_eq!(session.utm_source, Some("google".to_string()));
        assert_eq!(session.utm_medium, Some("cpc".to_string()));
        assert_eq!(session.utm_campaign, Some("summer_sale".to_string()));
    }

    #[test]
    fn session_stores_bot_flags() {
        let session = create_session();
        assert!(!session.is_bot);
        assert_eq!(session.is_scanner, Some(false));
        assert_eq!(session.is_behavioral_bot, Some(false));
    }

    #[test]
    fn session_stores_activity_counts() {
        let session = create_session();
        assert_eq!(session.request_count, Some(25));
        assert_eq!(session.task_count, Some(5));
        assert_eq!(session.ai_request_count, Some(10));
        assert_eq!(session.message_count, Some(15));
    }

    #[test]
    fn session_is_clone() {
        let session = create_session();
        let cloned = session.clone();

        assert_eq!(session.session_id.as_str(), cloned.session_id.as_str());
        assert_eq!(session.browser, cloned.browser);
    }

    #[test]
    fn session_is_debug() {
        let session = create_session();
        let debug_str = format!("{:?}", session);
        assert!(debug_str.contains("AnalyticsSession"));
    }

    #[test]
    fn session_serializes() {
        let session = create_session();
        let json = serde_json::to_string(&session).unwrap();

        assert!(json.contains("sess_123"));
        assert!(json.contains("Chrome"));
        assert!(json.contains("google"));
    }

    #[test]
    fn session_with_minimal_data() {
        let session = AnalyticsSession {
            session_id: SessionId::new("sess_min".to_string()),
            user_id: None,
            fingerprint_hash: None,
            ip_address: None,
            user_agent: None,
            device_type: None,
            browser: None,
            os: None,
            country: None,
            city: None,
            referrer_url: None,
            utm_source: None,
            utm_medium: None,
            utm_campaign: None,
            is_bot: true,
            is_scanner: None,
            is_behavioral_bot: None,
            behavioral_bot_reason: None,
            started_at: None,
            last_activity_at: None,
            ended_at: None,
            request_count: None,
            task_count: None,
            ai_request_count: None,
            message_count: None,
        };

        assert_eq!(session.session_id.as_str(), "sess_min");
        assert!(session.user_id.is_none());
        assert!(session.is_bot);
    }
}

mod analytics_event_tests {
    use super::*;

    fn create_event() -> AnalyticsEvent {
        AnalyticsEvent {
            id: "evt_123".to_string(),
            event_type: "page_view".to_string(),
            event_category: "navigation".to_string(),
            severity: "info".to_string(),
            user_id: UserId::new("user_456".to_string()),
            session_id: Some(SessionId::new("sess_789".to_string())),
            message: Some("User viewed homepage".to_string()),
            metadata: Some(r#"{"page": "/home"}"#.to_string()),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn event_stores_id() {
        let event = create_event();
        assert_eq!(event.id, "evt_123");
    }

    #[test]
    fn event_stores_event_type() {
        let event = create_event();
        assert_eq!(event.event_type, "page_view");
    }

    #[test]
    fn event_stores_category() {
        let event = create_event();
        assert_eq!(event.event_category, "navigation");
    }

    #[test]
    fn event_stores_severity() {
        let event = create_event();
        assert_eq!(event.severity, "info");
    }

    #[test]
    fn event_stores_user_id() {
        let event = create_event();
        assert_eq!(event.user_id.as_str(), "user_456");
    }

    #[test]
    fn event_stores_session_id() {
        let event = create_event();
        assert!(event.session_id.is_some());
        assert_eq!(event.session_id.unwrap().as_str(), "sess_789");
    }

    #[test]
    fn event_stores_message() {
        let event = create_event();
        assert_eq!(event.message, Some("User viewed homepage".to_string()));
    }

    #[test]
    fn event_stores_metadata() {
        let event = create_event();
        assert!(event.metadata.is_some());
        assert!(event.metadata.unwrap().contains("page"));
    }

    #[test]
    fn event_is_clone() {
        let event = create_event();
        let cloned = event.clone();

        assert_eq!(event.id, cloned.id);
        assert_eq!(event.event_type, cloned.event_type);
    }

    #[test]
    fn event_is_debug() {
        let event = create_event();
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("AnalyticsEvent"));
    }

    #[test]
    fn event_serializes() {
        let event = create_event();
        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("evt_123"));
        assert!(json.contains("page_view"));
    }
}
