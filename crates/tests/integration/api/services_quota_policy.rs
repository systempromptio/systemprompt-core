//! `services::gateway::quota` + `services::gateway::policy` integration —
//! drives the quota repo for allow/deny decisions and the policy resolver
//! for the fall-through-to-permissive case, under both `QuotaFaultMode`
//! settings. Lives in the integration crate so we can pull the test-fixtures
//! DB pool.

use systemprompt_api::services::gateway::policy::{PolicyResolver, QuotaWindow};
use systemprompt_api::services::gateway::quota::{
    PostUpdateParams, post_update_tokens, precheck_and_reserve,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::services::QuotaFaultMode;

const ERROR_DIMENSION: &str = "quota_fault_error";
const EMPTY_DIMENSION: &str = "quota_fault_empty";
const ERROR_USER_PREFIX: &str = "quota-fault-error-";

#[derive(Debug)]
struct ErroringSubjectProvider;

#[async_trait::async_trait]
impl systemprompt_security::authz::SubjectAttributeProvider for ErroringSubjectProvider {
    fn dimension(&self) -> systemprompt_security::authz::SubjectDimension {
        systemprompt_security::authz::SubjectDimension {
            rule_type: systemprompt_security::authz::RuleType::extension(ERROR_DIMENSION)
                .expect("well-formed slug"),
            label: "Quota fault (error)",
            precedence: 900,
        }
    }

    async fn values_for(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<String>, systemprompt_security::authz::AuthzError> {
        if user_id.as_str().starts_with(ERROR_USER_PREFIX) {
            return Err(systemprompt_security::authz::AuthzError::Validation(
                "subject lookup unavailable".to_owned(),
            ));
        }
        Ok(vec!["tenant-a".to_owned()])
    }
}

#[derive(Debug)]
struct EmptySubjectProvider;

#[async_trait::async_trait]
impl systemprompt_security::authz::SubjectAttributeProvider for EmptySubjectProvider {
    fn dimension(&self) -> systemprompt_security::authz::SubjectDimension {
        systemprompt_security::authz::SubjectDimension {
            rule_type: systemprompt_security::authz::RuleType::extension(EMPTY_DIMENSION)
                .expect("well-formed slug"),
            label: "Quota fault (empty)",
            precedence: 901,
        }
    }

    async fn values_for(
        &self,
        _user_id: &UserId,
    ) -> Result<Vec<String>, systemprompt_security::authz::AuthzError> {
        Ok(Vec::new())
    }
}

systemprompt_security::register_subject_attribute_provider!(|_ctx| std::sync::Arc::new(
    ErroringSubjectProvider
));
systemprompt_security::register_subject_attribute_provider!(|_ctx| std::sync::Arc::new(
    EmptySubjectProvider
));

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
    let decision = precheck_and_reserve(&p, &quota_repo(&p), &user, &[], QuotaFaultMode::Open)
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
    let decision = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows, QuotaFaultMode::Open)
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
    let d1 = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows, QuotaFaultMode::Open)
        .await
        .expect("ok");
    assert!(d1.is_none());
    let d2 = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows, QuotaFaultMode::Open)
        .await
        .expect("ok");
    let dec = d2.expect("expected denial");
    assert!(!dec.allow);
    assert_eq!(dec.window_seconds, 60);
    assert!(
        dec.message.contains("request ceiling exceeded"),
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

    let before = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows, QuotaFaultMode::Open)
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

    let after = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows, QuotaFaultMode::Open)
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

fn subject_window(subject: &str) -> QuotaWindow {
    QuotaWindow {
        subject: subject.to_owned(),
        max_requests: Some(0),
        ..window(60)
    }
}

async fn fault_decision(
    p: &systemprompt_database::DbPool,
    user: &UserId,
    subject: &str,
    mode: QuotaFaultMode,
) -> Option<systemprompt_api::services::gateway::quota::QuotaDecision> {
    precheck_and_reserve(p, &quota_repo(p), user, &[subject_window(subject)], mode)
        .await
        .expect("ok")
}

#[tokio::test]
async fn a_window_with_no_registered_provider_is_skipped_when_open() {
    let p = pool().await;
    let user = UserId::new(format!("quota-orgless-{}", uuid::Uuid::new_v4()));
    let decision = fault_decision(&p, &user, "organization", QuotaFaultMode::Open).await;
    assert!(decision.is_none(), "unresolvable subject must skip");
}

#[tokio::test]
async fn a_window_with_no_registered_provider_denies_when_closed() {
    let p = pool().await;
    let user = UserId::new(format!("quota-orgless-{}", uuid::Uuid::new_v4()));
    let dec = fault_decision(&p, &user, "organization", QuotaFaultMode::Closed)
        .await
        .expect("closed mode must deny an unevaluable window");
    assert!(!dec.allow);
    assert!(
        dec.message.contains("could not be evaluated"),
        "unexpected message: {}",
        dec.message
    );
}

#[tokio::test]
async fn a_failing_subject_provider_is_skipped_when_open() {
    let p = pool().await;
    let user = UserId::new(format!("{ERROR_USER_PREFIX}{}", uuid::Uuid::new_v4()));
    let decision = fault_decision(&p, &user, ERROR_DIMENSION, QuotaFaultMode::Open).await;
    assert!(decision.is_none(), "open mode must skip a provider fault");
}

