//! Unit tests for banned IP repository types.
//!
//! Tests cover:
//! - BanDuration enum and to_expiry method
//! - BanIpParams builder pattern
//! - BanIpWithMetadataParams builder pattern
//! - BannedIp struct

use chrono::{Duration, Utc};
use systemprompt_users::{BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp};

// ============================================================================
// BanDuration Tests
// ============================================================================

mod ban_duration_tests {
    use super::*;

    #[test]
    fn hours_variant_creation() {
        let duration = BanDuration::Hours(24);
        match duration {
            BanDuration::Hours(h) => assert_eq!(h, 24),
            _ => panic!("Expected Hours variant"),
        }
    }

    #[test]
    fn days_variant_creation() {
        let duration = BanDuration::Days(7);
        match duration {
            BanDuration::Days(d) => assert_eq!(d, 7),
            _ => panic!("Expected Days variant"),
        }
    }

    #[test]
    fn permanent_variant_creation() {
        let duration = BanDuration::Permanent;
        assert!(matches!(duration, BanDuration::Permanent));
    }

    #[test]
    fn hours_to_expiry_returns_future_time() {
        let duration = BanDuration::Hours(24);
        let now = Utc::now();
        let expiry = duration.to_expiry();

        assert!(expiry.is_some());
        let expiry = expiry.unwrap();
        assert!(expiry > now);
        assert!(expiry <= now + Duration::hours(25)); // Allow some margin
    }

    #[test]
    fn days_to_expiry_returns_future_time() {
        let duration = BanDuration::Days(7);
        let now = Utc::now();
        let expiry = duration.to_expiry();

        assert!(expiry.is_some());
        let expiry = expiry.unwrap();
        assert!(expiry > now);
        assert!(expiry <= now + Duration::days(8)); // Allow some margin
    }

    #[test]
    fn permanent_to_expiry_returns_none() {
        let duration = BanDuration::Permanent;
        let expiry = duration.to_expiry();

        assert!(expiry.is_none());
    }

    #[test]
    fn hours_zero_returns_current_time_approx() {
        let duration = BanDuration::Hours(0);
        let now = Utc::now();
        let expiry = duration.to_expiry();

        assert!(expiry.is_some());
        let expiry = expiry.unwrap();
        // Should be very close to now
        assert!((expiry - now).num_seconds().abs() < 2);
    }

    #[test]
    fn days_zero_returns_current_time_approx() {
        let duration = BanDuration::Days(0);
        let now = Utc::now();
        let expiry = duration.to_expiry();

        assert!(expiry.is_some());
        let expiry = expiry.unwrap();
        // Should be very close to now
        assert!((expiry - now).num_seconds().abs() < 2);
    }

    #[test]
    fn hours_large_value() {
        let duration = BanDuration::Hours(8760); // 1 year
        let expiry = duration.to_expiry();

        assert!(expiry.is_some());
    }

    #[test]
    fn days_large_value() {
        let duration = BanDuration::Days(365);
        let expiry = duration.to_expiry();

        assert!(expiry.is_some());
    }

    #[test]
    fn ban_duration_is_copy() {
        let duration = BanDuration::Hours(24);
        let copied = duration;
        // Both should still be usable
        assert!(matches!(duration, BanDuration::Hours(24)));
        assert!(matches!(copied, BanDuration::Hours(24)));
    }

    #[test]
    fn ban_duration_is_clone() {
        let duration = BanDuration::Days(7);
        let cloned = duration;
        assert!(matches!(cloned, BanDuration::Days(7)));
    }

    #[test]
    fn ban_duration_debug() {
        let duration = BanDuration::Permanent;
        let debug = format!("{:?}", duration);
        assert!(debug.contains("Permanent"));
    }
}

// ============================================================================
// BanIpParams Tests
// ============================================================================

mod ban_ip_params_tests {
    use super::*;

