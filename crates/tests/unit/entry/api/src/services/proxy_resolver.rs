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
    format!(
        "{prefix}_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    )
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

// Why: this is the regression test for a defect that killed the API process.
// `start_services` reports Ok when it started nothing — an unregistered name
// filters to an empty target list — so the old code recursed on that Ok alone
// and spun forever on a row that never leaves `crashed`, exhausting the stack.
// The timeout is part of the assertion: a reintroduced recursion aborts the
// binary on stack overflow, and anything that merely hangs fails here rather
// than wedging the suite.
#[tokio::test]
async fn a_crashed_service_that_cannot_be_restarted_is_refused_rather_than_retried_forever() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");
    let name = unique_name("crashed_unregistered");
    seed_service(&pool, &name, "crashed").await;

    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        resolver_test_api::resolve(&name, &ctx),
    )
    .await
    .expect("resolve must terminate; retrying a restart that starts nothing never converges");

    let error = outcome
        .err()
        .expect("a service that did not come back cannot be proxied to");

    match error {
        ProxyError::ServiceNotRunning { service, status } => {
            assert_eq!(service, name);
            assert_eq!(
                status, "crashed",
                "the refusal must report the status the row still holds, so the operator sees the \
                 service never came back"
            );
        },
        other => panic!("expected ServiceNotRunning, got {other:?}"),
    }

    delete_service(&pool, &name).await;
}

// Why: the restart path is only worth having if a service that genuinely comes
// back is proxied to instead of refused. The row is flipped to `running`
// inside the restart's settle window, which is what the re-read is there to
// observe.
#[tokio::test]
async fn a_crashed_service_that_comes_back_running_is_returned_to_the_caller() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");
    let name = unique_name("crashed_recovers");
    seed_service(&pool, &name, "crashed").await;

    let flipper = {
        let pool = pool.clone();
        let name = name.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            seed_service(&pool, &name, "running").await;
        })
    };

    let resolved = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        resolver_test_api::resolve(&name, &ctx),
    )
    .await
    .expect("resolve must terminate")
    .expect("a service that came back running must resolve");

    flipper.await.expect("the flipping task does not panic");

    assert_eq!(resolved.name, name);
    assert_eq!(
        resolved.status, "running",
        "the re-read must hand back the recovered row, not the stale crashed one"
    );

    delete_service(&pool, &name).await;
}

// Why: the re-read after a restart is a second database round trip, and it has
// its own failure mode. An unreadable database there must not be reported as
// "the service is not running" — that would send an operator to the service
// logs for a database outage.
#[tokio::test]
async fn a_read_failure_on_the_restart_recheck_is_reported_as_a_database_error() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");
    let name = unique_name("crashed_then_outage");
    seed_service(&pool, &name, "crashed").await;

    let closer = {
        let pool = pool.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            if let Ok(inner) = pool.pool_arc() {
                inner.close().await;
            }
        })
    };

    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        resolver_test_api::resolve(&name, &ctx),
    )
    .await
    .expect("resolve must terminate");

    closer.await.expect("the closing task does not panic");

    let error = outcome
        .err()
        .expect("a resolve whose re-read cannot run has nothing to return");

    assert!(
        matches!(error, ProxyError::DatabaseError { .. }),
        "an outage during the re-check must stay a database error; got {error:?}"
    );
}
