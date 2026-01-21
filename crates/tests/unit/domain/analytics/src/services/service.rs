//! Tests for AnalyticsService static methods and related types.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_analytics::{AnalyticsService, SessionAnalytics};

mod analytics_service_tests {
    use super::*;

    fn create_headers_with_user_agent(ua: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_str(ua).unwrap());
        headers
    }

    fn create_headers_with_ip(ip: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_str(ip).unwrap());
        headers
    }

    #[test]
    fn is_bot_returns_true_for_bot_user_agent() {
        let headers = create_headers_with_user_agent("Googlebot/2.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(AnalyticsService::is_bot(&analytics));
    }

    #[test]
    fn is_bot_returns_true_for_bot_ip() {
        let mut headers = create_headers_with_ip("66.249.64.1");
        headers.insert(
            "user-agent",
            HeaderValue::from_static("Mozilla/5.0 (Windows) Chrome/120.0"),
        );
        let analytics = SessionAnalytics::from_headers(&headers);

        // User agent is legitimate Chrome, but IP is Google bot IP
        assert!(AnalyticsService::is_bot(&analytics));
    }

    #[test]
    fn is_bot_returns_false_for_regular_user() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "user-agent",
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64) AppleWebKit/537.36 Chrome/120.0",
            ),
        );
        headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(!AnalyticsService::is_bot(&analytics));
    }

    #[test]
    fn is_bot_returns_false_for_empty_analytics() {
        let headers = HeaderMap::new();
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(!AnalyticsService::is_bot(&analytics));
    }

    #[test]
    fn compute_fingerprint_uses_provided_hash() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "user-agent",
            HeaderValue::from_static("Mozilla/5.0 Chrome/120"),
        );
        headers.insert("x-fingerprint", HeaderValue::from_static("custom_hash_123"));
        headers.insert("accept-language", HeaderValue::from_static("en-US"));
        let analytics = SessionAnalytics::from_headers(&headers);

        let fingerprint = AnalyticsService::compute_fingerprint(&analytics);

        assert_eq!(fingerprint, "custom_hash_123");
    }

    #[test]
    fn compute_fingerprint_generates_hash_from_user_agent() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "user-agent",
            HeaderValue::from_static("Mozilla/5.0 Chrome/120"),
        );
        let analytics = SessionAnalytics::from_headers(&headers);

        let fingerprint = AnalyticsService::compute_fingerprint(&analytics);

        // Should be a hex string
        assert!(!fingerprint.is_empty());
        assert!(fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn compute_fingerprint_generates_hash_from_user_agent_and_locale() {
        let mut headers1 = HeaderMap::new();
        headers1.insert(
            "user-agent",
            HeaderValue::from_static("Mozilla/5.0 Chrome/120"),
        );
        headers1.insert("accept-language", HeaderValue::from_static("en-US"));
        let analytics1 = SessionAnalytics::from_headers(&headers1);

        let mut headers2 = HeaderMap::new();
        headers2.insert(
            "user-agent",
            HeaderValue::from_static("Mozilla/5.0 Chrome/120"),
        );
        headers2.insert("accept-language", HeaderValue::from_static("fr-FR"));
        let analytics2 = SessionAnalytics::from_headers(&headers2);

        let fp1 = AnalyticsService::compute_fingerprint(&analytics1);
        let fp2 = AnalyticsService::compute_fingerprint(&analytics2);

        // Different locales should produce different fingerprints
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn compute_fingerprint_same_headers_produce_same_hash() {
        let mut headers1 = HeaderMap::new();
        headers1.insert(
            "user-agent",
            HeaderValue::from_static("Mozilla/5.0 Firefox/121"),
        );
        headers1.insert("accept-language", HeaderValue::from_static("de-DE"));
        let analytics1 = SessionAnalytics::from_headers(&headers1);

        let mut headers2 = HeaderMap::new();
        headers2.insert(
            "user-agent",
            HeaderValue::from_static("Mozilla/5.0 Firefox/121"),
        );
        headers2.insert("accept-language", HeaderValue::from_static("de-DE"));
        let analytics2 = SessionAnalytics::from_headers(&headers2);

        let fp1 = AnalyticsService::compute_fingerprint(&analytics1);
        let fp2 = AnalyticsService::compute_fingerprint(&analytics2);

        assert_eq!(fp1, fp2);
    }

    #[test]
    fn compute_fingerprint_handles_no_user_agent() {
        let headers = HeaderMap::new();
        let analytics = SessionAnalytics::from_headers(&headers);

        let fingerprint = AnalyticsService::compute_fingerprint(&analytics);

        // Should still produce a hash using "unknown"
        assert!(!fingerprint.is_empty());
    }

    #[test]
    fn compute_fingerprint_different_user_agents_produce_different_hash() {
        let headers1 = create_headers_with_user_agent("Mozilla/5.0 Chrome/120");
        let analytics1 = SessionAnalytics::from_headers(&headers1);

        let headers2 = create_headers_with_user_agent("Mozilla/5.0 Firefox/121");
        let analytics2 = SessionAnalytics::from_headers(&headers2);

        let fp1 = AnalyticsService::compute_fingerprint(&analytics1);
        let fp2 = AnalyticsService::compute_fingerprint(&analytics2);

        assert_ne!(fp1, fp2);
    }
}

mod create_analytics_session_input_tests {
    use super::*;
    use chrono::Utc;
    use systemprompt_analytics::CreateAnalyticsSessionInput;
    use systemprompt_identifiers::{SessionId, UserId};

    #[test]
    fn input_stores_session_id() {
        let session_id = SessionId::new("sess_123".to_string());
        let headers = HeaderMap::new();
        let analytics = SessionAnalytics::from_headers(&headers);
        let expires_at = Utc::now();

        let input = CreateAnalyticsSessionInput {
            session_id: &session_id,
            user_id: None,
            analytics: &analytics,
            is_bot: false,
            expires_at,
        };

        assert_eq!(input.session_id.as_str(), "sess_123");
    }

    #[test]
    fn input_stores_user_id() {
        let session_id = SessionId::new("sess_456".to_string());
        let user_id = UserId::new("user_789".to_string());
        let headers = HeaderMap::new();
        let analytics = SessionAnalytics::from_headers(&headers);
        let expires_at = Utc::now();

        let input = CreateAnalyticsSessionInput {
            session_id: &session_id,
            user_id: Some(&user_id),
            analytics: &analytics,
            is_bot: true,
            expires_at,
        };

        assert!(input.user_id.is_some());
        assert_eq!(input.user_id.unwrap().as_str(), "user_789");
    }

    #[test]
    fn input_stores_is_bot() {
        let session_id = SessionId::new("sess_bot".to_string());
        let headers = HeaderMap::new();
        let analytics = SessionAnalytics::from_headers(&headers);
        let expires_at = Utc::now();

        let input = CreateAnalyticsSessionInput {
            session_id: &session_id,
            user_id: None,
            analytics: &analytics,
            is_bot: true,
            expires_at,
        };

        assert!(input.is_bot);
    }

    #[test]
    fn input_stores_expires_at() {
        let session_id = SessionId::new("sess_exp".to_string());
        let headers = HeaderMap::new();
        let analytics = SessionAnalytics::from_headers(&headers);
        let expires_at = Utc::now();

        let input = CreateAnalyticsSessionInput {
            session_id: &session_id,
            user_id: None,
            analytics: &analytics,
            is_bot: false,
            expires_at,
        };

        assert_eq!(input.expires_at, expires_at);
    }

    #[test]
    fn input_is_debug() {
        let session_id = SessionId::new("sess_dbg".to_string());
        let headers = HeaderMap::new();
        let analytics = SessionAnalytics::from_headers(&headers);
        let expires_at = Utc::now();

        let input = CreateAnalyticsSessionInput {
            session_id: &session_id,
            user_id: None,
            analytics: &analytics,
            is_bot: false,
            expires_at,
        };

        let debug_str = format!("{:?}", input);
        assert!(debug_str.contains("CreateAnalyticsSessionInput"));
    }
}
