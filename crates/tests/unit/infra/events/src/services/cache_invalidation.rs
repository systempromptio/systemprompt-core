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
use systemprompt_traits::{CacheDomain, cache_epoch};

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

async fn epoch_moves_past(domain: CacheDomain, baseline: u64) -> bool {
    for _ in 0..100 {
        if cache_epoch(domain) > baseline {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

#[test]
fn a_known_domain_advances_only_its_own_epoch() {
    let policy = cache_epoch(CacheDomain::GatewayPolicy);
    let jti = cache_epoch(CacheDomain::JtiRevocation);
    assert_eq!(
        receive_cache_invalidation("gateway_policy"),
        Some(CacheDomain::GatewayPolicy)
    );
    assert!(cache_epoch(CacheDomain::GatewayPolicy) > policy);
    assert!(cache_epoch(CacheDomain::JtiRevocation) >= jti);
}

#[test]
fn an_unknown_domain_is_ignored() {
    assert_eq!(receive_cache_invalidation("not-a-cache"), None);
    assert_eq!(receive_cache_invalidation(""), None);
}

#[test]
fn invalidating_all_domains_advances_every_epoch() {
    let before: Vec<u64> = CacheDomain::ALL.iter().map(|d| cache_epoch(*d)).collect();
    invalidate_all_cache_domains();
    for (domain, was) in CacheDomain::ALL.iter().zip(before) {
        assert!(cache_epoch(*domain) > was, "{}", domain.label());
    }
}

#[tokio::test]
async fn a_notify_on_the_cache_channel_reaches_the_bridge() {
    let pool = fixture_pool().await;
    let _guard = BRIDGE_LOCK.lock().await;
    let handle = PostgresEventBridge::new(pool.clone(), unique_instance()).start();
    await_listening(&handle).await;

    let baseline = cache_epoch(CacheDomain::Authz);
    sqlx::query("SELECT pg_notify($1, 'authz')")
        .bind(CACHE_INVALIDATION_CHANNEL)
        .execute(&pool)
        .await
        .expect("notify");
    let moved = epoch_moves_past(CacheDomain::Authz, baseline).await;
    handle.shutdown().await;
    assert!(moved, "the authz epoch did not advance after NOTIFY");
}

#[tokio::test]
async fn writes_to_cached_tables_notify_through_their_triggers() {
    let pool = fixture_pool().await;
    let _guard = BRIDGE_LOCK.lock().await;
    let handle = PostgresEventBridge::new(pool.clone(), unique_instance()).start();
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
        let baseline = cache_epoch(domain);
        sqlx::query(statement)
            .execute(&pool)
            .await
            .expect("statement runs");
        if !epoch_moves_past(domain, baseline).await {
            failures.push(domain.label());
        }
    }
    handle.shutdown().await;
    assert!(failures.is_empty(), "no notification for {failures:?}");
}
