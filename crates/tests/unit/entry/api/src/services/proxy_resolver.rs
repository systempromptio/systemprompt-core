//! `ServiceResolver::resolve` — the lookup every proxied MCP request opens
//! with.
//!
//! It decides three different things from one row: that the database is
//! reachable, that the service is known, and that it is running. Each is a
//! distinct refusal the caller renders as a different status, so collapsing
//! any two of them loses the operator's diagnosis.

use systemprompt_api::services::proxy::{ProxyError, resolver_test_api};
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, fixture_app_context, fixture_db_pool,
};

async fn live_pool() -> DbPool {
    let boot = ensure_test_bootstrap();
    fixture_db_pool(&boot.database_url)
        .await
        .expect("test database")
}

fn unique_name(prefix: &str) -> String {
    format!("{prefix}_{}", &uuid::Uuid::new_v4().simple().to_string()[..12])
}

async fn seed_service(pool: &DbPool, name: &str, status: &str) {
    let inner = pool.pool_arc().expect("write pool");
    sqlx::query(
        "INSERT INTO services (instance_id, name, module_name, status, port, pid)
         VALUES ('test-instance', $1, 'mcp_server', $2, 0, $3)
         ON CONFLICT (instance_id, name) DO UPDATE SET status = $2",
    )
    .bind(name)
    .bind(status)
    .bind(i32::try_from(std::process::id()).expect("pid fits in i32"))
    .execute(inner.as_ref())
    .await
    .expect("seed the services row");
}

async fn delete_service(pool: &DbPool, name: &str) {
    let inner = pool.pool_arc().expect("write pool");
    sqlx::query("DELETE FROM services WHERE name = $1")
        .bind(name)
        .execute(inner.as_ref())
        .await
        .expect("clean up the services row");
}

#[tokio::test]
async fn an_unreachable_database_is_reported_as_a_database_error_not_a_missing_service() {
    let boot = ensure_test_bootstrap();
    let pool = closed_db_pool().await;
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");

    let error = resolver_test_api::resolve("anything", &ctx)
        .await
        .err()
        .expect("a closed pool cannot resolve a service");

    assert!(
        matches!(error, ProxyError::DatabaseError { .. }),
        "an outage must not be rendered as a 404 for the service; got {error:?}"
    );
}

#[tokio::test]
async fn a_service_no_row_names_is_reported_as_not_found() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");

    let error = resolver_test_api::resolve(&unique_name("absent"), &ctx)
        .await
        .err()
        .expect("an unregistered service cannot resolve");

    assert!(
        matches!(error, ProxyError::ServiceNotFound { .. }),
        "got {error:?}"
    );
}

#[tokio::test]
async fn a_registered_but_stopped_service_reports_the_status_that_refused_it() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");
    let name = unique_name("stopped");
    seed_service(&pool, &name, "stopped").await;

    let error = resolver_test_api::resolve(&name, &ctx)
        .await
        .err()
        .expect("a stopped service cannot be proxied to");

    match error {
        ProxyError::ServiceNotRunning { service, status } => {
            assert_eq!(service, name);
            assert_eq!(
                status, "stopped",
                "the operator needs the status that refused the request, not a generic 503"
            );
        },
        other => panic!("expected ServiceNotRunning, got {other:?}"),
    }

    delete_service(&pool, &name).await;
}
