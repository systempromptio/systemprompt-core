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
    fn zero_stats_without_message_is_idle() {
        assert!(JobResult::success().with_stats(0, 0).is_idle());
    }

    #[test]
    fn unreported_stats_are_not_idle() {
        assert!(!JobResult::success().is_idle());
    }

    #[test]
    fn processed_items_are_not_idle() {
        assert!(!JobResult::success().with_stats(1, 0).is_idle());
    }

    #[test]
    fn failed_items_are_not_idle() {
        assert!(!JobResult::success().with_stats(0, 1).is_idle());
    }

    #[test]
    fn a_message_is_not_idle() {
        assert!(
            !JobResult::success()
                .with_stats(0, 0)
                .with_message("pruned nothing")
                .is_idle()
        );
    }

    #[test]
    fn failure_is_not_idle() {
        assert!(!JobResult::failure("boom").with_stats(0, 0).is_idle());
    }
}

mod job_context_tests {
    use super::*;
    use systemprompt_provider_contracts::{Dependencies, ProviderError};

    fn create_context() -> JobContext {
        JobContext::new(
            test_actor(),
            Dependencies::new().with(42i32).with("app".to_string()),
        )
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
    fn get_returns_each_inserted_handle_by_type() {
        let ctx = create_context();
        assert_eq!(ctx.get::<i32>().copied(), Ok(42));
        assert_eq!(ctx.get::<String>().map(String::as_str), Ok("app"));
    }

    #[test]
    fn get_of_an_absent_type_names_it() {
        let ctx = create_context();
        let err = ctx.get::<u64>().expect_err("u64 was never inserted");
        assert_eq!(err.type_name(), "u64");
        let provider: ProviderError = err.into();
        assert!(matches!(provider, ProviderError::MissingDependency(_)));
        assert!(provider.to_string().contains("u64"));
    }

    #[test]
    fn an_arc_and_its_target_are_distinct_keys() {
        let ctx = JobContext::new(test_actor(), Dependencies::new().with(Arc::new(7u8)));
        assert_eq!(ctx.get::<Arc<u8>>().map(|v| **v), Ok(7));
        ctx.get::<u8>().expect_err("u8 is not Arc<u8>");
    }

    #[test]
    fn context_is_debug_and_lists_dependency_types() {
        let ctx = create_context();
        let debug = format!("{:?}", ctx);
        assert!(debug.contains("i32"), "{debug}");
        assert!(debug.contains("String"), "{debug}");
    }

    #[test]
    fn actor_and_dependencies_expose_the_constructed_values() {
        let ctx = create_context();
        assert_eq!(ctx.actor().user_id, test_actor().user_id);
        assert_eq!(ctx.dependencies().get::<i32>().copied(), Ok(42));
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
            systemprompt_provider_contracts::Dependencies::new(),
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
        assert!(matches!(err, ProviderError::InvalidParameter { .. }));
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
