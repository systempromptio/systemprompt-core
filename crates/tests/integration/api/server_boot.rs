//! Boot the full API server router via `setup_api_server`. Exercises the
//! per-route wiring functions (oauth/agent/mcp/stream/content/misc/extensions
//! mounts), global middleware stack assembly (analytics, jwt-context, session,
//! cors, trailing slash, trace, served-by, content-negotiation, security
//! headers, ip-ban, jti-revocation, metrics), and the discovery /
//! authenticated-discovery / wellknown / static routers.
//!
//! The prometheus recorder is process-global, so this entire file builds a
//! single API router.

use systemprompt_api::services::server::setup_api_server;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, test_db_pool,
};

#[tokio::test]
async fn setup_api_server_assembles_full_router() -> anyhow::Result<()> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;

    let mut config = fixture_config(&bootstrap.database_url);
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];

    let ctx = fixture_app_context_with_config(&pool, config)?;

    let router = setup_api_server(
        &ctx,
        None,
        &std::sync::Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    drop(router);
    Ok(())
}
