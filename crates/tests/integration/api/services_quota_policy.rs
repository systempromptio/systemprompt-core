//! `services::gateway::quota` + `services::gateway::policy` integration —
//! drives the quota repo for allow/deny decisions and the policy resolver
//! for the fall-through-to-permissive case, under both `QuotaFaultMode`
//! settings. Lives in the integration crate so we can pull the test-fixtures
//! DB pool.

use systemprompt_gateway::policies::{PolicyResolver, QuotaWindow, merge_policy_rows};
use systemprompt_gateway::quota::{
    AccountingOutcome, QuotaDecision, QuotaDimension, QuotaEstimate, QuotaReservation,
    QuotaSubjects, QuotaUsage, ReserveOutcome, ReserveParams, ReservedWindow, precheck_and_reserve,
    release, settle,
};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::QuotaFaultMode;
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_security::authz::{AuthzHookContext, NullAuditSink, SubjectProviderSet};

const ERROR_DIMENSION: &str = "quota_fault_error";
const EMPTY_DIMENSION: &str = "quota_fault_empty";
const ERROR_USER_PREFIX: &str = "quota-fault-error-";
const ERROR_RULE_TYPE: systemprompt_security::authz::RuleType =
    systemprompt_security::authz::RuleType::extension_static(ERROR_DIMENSION);
const EMPTY_RULE_TYPE: systemprompt_security::authz::RuleType =
    systemprompt_security::authz::RuleType::extension_static(EMPTY_DIMENSION);

#[derive(Debug)]
struct ErroringSubjectProvider;

#[async_trait::async_trait]
impl systemprompt_security::authz::SubjectAttributeProvider for ErroringSubjectProvider {
    fn dimension(&self) -> systemprompt_security::authz::SubjectDimension {
        systemprompt_security::authz::SubjectDimension {
            rule_type: ERROR_RULE_TYPE,
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
            rule_type: EMPTY_RULE_TYPE,
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
    systemprompt_ai::repository::AiQuotaBucketRepository::new(db)
}
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};

async fn pool() -> systemprompt_database::DbPool {
    ensure_test_bootstrap();
    test_db_pool().await
}

fn window(window_seconds: i32) -> QuotaWindow {
    QuotaWindow {
        window_seconds,
        ..QuotaWindow::default()
    }
}

fn providers(p: &systemprompt_database::DbPool) -> SubjectProviderSet {
    SubjectProviderSet::discover(&AuthzHookContext {
        pool: p.pool(),
        sink: std::sync::Arc::new(NullAuditSink),
    })
}

async fn reserve_with(
    p: &systemprompt_database::DbPool,
    user: &UserId,
    windows: &[QuotaWindow],
    mode: QuotaFaultMode,
    estimate: QuotaEstimate,
) -> ReserveOutcome {
    let attribution = RequestAttribution::none();
    precheck_and_reserve(
        &quota_repo(p),
        ReserveParams {
            providers: &providers(p),
            subjects: QuotaSubjects {
                user_id: user,
                api_key_id: None,
                attribution: &attribution,
            },
            windows,
            fault_mode: mode,
            estimate,
        },
    )
    .await
    .expect("reservation write succeeds")
}

async fn reserve(
    p: &systemprompt_database::DbPool,
    user: &UserId,
    windows: &[QuotaWindow],
    mode: QuotaFaultMode,
) -> Option<QuotaDecision> {
    match reserve_with(p, user, windows, mode, QuotaEstimate::default()).await {
        ReserveOutcome::Admitted(_) => None,
        ReserveOutcome::Denied { decision, .. } => Some(decision),
    }
}

async fn admitted(
    p: &systemprompt_database::DbPool,
    user: &UserId,
    windows: &[QuotaWindow],
) -> QuotaReservation {
    match reserve_with(
        p,
        user,
        windows,
        QuotaFaultMode::Open,
        QuotaEstimate::default(),
    )
    .await
    {
        ReserveOutcome::Admitted(reservation) => reservation,
        ReserveOutcome::Denied { decision, .. } => panic!("expected admission: {decision:?}"),
    }
}

async fn bucket(p: &systemprompt_database::DbPool, user: &UserId) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT requests, input_tokens, output_tokens, cost_microdollars FROM ai_quota_buckets \
         WHERE subject_kind = 'user' AND subject_id = $1",
    )
    .bind(user.as_str())
    .fetch_one(p.pool().as_ref())
    .await
    .expect("bucket row")
}

