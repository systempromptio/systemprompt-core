//! Tests for job provider types

use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_provider_contracts::{JobContext, JobResult};

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
    fn is_clone() {
        let result = JobResult::success().with_message("test");
        let cloned = result.clone();
        assert_eq!(cloned.message, result.message);
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
        JobContext::new(db_pool, app_context)
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
        let ctx = JobContext::new(db_pool, app_context);

        let pool: Option<&i32> = ctx.db_pool();
        assert_eq!(pool, Some(&42));
    }

    #[test]
    fn db_pool_downcast_wrong_type() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(42i32);
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let ctx = JobContext::new(db_pool, app_context);

        let pool: Option<&String> = ctx.db_pool();
        assert!(pool.is_none());
    }

    #[test]
    fn app_context_downcast_correct_type() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new("test".to_string());
        let ctx = JobContext::new(db_pool, app_context);

        let app: Option<&String> = ctx.app_context();
        assert_eq!(app, Some(&"test".to_string()));
    }

    #[test]
    fn app_context_downcast_wrong_type() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new("test".to_string());
        let ctx = JobContext::new(db_pool, app_context);

        let app: Option<&i32> = ctx.app_context();
        assert!(app.is_none());
    }

    #[test]
    fn db_pool_arc_returns_clone() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(42i32);
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let ctx = JobContext::new(db_pool.clone(), app_context);

        let returned_arc = ctx.db_pool_arc();
        assert!(Arc::ptr_eq(&db_pool, &returned_arc));
    }

    #[test]
    fn app_context_arc_returns_clone() {
        let db_pool: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let app_context: Arc<dyn std::any::Any + Send + Sync> = Arc::new("test".to_string());
        let ctx = JobContext::new(db_pool, app_context.clone());

        let returned_arc = ctx.app_context_arc();
        assert!(Arc::ptr_eq(&app_context, &returned_arc));
    }

    #[test]
    fn context_is_debug() {
        let ctx = create_context();
        let debug = format!("{:?}", ctx);
        assert!(debug.contains("type-erased"));
    }
}
