//! Tests for job provider types

use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::Actor;
use systemprompt_provider_contracts::{JobContext, JobResult};
use systemprompt_test_fixtures::fixture_actor;

fn test_actor() -> Actor {
    fixture_actor()
}

mod job_result_tests {
    use super::*;

    #[test]
    fn success_creates_successful_result() {
        let result = JobResult::success();
        assert!(result.success);
    }

    #[test]
    fn success_has_no_message() {
        let result = JobResult::success();
        assert!(result.message.is_none());
    }

    #[test]
    fn success_has_no_items_processed() {
        let result = JobResult::success();
        assert!(result.items_processed.is_none());
    }

    #[test]
    fn success_has_no_items_failed() {
        let result = JobResult::success();
        assert!(result.items_failed.is_none());
    }

    #[test]
    fn success_has_zero_duration() {
        let result = JobResult::success();
        assert_eq!(result.duration_ms, 0);
    }

    #[test]
    fn with_message() {
        let result = JobResult::success().with_message("Completed successfully");
        assert_eq!(result.message, Some("Completed successfully".to_string()));
    }

    #[test]
    fn with_stats() {
        let result = JobResult::success().with_stats(100, 5);
        assert_eq!(result.items_processed, Some(100));
        assert_eq!(result.items_failed, Some(5));
    }

    #[test]
    fn with_duration() {
        let result = JobResult::success().with_duration(1500);
        assert_eq!(result.duration_ms, 1500);
    }

    #[test]
    fn failure_creates_unsuccessful_result() {
        let result = JobResult::failure("Something went wrong");
        assert!(!result.success);
    }

    #[test]
    fn failure_has_message() {
        let result = JobResult::failure("Error message");
        assert_eq!(result.message, Some("Error message".to_string()));
    }

    #[test]
    fn failure_has_no_items_processed() {
        let result = JobResult::failure("error");
        assert!(result.items_processed.is_none());
    }

    #[test]
    fn failure_has_no_items_failed() {
        let result = JobResult::failure("error");
        assert!(result.items_failed.is_none());
    }

    #[test]
    fn failure_has_zero_duration() {
        let result = JobResult::failure("error");
        assert_eq!(result.duration_ms, 0);
    }

    #[test]
    fn builder_chain() {
        let result = JobResult::success()
            .with_message("Done")
            .with_stats(50, 2)
            .with_duration(500);

        assert!(result.success);
        assert_eq!(result.message, Some("Done".to_string()));
        assert_eq!(result.items_processed, Some(50));
        assert_eq!(result.items_failed, Some(2));
        assert_eq!(result.duration_ms, 500);
    }

    #[test]
    fn is_debug() {
        let result = JobResult::success();
        let debug = format!("{:?}", result);
        assert!(debug.contains("JobResult"));
    }
}

mod job_context_tests {
    use super::*;

    fn create_context() -> JobContext {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(42i32);
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new("app".to_string());
        JobContext::new(
            test_actor(),
            db_pool,
            app_context,
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
        )
    }

    #[test]
    fn new_creates_context() {
        let ctx = create_context();
        let debug = format!("{:?}", ctx);
        assert!(debug.contains("JobContext"));
    }

    #[test]
    fn parameters_is_empty_by_default() {
        let ctx = create_context();
        assert!(ctx.parameters().is_empty());
    }

    #[test]
    fn with_parameters() {
        let mut params = HashMap::new();
        params.insert("key".to_string(), "value".to_string());

        let ctx = create_context().with_parameters(params);
        assert_eq!(ctx.parameters().len(), 1);
    }

    #[test]
    fn get_parameter_existing() {
        let mut params = HashMap::new();
        params.insert("key".to_string(), "value".to_string());

        let ctx = create_context().with_parameters(params);
        assert_eq!(ctx.get_parameter("key"), Some(&"value".to_string()));
    }

    #[test]
    fn get_parameter_missing() {
        let ctx = create_context();
        assert!(ctx.get_parameter("missing").is_none());
    }

    #[test]
    fn db_pool_downcast_correct_type() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(42i32);
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let ctx = JobContext::new(
            test_actor(),
            db_pool,
            app_context,
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
        );