#[tokio::test]
async fn precheck_with_empty_windows_admits_with_an_empty_reservation() {
    let p = pool().await;
    let user = UserId::new(format!("quota-test-{}", uuid::Uuid::new_v4()));
    let reservation = admitted(&p, &user, &[]).await;
    assert!(reservation.is_empty());
}

#[tokio::test]
async fn precheck_within_limit_allows() {
    let p = pool().await;
    let user = UserId::new(format!("quota-allow-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        max_requests: Some(100),
        ..window(60)
    }];
    let decision = reserve(&p, &user, &windows, QuotaFaultMode::Open).await;
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
    assert!(
        reserve(&p, &user, &windows, QuotaFaultMode::Open)
            .await
            .is_none()
    );
    let dec = reserve(&p, &user, &windows, QuotaFaultMode::Open)
        .await
        .expect("expected denial");
    assert!(!dec.allow);
    assert_eq!(dec.window_seconds, 60);
    assert!(
        dec.message.contains("request ceiling exceeded"),
        "unexpected message: {}",
        dec.message
    );
    assert_eq!(dec.detail.dimension, Some(QuotaDimension::Requests));
    assert_eq!(dec.detail.limit, Some(1));
    assert_eq!(dec.detail.used, Some(2));
    assert_eq!(dec.detail.subject, "user");
    assert!((1..=60).contains(&dec.detail.retry_after_seconds));
}

#[tokio::test]
async fn admission_reserves_the_estimate_in_the_bucket() {
    let p = pool().await;
    let user = UserId::new(format!("quota-estimate-{}", uuid::Uuid::new_v4()));
    let windows = vec![window(3600)];
    let estimate = QuotaEstimate {
        input_tokens: 10,
        output_tokens: 20,
        cost_microdollars: 30,
    };
    let outcome = reserve_with(&p, &user, &windows, QuotaFaultMode::Open, estimate).await;
    assert!(matches!(outcome, ReserveOutcome::Admitted(_)));
    assert_eq!(bucket(&p, &user).await, (1, 10, 20, 30));
}

#[tokio::test]
async fn an_estimate_above_the_cost_ceiling_denies_the_first_request() {
    let p = pool().await;
    let user = UserId::new(format!("quota-cost-first-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        max_cost_microdollars: Some(1_000),
        ..window(3600)
    }];
    let estimate = QuotaEstimate {
        cost_microdollars: 1_500,
        ..QuotaEstimate::default()
    };
    let ReserveOutcome::Denied { decision, .. } =
        reserve_with(&p, &user, &windows, QuotaFaultMode::Open, estimate).await
    else {
        panic!("in-flight spend above the ceiling must deny");
    };
    assert_eq!(
        decision.detail.dimension,
        Some(QuotaDimension::CostMicrodollars)
    );
    assert_eq!(decision.detail.limit, Some(1_000));
    assert_eq!(decision.detail.used, Some(1_500));
}

#[tokio::test]
async fn release_after_denial_keeps_request_count_and_returns_cost() {
    let p = pool().await;
    let user = UserId::new(format!("quota-release-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        max_cost_microdollars: Some(1_000),
        ..window(3600)
    }];
    let estimate = QuotaEstimate {
        input_tokens: 40,
        output_tokens: 60,
        cost_microdollars: 1_500,
    };
    let ReserveOutcome::Denied { reservation, .. } =
        reserve_with(&p, &user, &windows, QuotaFaultMode::Open, estimate).await
    else {
        panic!("expected denial");
    };
    let outcome = release(&quota_repo(&p), &reservation).await;
    assert!(matches!(outcome, AccountingOutcome::Counted));
    assert_eq!(bucket(&p, &user).await, (1, 0, 0, 0));
}

