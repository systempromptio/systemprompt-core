//! `Cache-Control: no-cache` on the bridge manifest bypasses the per-user
//! memo.
//!
//! The memo's key is the disk fingerprint, the user and a stamp over the
//! managed tables; a connector the user has just linked changes none of
//! them, so for up to a minute a plain fetch answered with the server set
//! from before the link. A user-initiated sync sends `no-cache` and gets a
//! rebuild; the rebuild is stored so the plugin-file downloads behind it
//! stay warm.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use axum::http::{HeaderMap, header};
use systemprompt_api::routes::gateway::bridge_manifest;
use systemprompt_api::routes::gateway::bridge_resolved::Freshness;
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::{MarketplaceCandidate, MarketplaceFilter, MarketplaceFilterError};
use systemprompt_models::profile::PathsConfig;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context_with, fixture_db_pool, init_isolated_bootstrap,
    install_test_signing_key, seed_bridge_credential, seed_user_row,
};
use systemprompt_traits::AppContext as _;

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

// The filter runs once per catalogue rebuild and never on a memo hit, so
// its call count is the number of rebuilds.
#[derive(Debug, Default)]
struct CountingFilter {
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl MarketplaceFilter for CountingFilter {
    async fn filter(
        &self,
        _user_id: &UserId,
        candidate: MarketplaceCandidate,
    ) -> Result<MarketplaceCandidate, MarketplaceFilterError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(candidate)
    }
}

struct Harness {
    ctx: AppContext,
    extractor: Arc<JwtContextExtractor>,
    filter: Arc<CountingFilter>,
    jwt: String,
}

async fn harness(mailbox: &str) -> Harness {
    let boot = boot();
    install_test_signing_key();
    let pool = fixture_db_pool(&boot.database_url)
        .await
        .expect("test database");
    let filter = Arc::new(CountingFilter::default());
    let ctx = fixture_app_context_with(
        &pool,
        &boot.database_url,
        boot_paths(boot),
        Arc::clone(&filter) as Arc<dyn MarketplaceFilter>,
    )
    .expect("fixture context");
    let owner = ctx.system_admin().id();
    seed_user_row(&pool, owner, &format!("{owner}@manifest-freshness.invalid"))
        .await
        .expect("seed the organisation owner");
    let extractor = Arc::new(JwtContextExtractor::new(
        ctx.session_provider().expect("session provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_repository(ctx.oauth_repositories().oauth.clone()),
    ));
    let consumer = seed_bridge_credential(&pool, mailbox)
        .await
        .expect("consumer credential");
    Harness {
        ctx: (*ctx).clone(),
        extractor,
        filter,
        jwt: consumer.jwt.as_str().to_owned(),
    }
}

async fn fetch(harness: &Harness, cache_control: Option<&str>) {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", harness.jwt)
            .parse()
            .expect("bearer header"),
    );
    if let Some(value) = cache_control {
        headers.insert(
            header::CACHE_CONTROL,
            value.parse().expect("cache-control header"),
        );
    }
    bridge_manifest::manifest(Arc::clone(&harness.extractor), harness.ctx.clone(), headers)
        .await
        .expect("a signed-in consumer receives a manifest");
}

#[test]
fn no_cache_is_read_from_any_cache_control_directive_list() {
    let mut headers = HeaderMap::new();
    assert_eq!(Freshness::from_headers(&headers), Freshness::Memo);
    headers.insert(header::CACHE_CONTROL, "max-age=0".parse().unwrap());
    assert_eq!(Freshness::from_headers(&headers), Freshness::Memo);
    headers.insert(
        header::CACHE_CONTROL,
        "max-age=0, No-Cache".parse().unwrap(),
    );
    assert_eq!(Freshness::from_headers(&headers), Freshness::Fresh);
}

#[tokio::test]
async fn a_plain_fetch_within_the_ttl_is_served_from_the_memo() {
    let harness = harness("manifest-memo@example.invalid").await;

    fetch(&harness, None).await;
    fetch(&harness, None).await;

    assert_eq!(
        harness.filter.calls.load(Ordering::SeqCst),
        1,
        "the second fetch is a memo hit and never re-runs the filter"
    );
}

#[tokio::test]
async fn no_cache_rebuilds_the_catalogue_and_warms_the_memo_again() {
    let harness = harness("manifest-fresh@example.invalid").await;

    fetch(&harness, None).await;
    fetch(&harness, Some("no-cache")).await;
    assert_eq!(
        harness.filter.calls.load(Ordering::SeqCst),
        2,
        "no-cache bypasses a warm memo and rebuilds"
    );

    fetch(&harness, None).await;
    assert_eq!(
        harness.filter.calls.load(Ordering::SeqCst),
        2,
        "the rebuild was stored, so the plain fetch behind it is a hit"
    );
}
