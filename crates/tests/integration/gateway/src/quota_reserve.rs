//! `precheck_and_reserve` / `settle` / `release` against live quota buckets:
//! windows that share a bucket charge it once, a window past its ceiling
//! denies and takes back the charges of the windows after it, an unresolvable
//! subject follows the fault mode, and settlement trues the reservation up to
//! audited usage.

use systemprompt_ai::repository::AiQuotaBucketRepository;
use systemprompt_database::DbPool;
use systemprompt_gateway::GatewayRepositories;
use systemprompt_gateway::policies::QuotaWindow;
use systemprompt_gateway::quota::{
    AccountingOutcome, QuotaEstimate, QuotaReservation, QuotaSubjects, QuotaUsage, ReserveOutcome,
    ReserveParams, precheck_and_reserve, release, settle,
};
use systemprompt_identifiers::{QuotaSubjectKind, UserId};
use systemprompt_manifest::services::QuotaFaultMode;
use systemprompt_models::attribution::RequestAttribution;

use crate::support::{seed_user, setup_db};

fn repos(db: &DbPool) -> GatewayRepositories {
    GatewayRepositories::new(
        db,
        systemprompt_gateway::audit::journal::GatewayJournal::open(
            systemprompt_test_fixtures::ensure_test_bootstrap()
                .app_paths
                .storage()
                .data(),
            systemprompt_config::SecretsBootstrap::encryption_master_key()
                .expect("encryption master key"),
        )
        .expect("gateway journal opens"),
        std::sync::Arc::new(systemprompt_agent::services::ContextProviderService::new(
            systemprompt_agent::repository::ContextRepository::new(db),
        )),
        systemprompt_gateway::GatewayHandles {
            background: systemprompt_traits::BackgroundTasks::new(),
            cache_epochs: systemprompt_traits::CacheEpochs::new(),
            logging: systemprompt_test_fixtures::fixture_logging_runtime(),
            services: systemprompt_test_fixtures::fixture_services_view(),
            subject_providers: systemprompt_test_fixtures::fixture_subject_providers(db),
        },
    )
}

fn dead_pool() -> DbPool {
    let dead = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_millis(250))
        .connect_lazy("postgres://nobody:nobody@127.0.0.1:1/does-not-exist")
        .expect("lazy pool");
    std::sync::Arc::new(systemprompt_database::Database::from_pools(
        std::sync::Arc::new(dead.clone()),
        Some(std::sync::Arc::new(dead)),
    ))
}

fn window(subject: &str, seconds: i32, max_requests: Option<i64>) -> QuotaWindow {
    QuotaWindow {
        window_seconds: seconds,
        subject: subject.to_owned(),
        max_requests,
        ..QuotaWindow::default()
    }
}

const ESTIMATE: QuotaEstimate = QuotaEstimate {
    input_tokens: 50,
    output_tokens: 60,
    cost_microdollars: 70,
};

async fn reserve(
    repos: &GatewayRepositories,
    user: &UserId,
    windows: &[QuotaWindow],
    fault_mode: QuotaFaultMode,
) -> ReserveOutcome {
    let attribution = RequestAttribution::none();
    precheck_and_reserve(
        &repos.quota_buckets,
        ReserveParams {
            sharding: systemprompt_manifest::profile::QuotaConfig::default(),
            providers: &repos.subject_providers,
            subjects: QuotaSubjects {
                user_id: user,
                api_key_id: None,
                attribution: &attribution,
            },
            windows,
            fault_mode,
            estimate: ESTIMATE,
        },
    )
    .await
    .expect("reserve")
}

async fn bucket(db: &DbPool, user: &UserId, seconds: i32) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT COALESCE(SUM(requests),0)::BIGINT, COALESCE(SUM(input_tokens),0)::BIGINT, \
         COALESCE(SUM(output_tokens),0)::BIGINT, COALESCE(SUM(cost_microdollars),0)::BIGINT \
         FROM ai_quota_buckets WHERE subject_kind = 'user' AND subject_id = $1 \
         AND window_seconds = $2",
    )
    .bind(user.as_str())
    .bind(seconds)
    .fetch_one(db.pool().as_ref())
    .await
    .expect("bucket totals")
}

fn admitted(outcome: ReserveOutcome) -> QuotaReservation {
    match outcome {
        ReserveOutcome::Admitted(reservation) => reservation,
        ReserveOutcome::Denied { decision, .. } => panic!("expected admission: {decision:?}"),
    }
}