    #[test]
    fn new_creates_params_with_required_fields() {
        let params = BanIpParams::new(
            "192.168.1.1",
            "Suspicious activity",
            BanDuration::Hours(24),
            "manual",
        );

        assert_eq!(params.ip_address, "192.168.1.1");
        assert_eq!(params.reason, "Suspicious activity");
        assert!(matches!(params.duration, BanDuration::Hours(24)));
        assert_eq!(params.ban_source, "manual");
        assert!(params.source_fingerprint.is_none());
    }

    #[test]
    fn new_with_permanent_duration() {
        let params = BanIpParams::new(
            "10.0.0.1",
            "Permanent ban",
            BanDuration::Permanent,
            "admin",
        );

        assert!(matches!(params.duration, BanDuration::Permanent));
    }

    #[test]
    fn with_source_fingerprint_sets_fingerprint() {
        let params = BanIpParams::new(
            "192.168.1.1",
            "Test",
            BanDuration::Hours(1),
            "test",
        )
        .with_source_fingerprint("fp123abc");

        assert_eq!(params.source_fingerprint, Some("fp123abc"));
    }

    #[test]
    fn with_source_fingerprint_can_be_chained() {
        let params = BanIpParams::new(
            "192.168.1.1",
            "Test",
            BanDuration::Days(1),
            "test",
        )
        .with_source_fingerprint("fingerprint-1");

        assert_eq!(params.ip_address, "192.168.1.1");
        assert_eq!(params.source_fingerprint, Some("fingerprint-1"));
    }

    #[test]
    fn params_preserves_all_fields() {
        let params = BanIpParams::new(
            "1.2.3.4",
            "Multiple reasons for ban",
            BanDuration::Days(30),
            "security_system",
        )
        .with_source_fingerprint("unique_fp");

        assert_eq!(params.ip_address, "1.2.3.4");
        assert_eq!(params.reason, "Multiple reasons for ban");
        assert!(matches!(params.duration, BanDuration::Days(30)));
        assert_eq!(params.ban_source, "security_system");
        assert_eq!(params.source_fingerprint, Some("unique_fp"));
    }

    #[test]
    fn params_with_empty_strings() {
        let params = BanIpParams::new("", "", BanDuration::Hours(1), "");

        assert_eq!(params.ip_address, "");
        assert_eq!(params.reason, "");
        assert_eq!(params.ban_source, "");
    }

    #[test]
    fn params_with_ipv6_address() {
        let params = BanIpParams::new(
            "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
            "IPv6 ban",
            BanDuration::Hours(12),
            "auto",
        );

        assert_eq!(params.ip_address, "2001:0db8:85a3:0000:0000:8a2e:0370:7334");
    }

    #[test]
    fn params_with_special_characters_in_reason() {
        let params = BanIpParams::new(
            "192.168.1.1",
            "Reason with 'quotes' and \"double quotes\" and <html>",
            BanDuration::Hours(1),
            "test",
        );

        assert!(params.reason.contains("'quotes'"));
        assert!(params.reason.contains("\"double quotes\""));
    }
}

// ============================================================================
// BanIpWithMetadataParams Tests
// ============================================================================

mod ban_ip_with_metadata_params_tests {
    use super::*;

    #[test]
    fn new_creates_params_with_required_fields() {
        let params = BanIpWithMetadataParams::new(
            "192.168.1.1",
            "Suspicious activity",
            BanDuration::Hours(24),
            "manual",
        );

        assert_eq!(params.ip_address, "192.168.1.1");
        assert_eq!(params.reason, "Suspicious activity");
        assert!(matches!(params.duration, BanDuration::Hours(24)));
        assert_eq!(params.ban_source, "manual");
        assert!(params.source_fingerprint.is_none());
        assert!(params.offense_path.is_none());
        assert!(params.user_agent.is_none());
        assert!(params.session_id.is_none());
    }

