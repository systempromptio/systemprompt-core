//! `services::gateway::quota` + `services::gateway::policy` integration —
//! drives the quota repo for allow/deny decisions and the policy resolver
//! for the fall-through-to-permissive case. Lives in the integration crate
//! so we can pull the test-fixtures DB pool.

use systemprompt_api::services::gateway::policy::{PolicyResolver, QuotaWindow};
use systemprompt_api::services::gateway::quota::{
    PostUpdateParams, post_update_tokens, precheck_and_reserve,
};
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool};

async fn pool() -> systemprompt_database::DbPool {
    let b = ensure_test_bootstrap();
    fixture_db_pool(&b.database_url).await.expect("pool")
}

#[tokio::test]
async fn precheck_with_empty_windows_returns_none() {
    let p = pool().await;
    let user = UserId::new(format!("quota-test-{}", uuid::Uuid::new_v4()));
    let decision = precheck_and_reserve(&p, &user, &[]).await.expect("ok");
    assert!(decision.is_none());
}

#[tokio::test]
async fn precheck_within_limit_allows() {
    let p = pool().await;
    let user = UserId::new(format!("quota-allow-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        window_seconds: 60,
        max_requests: Some(100),
        max_input_tokens: None,
        max_output_tokens: None,
    }];
    let decision = precheck_and_reserve(&p, &user, &windows).await.expect("ok");
    assert!(decision.is_none(), "expected allow, got {decision:?}");
}

#[tokio::test]
async fn precheck_over_limit_denies_second_call() {
    let p = pool().await;
    let user = UserId::new(format!("quota-deny-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        window_seconds: 60,
        max_requests: Some(1),
        max_input_tokens: None,
        max_output_tokens: None,
    }];
    let d1 = precheck_and_reserve(&p, &user, &windows).await.expect("ok");
    assert!(d1.is_none());
    let d2 = precheck_and_reserve(&p, &user, &windows).await.expect("ok");
    let dec = d2.expect("expected denial");
    assert!(!dec.allow);
    assert_eq!(dec.limit_requests, Some(1));
    assert_eq!(dec.window_seconds, 60);
}

#[tokio::test]
async fn post_update_with_empty_windows_is_noop() {
    let p = pool().await;
    let user = UserId::new("quota-post-empty");
    post_update_tokens(
        &p,
        PostUpdateParams {
            user_id: &user,
            windows: &[],
            input_tokens: 100,
            output_tokens: 50,
        },
    )
    .await;
}

#[tokio::test]
async fn post_update_increments_token_counts() {
    let p = pool().await;
    let user = UserId::new(format!("quota-post-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        window_seconds: 60,
        max_requests: Some(1000),
        max_input_tokens: Some(1000),
        max_output_tokens: Some(1000),
    }];
    post_update_tokens(
        &p,
        PostUpdateParams {
            user_id: &user,
            windows: &windows,
            input_tokens: 10,
            output_tokens: 20,
        },
    )
    .await;
}

#[tokio::test]
async fn policy_resolver_falls_back_when_empty() {
    let p = pool().await;
    let resolver = PolicyResolver::new(&p).expect("resolver");
    let _spec1 = resolver.resolve().await;
    // Second call hits the in-memory cache path.
    let _spec2 = resolver.resolve().await;
}
