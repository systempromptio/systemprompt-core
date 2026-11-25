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
use systemprompt_test_fixtures::AppContextSeeds as _;

use axum::http::{HeaderMap, header};
use systemprompt_extension::bridge_catalog::CatalogFreshness as Freshness;
use systemprompt_gateway_entry::http::bridge::resolved::freshness_from_headers;
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::{MarketplaceCandidate, MarketplaceFilter, MarketplaceFilterError};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    DisposableDb, TestBootstrap, fixture_config, init_isolated_bootstrap, seed_bridge_credential,
    seed_user_row,
};

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_isolated_bootstrap("http://127.0.0.1", "plugins: {}\n"))
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

// The memo key carries a stamp over every managed table row of the
// organisation owner, and every context on the shared database has the same
// owner. A sibling test publishing an organisation resource there changes the
// stamp and forces a rebuild these tests would count, so each harness owns its
// database.
struct Harness {
    database: DisposableDb,
    ctx: AppContext,
    filter: Arc<CountingFilter>,
    jwt: String,
}

async fn harness(mailbox: &str) -> Harness {
    let boot = boot();
    let database = DisposableDb::with_schema("manifest_freshness").await;
    let pool = database.test_pool().await;
    let filter = Arc::new(CountingFilter::default());
    let ctx = crate::support::context_on(
        boot,
        fixture_config(database.url()),
        Arc::clone(&filter) as Arc<dyn MarketplaceFilter>,
    )
    .await;
    let owner = ctx.system_admin().id();
    seed_user_row(&pool, owner, &format!("{owner}@manifest-freshness.invalid"))
        .await
        .expect("seed the organisation owner");
    let consumer = seed_bridge_credential(&pool, mailbox)
        .await
        .expect("consumer credential");
    Harness {
        database,
        ctx: (*ctx).clone(),
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
    let response = super::bridge_support::manifest(&harness.ctx, headers)
        .await
        .expect("a signed-in consumer receives a manifest");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("manifest body");
    let envelope: systemprompt_models::bridge::manifest::SignedManifestEnvelope =
        serde_json::from_slice(&body).expect("the body is a signed envelope");
    assert!(!envelope.payload.is_empty());
    assert!(!envelope.signature.as_str().is_empty());
}

#[test]
fn no_cache_is_read_from_any_cache_control_directive_list() {
    let mut headers = HeaderMap::new();
    assert_eq!(freshness_from_headers(&headers), Freshness::Memo);
    headers.insert(header::CACHE_CONTROL, "max-age=0".parse().unwrap());
    assert_eq!(freshness_from_headers(&headers), Freshness::Memo);
    headers.insert(
        header::CACHE_CONTROL,
        "max-age=0, No-Cache".parse().unwrap(),
    );
    assert_eq!(freshness_from_headers(&headers), Freshness::Fresh);
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
    harness.database.drop_now().await;
}

#[tokio::test]
async fn no_cache_within_the_revalidate_window_is_served_from_the_memo() {
    let harness = harness("manifest-fresh@example.invalid").await;

    fetch(&harness, None).await;
    fetch(&harness, Some("no-cache")).await;
    fetch(&harness, None).await;
    assert_eq!(
        harness.filter.calls.load(Ordering::SeqCst),
        1,
        "a memo younger than FRESH_REVALIDATE_AFTER satisfies no-cache"
    );
    harness.database.drop_now().await;
}
