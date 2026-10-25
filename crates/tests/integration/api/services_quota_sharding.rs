//! Sharded quota windows: a shared-subject window spreads its rows over
//! `shared_subject_shards`, admission checks the summed total, and
//! settlement lands on the shard the reservation used.

use systemprompt_gateway::policies::QuotaWindow;
use systemprompt_gateway::quota::{
    QuotaEstimate, QuotaReservation, QuotaSubjects, QuotaUsage, ReserveOutcome, ReserveParams,
    precheck_and_reserve, settle,
};
use systemprompt_governance::authz::{AuthzHookContext, NullAuditSink, SubjectProviderSet};
use systemprompt_identifiers::{ScopeDimension, UserId};
use systemprompt_manifest::profile::QuotaConfig;
use systemprompt_manifest::services::QuotaFaultMode;
use systemprompt_models::attribution::{AttributionEntry, AttributionSource, RequestAttribution};
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};

const DIMENSION: &str = "project";

async fn pool() -> systemprompt_database::DbPool {
    ensure_test_bootstrap();
    test_db_pool().await
}

fn project_window(max_requests: i64) -> QuotaWindow {
    QuotaWindow {
        window_seconds: 3600,
        subject: DIMENSION.to_owned(),
        max_requests: Some(max_requests),
        ..QuotaWindow::default()
    }
}

fn attributed(project: &str) -> RequestAttribution {
    RequestAttribution {
        entries: vec![AttributionEntry {
            dimension: ScopeDimension::new(DIMENSION),
            value: project.to_owned(),
            source: AttributionSource::Header,
        }],
        api_key_id: None,
    }
}

async fn reserve(
    db: &systemprompt_database::DbPool,
    project: &str,
    windows: &[QuotaWindow],
    sharding: QuotaConfig,
) -> ReserveOutcome {
    let attribution = attributed(project);
    let user = UserId::new(format!("quota-shard-{}", uuid::Uuid::new_v4()));
    precheck_and_reserve(
        &systemprompt_ai::repository::AiQuotaBucketRepository::new(db),
        ReserveParams {
            providers: &SubjectProviderSet::discover(&AuthzHookContext {
                pool: db.pool(),
                sink: std::sync::Arc::new(NullAuditSink),
                services: systemprompt_test_fixtures::fixture_authz_services(),
            }),
            subjects: QuotaSubjects {
                user_id: &user,
                api_key_id: None,
                attribution: &attribution,
            },
            windows,
            fault_mode: QuotaFaultMode::Closed,
            estimate: QuotaEstimate {
                cost_microdollars: 1_000,
                ..QuotaEstimate::default()
            },
            sharding,
        },
    )
    .await
    .expect("reservation write succeeds")
}

async fn shard_rows(db: &systemprompt_database::DbPool, project: &str) -> Vec<(i16, i64, i64)> {
    sqlx::query!(
        r#"SELECT shard, requests, cost_microdollars FROM ai_quota_buckets
           WHERE subject_kind = $1 AND subject_id = $2 ORDER BY shard"#,
        DIMENSION,
        project
    )
    .fetch_all(db.pool().as_ref())
    .await
    .expect("shard rows")
    .into_iter()
    .map(|r| (r.shard, r.requests, r.cost_microdollars))
    .collect()
}

#[tokio::test]
async fn a_sharded_window_denies_on_the_summed_total() {
    let db = pool().await;
    let project = format!("proj-{}", uuid::Uuid::new_v4());
    let windows = [project_window(5)];
    let sharding = QuotaConfig::default();

    for _ in 0..5 {
        assert!(matches!(
            reserve(&db, &project, &windows, sharding).await,
            ReserveOutcome::Admitted(_)
        ));
    }
    match reserve(&db, &project, &windows, sharding).await {
        ReserveOutcome::Denied { decision, .. } => {
            assert_eq!(decision.detail.used, Some(6));
            assert_eq!(decision.detail.limit, Some(5));
        },
        ReserveOutcome::Admitted(_) => panic!("sixth request must be denied on the total"),
    }

    let rows = shard_rows(&db, &project).await;
    assert!(rows.iter().all(|(shard, _, _)| (0..8).contains(shard)));
    assert_eq!(rows.iter().map(|(_, r, _)| r).sum::<i64>(), 6);
}

#[tokio::test]
async fn a_single_shard_config_keeps_one_row() {
    let db = pool().await;
    let project = format!("proj-{}", uuid::Uuid::new_v4());
    let windows = [project_window(100)];
    let sharding = QuotaConfig {
        shared_subject_shards: 1,
        principal_subject_shards: 1,
    };
    for _ in 0..6 {
        reserve(&db, &project, &windows, sharding).await;
    }
    assert_eq!(shard_rows(&db, &project).await, vec![(0, 6, 6_000)]);
}

#[tokio::test]
async fn settlement_trues_up_the_shard_it_reserved_on() {
    let db = pool().await;
    let project = format!("proj-{}", uuid::Uuid::new_v4());
    let windows = [project_window(100)];
    let reservation: QuotaReservation =
        match reserve(&db, &project, &windows, QuotaConfig::default()).await {
            ReserveOutcome::Admitted(reservation) => reservation,
            ReserveOutcome::Denied { .. } => panic!("first reservation is admitted"),
        };
    let shard = reservation.windows[0].shard;

    settle(
        &systemprompt_ai::repository::AiQuotaBucketRepository::new(&db),
        &reservation,
        QuotaUsage {
            cost_microdollars: 400,
            ..QuotaUsage::default()
        },
    )
    .await;

    assert_eq!(shard_rows(&db, &project).await, vec![(shard, 1, 400)]);
}

#[tokio::test]
async fn windows_sharing_one_bucket_charge_it_once() {
    let db = pool().await;
    let project = format!("proj-{}", uuid::Uuid::new_v4());
    let daily = QuotaWindow {
        window_seconds: 86_400,
        subject: DIMENSION.to_owned(),
        max_requests: Some(100),
        ..QuotaWindow::default()
    };
    let calendar_day = QuotaWindow {
        window_seconds: 0,
        calendar: Some(systemprompt_gateway::policies::CalendarPeriod::Day),
        ..daily.clone()
    };
    let sharding = QuotaConfig {
        shared_subject_shards: 1,
        principal_subject_shards: 1,
    };
    let reservation = match reserve(&db, &project, &[daily, calendar_day], sharding).await {
        ReserveOutcome::Admitted(reservation) => reservation,
        ReserveOutcome::Denied { .. } => panic!("first request must be admitted"),
    };
    assert_eq!(reservation.windows.len(), 1);
    assert_eq!(shard_rows(&db, &project).await, vec![(0, 1, 1_000)]);
}
