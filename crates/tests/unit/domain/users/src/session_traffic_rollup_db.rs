//! The traffic rollup rebuild is idempotent: running it twice leaves the
//! same counts, and those counts equal a direct classification of the
//! sessions whose user is not deleted.

use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use systemprompt_users::SessionTrafficRollupRepository;

const ROLLUP_ROWS: &str = "SELECT traffic_class, SUM(sessions)::bigint FROM session_traffic_rollup_hourly \
     GROUP BY traffic_class ORDER BY traffic_class";

const DIRECT_ROWS: &str = "SELECT session_traffic_class(s.is_bot, s.is_ai_crawler, s.is_scanner, s.is_behavioral_bot, s.landing_page, s.request_count), \
            COUNT(*)::bigint \
     FROM user_sessions s \
     WHERE NOT EXISTS (SELECT 1 FROM users d WHERE d.id = s.user_id AND d.status = 'deleted') \
     GROUP BY 1 ORDER BY 1";

// Why: sibling tests insert and delete sessions concurrently, so the
// comparison runs inside one transaction, which sees one snapshot.
#[tokio::test]
async fn rebuild_is_idempotent_and_matches_a_direct_classification() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let pg = pool.write_pool();
    SessionTrafficRollupRepository::new(&pool)
        .rebuild()
        .await
        .expect("rebuild through the repository");

    let mut tx = pg.begin().await.expect("begin");
    sqlx::query("SELECT session_traffic_rollup_rebuild()")
        .execute(&mut *tx)
        .await
        .expect("first rebuild");
    let first: Vec<(String, i64)> = sqlx::query_as(ROLLUP_ROWS)
        .fetch_all(&mut *tx)
        .await
        .expect("rollup rows");
    sqlx::query("SELECT session_traffic_rollup_rebuild()")
        .execute(&mut *tx)
        .await
        .expect("second rebuild");
    let second: Vec<(String, i64)> = sqlx::query_as(ROLLUP_ROWS)
        .fetch_all(&mut *tx)
        .await
        .expect("rollup rows");
    let direct: Vec<(String, i64)> = sqlx::query_as(DIRECT_ROWS)
        .fetch_all(&mut *tx)
        .await
        .expect("direct rows");
    tx.rollback().await.expect("rollback");

    assert_eq!(first, second, "a second rebuild changes nothing");
    assert_eq!(
        second, direct,
        "the rebuilt counter equals the direct classification"
    );
}
