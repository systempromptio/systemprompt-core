//! Cross-replica cache invalidation: a `NOTIFY systemprompt_cache` — sent by
//! the statement triggers on the cached tables — reaches a running bridge and
//! advances the matching cache epoch.

use std::time::Duration;

use systemprompt_database::DbPool;
use systemprompt_events::{
    CACHE_INVALIDATION_CHANNEL, EventBridgeHandle, PostgresEventBridge,
    invalidate_all_cache_domains, receive_cache_invalidation,
};
use systemprompt_test_fixtures::{test_db_pool, unique_instance};
use systemprompt_traits::{CacheDomain, CacheEpochs};

static BRIDGE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn fixture_pool() -> sqlx::PgPool {
    let db: DbPool = test_db_pool().await;
    let arc = db.pool();
    (*arc).clone()
}

async fn await_listening(handle: &EventBridgeHandle) {
    let listening = tokio::time::timeout(Duration::from_secs(10), handle.listening())
        .await
        .expect("the bridge establishes its LISTEN within 10s");
    assert!(listening, "the bridge stopped before it was listening");
}

async fn epoch_moves_past(epochs: &CacheEpochs, domain: CacheDomain, baseline: u64) -> bool {
    for _ in 0..100 {
        if epochs.current(domain) > baseline {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

#[test]
fn a_known_domain_advances_only_its_own_epoch() {
    let epochs = CacheEpochs::new();
    assert_eq!(
        receive_cache_invalidation(&epochs, "gateway_policy"),
        Some(CacheDomain::GatewayPolicy)
    );
    assert_eq!(epochs.current(CacheDomain::GatewayPolicy), 1);
    assert_eq!(epochs.current(CacheDomain::JtiRevocation), 0);
    assert_eq!(epochs.current(CacheDomain::Authz), 0);
}

#[test]
fn an_unknown_domain_is_ignored() {
    let epochs = CacheEpochs::new();
    assert_eq!(receive_cache_invalidation(&epochs, "not-a-cache"), None);
    assert_eq!(receive_cache_invalidation(&epochs, ""), None);
    for domain in CacheDomain::ALL {
        assert_eq!(epochs.current(domain), 0);
    }
}

#[test]
fn invalidating_all_domains_advances_every_epoch() {
    let epochs = CacheEpochs::new();
    invalidate_all_cache_domains(&epochs);
    for domain in CacheDomain::ALL {
        assert_eq!(epochs.current(domain), 1, "{}", domain.label());
    }
}

#[tokio::test]
async fn a_notify_on_the_cache_channel_reaches_the_bridge() {
    let pool = fixture_pool().await;
    let _guard = BRIDGE_LOCK.lock().await;
    let epochs = CacheEpochs::new();
    assert!(!epochs.relay_live(), "a handle starts not live");
    let handle = PostgresEventBridge::new(pool.clone(), unique_instance(), epochs.clone()).start();
    await_listening(&handle).await;
    assert!(
        epochs.relay_live(),
        "a listening relay marks the handle live"
    );

    let baseline = epochs.current(CacheDomain::Authz);
    sqlx::query("SELECT pg_notify($1, 'authz')")
        .bind(CACHE_INVALIDATION_CHANNEL)
        .execute(&pool)
        .await
        .expect("notify");
    let moved = epoch_moves_past(&epochs, CacheDomain::Authz, baseline).await;
    handle.shutdown().await;
    assert!(!epochs.relay_live(), "a stopped relay clears liveness");
    assert!(moved, "the authz epoch did not advance after NOTIFY");
}

#[tokio::test]
async fn writes_to_cached_tables_notify_through_their_triggers() {
    let pool = fixture_pool().await;
    let _guard = BRIDGE_LOCK.lock().await;
    let epochs = CacheEpochs::new();
    let handle = PostgresEventBridge::new(pool.clone(), unique_instance(), epochs.clone()).start();
    await_listening(&handle).await;

    let cases = [
        (
            CacheDomain::GatewayPolicy,
            "DELETE FROM ai_gateway_policies WHERE name = '__no_such_policy__'",
        ),
        (
            CacheDomain::Authz,
            "DELETE FROM access_control_rules WHERE entity_id = '__no_such_entity__'",
        ),
        (
            CacheDomain::JtiRevocation,
            "INSERT INTO oauth_jti_revocations (jti, user_id, exp) \
             SELECT 'none', 'none', now() WHERE false",
        ),
    ];
    let mut failures = Vec::new();
    for (domain, statement) in cases {
        let baseline = epochs.current(domain);
        sqlx::query(statement)
            .execute(&pool)
            .await
            .expect("statement runs");
        if !epoch_moves_past(&epochs, domain, baseline).await {
            failures.push(domain.label());
        }
    }
    handle.shutdown().await;
    assert!(failures.is_empty(), "no notification for {failures:?}");
}