#[tokio::test]
async fn settle_trues_the_bucket_up_to_actual_usage() {
    let p = pool().await;
    let user = UserId::new(format!("quota-settle-{}", uuid::Uuid::new_v4()));
    let windows = vec![window(3600)];
    let estimate = QuotaEstimate {
        input_tokens: 100,
        output_tokens: 400,
        cost_microdollars: 900,
    };
    let ReserveOutcome::Admitted(reservation) =
        reserve_with(&p, &user, &windows, QuotaFaultMode::Open, estimate).await
    else {
        panic!("expected admission");
    };
    let actual = QuotaUsage {
        input_tokens: 70,
        output_tokens: 30,
        cost_microdollars: 120,
    };
    assert!(matches!(
        settle(&quota_repo(&p), &reservation, actual).await,
        AccountingOutcome::Counted
    ));
    assert_eq!(bucket(&p, &user).await, (1, 70, 30, 120));
}

#[tokio::test]
async fn settle_trues_up_the_reserved_bucket_across_a_window_boundary() {
    let p = pool().await;
    let user = UserId::new(format!("quota-boundary-{}", uuid::Uuid::new_v4()));
    let repo = quota_repo(&p);
    let admitted_at = chrono::Utc::now() - chrono::Duration::hours(2);
    let window_start = chrono::DateTime::from_timestamp((admitted_at.timestamp() / 60) * 60, 0)
        .expect("aligned start");
    let delta = systemprompt_ai::repository::QuotaBucketDelta {
        requests: 1,
        input_tokens: 50,
        output_tokens: 50,
        cost_microdollars: 500,
    };
    repo.increment(systemprompt_ai::repository::IncrementParams {
        subject_kind: "user",
        subject_id: user.as_str(),
        window_seconds: 60,
        window_start,
        delta,
    })
    .await
    .expect("seed the admitted bucket");
    let reservation = QuotaReservation {
        windows: vec![ReservedWindow {
            subject_kind: "user".to_owned(),
            subject_id: user.as_str().to_owned(),
            window_seconds: 60,
            window_start,
            delta,
        }],
    };
    let actual = QuotaUsage {
        input_tokens: 5,
        output_tokens: 6,
        cost_microdollars: 7,
    };
    assert!(matches!(
        settle(&repo, &reservation, actual).await,
        AccountingOutcome::Counted
    ));
    let rows: Vec<(chrono::DateTime<chrono::Utc>, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT window_start, requests, input_tokens, output_tokens, cost_microdollars \
         FROM ai_quota_buckets WHERE subject_kind = 'user' AND subject_id = $1",
    )
    .bind(user.as_str())
    .fetch_all(p.pool().as_ref())
    .await
    .expect("bucket rows");
    assert_eq!(rows, vec![(window_start, 1, 5, 6, 7)]);
}

#[tokio::test]
async fn an_attributed_scope_keys_its_window_by_the_attributed_value() {
    let p = pool().await;
    let user = UserId::new(format!("quota-scope-{}", uuid::Uuid::new_v4()));
    let value = format!("tenant-{}", uuid::Uuid::new_v4());
    let attribution = RequestAttribution {
        entries: vec![systemprompt_models::attribution::AttributionEntry {
            dimension: systemprompt_identifiers::ScopeDimension::try_new(ERROR_DIMENSION)
                .expect("dimension"),
            value: value.clone(),
            source: systemprompt_models::attribution::AttributionSource::Header,
        }],
        api_key_id: None,
    };
    let windows = vec![QuotaWindow {
        subject: ERROR_DIMENSION.to_owned(),
        max_requests: Some(10),
        ..window(60)
    }];
    let outcome = precheck_and_reserve(
        &quota_repo(&p),
        ReserveParams {
            providers: &providers(&p),
            subjects: QuotaSubjects {
                user_id: &user,
                api_key_id: None,
                attribution: &attribution,
            },
            windows: &windows,
            fault_mode: QuotaFaultMode::Closed,
            estimate: QuotaEstimate::default(),
        },
    )
    .await
    .expect("reserve");
    let ReserveOutcome::Admitted(reservation) = outcome else {
        panic!("expected admission");
    };
    assert_eq!(reservation.windows[0].subject_kind, ERROR_DIMENSION);
    assert_eq!(reservation.windows[0].subject_id, value);
}

