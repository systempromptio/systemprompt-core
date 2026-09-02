use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use systemprompt_bridge::gateway::types::HelperOutput;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_bridge::proxy::token_cache::{AuthState, RefreshFn, TokenCache};

fn fake_token(ttl: u64) -> HelperOutput {
    HelperOutput {
        token: BearerToken::new("fake"),
        ttl,
        headers: Default::default(),
    }
}

fn counting_refresh(counter: Arc<AtomicUsize>, ttl: u64) -> RefreshFn {
    Arc::new(move |_threshold| {
        let counter = Arc::clone(&counter);
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            Some(fake_token(ttl))
        })
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
        let counter_for_refresh = Arc::clone(&counter_for_refresh);
        Box::pin(async move {
            let n = counter_for_refresh.fetch_add(1, Ordering::SeqCst) + 1;
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            let ttl = if n == 1 { 10 } else { 3600 };
            Some(fake_token(ttl))
        })
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
    let cache = TokenCache::new(Arc::new(|_| Box::pin(async { None })));
    let err = cache.current(300).await.expect_err("no token must fail");
    let msg = format!("{err}");
    assert!(msg.contains("authentication"), "got: {msg}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_refresh_tick_never_mints_from_an_empty_cache() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 10));

    cache
        .refresh_if_cached(300)
        .await
        .expect("nothing to renew is not an error");
    cache
        .refresh_if_cached(300)
        .await
        .expect("still nothing to renew");
    assert_eq!(
        counter.load(Ordering::SeqCst),
        0,
        "a signed-out install is left alone"
    );

    cache
        .current(0)
        .await
        .expect("a request-driven mint seeds the cache");
    cache
        .refresh_if_cached(300)
        .await
        .expect("a token inside the threshold is renewed");
    assert_eq!(
        counter.load(Ordering::SeqCst),
        2,
        "the tick renews what is cached"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_exhausted_chain_latches_and_stops_calling_the_refresh_fn() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_refresh = Arc::clone(&counter);
    let refresh: RefreshFn = Arc::new(move |_| {
        counter_for_refresh.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { None })
    });
    let cache = TokenCache::new(refresh);
    let mut state = cache.auth_state();
    assert_eq!(*state.borrow_and_update(), AuthState::Ok);

    cache.current(300).await.expect_err("nothing to mint");
    cache.current(300).await.expect_err("still latched");
    cache.current(300).await.expect_err("still latched");

    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "once latched, callers are answered locally instead of re-running the chain"
    );
    assert!(
        state.has_changed().expect("sender alive"),
        "the latch is published"
    );
    assert!(
        state.borrow_and_update().sign_in_required(),
        "the published state names a sign-in"
    );
    assert!(cache.sign_in_required());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_re_arms_a_latched_cache() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_refresh = Arc::clone(&counter);
    let refresh: RefreshFn = Arc::new(move |_| {
        let n = counter_for_refresh.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { (n > 0).then(|| fake_token(3600)) })
    });
    let cache = TokenCache::new(refresh);
    let mut state = cache.auth_state();

    cache.current(300).await.expect_err("first attempt latches");
    cache.reset().await;
    assert!(!cache.sign_in_required(), "reset clears the latch");
    cache
        .current(300)
        .await
        .expect("minting resumes after reset");

    assert_eq!(counter.load(Ordering::SeqCst), 2);
    assert_eq!(
        *state.borrow_and_update(),
        AuthState::Ok,
        "a successful mint publishes the recovery"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fresh_token_rejected_upstream_latches_instead_of_re_minting() {
    let counter = Arc::new(AtomicUsize::new(0));
    let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 3600));
    let mut state = cache.auth_state();

    cache.current(300).await.expect("mint");
    cache.reject_upstream("/v1/bridge/heartbeat").await;

    let err = cache
        .current(300)
        .await
        .expect_err("a revoked credential is not renewed");
    assert!(format!("{err}").contains("sign in"), "{err}");
    assert_eq!(counter.load(Ordering::SeqCst), 1, "no second mint");
    match state.borrow_and_update().clone() {
        AuthState::SignInRequired { reason } => {
            assert!(reason.contains("/v1/bridge/heartbeat"), "{reason}");
        },
        AuthState::Ok => panic!("the rejection must be published"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_rejections_publish_a_single_transition() {
    let cache = TokenCache::new(counting_refresh(Arc::new(AtomicUsize::new(0)), 3600));
    let mut state = cache.auth_state();

    cache.current(300).await.expect("mint");
    cache.reject_upstream("/v1/bridge/heartbeat").await;
    cache.reject_upstream("/v1/bridge/stream").await;
    cache.current(300).await.expect_err("latched");

    assert!(state.has_changed().expect("sender alive"));
    state.borrow_and_update();
    assert!(
        !state.has_changed().expect("sender alive"),
        "one rejection, one notification — the GUI toasts once"
    );
}

#[test]
fn external_credential_change_invalidates_the_cached_token() {
    let temp = tempfile::tempdir().expect("temp config dir");
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async {
            let paths = systemprompt_bridge::auth::setup::resolve_paths().expect("paths");
            std::fs::create_dir_all(&paths.config_dir).expect("config dir");
            std::fs::write(&paths.pat_file, "sp-live-a.b").expect("write pat");

            let counter = Arc::new(AtomicUsize::new(0));
            let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 3600))
                .with_stamp_check_interval(std::time::Duration::ZERO);

            cache.current(300).await.expect("first mint");
            cache.current(300).await.expect("unchanged files hit cache");
            assert_eq!(counter.load(Ordering::SeqCst), 1, "no change, no re-mint");

            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(&paths.pat_file)
                .expect("reopen pat");
            file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2))
                .expect("bump mtime");

            cache
                .current(300)
                .await
                .expect("stale stamp re-mints from disk");
            assert_eq!(
                counter.load(Ordering::SeqCst),
                2,
                "an external login (PAT mtime change) must invalidate the in-memory token"
            );
        });
    });
}
