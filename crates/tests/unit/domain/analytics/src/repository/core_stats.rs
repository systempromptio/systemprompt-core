//! DB-backed tests for `CoreStatsRepository`: the platform, cost, and
//! user-trend overview aggregates. These queries aggregate over shared
//! tables, so assertions are lower-bound invariants against seeded data
//! rather than exact counts.

use systemprompt_analytics::{CoreStatsRepository, SessionRepository};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

use super::session_support::{delete_session, seed_session, unique_session_id};

#[tokio::test]
async fn platform_overview_counts_seeded_session() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = CoreStatsRepository::new(&pool).expect("repo");
    let sessions = SessionRepository::new(&pool).expect("session repo");

    let sid = unique_session_id();
    seed_session(&sessions, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    let overview = repo.get_platform_overview().await.expect("overview");
    assert!(overview.total_sessions >= 1);
    assert!(overview.active_sessions >= 1);
    assert!(overview.total_users >= 0);
    assert!(overview.active_users_24h <= overview.active_users_7d);

    delete_session(&pool, &sid).await;
}

#[tokio::test]
async fn cost_overview_reflects_window_ordering() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = CoreStatsRepository::new(&pool).expect("repo");

    let costs = repo.get_cost_overview().await.expect("costs");
    assert!(costs.total_cost >= 0.0);
    assert!(costs.cost_24h <= costs.cost_7d);
    assert!(costs.cost_7d <= costs.cost_30d);
    assert!(costs.cost_30d <= costs.total_cost);
    assert!(costs.avg_cost_per_request >= 0.0);
}

#[tokio::test]
async fn user_metrics_with_trends_are_consistent() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = CoreStatsRepository::new(&pool).expect("repo");
    let sessions = SessionRepository::new(&pool).expect("session repo");

    let sid = unique_session_id();
    seed_session(&sessions, &sid, &format!("fp-{}", Uuid::new_v4())).await;

    let metrics = repo.get_user_metrics_with_trends().await.expect("metrics");
    assert!(metrics.count_24h >= 1);
    assert!(metrics.count_24h <= metrics.count_7d);
    assert!(metrics.count_7d <= metrics.count_30d);
    assert!(metrics.prev_24h >= 0);

    delete_session(&pool, &sid).await;
}
