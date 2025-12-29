//! Unit tests for user-related jobs.
//!
//! Tests cover:
//! - CleanupAnonymousUsersJob trait methods (name, description, schedule)
//!
//! Note: The execute method requires database integration and is tested
//! in integration tests.

use systemprompt_core_users::jobs::CleanupAnonymousUsersJob;
use systemprompt_traits::Job;

// ============================================================================
// CleanupAnonymousUsersJob Tests
// ============================================================================

mod cleanup_anonymous_users_job_tests {
    use super::*;

    #[test]
    fn job_name_is_cleanup_anonymous_users() {
        let job = CleanupAnonymousUsersJob;
        assert_eq!(job.name(), "cleanup_anonymous_users");
    }

    #[test]
    fn job_description_mentions_cleanup() {
        let job = CleanupAnonymousUsersJob;
        let description = job.description();

        assert!(description.contains("Clean"));
        assert!(description.contains("anonymous"));
    }

    #[test]
    fn job_description_mentions_duration() {
        let job = CleanupAnonymousUsersJob;
        let description = job.description();

        // Should mention the 30 day period
        assert!(description.contains("30"));
    }

    #[test]
    fn job_schedule_is_valid_cron() {
        let job = CleanupAnonymousUsersJob;
        let schedule = job.schedule();

        // The schedule should be a cron expression
        // Format: "0 0 * * * *" (at minute 0, every hour)
        assert!(!schedule.is_empty());

        // Should have 6 fields (seconds, minutes, hours, day of month, month, day of week)
        let fields: Vec<&str> = schedule.split_whitespace().collect();
        assert_eq!(fields.len(), 6);
    }

    #[test]
    fn job_schedule_runs_hourly() {
        let job = CleanupAnonymousUsersJob;
        let schedule = job.schedule();

        // "0 0 * * * *" means at second 0, minute 0, every hour
        let fields: Vec<&str> = schedule.split_whitespace().collect();
        assert_eq!(fields[0], "0"); // seconds
        assert_eq!(fields[1], "0"); // minutes
        assert_eq!(fields[2], "*"); // hours (every hour)
    }

    #[test]
    fn job_is_copy() {
        let job = CleanupAnonymousUsersJob;
        let copied = job;
        // Both should still be usable
        assert_eq!(job.name(), copied.name());
    }

    #[test]
    fn job_is_clone() {
        let job = CleanupAnonymousUsersJob;
        let cloned = job;
        assert_eq!(job.name(), cloned.name());
    }

    #[test]
    fn job_debug() {
        let job = CleanupAnonymousUsersJob;
        let debug = format!("{:?}", job);
        assert!(debug.contains("CleanupAnonymousUsersJob"));
    }

    #[test]
    fn job_name_is_static_str() {
        let job = CleanupAnonymousUsersJob;
        let name: &'static str = job.name();
        assert_eq!(name, "cleanup_anonymous_users");
    }

    #[test]
    fn job_description_is_static_str() {
        let job = CleanupAnonymousUsersJob;
        let desc: &'static str = job.description();
        assert!(!desc.is_empty());
    }

    #[test]
    fn job_schedule_is_static_str() {
        let job = CleanupAnonymousUsersJob;
        let schedule: &'static str = job.schedule();
        assert!(!schedule.is_empty());
    }

    #[test]
    fn job_implements_job_trait() {
        fn assert_job<T: Job>(_: &T) {}

        let job = CleanupAnonymousUsersJob;
        assert_job(&job);
    }
}
