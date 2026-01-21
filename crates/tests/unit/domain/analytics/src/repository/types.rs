//! Tests for repository type structures.

use systemprompt_analytics::SessionMigrationResult;

mod session_migration_result_tests {
    use super::*;

    #[test]
    fn stores_sessions_migrated_count() {
        let result = SessionMigrationResult {
            sessions_migrated: 42,
        };

        assert_eq!(result.sessions_migrated, 42);
    }

    #[test]
    fn total_records_migrated_returns_sessions_count() {
        let result = SessionMigrationResult {
            sessions_migrated: 100,
        };

        assert_eq!(result.total_records_migrated(), 100);
    }

    #[test]
    fn zero_migrations() {
        let result = SessionMigrationResult {
            sessions_migrated: 0,
        };

        assert_eq!(result.sessions_migrated, 0);
        assert_eq!(result.total_records_migrated(), 0);
    }

    #[test]
    fn large_migration_count() {
        let result = SessionMigrationResult {
            sessions_migrated: 1_000_000,
        };

        assert_eq!(result.total_records_migrated(), 1_000_000);
    }

    #[test]
    fn result_is_copy() {
        let result = SessionMigrationResult {
            sessions_migrated: 50,
        };
        let copied = result;

        assert_eq!(result.sessions_migrated, copied.sessions_migrated);
    }

    #[test]
    fn result_is_clone() {
        let result = SessionMigrationResult {
            sessions_migrated: 75,
        };
        let cloned = result.clone();

        assert_eq!(result.sessions_migrated, cloned.sessions_migrated);
    }

    #[test]
    fn result_is_debug() {
        let result = SessionMigrationResult {
            sessions_migrated: 25,
        };
        let debug_str = format!("{:?}", result);

        assert!(debug_str.contains("SessionMigrationResult"));
        assert!(debug_str.contains("25"));
    }
}

mod repository_constants_tests {
    use systemprompt_analytics::{
        ABUSE_THRESHOLD_FOR_BAN, HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM,
        MAX_SESSIONS_PER_FINGERPRINT, SUSTAINED_VELOCITY_MINUTES,
    };

    #[test]
    fn max_sessions_per_fingerprint_is_5() {
        assert_eq!(MAX_SESSIONS_PER_FINGERPRINT, 5);
    }

    #[test]
    fn high_request_threshold_is_100() {
        assert_eq!(HIGH_REQUEST_THRESHOLD, 100);
    }

    #[test]
    fn high_velocity_rpm_is_10() {
        assert!((HIGH_VELOCITY_RPM - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn sustained_velocity_minutes_is_60() {
        assert_eq!(SUSTAINED_VELOCITY_MINUTES, 60);
    }

    #[test]
    fn abuse_threshold_for_ban_is_3() {
        assert_eq!(ABUSE_THRESHOLD_FOR_BAN, 3);
    }
}
