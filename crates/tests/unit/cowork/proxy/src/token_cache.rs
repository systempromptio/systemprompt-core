use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use systemprompt_bridge::auth::types::HelperOutput;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_bridge::proxy::token_cache::{RefreshFn, TokenCache};

fn fake_token(ttl: u64) -> HelperOutput {
    HelperOutput {
        token: BearerToken::new("fake"),
        ttl,
        headers: Default::default(),
    }
}

fn counting_refresh(counter: Arc<AtomicUsize>, ttl: u64) -> RefreshFn {
    Arc::new(move |_threshold| {
        counter.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(80));
        Some(fake_token(ttl))
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_misses_collapse_to_single_refresh() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = Arc::new(TokenCache::new(counting_refresh(
        Arc::clone(&counter),
        3600,
    )));

    let mut handles = Vec::new();
    for _ in 0..50 {
        let cache = Arc::clone(&cache);
        handles.push(tokio::spawn(async move {
            cache.current(300).await.expect("should yield token")
        }));
    }
    for h in handles {
        h.await.expect("task panic");
    }

    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "single-flight: 50 concurrent misses must collapse to one refresh"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cached_hit_does_not_refresh() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 3600));

    cache.current(300).await.expect("first should mint");
    cache.current(300).await.expect("second should hit cache");
    cache.current(300).await.expect("third should hit cache");

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn near_expiry_triggers_refresh() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 10));

    cache.current(300).await.expect("first miss");
    cache
        .current(300)
        .await
        .expect("ttl 10 within threshold 300 → must refresh again");
    assert!(counter.load(Ordering::SeqCst) >= 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn near_expiry_concurrent_refresh_collapses_to_one() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_refresh = Arc::clone(&counter);
    let refresh: RefreshFn = Arc::new(move |_threshold| {
        let n = counter_for_refresh.fetch_add(1, Ordering::SeqCst) + 1;
        std::thread::sleep(std::time::Duration::from_millis(80));
        let ttl = if n == 1 { 10 } else { 3600 };
        Some(fake_token(ttl))
    });
    let cache = Arc::new(TokenCache::new(refresh));

    cache.current(0).await.expect("seed mints ttl=10");
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    let mut handles = Vec::new();
    for i in 0u64..50 {
        let cache = Arc::clone(&cache);
        let threshold = 60 + (i % 5);
        handles.push(tokio::spawn(async move {
            let _ = cache.current(threshold).await;
        }));
    }
    for h in handles {
        h.await.expect("task panic");
    }

    assert_eq!(
        counter.load(Ordering::SeqCst),
        2,
        "single-flight: 50 near-expiry callers must collapse to one additional refresh",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_failure_propagates() {
    let cache = TokenCache::new(Arc::new(|_| None));
    let err = cache.current(300).await.expect_err("no token must fail");
    let msg = format!("{err}");
    assert!(msg.contains("authentication"), "got: {msg}");
}
