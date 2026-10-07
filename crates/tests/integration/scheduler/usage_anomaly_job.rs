use std::sync::Arc;

use chrono::{DateTime, Duration, TimeZone, Utc};
use systemprompt_ai::repository::{AiUsageAnomalyRepository, HourlyUsageProfile};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_scheduler::UsageAnomalyScanJob;
use systemprompt_scheduler::jobs::usage_anomaly_scan::{
    AnomalyKind, evaluate, previous_hour, scan_window,
};
use systemprompt_test_fixtures::{DisposableDb, fixture_actor};
use systemprompt_traits::{Dependencies, Job, JobContext};

async fn seed(pool: &DbPool, user: &str, at: DateTime<Utc>, count: i64, cost_each: i64) {
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, status, \
         cost_microdollars, actor_kind, actor_id, created_at) \
         SELECT 'req-' || md5(random()::text || g::text), 'rq-' || md5(random()::text || g::text), \
                $1, 'ctx-anomaly', 'anthropic', 'claude-test', 'completed', $2, 'user', $1, \
                $3 + (g || ' seconds')::interval \
         FROM generate_series(1, $4) AS g",
    )
    .bind(user)
    .bind(cost_each)
    .bind(at)
    .bind(count)
    .execute(pool.pool().as_ref())
    .await
    .expect("seed ai_requests");
}

fn window() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 30, 14, 0, 0)
        .single()
        .expect("window start")
}

async fn seed_baseline(pool: &DbPool, user: &str) {
    for day in 1..=6 {
        for hour in [9, 13, 17] {
            let at = window() - Duration::days(day) + Duration::hours(hour - 14);
            seed(pool, user, at, 10, 20_000).await;
        }
    }
}

#[tokio::test]
async fn a_spike_over_the_baseline_is_flagged_and_a_flat_profile_is_not() {
    let database = DisposableDb::with_schema("usage_anomaly").await;
    let pool = database.test_pool().await;
    seed_baseline(&pool, "anomaly-spiky").await;
    seed_baseline(&pool, "anomaly-flat").await;
    seed(
        &pool,
        "anomaly-spiky",
        window() + Duration::minutes(5),
        400,
        10_000,
    )
    .await;
    seed(
        &pool,
        "anomaly-flat",
        window() + Duration::minutes(5),
        10,
        20_000,
    )
    .await;
    seed(
        &pool,
        "anomaly-spiky",
        window() + Duration::hours(1),
        900,
        10_000,
    )
    .await;

    let repository = AiUsageAnomalyRepository::new(&pool);
    let anomalies = scan_window(&repository, window()).await.expect("scan");
    let flagged: Vec<(&str, AnomalyKind)> = anomalies
        .iter()
        .map(|a| (a.profile.user_id.as_str(), a.kind))
        .collect();
    assert_eq!(
        flagged,
        vec![
            ("anomaly-spiky", AnomalyKind::Spend),
            ("anomaly-spiky", AnomalyKind::RequestRate),
        ]
    );
    let spike = &anomalies[0].profile;
    assert_eq!(spike.observed_requests, 400);
    assert_eq!(spike.observed_cost_microdollars, 4_000_000);
    assert!((spike.baseline_requests_per_hour - 180.0 / 168.0).abs() < 1e-9);

    let ctx = JobContext::new(fixture_actor(), Dependencies::new().with(Arc::clone(&pool)));
    let result = UsageAnomalyScanJob.execute(&ctx).await.expect("job runs");
    assert!(result.success);
    pool.pool().close().await;
    database.drop_now().await;
}

fn profile(requests: i64, cost: i64, base_requests: f64, base_cost: f64) -> HourlyUsageProfile {
    HourlyUsageProfile {
        user_id: UserId::new("anomaly-unit"),
        observed_requests: requests,
        observed_cost_microdollars: cost,
        baseline_requests_per_hour: base_requests,
        baseline_cost_per_hour: base_cost,
    }
}

#[test]
fn a_user_without_history_is_held_to_the_floors() {
    assert!(evaluate(&profile(49, 999_999, 0.0, 0.0)).is_empty());
    assert_eq!(
        evaluate(&profile(50, 1_000_000, 0.0, 0.0)),
        vec![AnomalyKind::Spend, AnomalyKind::RequestRate]
    );
}

#[test]
fn a_heavy_user_needs_three_times_their_own_baseline() {
    assert!(evaluate(&profile(500, 5_000_000, 200.0, 2_000_000.0)).is_empty());
    assert_eq!(
        evaluate(&profile(600, 6_000_000, 200.0, 2_000_000.0)),
        vec![AnomalyKind::Spend, AnomalyKind::RequestRate]
    );
}

#[test]
fn the_scan_window_is_the_previous_closed_hour() {
    let now = Utc
        .with_ymd_and_hms(2026, 10, 7, 10, 2, 13)
        .single()
        .expect("now");
    assert_eq!(
        previous_hour(now),
        Utc.with_ymd_and_hms(2026, 10, 7, 9, 0, 0)
            .single()
            .expect("hour")
    );
}
