//! Shutdown drain: the two bounded waits the run loop is built out of.
//!
//! `join_within_drain_grace` bounds only the axum drain, and does so from the
//! readiness broadcast rather than a timer — it has to return the server's own
//! result untouched when the server finishes first. Child termination is the
//! step after it, and it runs while the process is already on its way out: a
//! failure to even enumerate the children must not abort the shutdown.

use systemprompt_api::services::server::readiness::init_readiness;
use systemprompt_api::services::server::shutdown_test_api;
use systemprompt_test_fixtures::{closed_db_pool, ensure_test_bootstrap, fixture_app_context};

#[tokio::test]
async fn a_server_that_finishes_first_has_its_own_result_returned_unchanged() {
    init_readiness();

    let outcome =
        shutdown_test_api::join_within_drain_grace(async { Err(anyhow::anyhow!("bind lost")) })
            .await;

    let error = outcome.expect_err("the server's failure is the caller's failure");
    assert_eq!(
        error.to_string(),
        "bind lost",
        "the drain wrapper must not swallow or rewrite why the server stopped"
    );
}

// Why: this runs after the response path is already gone. There is nobody to
// report to, so an unreadable registry has to be absorbed — a panic or an
// early return here would skip the rest of teardown.
#[tokio::test]
async fn an_unreadable_service_registry_does_not_abort_child_termination() {
    let boot = ensure_test_bootstrap();
    let pool = closed_db_pool().await;
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");

    let completed = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        shutdown_test_api::terminate_children(&ctx),
    )
    .await;

    assert!(
        completed.is_ok(),
        "termination must complete even when neither the agent nor the MCP listing can be read"
    );
}
