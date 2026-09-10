use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use systemprompt_bridge::gateway::types::HelperOutput;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_bridge::proxy::forward::ForwardError;
use systemprompt_bridge::proxy::token_cache::{AuthState, RefreshFn, TokenCache};

const PAT_ENV: &str = "SP_BRIDGE_PAT";

fn fake_token(ttl: u64) -> HelperOutput {
    HelperOutput {
        token: BearerToken::new("fake"),
        ttl,
        headers: Default::default(),
    }
}

fn exhausted() -> ForwardError {
    ForwardError::Auth("no credential provider produced a token".into())
}

fn counting_refresh(counter: Arc<AtomicUsize>, ttl: u64) -> RefreshFn {
    Arc::new(move |_threshold| {
        let counter = Arc::clone(&counter);
        Box::pin(async move {
            counter.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            Ok(fake_token(ttl))
        })
    })
}

// Why: the cache binds every token to the credential identity on disk, so a
// cache with no credentials configured cannot mint at all. Each test runs in a
// sandboxed config dir with a PAT supplied through the environment.
fn with_credentials<F, T>(worker_threads: usize, f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let temp = tempfile::tempdir().expect("config tempdir");
    temp_env::with_vars(
        [
            ("XDG_CONFIG_HOME", Some(temp.path().as_os_str().to_owned())),
            (PAT_ENV, Some("sp-live-a.b".into())),
        ],
        || {
            let mut builder = if worker_threads == 0 {
                tokio::runtime::Builder::new_current_thread()
            } else {
                let mut b = tokio::runtime::Builder::new_multi_thread();
                b.worker_threads(worker_threads);
                b
            };
            builder.enable_all().build().expect("runtime").block_on(f)
        },
    )
}

#[test]
fn concurrent_misses_collapse_to_single_refresh() {
    with_credentials(4, async {
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
    });
}

#[test]
fn cached_hit_does_not_refresh() {
    with_credentials(2, async {
        let counter = Arc::new(AtomicUsize::new(0));
        let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 3600));

        cache.current(300).await.expect("first should mint");
        cache.current(300).await.expect("second should hit cache");
        cache.current(300).await.expect("third should hit cache");

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn near_expiry_triggers_refresh() {
    with_credentials(2, async {
        let counter = Arc::new(AtomicUsize::new(0));
        let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 10));

        cache.current(300).await.expect("first miss");
        cache
            .current(300)
            .await
            .expect("ttl 10 within threshold 300 → must refresh again");
        assert!(counter.load(Ordering::SeqCst) >= 2);
    });
}

