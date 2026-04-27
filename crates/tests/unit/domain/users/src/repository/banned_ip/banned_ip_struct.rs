//! Tests for BannedIp struct.

use chrono::{Duration, Utc};
use systemprompt_users::BannedIp;

mod banned_ip_tests {
    use super::*;

    fn create_test_banned_ip() -> BannedIp {
        BannedIp {
            ip_address: "192.168.1.100".to_string(),
            reason: "Automated attack detected".to_string(),
            banned_at: Utc::now(),
            expires_at: Some(Utc::now() + Duration::hours(24)),
            ban_count: 1,
            last_offense_path: Some("/api/v1/login".to_string()),
            last_user_agent: Some("curl/7.68.0".to_string()),
            is_permanent: false,
            source_fingerprint: Some("fp-abc123".to_string()),
            ban_source: Some("rate_limiter".to_string()),
            associated_session_ids: Some(vec!["session-1".to_string(), "session-2".to_string()]),
        }
    }

    #[test]
    fn banned_ip_creation() {
        let banned = create_test_banned_ip();

        assert_eq!(banned.ip_address, "192.168.1.100");
        assert_eq!(banned.reason, "Automated attack detected");
        assert_eq!(banned.ban_count, 1);
        assert!(!banned.is_permanent);
    }

    #[test]
    fn banned_ip_clone() {
        let banned = create_test_banned_ip();
        let cloned = banned.clone();

        assert_eq!(banned.ip_address, cloned.ip_address);
        assert_eq!(banned.reason, cloned.reason);
        assert_eq!(banned.ban_count, cloned.ban_count);
    }

    #[test]
    fn banned_ip_debug() {
        let banned = create_test_banned_ip();
        let debug = format!("{:?}", banned);

        assert!(debug.contains("BannedIp"));
        assert!(debug.contains("192.168.1.100"));
    }

    #[test]
    fn banned_ip_with_no_expiry_is_permanent() {
        let banned = BannedIp {
            ip_address: "10.0.0.1".to_string(),
            reason: "Permanent ban".to_string(),
            banned_at: Utc::now(),
            expires_at: None,
            ban_count: 5,
            last_offense_path: None,
            last_user_agent: None,
            is_permanent: true,
            source_fingerprint: None,
            ban_source: None,
            associated_session_ids: None,
        };

        assert!(banned.is_permanent);
        assert!(banned.expires_at.is_none());
    }

    #[test]
    fn banned_ip_with_multiple_ban_count() {
        let mut banned = create_test_banned_ip();
        banned.ban_count = 10;

        assert_eq!(banned.ban_count, 10);
    }

    #[test]
    fn banned_ip_with_empty_session_ids() {
        let mut banned = create_test_banned_ip();
        banned.associated_session_ids = Some(vec![]);

        assert!(banned.associated_session_ids.as_ref().unwrap().is_empty());
    }

    #[test]
    fn banned_ip_with_many_session_ids() {
        let mut banned = create_test_banned_ip();
        banned.associated_session_ids = Some((0..100).map(|i| format!("session-{}", i)).collect());

        assert_eq!(banned.associated_session_ids.as_ref().unwrap().len(), 100);
    }

    #[test]
    fn banned_ip_minimal_fields() {
        let banned = BannedIp {
            ip_address: "1.1.1.1".to_string(),
            reason: "Test".to_string(),
            banned_at: Utc::now(),
            expires_at: None,
            ban_count: 0,
            last_offense_path: None,
            last_user_agent: None,
            is_permanent: false,
            source_fingerprint: None,
            ban_source: None,
            associated_session_ids: None,
        };

        assert!(banned.last_offense_path.is_none());
        assert!(banned.last_user_agent.is_none());
        assert!(banned.source_fingerprint.is_none());
        assert!(banned.ban_source.is_none());
        assert!(banned.associated_session_ids.is_none());
    }

    #[test]
    fn banned_ip_ipv6_address() {
        let banned = BannedIp {
            ip_address: "::1".to_string(),
            reason: "IPv6 localhost ban".to_string(),
            banned_at: Utc::now(),
            expires_at: None,
            ban_count: 1,
            last_offense_path: None,
            last_user_agent: None,
            is_permanent: true,
            source_fingerprint: None,
            ban_source: None,
            associated_session_ids: None,
        };

        assert_eq!(banned.ip_address, "::1");
    }

    #[test]
    fn banned_ip_json_includes_all_fields() {
        let banned = create_test_banned_ip();
        let json = serde_json::to_string(&banned).unwrap();

        assert!(json.contains("ip_address"));
        assert!(json.contains("reason"));
        assert!(json.contains("banned_at"));
        assert!(json.contains("expires_at"));
        assert!(json.contains("ban_count"));
        assert!(json.contains("last_offense_path"));
        assert!(json.contains("last_user_agent"));
        assert!(json.contains("is_permanent"));
        assert!(json.contains("source_fingerprint"));
        assert!(json.contains("ban_source"));
        assert!(json.contains("associated_session_ids"));
    }
}