        let pool: Option<&i32> = ctx.db_pool();
        assert_eq!(pool, Some(&42));
    }

    #[test]
    fn db_pool_downcast_wrong_type() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(42i32);
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let ctx = JobContext::new(
            test_actor(),
            db_pool,
            app_context,
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
        );

        let pool: Option<&String> = ctx.db_pool();
        assert!(pool.is_none());
    }

    #[test]
    fn app_context_downcast_correct_type() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new("test".to_string());
        let ctx = JobContext::new(
            test_actor(),
            db_pool,
            app_context,
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
        );

        let app: Option<&String> = ctx.app_context();
        assert_eq!(app, Some(&"test".to_string()));
    }

    #[test]
    fn app_context_downcast_wrong_type() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new("test".to_string());
        let ctx = JobContext::new(
            test_actor(),
            db_pool,
            app_context,
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
        );

        let app: Option<&i32> = ctx.app_context();
        assert!(app.is_none());
    }

    #[test]
    fn db_pool_arc_returns_clone() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(42i32);
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let ctx = JobContext::new(
            test_actor(),
            db_pool.clone(),
            app_context,
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
        );

        let returned_arc = ctx.db_pool_arc();
        assert!(Arc::ptr_eq(&db_pool, &returned_arc));
    }

    #[test]
    fn app_context_arc_returns_clone() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new("test".to_string());
        let ctx = JobContext::new(
            test_actor(),
            db_pool,
            app_context.clone(),
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
        );

        let returned_arc = ctx.app_context_arc();
        assert!(Arc::ptr_eq(&app_context, &returned_arc));
    }

    #[test]
    fn context_is_debug() {
        let ctx = create_context();
        let debug = format!("{:?}", ctx);
        assert!(debug.contains("type-erased"));
    }

    #[test]
    fn actor_and_app_paths_arc_expose_the_constructed_values() {
        let app_paths: Arc<dyn std::any::Any + Send + Sync> = Arc::new(7u8);
        let ctx = JobContext::new(
            test_actor(),
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
            app_paths.clone(),
        );

        assert_eq!(ctx.actor().user_id, test_actor().user_id);
        assert!(Arc::ptr_eq(&app_paths, &ctx.app_paths_arc()));
    }
}

mod job_trait_default_tests {
    use super::*;
    use systemprompt_provider_contracts::{Job, ProviderResult};

    struct MinimalJob;

    #[async_trait::async_trait]
    impl Job for MinimalJob {
        fn name(&self) -> &'static str {
            "minimal"
        }

        fn schedule(&self) -> &'static str {
            "@daily"
        }

        async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
            Ok(JobResult::success())
        }
    }

    struct PipelineStepJob;

    #[async_trait::async_trait]
    impl Job for PipelineStepJob {
        fn name(&self) -> &'static str {
            "pipeline_step"
        }

        fn schedule(&self) -> &'static str {
            "@daily"
        }

        fn schedulable(&self) -> bool {
            false
        }

        async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
            Ok(JobResult::success())
        }
    }

    #[test]
    fn unoverridden_jobs_are_enabled_untagged_and_undescribed() {
        assert!(MinimalJob.enabled());
        assert!(MinimalJob.tags().is_empty());
        assert_eq!(MinimalJob.description(), "");
    }

    #[test]
    fn jobs_are_schedulable_unless_they_opt_out() {
        assert!(
            MinimalJob.schedulable(),
            "a job with no cron entry is a real signal by default"
        );
        assert!(
            !PipelineStepJob.schedulable(),
            "an inline pipeline step opts out of the unscheduled-job warning"
        );
    }
}

mod get_parameter_parsed_tests {
    use super::*;
    use systemprompt_provider_contracts::ProviderError;

    fn context_with(params: &[(&str, &str)]) -> JobContext {
        let map: HashMap<String, String> = params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        JobContext::new(
            test_actor(),
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
            Arc::new(()) as Arc<dyn std::any::Any + Send + Sync>,
        )
        .with_parameters(map)
    }

    #[test]
    fn absent_key_is_ok_none_so_the_job_falls_back_to_its_default() {
        let ctx = context_with(&[]);
        let parsed = ctx
            .get_parameter_parsed::<i32>("retention_hours")
            .expect("absent key is not an error");
        assert_eq!(parsed, None);
    }

    #[test]
    fn parseable_value_is_returned() {
        let ctx = context_with(&[("retention_hours", "42")]);
        let parsed = ctx
            .get_parameter_parsed::<i32>("retention_hours")
            .expect("parses");
        assert_eq!(parsed, Some(42));
    }

    #[test]
    fn unparseable_value_fails_the_run_and_names_the_key() {
        let ctx = context_with(&[("retention_hours", "abc")]);
        let err = ctx
            .get_parameter_parsed::<i32>("retention_hours")
            .expect_err("a mistyped override must not silently fall back");
        assert!(matches!(err, ProviderError::Configuration(_)));
        let message = err.to_string();
        assert!(
            message.contains("retention_hours") && message.contains("abc"),
            "error should name the offending key and value: {message}"
        );
    }

    #[test]
    fn enforce_defaults_to_false_and_is_opt_in() {
        let ctx = context_with(&[]);
        assert!(!ctx.enforce());
        assert!(ctx.with_enforce(true).enforce());
    }
}