#[test]
fn near_expiry_concurrent_refresh_collapses_to_one() {
    with_credentials(4, async {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_refresh = Arc::clone(&counter);
        let refresh: RefreshFn = Arc::new(move |_threshold| {
            let counter_for_refresh = Arc::clone(&counter_for_refresh);
            Box::pin(async move {
                let n = counter_for_refresh.fetch_add(1, Ordering::SeqCst) + 1;
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
                let ttl = if n == 1 { 10 } else { 3600 };
                Ok(fake_token(ttl))
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
    });
}

#[test]
fn refresh_failure_propagates_as_the_typed_error() {
    with_credentials(2, async {
        let cache = TokenCache::new(Arc::new(|_| {
            Box::pin(async { Err(ForwardError::Auth("keystore: certificate not found".into())) })
        }));
        let err = cache.current(300).await.expect_err("no token must fail");
        let ForwardError::Auth(detail) = &err else {
            panic!("a refresh failure is an auth failure, not a routing one: {err:?}");
        };
        assert_eq!(
            detail, "keystore: certificate not found",
            "the provider's own reason reaches the caller unaltered"
        );
        assert_eq!(err.status().as_u16(), 503);
    });
}

#[test]
fn a_refresh_that_never_returns_times_out() {
    // Why: `tokio::time::pause` is only supported on the current-thread runtime.
    with_credentials(0, async {
        let cache = TokenCache::new(Arc::new(|_| {
            Box::pin(async {
                std::future::pending::<()>().await;
                Ok(fake_token(3600))
            })
        }));
        tokio::time::pause();
        let err = cache
            .current(300)
            .await
            .expect_err("a hung provider must not hang the request");
        assert!(matches!(err, ForwardError::AuthTimeout), "{err:?}");
    });
}

#[test]
fn a_missing_credential_identity_is_reported_before_the_refresh_runs() {
    // Why: without a credential to bind the token to, a mint could never be
    // invalidated by a later sign-in, so the cache refuses rather than guesses.
    let temp = tempfile::tempdir().expect("config tempdir");
    temp_env::with_vars(
        [
            ("XDG_CONFIG_HOME", Some(temp.path().as_os_str().to_owned())),
            (PAT_ENV, None),
        ],
        || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime")
                .block_on(async {
                    let counter = Arc::new(AtomicUsize::new(0));
                    let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 3600));
                    let err = cache.current(300).await.expect_err("nothing to bind to");
                    let ForwardError::Auth(detail) = &err else {
                        panic!("{err:?}");
                    };
                    assert!(
                        detail.contains("no credential identity configured"),
                        "{detail}"
                    );
                    assert_eq!(
                        counter.load(Ordering::SeqCst),
                        0,
                        "the provider chain is not run for an unbindable token"
                    );
                });
        },
    );
}

#[test]
fn the_refresh_tick_never_mints_from_an_empty_cache() {
    with_credentials(2, async {
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
    });
}

#[test]
fn an_unreachable_gateway_does_not_latch_and_recovers_by_itself() {
    // Why: the 2026-09-10 astound incident. A refresh tick fired while the machine
    // was waking, the PAT exchange could not reach the gateway, and the cache
    // latched "sign in required" permanently — the latch releases only on a changed
    // credential, so an unchanged, valid PAT could never clear it. The bridge
    // served 503s for five hours after the network came back.
    with_credentials(2, async {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_refresh = Arc::clone(&counter);
        let refresh: RefreshFn = Arc::new(move |_| {
            let attempt = counter_for_refresh.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if attempt < 2 {
                    Err(ForwardError::AuthRetryable(
                        "credential providers failed: pat: gateway PAT request failed: error \
                         sending request for url"
                            .into(),
                    ))
                } else {
                    Ok(fake_token(3600))
                }
            })
        });
        let cache = TokenCache::new(refresh);

        for attempt in 0..2 {
            let err = cache
                .current(300)
                .await
                .expect_err("gateway is unreachable");
            assert!(
                matches!(&err, ForwardError::AuthRetryable(d) if d.contains("error sending request")),
                "attempt {attempt} keeps the provider's reason: {err:?}"
            );
            assert_eq!(err.status().as_u16(), 503);
            assert!(
                !cache.sign_in_required(),
                "a network failure must never ask the user to sign in (attempt {attempt})"
            );
        }

        let token = cache
            .current(300)
            .await
            .expect("the network came back; the cache must mint without a sign-in");
        assert_eq!(token.ttl, 3600);
        assert_eq!(
            counter.load(Ordering::SeqCst),
            3,
            "every tick retries; the chain is not answered from a latch"
        );
        assert_eq!(*cache.auth_state().borrow(), AuthState::Ok);
    });
}

#[test]
fn an_exhausted_chain_latches_and_stops_calling_the_refresh_fn() {
    with_credentials(2, async {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_refresh = Arc::clone(&counter);
        let refresh: RefreshFn = Arc::new(move |_| {
            counter_for_refresh.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err(exhausted()) })
        });
        let cache = TokenCache::new(refresh);
        let mut state = cache.auth_state();
        assert_eq!(*state.borrow_and_update(), AuthState::Ok);

        let first = cache.current(300).await.expect_err("nothing to mint");
        assert!(
            matches!(&first, ForwardError::Auth(d) if d == "no credential provider produced a token"),
            "the first failure carries the provider's reason: {first:?}"
        );
        let latched = cache.current(300).await.expect_err("still latched");
        assert!(
            matches!(&latched, ForwardError::Auth(d) if d.contains("sign in")),
            "later callers are told to sign in: {latched:?}"
        );
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
        match state.borrow_and_update().clone() {
            AuthState::SignInRequired { reason } => {
                assert!(
                    reason.contains("no credential provider produced a token"),
                    "the published reason is the provider's: {reason}"
                );
            },
            AuthState::Ok => panic!("the published state names a sign-in"),
        }
        assert!(cache.sign_in_required());
    });
}

