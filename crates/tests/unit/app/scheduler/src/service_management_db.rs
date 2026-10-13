//! DB-backed tests for [`ServiceManagementService`].
//!
//! The service wraps `ServiceRepository` from `systemprompt-database`. Tests
//! assert that construction succeeds and that each public query method returns
//! a well-formed result against the freshly-migrated DB. Tests fail when
//! `DATABASE_URL` is unset.

use systemprompt_database::ServiceModule;
use systemprompt_identifiers::ServiceName;
use systemprompt_scheduler::ServiceManagementService;
use systemprompt_test_fixtures::{test_db_pool, unique_instance};

mod service_management_db {
    use super::*;

    #[tokio::test]
    async fn get_services_by_type_mcp_returns_vec() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            unique_instance(),
        ));

        let rows = svc
            .get_services_by_type(ServiceModule::Mcp)
            .await
            .expect("get_services_by_type('mcp') must succeed");

        // Result is well-formed; contents depend on the DB state.
        let _ = rows;
    }

    #[tokio::test]
    async fn get_services_by_type_agent_returns_vec() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            unique_instance(),
        ));

        let rows = svc
            .get_services_by_type(ServiceModule::Agent)
            .await
            .expect("get_services_by_type('agent') must succeed");

        let _ = rows;
    }

    #[tokio::test]
    async fn get_running_services_with_pid_returns_vec() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            unique_instance(),
        ));

        let rows = svc
            .get_running_services_with_pid()
            .await
            .expect("get_running_services_with_pid must succeed");

        // On a freshly-migrated DB there are no running services; every
        // returned record must carry a non-null pid (the query filters on pid
        // IS NOT NULL + status = 'running').
        for row in &rows {
            assert!(
                row.pid.is_some(),
                "get_running_services_with_pid must only return rows with a non-null pid"
            );
        }
    }

    #[tokio::test]
    async fn cleanup_stale_entries_returns_count() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            unique_instance(),
        ));

        let affected = svc
            .cleanup_stale_entries()
            .await
            .expect("cleanup_stale_entries must succeed against a migrated DB");

        // Exact count depends on DB state; just assert the call succeeded.
        let _ = affected;
    }

    #[tokio::test]
    async fn mark_service_stopped_noop_on_unknown_service() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            unique_instance(),
        ));

        // An UPDATE that matches zero rows is still a successful query; the
        // service must not error when the name is not in the table.
        svc.mark_service_stopped(&ServiceName::new("nonexistent-service-xyz-987"))
            .await
            .expect("mark_service_stopped must not error for an unknown service name");
    }

    #[tokio::test]
    async fn cleanup_stale_entries_is_idempotent() {
        let pool = test_db_pool().await;
        let svc = ServiceManagementService::new(systemprompt_database::ServiceRepository::new(
            &pool,
            unique_instance(),
        ));

        let first = svc
            .cleanup_stale_entries()
            .await
            .expect("first cleanup_stale_entries");
        let second = svc
            .cleanup_stale_entries()
            .await
            .expect("second cleanup_stale_entries");

        // Second run should find nothing to clean up after the first.
        assert_eq!(
            second, 0,
            "second cleanup_stale_entries run must delete 0 rows when the first already ran"
        );
        let _ = first;
    }
}
