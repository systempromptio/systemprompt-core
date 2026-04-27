//! Tests for BanDuration and BanIpParams.

use chrono::{Duration, Utc};
use systemprompt_users::{BanDuration, BanIpParams};

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

        let expiry = expiry.expect("hours should produce expiry");
        assert!(expiry > now);
        assert!(expiry <= now + Duration::hours(25));
    }

    #[test]
    fn days_to_expiry_returns_future_time() {
        let duration = BanDuration::Days(7);
        let now = Utc::now();
        let expiry = duration.to_expiry();

        let expiry = expiry.expect("days should produce expiry");
        assert!(expiry > now);
        assert!(expiry <= now + Duration::days(8));
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

        let expiry = expiry.expect("zero hours should produce expiry");
        assert!((expiry - now).num_seconds().abs() < 2);
    }

    #[test]
    fn days_zero_returns_current_time_approx() {
        let duration = BanDuration::Days(0);
        let now = Utc::now();
        let expiry = duration.to_expiry();

        let expiry = expiry.expect("zero days should produce expiry");
        assert!((expiry - now).num_seconds().abs() < 2);
    }

    #[test]
    fn hours_large_value() {
        let duration = BanDuration::Hours(8760);
        let expiry = duration.to_expiry();

        expiry.expect("large hours should produce expiry");
    }

    #[test]
    fn days_large_value() {
        let duration = BanDuration::Days(365);
        let expiry = duration.to_expiry();

        expiry.expect("large days should produce expiry");
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
        let params = BanIpParams::new("10.0.0.1", "Permanent ban", BanDuration::Permanent, "admin");

        assert!(matches!(params.duration, BanDuration::Permanent));
    }

    #[test]
    fn with_source_fingerprint_sets_fingerprint() {
        let params = BanIpParams::new("192.168.1.1", "Test", BanDuration::Hours(1), "test")
            .with_source_fingerprint("fp123abc");

        assert_eq!(params.source_fingerprint, Some("fp123abc"));
    }

    #[test]
    fn with_source_fingerprint_can_be_chained() {
        let params = BanIpParams::new("192.168.1.1", "Test", BanDuration::Days(1), "test")
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
