use systemprompt_traits::Job;

mod job_properties_tests {
    use super::*;
    use systemprompt_scheduler::{
        BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob,
        DatabaseCleanupJob,
    };

    #[test]
    fn behavioral_analysis_job_properties() {
        let job = BehavioralAnalysisJob;
        assert_eq!(job.name(), "behavioral_analysis");
        assert_eq!(
            job.description(),
            "Analyzes fingerprint behavior patterns and flags suspicious activity"
        );
        assert_eq!(job.schedule(), "0 0 * * * *");
    }

    #[test]
    fn database_cleanup_job_properties() {
        let job = DatabaseCleanupJob;
        assert_eq!(job.name(), "database_cleanup");
        assert_eq!(
            job.description(),
            "Cleans up orphaned logs, MCP executions, and expired OAuth tokens"
        );
        assert_eq!(job.schedule(), "0 0 3 * * *");
    }

    #[test]
    fn cleanup_empty_contexts_job_properties() {
        let job = CleanupEmptyContextsJob;
        assert_eq!(job.name(), "cleanup_empty_contexts");
        assert_eq!(
            job.description(),
            "Deletes empty conversation contexts older than 1 hour"
        );
        assert_eq!(job.schedule(), "0 0 */2 * * *");
    }

    #[test]
    fn cleanup_inactive_sessions_job_properties() {
        let job = CleanupInactiveSessionsJob;
        assert_eq!(job.name(), "cleanup_inactive_sessions");
        assert_eq!(
            job.description(),
            "Cleans up inactive sessions (1 hour threshold)"
        );
        assert_eq!(job.schedule(), "0 */10 * * * *");
    }
}

mod job_schedule_validation_tests {
    use super::*;
    use systemprompt_scheduler::{
        BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob,
        DatabaseCleanupJob,
    };

    #[test]
    fn all_schedules_are_valid_cron_format() {
        let jobs: Vec<&dyn Job> = vec![
            &BehavioralAnalysisJob,
            &DatabaseCleanupJob,
            &CleanupEmptyContextsJob,
            &CleanupInactiveSessionsJob,
        ];

        for job in jobs {
            let schedule = job.schedule();
            let parts: Vec<&str> = schedule.split_whitespace().collect();
            assert_eq!(
                parts.len(),
                6,
                "Job {} has invalid cron schedule: {}",
                job.name(),
                schedule
            );
        }
    }

    #[test]
    fn all_jobs_have_unique_names() {
        let names: Vec<&str> = vec![
            BehavioralAnalysisJob.name(),
            DatabaseCleanupJob.name(),
            CleanupEmptyContextsJob.name(),
            CleanupInactiveSessionsJob.name(),
        ];

        let mut unique_names = names.clone();
        unique_names.sort();
        unique_names.dedup();

        assert_eq!(
            names.len(),
            unique_names.len(),
            "Job names are not unique: {:?}",
            names
        );
    }

    #[test]
    fn all_jobs_have_non_empty_names() {
        let jobs: Vec<&dyn Job> = vec![
            &BehavioralAnalysisJob,
            &DatabaseCleanupJob,
            &CleanupEmptyContextsJob,
            &CleanupInactiveSessionsJob,
        ];

        for job in jobs {
            assert!(!job.name().is_empty(), "Job name should not be empty");
            assert!(
                !job.description().is_empty(),
                "Job description should not be empty for {}",
                job.name()
            );
        }
    }

    #[test]
    fn all_jobs_have_snake_case_names() {
        let jobs: Vec<&dyn Job> = vec![
            &BehavioralAnalysisJob,
            &DatabaseCleanupJob,
            &CleanupEmptyContextsJob,
            &CleanupInactiveSessionsJob,
        ];

        for job in jobs {
            let name = job.name();
            assert!(
                name.chars().all(|c| c.is_lowercase() || c == '_'),
                "Job name '{}' should be snake_case",
                name
            );
        }
    }
}