#[tokio::test]
async fn an_api_key_window_without_a_key_denies_when_closed() {
    let p = pool().await;
    let user = UserId::new(format!("quota-nokey-{}", uuid::Uuid::new_v4()));
    let dec = reserve(
        &p,
        &user,
        &[subject_window("api_key")],
        QuotaFaultMode::Closed,
    )
    .await
    .expect("closed mode denies an api_key window with no key");
    assert!(
        dec.message.contains("not authenticated by an API key"),
        "{}",
        dec.message
    );
    assert_eq!(dec.detail.dimension, None);
}

#[tokio::test]
async fn precheck_denies_once_the_cost_ceiling_is_spent() {
    let p = pool().await;
    let user = UserId::new(format!("quota-cost-{}", uuid::Uuid::new_v4()));
    let windows = vec![QuotaWindow {
        max_cost_microdollars: Some(1_000),
        ..window(3600)
    }];
    let reservation = admitted(&p, &user, &windows).await;
    settle(
        &quota_repo(&p),
        &reservation,
        QuotaUsage {
            input_tokens: 10,
            output_tokens: 20,
            cost_microdollars: 1_500,
        },
    )
    .await;
    let dec = reserve(&p, &user, &windows, QuotaFaultMode::Open)
        .await
        .expect("spend exceeds the ceiling, must deny");
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
) -> Option<QuotaDecision> {
    reserve(p, user, &[subject_window(subject)], mode).await
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
    let reservation = admitted(&p, &user, &windows).await;
    settle(
        &quota_repo(&p),
        &reservation,
        QuotaUsage {
            input_tokens: 500,
            ..QuotaUsage::default()
        },
    )
    .await;
    let dec = reserve(&p, &user, &windows, QuotaFaultMode::Open)
        .await
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
    let reservation = admitted(&p, &user, &windows).await;
    settle(
        &quota_repo(&p),
        &reservation,
        QuotaUsage {
            output_tokens: 500,
            ..QuotaUsage::default()
        },
    )
    .await;
    let dec = reserve(&p, &user, &windows, QuotaFaultMode::Open)
        .await
        .expect("output tokens exceed the ceiling, must deny");
    assert!(!dec.allow);
    assert!(
        dec.message.contains("output token ceiling exceeded"),
        "unexpected message: {}",
        dec.message
    );
}

#[tokio::test]
async fn settling_an_empty_reservation_writes_nothing() {
    let p = pool().await;
    let outcome = settle(
        &quota_repo(&p),
        &QuotaReservation::default(),
        QuotaUsage {
            input_tokens: 100,
            output_tokens: 50,
            cost_microdollars: 10,
        },
    )
    .await;
    assert!(matches!(outcome, AccountingOutcome::Counted));
}

#[tokio::test]
async fn policy_resolver_falls_back_when_empty() {
    let p = pool().await;
    let resolver = PolicyResolver::from_repository(
        systemprompt_ai::repository::AiGatewayPolicyRepository::new(&p),
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
    PolicyResolver::from_repository(systemprompt_ai::repository::AiGatewayPolicyRepository::new(
        &db,
    ))
}

fn policy_row(name: &str, spec: serde_json::Value) -> systemprompt_ai::GatewayPolicyRow {
    systemprompt_ai::GatewayPolicyRow {
        id: systemprompt_identifiers::AiGatewayPolicyId::generate(),
        name: name.to_owned(),
        spec,
        enabled: true,
        priority: 0,
    }
}

#[test]
fn well_formed_policy_rows_merge() {
    let merged = merge_policy_rows(vec![policy_row("base", serde_json::json!({}))]);
    assert!(merged.is_ok(), "{merged:?}");
}

#[test]
fn a_malformed_policy_row_fails_the_merge_instead_of_being_skipped() {
    let rows = vec![
        policy_row("base", serde_json::json!({})),
        policy_row("typo", serde_json::json!({ "quota_windos": [] })),
    ];
    let err = merge_policy_rows(rows).expect_err("a malformed row must not be dropped");
    assert_eq!(err.name, "typo");
}
