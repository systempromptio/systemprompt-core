//! The bridge manifest is a signed policy document: when the revocation list
//! cannot be read, no manifest is signed — the route fails rather than
//! serving "nothing revoked".
//!
//! The revocation list is the only manifest input read from
//! `user_api_keys`. The test holds an exclusive lock on that table while a
//! context whose connections carry a short `lock_timeout` checks the
//! credential and serves the manifest, so exactly that read fails while the
//! credential check, the catalogue and every other read keep working.

use std::sync::{Arc, OnceLock};

use axum::http::{HeaderMap, header};
use systemprompt_api::routes::gateway::bridge_manifest;
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_marketplace::{AllowAllFilter, MarketplaceFilter};
use systemprompt_models::api::ErrorCode;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_config, init_isolated_bootstrap, seed_bridge_credential, seed_user_row,
    test_db_pool,
};
use systemprompt_traits::AppContext as _;

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_isolated_bootstrap("http://127.0.0.1", "plugins: {}\n"))
}

fn with_lock_timeout(url: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}options=-c%20lock_timeout%3D300")
}

async fn context(
    boot: &TestBootstrap,
    database_url: &str,
) -> Arc<systemprompt_runtime::AppContext> {
    crate::support::context_on(
        boot,
        fixture_config(database_url),
        Arc::new(AllowAllFilter) as Arc<dyn MarketplaceFilter>,
    )
    .await
}

#[tokio::test]
async fn manifest_is_not_signed_when_the_revocation_read_fails() {
    let boot = boot();
    let pool = test_db_pool().await;
    let ctx = context(boot, &with_lock_timeout(&boot.database_url)).await;
    let owner = ctx.system_admin().id();
    seed_user_row(&pool, owner, &format!("{owner}@manifest-policy.invalid"))
        .await
        .expect("seed the organisation owner");
    let extractor = Arc::new(JwtContextExtractor::new(
        ctx.session_provider().expect("session provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_repository(
            ctx.oauth_repositories().oauth.clone(),
            systemprompt_traits::CacheEpochs::new(),
        ),
        ctx.config().jwt_issuer.clone(),
        systemprompt_test_fixtures::test_signing_keys(),
    ));
    let consumer = seed_bridge_credential(&pool, "manifest-policy@example.invalid")
        .await
        .expect("consumer credential");

    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", consumer.jwt.as_str())
            .parse()
            .expect("bearer header"),
    );
    let mut lock = pool.write_pool().begin().await.expect("lock transaction");
    sqlx::query("LOCK TABLE user_api_keys IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .expect("lock the revocation table");
    let served = bridge_manifest::manifest(extractor, (*ctx).clone(), headers).await;
    lock.rollback().await.expect("release the lock");

    let error = served
        .expect_err("a manifest whose revocation list cannot be read is not served")
        .into_inner();
    assert_eq!(error.code, ErrorCode::InternalError, "{}", error.message);
    assert_eq!(error.message, "manifest: revocations unavailable");
    assert!(
        error.source().is_some(),
        "the read failure is kept for the log"
    );
    let wire = serde_json::to_value(&error).expect("serialise the error");
    assert_eq!(wire["message"], "Internal server error");
}