#[test]
fn reset_re_arms_a_latched_cache() {
    with_credentials(2, async {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_refresh = Arc::clone(&counter);
        let refresh: RefreshFn = Arc::new(move |_| {
            let n = counter_for_refresh.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if n > 0 {
                    Ok(fake_token(3600))
                } else {
                    Err(exhausted())
                }
            })
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
    });
}

#[test]
fn a_fresh_token_rejected_upstream_latches_instead_of_re_minting() {
    with_credentials(2, async {
        let counter = Arc::new(AtomicUsize::new(0));
        let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 3600));
        let mut state = cache.auth_state();

        cache.current(300).await.expect("mint");
        cache.reject_upstream("/v1/bridge/heartbeat").await;

        let err = cache
            .current(300)
            .await
            .expect_err("a revoked credential is not renewed");
        assert!(
            matches!(&err, ForwardError::Auth(d) if d.contains("sign in")),
            "{err:?}"
        );
        assert_eq!(counter.load(Ordering::SeqCst), 1, "no second mint");
        match state.borrow_and_update().clone() {
            AuthState::SignInRequired { reason } => {
                assert!(reason.contains("/v1/bridge/heartbeat"), "{reason}");
            },
            AuthState::Ok => panic!("the rejection must be published"),
        }
    });
}

#[test]
fn repeated_rejections_publish_a_single_transition() {
    with_credentials(2, async {
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
    });
}

#[test]
fn external_credential_change_invalidates_the_cached_token() {
    let temp = tempfile::tempdir().expect("temp config dir");
    temp_env::with_vars(
        [
            ("XDG_CONFIG_HOME", Some(temp.path().as_os_str().to_owned())),
            (PAT_ENV, None),
        ],
        || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            rt.block_on(async {
                let paths = systemprompt_bridge::auth::setup::resolve_paths().expect("paths");
                std::fs::create_dir_all(&paths.config_dir).expect("config dir");
                std::fs::write(&paths.pat_file, "sp-live-a.b").expect("write pat");
                std::fs::write(
                    &paths.config_file,
                    format!("[pat]\nfile = {:?}\n", paths.pat_file.display().to_string()),
                )
                .expect("point the config at the pat");

                let counter = Arc::new(AtomicUsize::new(0));
                let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 3600));

                cache.current(300).await.expect("first mint");
                cache.current(300).await.expect("unchanged files hit cache");
                assert_eq!(counter.load(Ordering::SeqCst), 1, "no change, no re-mint");

                // Why: the stamp is content-derived, so rewriting the same PAT
                // is not a credential change and must not cost a mint.
                std::fs::write(&paths.pat_file, "sp-live-a.b").expect("rewrite pat");
                cache
                    .current(300)
                    .await
                    .expect("same credential hits cache");
                assert_eq!(
                    counter.load(Ordering::SeqCst),
                    1,
                    "same content, no re-mint"
                );

                std::fs::write(&paths.pat_file, "sp-live-c.d").expect("replace pat");
                // Why: the stamp is re-read at most once per interval, so a
                // change lands on the next check rather than the next request.
                tokio::time::pause();
                tokio::time::advance(std::time::Duration::from_secs(6)).await;
                cache
                    .current(300)
                    .await
                    .expect("a changed credential re-mints from disk");
                assert_eq!(
                    counter.load(Ordering::SeqCst),
                    2,
                    "an external login (new PAT) must invalidate the in-memory token"
                );
            });
        },
    );
}