    #[test]
    fn with_source_fingerprint_sets_fingerprint() {
        let params = BanIpWithMetadataParams::new(
            "192.168.1.1",
            "Test",
            BanDuration::Hours(1),
            "test",
        )
        .with_source_fingerprint("fp123");

        assert_eq!(params.source_fingerprint, Some("fp123"));
    }

    #[test]
    fn with_offense_path_sets_path() {
        let params = BanIpWithMetadataParams::new(
            "192.168.1.1",
            "Test",
            BanDuration::Hours(1),
            "test",
        )
        .with_offense_path("/api/v1/users");

        assert_eq!(params.offense_path, Some("/api/v1/users"));
    }

    #[test]
    fn with_user_agent_sets_agent() {
        let params = BanIpWithMetadataParams::new(
            "192.168.1.1",
            "Test",
            BanDuration::Hours(1),
            "test",
        )
        .with_user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");

        assert_eq!(
            params.user_agent,
            Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        );
    }

    #[test]
    fn with_session_id_sets_session() {
        let params = BanIpWithMetadataParams::new(
            "192.168.1.1",
            "Test",
            BanDuration::Hours(1),
            "test",
        )
        .with_session_id("session-abc-123");

        assert_eq!(params.session_id, Some("session-abc-123"));
    }

    #[test]
    fn builder_methods_can_be_chained() {
        let params = BanIpWithMetadataParams::new(
            "192.168.1.1",
            "Complete ban",
            BanDuration::Permanent,
            "security",
        )
        .with_source_fingerprint("fp123")
        .with_offense_path("/admin/login")
        .with_user_agent("curl/7.68.0")
        .with_session_id("session-xyz");

        assert_eq!(params.ip_address, "192.168.1.1");
        assert_eq!(params.reason, "Complete ban");
        assert!(matches!(params.duration, BanDuration::Permanent));
        assert_eq!(params.ban_source, "security");
        assert_eq!(params.source_fingerprint, Some("fp123"));
        assert_eq!(params.offense_path, Some("/admin/login"));
        assert_eq!(params.user_agent, Some("curl/7.68.0"));
        assert_eq!(params.session_id, Some("session-xyz"));
    }

    #[test]
    fn partial_builder_chain() {
        let params = BanIpWithMetadataParams::new(
            "10.0.0.1",
            "Partial metadata",
            BanDuration::Days(7),
            "auto",
        )
        .with_offense_path("/api/exploit");

        assert_eq!(params.offense_path, Some("/api/exploit"));
        assert!(params.source_fingerprint.is_none());
        assert!(params.user_agent.is_none());
        assert!(params.session_id.is_none());
    }

    #[test]
    fn params_with_long_user_agent() {
        let long_ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";
        let params = BanIpWithMetadataParams::new(
            "192.168.1.1",
            "Test",
            BanDuration::Hours(1),
            "test",
        )
        .with_user_agent(long_ua);

        assert_eq!(params.user_agent, Some(long_ua));
    }

    #[test]
    fn params_with_complex_path() {
        let params = BanIpWithMetadataParams::new(
            "192.168.1.1",
            "Test",
            BanDuration::Hours(1),
            "test",
        )
        .with_offense_path("/api/v2/users/123/profile?include=settings&format=json");

        assert!(params.offense_path.unwrap().contains("include=settings"));
    }
}

// ============================================================================
// BannedIp Tests
// ============================================================================

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
    fn banned_ip_serialization_roundtrip() {
        let banned = create_test_banned_ip();
        let json = serde_json::to_string(&banned).unwrap();
        let deserialized: BannedIp = serde_json::from_str(&json).unwrap();

        assert_eq!(banned.ip_address, deserialized.ip_address);
        assert_eq!(banned.reason, deserialized.reason);
        assert_eq!(banned.ban_count, deserialized.ban_count);
        assert_eq!(banned.is_permanent, deserialized.is_permanent);
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
        banned.associated_session_ids = Some(
            (0..100)
                .map(|i| format!("session-{}", i))
                .collect(),
        );

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
