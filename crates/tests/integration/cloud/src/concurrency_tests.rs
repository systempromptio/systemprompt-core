//! Concurrent operations on independent tenant contexts must not
//! interfere. The cloud crate's session/tenant stores hold no global
//! mutable state — each `CloudContext`-equivalent owns its own
//! `TempDir`-backed `SessionStore`. This harness asserts that property.

use std::sync::Arc;

use systemprompt_cloud::cli_session::SessionStore;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use crate::support::{TenantFixture, build_session_for};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_tenants_in_separate_stores_do_not_interfere() {
    // Each "tenant context" owns its own fixture and store. Two tokio
    // tasks rotate their tokens 200 times in parallel; neither must
    // observe the other's tokens.
    let fx_a = Arc::new(TenantFixture::new());
    let fx_b = Arc::new(TenantFixture::new());

    let mut set = JoinSet::new();

    {
        let fx = fx_a.clone();
        set.spawn(async move {
            let mut store = SessionStore::new();
            for i in 0..200 {
                let token = format!("a-token-{i}");
                let session = build_session_for(
                    "profile-a",
                    &fx.key_a(),
                    &token,
                    "00000000-0000-4000-8000-00000000000a",
                );
                store.upsert_session(&fx.key_a(), session);
                let live = store.get_valid_session(&fx.key_a()).unwrap();
                assert!(
                    live.session_token.as_str().starts_with("a-token-"),
                    "tenant A must never observe a non-A token"
                );
            }
            store
        });
    }
    {
        let fx = fx_b.clone();
        set.spawn(async move {
            let mut store = SessionStore::new();
            for i in 0..200 {
                let token = format!("b-token-{i}");
                let session = build_session_for(
                    "profile-b",
                    &fx.key_b(),
                    &token,
                    "00000000-0000-4000-8000-00000000000b",
                );
                store.upsert_session(&fx.key_b(), session);
                let live = store.get_valid_session(&fx.key_b()).unwrap();
                assert!(
                    live.session_token.as_str().starts_with("b-token-"),
                    "tenant B must never observe a non-B token"
                );
            }
            store
        });
    }

    while let Some(joined) = set.join_next().await {
        joined.expect("task panicked");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shared_store_under_mutex_keeps_per_tenant_keys_distinct() {
    // Stress the second axis: one shared store, two tenants, concurrent
    // upserts via a Mutex. Each tenant must end up with *only* its own
    // token, never the other's.
    let fx = TenantFixture::new();
    let store = Arc::new(Mutex::new(SessionStore::new()));
    let key_a = fx.key_a();
    let key_b = fx.key_b();

    let mut set = JoinSet::new();
    for i in 0..100 {
        let store = store.clone();
        let key = key_a.clone();
        set.spawn(async move {
            let token = format!("a-{i}");
            let session = build_session_for(
                "profile-a",
                &key,
                &token,
                "00000000-0000-4000-8000-0000000000a1",
            );
            store.lock().await.upsert_session(&key, session);
        });
    }
    for i in 0..100 {
        let store = store.clone();
        let key = key_b.clone();
        set.spawn(async move {
            let token = format!("b-{i}");
            let session = build_session_for(
                "profile-b",
                &key,
                &token,
                "00000000-0000-4000-8000-0000000000b1",
            );
            store.lock().await.upsert_session(&key, session);
        });
    }
    while let Some(joined) = set.join_next().await {
        joined.expect("task panicked");
    }

    let guard = store.lock().await;
    let a = guard.get_valid_session(&key_a).expect("A present");
    let b = guard.get_valid_session(&key_b).expect("B present");
    assert!(
        a.session_token.as_str().starts_with("a-"),
        "tenant A storage key must never hold a B-token, got {}",
        a.session_token.as_str()
    );
    assert!(
        b.session_token.as_str().starts_with("b-"),
        "tenant B storage key must never hold an A-token, got {}",
        b.session_token.as_str()
    );
}
