//! Tests for BanIpWithMetadataParams.

use chrono::Utc;
use systemprompt_users::{BanDuration, BanIpWithMetadataParams};

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