#[tokio::test]
async fn a_failing_subject_provider_denies_when_closed() {
    let p = pool().await;
    let user = UserId::new(format!("{ERROR_USER_PREFIX}{}", uuid::Uuid::new_v4()));
    let dec = fault_decision(&p, &user, ERROR_DIMENSION, QuotaFaultMode::Closed)
        .await
        .expect("closed mode must deny on a provider fault");
    assert!(!dec.allow);
    assert!(
        dec.message.contains("subject attribute provider failed"),
        "unexpected message: {}",
        dec.message
    );
}

#[tokio::test]
async fn an_empty_subject_provider_answer_is_skipped_when_open() {
    let p = pool().await;
    let user = UserId::new(format!("quota-empty-{}", uuid::Uuid::new_v4()));
    let decision = fault_decision(&p, &user, EMPTY_DIMENSION, QuotaFaultMode::Open).await;
    assert!(decision.is_none(), "open mode must skip an empty answer");
}

#[tokio::test]
async fn an_empty_subject_provider_answer_denies_when_closed() {
    let p = pool().await;
    let user = UserId::new(format!("quota-empty-{}", uuid::Uuid::new_v4()));
    let dec = fault_decision(&p, &user, EMPTY_DIMENSION, QuotaFaultMode::Closed)
        .await
        .expect("closed mode must deny on an empty answer");
    assert!(!dec.allow);
    assert!(
        dec.message.contains("returned no value"),
        "unexpected message: {}",
        dec.message
    );
}

#[tokio::test]
async fn a_resolvable_subject_still_enforces_its_ceiling_under_both_modes() {
    let p = pool().await;
    for mode in [QuotaFaultMode::Open, QuotaFaultMode::Closed] {
        let user = UserId::new(format!("quota-resolvable-{}", uuid::Uuid::new_v4()));
        let dec = fault_decision(&p, &user, ERROR_DIMENSION, mode)
            .await
            .expect("max_requests 0 must deny");
        assert!(!dec.allow);
        assert!(
            dec.message.contains("request ceiling exceeded"),
            "unexpected message for {mode:?}: {}",
            dec.message
        );
    }
}

#[tokio::test]
async fn precheck_denies_once_the_input_token_ceiling_is_spent() {
    let p = pool().await;
    let user = UserId::new(format!("quota-input-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        max_input_tokens: Some(100),
        ..window(3600)
    }];
    assert!(
        precheck_and_reserve(&p, &quota_repo(&p), &user, &windows, QuotaFaultMode::Open)
            .await
            .expect("ok")
            .is_none()
    );

    post_update_tokens(
        &p,
        &quota_repo(&p),
        PostUpdateParams {
            user_id: &user,
            windows: &windows,
            input_tokens: 500,
            output_tokens: 0,
            cost_microdollars: 0,
        },
    )
    .await;

    let dec = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows, QuotaFaultMode::Open)
        .await
        .expect("ok")
        .expect("input tokens exceed the ceiling, must deny");
    assert!(!dec.allow);
    assert!(
        dec.message.contains("input token ceiling exceeded"),
        "unexpected message: {}",
        dec.message
    );
}

#[tokio::test]
async fn precheck_denies_once_the_output_token_ceiling_is_spent() {
    let p = pool().await;
    let user = UserId::new(format!("quota-output-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        max_output_tokens: Some(100),
        ..window(3600)
    }];
    post_update_tokens(
        &p,
        &quota_repo(&p),
        PostUpdateParams {
            user_id: &user,
            windows: &windows,
            input_tokens: 0,
            output_tokens: 500,
            cost_microdollars: 0,
        },
    )
    .await;

    let dec = precheck_and_reserve(&p, &quota_repo(&p), &user, &windows, QuotaFaultMode::Open)
        .await
        .expect("ok")
        .expect("output tokens exceed the ceiling, must deny");
    assert!(!dec.allow);
    assert!(
        dec.message.contains("output token ceiling exceeded"),
        "unexpected message: {}",
        dec.message
    );
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
    let _spec1 = resolver
        .resolve(QuotaFaultMode::Open)
        .await
        .expect("healthy DB resolves");
    // Second call hits the in-memory cache path.
    let _spec2 = resolver
        .resolve(QuotaFaultMode::Closed)
        .await
        .expect("healthy DB resolves in closed mode too");
}

#[tokio::test]
async fn policy_resolver_degrades_to_permissive_when_open_and_the_read_fails() {
    let resolver = unreadable_policy_resolver();
    let spec = resolver
        .resolve(QuotaFaultMode::Open)
        .await
        .expect("open mode must degrade rather than fail");
    assert!(
        spec.quota_windows.is_empty() && spec.safety.scanners.is_empty(),
        "the permissive fallback carries neither quotas nor scanners"
    );
}

#[tokio::test]
async fn policy_resolver_denies_when_closed_and_the_read_fails() {
    let resolver = unreadable_policy_resolver();
    let err = resolver
        .resolve(QuotaFaultMode::Closed)
        .await
        .expect_err("closed mode must refuse to guess a policy");
    assert!(
        err.to_string().contains("gateway policy unavailable"),
        "unexpected error: {err}"
    );
}

fn unreadable_policy_resolver() -> PolicyResolver {
    // A lazily-connected pool aimed at a database that does not exist: every
    // query fails at connect time, which is the fault this exercises.
    let dead = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://nobody:nobody@127.0.0.1:1/does-not-exist")
        .expect("lazy pool");
    let db: systemprompt_database::DbPool =
        std::sync::Arc::new(systemprompt_database::Database::from_pools(
            std::sync::Arc::new(dead.clone()),
            Some(std::sync::Arc::new(dead)),
        ));
    PolicyResolver::from_repository(
        systemprompt_ai::repository::AiGatewayPolicyRepository::new(&db).expect("policy repo"),
    )
}
