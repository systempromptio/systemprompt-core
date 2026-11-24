//! A node sized from a connection budget never holds more Postgres sessions
//! than the budget, counted by the server in `pg_stat_activity`.

use std::time::Duration;

use systemprompt_database::{
    ConnectionBudget, Database, ListenLink, PinnedPools, PoolConfig, PoolSeat, PoolShape,
};
use systemprompt_test_fixtures::DisposableDb;
use systemprompt_traits::BackgroundTasks;

const BUDGET: u32 = 12;

#[tokio::test]
async fn a_saturated_node_holds_no_more_sessions_than_its_budget() {
    let db = DisposableDb::empty("connection_budget").await;
    let sizes = ConnectionBudget::new(BUDGET)
        .divide(
            PoolSeat::Node,
            PoolShape {
                separate_write: true,
                separate_session: true,
            },
            PinnedPools::default(),
        )
        .expect("divides");
    let pool = PoolConfig {
        max_connections: sizes.main,
        acquire_timeout: Duration::from_secs(2),
        ..PoolConfig::default()
    };
    let session = PoolConfig {
        max_connections: sizes.session,
        ..pool
    };
    let database = Database::connect(db.url(), Some(db.url()), &pool)
        .await
        .expect("connect")
        .with_session_url(Some(db.url()), &session)
        .await
        .expect("session pool");

    let tasks = BackgroundTasks::default();
    database.bind_notifications(&tasks);
    let mut subscription = database.notifications().subscribe(&["connection_budget_probe"]);
    tokio::time::timeout(Duration::from_secs(10), async {
        while !subscription.is_listening() {
            subscription.recv().await;
        }
    })
    .await
    .expect("the hub listens");
    assert!(matches!(database.notifications().link(), ListenLink::Up { .. }));

    let mut held = Vec::new();
    for pool in [database.pool(), database.write_pool()] {
        for _ in 0..sizes.main {
            held.push(pool.acquire().await.expect("acquire within the pool"));
        }
    }
    for _ in 1..sizes.session {
        held.push(database.session_pool().acquire().await.expect("session"));
    }
    assert!(
        database.pool().acquire().await.is_err(),
        "the main pool is full at its divided size"
    );

    let admin = systemprompt_test_fixtures::test_pg_pool().await;
    let sessions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pg_stat_activity WHERE datname = $1")
            .bind(db.name())
            .fetch_one(&admin)
            .await
            .expect("pg_stat_activity");
    assert!(
        sessions <= i64::from(BUDGET - sizes.children),
        "{sessions} sessions for a node budget of {BUDGET} ({} reserved for children)",
        sizes.children
    );

    drop(held);
    tasks.cancellation_token().cancel();
    drop(database);
    db.drop_now().await;
}
