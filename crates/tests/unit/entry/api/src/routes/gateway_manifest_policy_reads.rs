//! The bridge manifest is a signed policy document: when the revocation list
//! or the enabled-host restriction cannot be read, no manifest is signed —
//! the route fails rather than serving "nothing revoked, every host enabled".

use std::sync::{Arc, OnceLock};

use axum::http::{HeaderMap, header};
use systemprompt_api::routes::gateway::bridge_manifest;
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_database::Database;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_models::api::ErrorCode;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context_with, fixture_app_context_with_user_repository,
    init_isolated_bootstrap, install_test_signing_key, seed_bridge_credential, seed_user_row,
    test_db_pool,
};
use systemprompt_traits::AppContext as _;
use systemprompt_users::UserRepository;

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_isolated_bootstrap("http://127.0.0.1", "plugins: {}\n"))
}

fn boot_paths(boot: &TestBootstrap) -> PathsConfig {
    PathsConfig {
        system: boot.system_path.display().to_string(),
        services: boot.services_path.display().to_string(),
        bin: boot.bin_path.display().to_string(),
        web_path: Some(boot.system_path.join("web").display().to_string()),
        storage: Some(boot.storage_path.display().to_string()),
        geoip_database: None,
    }
}

// The revocation list is read through the user repository's write pool; a
// closed pool makes exactly that read fail while the credential check, the
// catalogue and every other repository keep working.
async fn closed_write_pool(url: &str) -> Arc<sqlx::PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy(url)
        .expect("lazy pool");
    pool.close().await;
    Arc::new(pool)
}

#[tokio::test]
async fn manifest_is_not_signed_when_the_revocation_read_fails() {
    let boot = boot();
    install_test_signing_key();
    let pool = test_db_pool().await;
    let healthy = fixture_app_context_with(
        &pool,
        &boot.database_url,
        boot_paths(boot),
        Arc::new(AllowAllFilter),
    )
    .expect("healthy context");
    let owner = healthy.system_admin().id();
    seed_user_row(&pool, owner, &format!("{owner}@manifest-policy.invalid"))
        .await
        .expect("seed the organisation owner");
    let extractor = Arc::new(JwtContextExtractor::new(
        healthy.session_provider().expect("session provider"),
        healthy.user_provider().expect("user provider"),
        JtiRevocationChecker::from_repository(healthy.oauth_repositories().oauth.clone()),
        healthy.config().jwt_issuer.clone(),
    ));
    let consumer = seed_bridge_credential(&pool, "manifest-policy@example.invalid")
        .await
        .expect("consumer credential");

    let broken_users = Arc::new(Database::from_pools(
        pool.pool(),
        Some(closed_write_pool(&boot.database_url).await),
    ));
    let user_repository = Arc::new(UserRepository::new(&broken_users));
    let degraded = fixture_app_context_with_user_repository(
        &pool,
        &boot.database_url,
        boot_paths(boot),
        Arc::new(AllowAllFilter),
        user_repository,
    )
    .expect("degraded context");

    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", consumer.jwt.as_str())
            .parse()
            .expect("bearer header"),
    );
    let error = bridge_manifest::manifest(extractor, (*degraded).clone(), headers)
        .await
        .expect_err("a manifest whose revocation list cannot be read is not served")
        .into_inner();

    assert_eq!(error.code, ErrorCode::InternalError, "{}", error.message);
    assert_eq!(error.message, "manifest: revocations unavailable");
    assert!(error.source().is_some(), "the read failure is kept for the log");
    let wire = serde_json::to_value(&error).expect("serialise the error");
    assert_eq!(wire["message"], "Internal server error");
}
