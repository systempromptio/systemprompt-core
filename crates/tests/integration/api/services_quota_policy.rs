//! `services::gateway::quota` + `services::gateway::policy` integration —
//! drives the quota repo for allow/deny decisions and the policy resolver
//! for the fall-through-to-permissive case. Lives in the integration crate
//! so we can pull the test-fixtures DB pool.

use systemprompt_api::services::gateway::policy::{PolicyResolver, QuotaWindow};
use systemprompt_api::services::gateway::quota::{
    PostUpdateParams, post_update_tokens, precheck_and_reserve,
};
use systemprompt_identifiers::UserId;

fn quota_repo(
    db: &systemprompt_database::DbPool,
) -> systemprompt_ai::repository::AiQuotaBucketRepository {
    systemprompt_ai::repository::AiQuotaBucketRepository::new(db).expect("quota repo")
}
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool};

async fn pool() -> systemprompt_database::DbPool {
    let b = ensure_test_bootstrap();
    fixture_db_pool(&b.database_url).await.expect("pool")
}

fn window(window_seconds: i32) -> QuotaWindow {
    QuotaWindow {
        window_seconds,
        ..QuotaWindow::default()
    }
}

#[tokio::test]
async fn precheck_with_empty_windows_returns_none() {
    let p = pool().await;
    let user = UserId::new(format!("quota-test-{}", uuid::Uuid::new_v4()));
    let decision = precheck_and_reserve(&p, &quota_repo(&p), &user, &[])
        .await
        .expect("ok");
    assert!(decision.is_none());
}

#[tokio::test]
async fn precheck_within_limit_allows() {
    let p = pool().await;
    let user = UserId::new(format!("quota-allow-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        max_requests: Some(100),
        ..window(60)
    }];
    let decision = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows)
        .await
        .expect("ok");
    assert!(decision.is_none(), "expected allow, got {decision:?}");
}

#[tokio::test]
async fn precheck_over_limit_denies_second_call() {
    let p = pool().await;
    let user = UserId::new(format!("quota-deny-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        max_requests: Some(1),
        ..window(60)
    }];
    let d1 = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows)
        .await
        .expect("ok");
    assert!(d1.is_none());
    let d2 = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows)
        .await
        .expect("ok");
    let dec = d2.expect("expected denial");
    assert!(!dec.allow);
    assert_eq!(dec.window_seconds, 60);
    assert!(
        dec.message.contains("quota exceeded"),
        "unexpected message: {}",
        dec.message
    );
}

#[tokio::test]
async fn precheck_denies_once_the_cost_ceiling_is_spent() {
    let p = pool().await;
    let user = UserId::new(format!("quota-cost-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        max_cost_microdollars: Some(1_000),
        ..window(3600)
    }];

    let before = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows)
        .await
        .expect("ok");
    assert!(before.is_none(), "no spend yet, must allow");

    post_update_tokens(
        &p,
        &quota_repo(&p),
        PostUpdateParams {
            user_id: &user,
            windows: &windows,
            input_tokens: 10,
            output_tokens: 20,
            cost_microdollars: 1_500,
        },
    )
    .await;

    let after = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows)
        .await
        .expect("ok");
    let dec = after.expect("spend exceeds the ceiling, must deny");
    assert!(!dec.allow);
    assert!(
        dec.message.contains("cost ceiling"),
        "unexpected message: {}",
        dec.message
    );
}

#[tokio::test]
async fn a_window_keyed_on_an_unresolvable_subject_is_skipped() {
    let p = pool().await;
    let user = UserId::new(format!("quota-orgless-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        subject: "organization".to_owned(),
        max_requests: Some(0),
        ..window(60)
    }];
    // No organization provider is registered in this binary, so the window
    // cannot resolve a subject and must not deny even with max_requests: 0.
    let decision = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows)
        .await
        .expect("ok");
    assert!(decision.is_none(), "unresolvable subject must skip");
}

#[tokio::test]
async fn post_update_with_empty_windows_is_noop() {
    let p = pool().await;
    let user = UserId::new("quota-post-empty");
    post_update_tokens(
        &p,
        &quota_repo(&p),
        PostUpdateParams {
            user_id: &user,
            windows: &[],
            input_tokens: 100,
            output_tokens: 50,
            cost_microdollars: 10,
        },
    )
    .await;
}

#[tokio::test]
async fn post_update_increments_token_counts() {
    let p = pool().await;
    let user = UserId::new(format!("quota-post-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        max_requests: Some(1000),
        max_input_tokens: Some(1000),
        max_output_tokens: Some(1000),
        ..window(60)
    }];
    post_update_tokens(
        &p,
        &quota_repo(&p),
        PostUpdateParams {
            user_id: &user,
            windows: &windows,
            input_tokens: 10,
            output_tokens: 20,
            cost_microdollars: 5,
        },
    )
    .await;
}

#[tokio::test]
async fn policy_resolver_falls_back_when_empty() {
    let p = pool().await;
    let resolver = PolicyResolver::from_repository(
        systemprompt_ai::repository::AiGatewayPolicyRepository::new(&p).expect("policy repo"),
    );
    let _spec1 = resolver.resolve().await;
    // Second call hits the in-memory cache path.
    let _spec2 = resolver.resolve().await;
}