#[tokio::test]
async fn no_windows_admit_with_an_empty_reservation() {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let reservation = admitted(reserve(&repos(&db), &user, &[], QuotaFaultMode::Closed).await);
    assert!(reservation.is_empty());
}

#[tokio::test]
async fn two_windows_on_one_bucket_charge_it_once() {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let windows = [window("user", 3600, Some(5)), window("user", 3600, Some(5))];
    let reservation = admitted(reserve(&repos(&db), &user, &windows, QuotaFaultMode::Closed).await);
    assert_eq!(reservation.windows.len(), 1);
    assert_eq!(bucket(&db, &user, 3600).await, (1, 50, 60, 70));
}

#[tokio::test]
async fn a_ceiling_denies_and_takes_back_the_later_windows() {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let repos = repos(&db);
    let windows = [window("user", 600, Some(1)), window("user", 7200, None)];

    admitted(reserve(&repos, &user, &windows, QuotaFaultMode::Closed).await);
    let outcome = reserve(&repos, &user, &windows, QuotaFaultMode::Closed).await;
    let ReserveOutcome::Denied {
        decision,
        reservation,
    } = outcome
    else {
        panic!("the second request exceeds max_requests 1");
    };
    assert!(!decision.allow);
    assert_eq!(decision.window_seconds, 600);
    assert_eq!(
        reservation.windows.len(),
        1,
        "the denying window's charge is handed back to the caller"
    );
    assert_eq!(bucket(&db, &user, 600).await.0, 2);
    assert_eq!(
        bucket(&db, &user, 7200).await,
        (1, 50, 60, 70),
        "the window after the denial is unreserved"
    );
}

#[tokio::test]
async fn an_unresolvable_subject_denies_when_closed() {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let windows = [window("user", 900, None), window("api_key", 900, Some(1))];
    let outcome = reserve(&repos(&db), &user, &windows, QuotaFaultMode::Closed).await;
    let ReserveOutcome::Denied {
        decision,
        reservation,
    } = outcome
    else {
        panic!("no API key under a closed fault mode must deny");
    };
    assert!(!decision.allow);
    assert_eq!(reservation.windows.len(), 1);
}

#[tokio::test]
async fn an_unresolvable_subject_is_skipped_when_open() {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let windows = [window("api_key", 1800, Some(1)), window("user", 1800, None)];
    let reservation = admitted(reserve(&repos(&db), &user, &windows, QuotaFaultMode::Open).await);
    assert_eq!(reservation.windows.len(), 1);
    assert_eq!(reservation.windows[0].subject_kind, QuotaSubjectKind::User);
}

#[tokio::test]
async fn settle_trues_up_and_release_returns_the_estimate() {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let repos = repos(&db);
    let windows = [window("user", 1200, None)];

    let first = admitted(reserve(&repos, &user, &windows, QuotaFaultMode::Closed).await);
    let outcome = settle(
        &repos.quota_buckets,
        &first,
        QuotaUsage {
            input_tokens: 10,
            output_tokens: 20,
            cost_microdollars: 5,
        },
    )
    .await;
    assert!(matches!(outcome, AccountingOutcome::Counted), "{outcome:?}");
    assert_eq!(bucket(&db, &user, 1200).await, (1, 10, 20, 5));

    let second = admitted(reserve(&repos, &user, &windows, QuotaFaultMode::Closed).await);
    let outcome = release(&repos.quota_buckets, &second).await;
    assert!(matches!(outcome, AccountingOutcome::Counted), "{outcome:?}");
    assert_eq!(
        bucket(&db, &user, 1200).await,
        (2, 10, 20, 5),
        "a released request keeps its request count but returns its tokens and cost"
    );
}

#[tokio::test]
async fn settle_against_an_unreachable_database_is_faulted() {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let reservation = admitted(
        reserve(
            &repos(&db),
            &user,
            &[window("user", 2400, None)],
            QuotaFaultMode::Closed,
        )
        .await,
    );
    let dead = AiQuotaBucketRepository::new(&dead_pool());
    match settle(&dead, &reservation, QuotaUsage::default()).await {
        AccountingOutcome::Faulted { message } => {
            assert!(message.contains("1 window(s)"), "{message}");
        },
        AccountingOutcome::Counted => panic!("a dead pool cannot count"),
    }
}
