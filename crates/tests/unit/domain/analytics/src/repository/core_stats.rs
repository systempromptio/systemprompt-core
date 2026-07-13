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

#[tokio::test]
async fn recent_conversations_respects_limit_and_decodes_rows() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = CoreStatsRepository::new(&pool).expect("repo");

    let rows = repo.get_recent_conversations(5).await.expect("recent");
    assert!(rows.len() <= 5, "LIMIT must bound the result set");
    for row in &rows {
        // COALESCEd columns are never null; message_count is a non-negative count.
        assert!(!row.agent_name.is_empty());
        assert!(!row.user_name.is_empty());
        assert!(!row.status.is_empty());
        assert!(row.message_count >= 0);
    }
}

#[tokio::test]
async fn content_stats_windows_are_monotonic() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = CoreStatsRepository::new(&pool).expect("repo");

    let stats = repo.get_content_stats(10).await.expect("content stats");
    assert!(stats.len() <= 10, "LIMIT must bound the result set");
    for stat in &stats {
        // Nested view windows are cumulative: each wider window counts at
        // least as many views as the narrower one it contains.
        assert!(stat.views_5m <= stat.views_1h);
        assert!(stat.views_1h <= stat.views_1d);
        assert!(stat.views_1d <= stat.views_7d);
        assert!(stat.views_7d <= stat.views_30d);
    }
}