#[test]
fn a_latched_cache_releases_when_the_credentials_change_on_disk() {
    let temp = tempfile::tempdir().expect("temp config dir");
    temp_env::with_vars(
        [
            ("XDG_CONFIG_HOME", Some(temp.path().as_os_str().to_owned())),
            (PAT_ENV, None),
        ],
        || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            rt.block_on(async {
                let paths = systemprompt_bridge::auth::setup::resolve_paths().expect("paths");
                std::fs::create_dir_all(&paths.config_dir).expect("config dir");
                std::fs::write(&paths.pat_file, "sp-live-a.b").expect("write pat");
                std::fs::write(
                    &paths.config_file,
                    format!("[pat]\nfile = {:?}\n", paths.pat_file.display().to_string()),
                )
                .expect("point the config at the pat");

                let counter = Arc::new(AtomicUsize::new(0));
                let counter_for_refresh = Arc::clone(&counter);
                let refresh: RefreshFn = Arc::new(move |_| {
                    let n = counter_for_refresh.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async move {
                        if n > 0 {
                            Ok(fake_token(3600))
                        } else {
                            Err(exhausted())
                        }
                    })
                });
                let cache = TokenCache::new(refresh);

                cache
                    .current(300)
                    .await
                    .expect_err("the first attempt latches");
                assert!(cache.sign_in_required());
                cache
                    .current(300)
                    .await
                    .expect_err("answered from the latch");
                assert_eq!(counter.load(Ordering::SeqCst), 1);

                std::fs::write(&paths.pat_file, "sp-live-c.d").expect("sign in elsewhere");
                assert!(
                    !cache.sign_in_required(),
                    "a sign-in performed outside this process releases the latch"
                );
                cache
                    .current(300)
                    .await
                    .expect("minting resumes with the new credential");
                assert_eq!(counter.load(Ordering::SeqCst), 2);
            });
        },
    );
}

// Why: the stamp capture reads the config, hashes the PAT and opens the
// keystore, so it runs at most once per interval on tokio's clock. These two
// tests pin both sides of that interval; without them the interval could be
// removed and every other test here would still pass.
fn with_pat_on_disk<F, T>(f: impl FnOnce(systemprompt_bridge::auth::setup::PathLayout) -> F) -> T
where
    F: std::future::Future<Output = T>,
{
    let temp = tempfile::tempdir().expect("temp config dir");
    temp_env::with_vars(
        [
            ("XDG_CONFIG_HOME", Some(temp.path().as_os_str().to_owned())),
            (PAT_ENV, None),
        ],
        || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime")
                .block_on(async {
                    let paths = systemprompt_bridge::auth::setup::resolve_paths().expect("paths");
                    std::fs::create_dir_all(&paths.config_dir).expect("config dir");
                    std::fs::write(&paths.pat_file, "sp-live-a.b").expect("write pat");
                    std::fs::write(
                        &paths.config_file,
                        format!("[pat]\nfile = {:?}\n", paths.pat_file.display().to_string()),
                    )
                    .expect("point the config at the pat");
                    f(paths).await
                })
        },
    )
}

#[test]
fn a_credential_replaced_inside_the_interval_is_not_noticed_until_it_elapses() {
    with_pat_on_disk(|paths| async move {
        let counter = Arc::new(AtomicUsize::new(0));
        let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 3600));

        tokio::time::pause();
        cache.current(300).await.expect("first mint");
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        std::fs::write(&paths.pat_file, "sp-live-c.d").expect("replace pat");
        tokio::time::advance(std::time::Duration::from_secs(4)).await;
        cache
            .current(300)
            .await
            .expect("a cached token is still served inside the interval");
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "the stamp is not re-read on every request; that is the point of the interval"
        );

        tokio::time::advance(std::time::Duration::from_secs(2)).await;
        cache
            .current(300)
            .await
            .expect("the changed credential re-mints once the interval elapses");
        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "a rotated credential is caught within seconds, not never"
        );
    })
}

#[test]
fn a_credential_that_becomes_unreadable_is_never_served_from_the_cache() {
    with_pat_on_disk(|paths| async move {
        let counter = Arc::new(AtomicUsize::new(0));
        let cache = TokenCache::new(counting_refresh(Arc::clone(&counter), 3600));

        tokio::time::pause();
        cache.current(300).await.expect("first mint");

        std::fs::remove_file(&paths.pat_file).expect("the credential disappears");
        tokio::time::advance(std::time::Duration::from_secs(6)).await;

        let err = cache
            .current(300)
            .await
            .expect_err("a token whose credential can no longer be identified is not served");
        let ForwardError::Auth(detail) = &err else {
            panic!("an unbindable credential is an auth failure: {err:?}");
        };
        assert!(
            detail.contains("read PAT") && detail.contains("systemprompt-bridge.pat"),
            "the failure names the credential file that went missing: {detail}"
        );
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "the cached token is discarded rather than replayed against the gateway"
        );
    })
}
